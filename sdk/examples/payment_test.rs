#![allow(unused)]
// Copyright © Aptos Foundation

use anyhow::{Context, Result};
use aptos_sdk::{
    coin_client::CoinClient,
    crypto::ed25519::{Ed25519PrivateKey, Ed25519PublicKey},
    rest_client::{Client, FaucetClient},
    types::LocalAccount,
};
use aptos_types::transaction::SignedTransaction;
use futures::{executor::block_on, future::join_all};
use once_cell::sync::Lazy;
use rand::SeedableRng;
use rand::{rngs, RngCore};
use std::collections::HashMap;
use std::str::FromStr;
use std::time::{Duration, Instant};
use url::Url;
use std::env;
use move_core_types::account_address::AccountAddress;

const CHAINID: u8 = 4;
const SQN_BATCH: u64 = 20;
// const FANOUT: u64 = 100;
// const BATCH_SIZE: u64 = 100;
// const NUM_BATCHES: u64 = 30;
const APTO_BATCH: usize = 10;

/**
 * create account array, segment by segment.
 * query chain for sqn
 */

async fn recreate_accounts(url: Url, start_seed: u64, num_seeds: u64) -> (Vec<LocalAccount>, Vec<AccountAddress>){
    let start: Instant = Instant::now();
    let rest_client = Client::new(url);
    let mut accounts: Vec<LocalAccount> = Vec::new();
    let mut addresses: Vec<AccountAddress> = Vec::new();
    let last_seed = start_seed + num_seeds - 1;
    let mut seed = start_seed;

    loop {
        let mut account_map = HashMap::new();
        let mut results: Vec<_> = Vec::new();
        for j in 0..SQN_BATCH {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            let a = LocalAccount::generate(&mut rng);
            addresses.push(a.address());
            results.push(rest_client.get_account(a.address()));
            account_map.insert(a.authentication_key(), a);
            if seed == last_seed { break; } else { seed += 1; }
        }
        for r in results {
            let a = r.await.context("Failed to get account").unwrap();
            *account_map
                .get_mut(&a.inner().authentication_key)
                .unwrap()
                .sequence_number_mut() = a.inner().sequence_number;
        }
        let mut accs: Vec<LocalAccount> = account_map.into_values().collect();
        accounts.append(&mut accs);
        if seed == last_seed { break; }
    }

    println!("accounts ready, current seed {}, took {:?}", seed, start.elapsed());
    (accounts, addresses)
}

fn get_indexes(rng: &mut rand::rngs::StdRng, senders: &Vec<LocalAccount>, receivers: &Vec<AccountAddress>) -> (usize, usize) {
    let num_senders: u64 = senders.len() as u64;
    let num_receivers = receivers.len() as u64;
    let n1 = rng.next_u64() % num_senders;
    let mut n2 = rng.next_u64() % num_receivers;
    let a = (*senders)[n1 as usize].address();
    while a == receivers[n2 as usize] {
        n2 = rng.next_u64() % num_receivers;
    }
    let n1: usize = n1 as usize;
    let n2: usize = n2 as usize;
    (n1, n2)
}

async fn fanout(mut senders: Vec<LocalAccount>, receivers: Vec<AccountAddress>, num_batches: u64, batch_size: u64, url: Url)
{
    let rest_client = Client::new(url);
    let coin_client = CoinClient::new(&rest_client);

    let mut rng = rand::rngs::StdRng::seed_from_u64(0);
    let mut txns_results = Vec::new();
    let start = Instant::now();
    for i in (0..num_batches) {
        let mut txns/*: Vec<aptos_types::transaction::SignedTransaction>*/ = Vec::new();
        for j in 0..batch_size {
            let mut tx_batch = Vec::new();
            for k in 0..APTO_BATCH{
                let (n1, n2) = get_indexes(&mut rng, &senders, &receivers);
                let bob = receivers[n2];
                let alice = &mut senders[n1];
                tx_batch.push(coin_client.build(alice, bob, 50, CHAINID, None));
            }
            txns.push(tx_batch);//coin_client.build(alice, bob, 50, CHAINID, None)
        }
        let mut results: Vec<_> = Vec::new();
        //let round_start = Instant::now();
        for tx_batch in &mut txns {
            results.push(rest_client.submit_batch(tx_batch));
        }
        let mut round_txns_results = join_all(results).await;
        txns_results.append(&mut round_txns_results);
        //println!("round {}, {:?}", i, round_start.elapsed());
    }
    //println!("before waiting txns {:?}", start.elapsed());

    let mut submit_failures: u32 = 0;
    // let mut consensus_failures: u32 = 0;
    let mut submit_successes: u32 = 0;
    for r in txns_results {
        match r {
            Ok(re) => {
                // let p_tx = re.inner();
                // let tx_r = rest_client.wait_for_transaction(&p_tx).await;
                // match tx_r {
                //     Ok(tx_rr) => {
                //         if tx_rr.inner().success() {
                //             successes += 1;
                //         } else {
                //             println!("tx {:?}", tx_rr.inner().transaction_info());
                //         }
                //     },
                //     Err(e) => {
                //         consensus_failures += 1;
                //     },
                // }
                submit_successes += 1;
            },
            Err(e) => {
                submit_failures += 1;
            },
        }
    }
    println!(
        "after waiting txns {:?}, good: {}, bad: {}",// {}",
        start.elapsed(),
        submit_successes,
        submit_failures//,
        //consensus_failures
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    assert_eq!(args.len(), 7);
    let url = Url::from_str(args[1].as_str()).unwrap();
    let start_seed: u64 = args[2].parse().unwrap();
    let num_seeds: u64 = args[3].parse().unwrap();
    let num_spawns: u64 = args[4].parse().unwrap();
    let num_batches: u64 = args[5].parse().unwrap();
    let batch_size: u64 = args[6].parse().unwrap();
    assert!(num_seeds > 0);
    assert_eq!(num_seeds % num_spawns, 0);
    println!("url {}, start_seed {}, num_seeds {}, fanout {}, num_batches {}, batch_size {}",
             args[1].as_str(), start_seed, num_seeds, num_spawns, num_batches, batch_size);

    let start = Instant::now();
    let (mut accounts, receivers) = recreate_accounts(url.clone(), start_seed, num_seeds).await;
    println!("total number of accounts {}, time: {:?}", accounts.len(), start.elapsed());

    if num_spawns != 0 {
        let mut handles: Vec<_> = Vec::new();
        let per_spawn = (num_seeds / num_spawns) as usize;
        for i in 0..num_spawns {
            let senders: Vec<LocalAccount> = accounts.drain(accounts.len() - per_spawn..).collect();
            let handle = tokio::task::spawn(fanout(senders, receivers.clone(), num_batches, batch_size, url.clone()));
            handles.push(handle);
        }
        assert!(accounts.is_empty());
        for h in handles {
            tokio::join!(h);
        }
        println!("total time: {:?}", start.elapsed());
    }
    Ok(())
}
// let (senders, remining_accounts) = remining_accounts.split_at_mut(per_spawn);
// let mut receivers: Vec<AccountAddress> = Vec::new();
// for a in accounts{
//     receivers.push(a.address());
// }
// let receivers = receivers;
// let mut rng = rand::rngs::StdRng::seed_from_u64(start_seed);
// let mut txns_results = Vec::new();
// let start = Instant::now();
// for i in (0..NUM_BATCHES) {
//     let mut txns: Vec<aptos_types::transaction::SignedTransaction> = Vec::new();
//     for j in 0..BATCH_SIZE {
//         let (n1, n2) = get_indexes(&mut rng);
//         let bob = accounts[n2].address();
//         let alice = &mut accounts[n1];
//         txns.push(coin_client.build(alice, bob, 50, CHAINID, None));
//     }
//     let mut results: Vec<_> = Vec::new();
//     let round_start = Instant::now();
//     for tx in &mut txns {
//         results.push(rest_client.submit(tx));
//     }
//     let mut round_txns_results = join_all(results).await;
//     txns_results.append(&mut round_txns_results);
//     println!("round {}, {:?}", i, round_start.elapsed());
// }
// println!("before waiting txns {:?}", start.elapsed());
//
// let mut submit_failures: u32 = 0;
// let mut consensus_failures: u32 = 0;
// let mut successes: u32 = 0;
// for r in txns_results {
//     match r {
//         Ok(re) => {
//             let p_tx = re.inner();
//             let tx_r = rest_client.wait_for_transaction(&p_tx).await;
//             match tx_r {
//                 Ok(tx_rr) => {
//                     if tx_rr.inner().success() {
//                         successes += 1;
//                     } else {
//                         println!("tx {:?}", tx_rr.inner().transaction_info());
//                     }
//                 },
//                 Err(e) => {
//                     consensus_failures += 1;
//                 },
//             }
//         },
//         Err(e) => {
//             submit_failures += 1;
//         },
//     }
// }
// println!(
//     "after waiting txns {:?}, {} {} {}",
//     start.elapsed(),
//     successes,
//     submit_failures,
//     consensus_failures
// );

//println!("rest_url entered {} {}", args[0], args[1]);
//let mut htp = "http://";
// let unl = if args.len() == 2 {
//     Url::from_str(args[1].as_str()).unwrap()
// } else {
//     Url::from_str("http://127.0.0.1:8080").unwrap()
// };


// let tx = r
//     .unwrap_or_else(|error| {
//         submit_failures += 1;
//     })
//     .inner();
// let tx = rest_client
//     .wait_for_transaction(&tx)
//     .await
//     .context("Failed when waiting for transaction");
//println!("tx {:?}", tx.unwrap().inner().transaction_info());
//        let tx = r.unwrap().into_inner();
// }
// let txn_hash = coin_client
//     .transfer(alice, bob, 50, None)
//     .await
//     .context("Failed to submit transaction to transfer coins")?;
// rest_client
//     .wait_for_transaction(&txn_hash)
//     .await
//     .context("Failed when waiting for the transfer transaction")?;

// : Vec<LocalAccount> = Vec::new();
// for i in (0..FANOUT) {
//     let mut rng = rand::rngs::StdRng::seed_from_u64(MASTER_SEED + i);
//     let mut alice = LocalAccount::generate(&mut rng);
//     *alice.sequence_number_mut() += PERSPAWN - 1;
//     accounts.push(alice);
//     for i in 1..PERSPAWN {
//         accounts.push(LocalAccount::generate(&mut rng));
//     }
// }

// let n1 = rng.next_u64() % NUM_ACCOUNTS;
// let mut n2 = rng.next_u64() % NUM_ACCOUNTS;
// while n1 == n2 {
//     n2 = rng.next_u64() % NUM_ACCOUNTS;
// }
// let n1: usize = n1.try_into().unwrap();
// let n2: usize = n2.try_into().unwrap();

//let mut alice = LocalAccount::generate(&mut rng);
//*alice.sequence_number_mut() += PERSPAWN - 1;
//accounts.push(alice);
// let mut accounts = vec![1,2,3,4,5,6];
// let per_spawn = 3usize;
// let mut handles: Vec<_> = Vec::new();
// for i in 0..2 {
// let d = accounts.len() - per_spawn;
// let senders : Vec<u64>= accounts.drain(d..).collect();
// let handle = tokio::task::spawn(fanout_print(senders));
// handles.push(handle);
// }
// assert!(accounts.is_empty());
// for h in handles {
// tokio::join!(h);
// }//
// // async fn fanout_print(mut senders: Vec<u64>)
// // {
// //     println!("senders {:?}",senders);
// // }
//static NODE_URL: Lazy<Url> = Lazy::new(|| Url::from_str("http://127.0.0.1:39397").unwrap());
// const PERSPAWN: u64 = 2;
// const NUM_ACCOUNTS: u64 = FANOUT * PERSPAWN;
