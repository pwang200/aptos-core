#![allow(unused)]
// Copyright © Aptos Foundation

use anyhow::{Context, Result};
use aptos_rest_client::PendingTransaction;
use aptos_sdk::{
    coin_client::CoinClient,
    crypto::ed25519::{Ed25519PrivateKey, Ed25519PublicKey},
    rest_client::{Client, FaucetClient},
    types::LocalAccount,
};
use aptos_types::account_address::AccountAddress;
use aptos_types::PeerId;
use core::time;
use futures::executor::block_on;
use futures::future::join_all;
use once_cell::sync::Lazy;
use rand::SeedableRng;
use rand::{rngs, RngCore};
use std::thread::spawn;
use std::time::{Duration, Instant};
use std::{env, str::FromStr, thread};
use url::Url;
//static NODE_URL: Lazy<Url> = Lazy::new(|| Url::from_str("http://127.0.0.1:8080").unwrap());
// static NODE_URL: Lazy<Url> = Lazy::new(|| Url::from_str("http://127.0.0.1:41599").unwrap());
// static FAUCET_URL: Lazy<Url> = Lazy::new(|| Url::from_str("http://127.0.0.1:8081").unwrap());

const ROUNDS: u64 = 1;
const FANOUT: u64 = 5;
const PERSPAWN: u64 = 10;
const MASTER_SEED: u64 = 0;
const PERACCOUNT: u64 = 100_000_000_000;//1k aptos

/**
 * spraw 20 tasks, each task will:
 * take a seed
 * create alice
 * fund alice with fauset
 * create 49 bobs
 * fund bob by alice
 */
async fn fanout(seed: u64, node_url: Url, fauc_url: Url) {
    println!("seed {}, creating {} accounts", seed, PERSPAWN*PERSPAWN);
    let rest_client = Client::new(node_url.clone());
    let faucet_client = FaucetClient::new(fauc_url.clone(), node_url.clone());
    let coin_client = CoinClient::new(&rest_client);
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let mut accounts: Vec<LocalAccount> = Vec::new();
    let mut alice = LocalAccount::generate(&mut rng);
    faucet_client
        .fund(alice.address(), PERACCOUNT * PERSPAWN * PERSPAWN)
        .await
        .context("Failed to fund Alice's account");

    // println!("account {:?}", alice.address());
    match rest_client.get_account(alice.address()).await {
        Ok(r) => {
            let a_info = rest_client.get_account(alice.address()).await.unwrap();
            *alice.sequence_number_mut() = a_info.inner().sequence_number;
        },
        Err(e) => {
            println!("seed {}, alice account error {:?}", seed, e);
            panic!("account creation");
        },
    }

    let mut txns: Vec<aptos_types::transaction::SignedTransaction> = Vec::new();
    for i in 1..PERSPAWN {
        let bob = LocalAccount::generate(&mut rng);
        txns.push(coin_client.create_and_pay(&mut alice, bob.address(), PERACCOUNT * PERSPAWN, 4, None));
        accounts.push(bob);
    }
    accounts.push(alice);
    let mut results: Vec<_> = Vec::new();
    for tx in &mut txns {
        results.push(rest_client.submit(tx));
    }
    let mut results = join_all(results).await;
    for r in results {
        let tx = r.unwrap().into_inner();
        let tx = rest_client
            .wait_for_transaction(&tx)
            .await
            .context("Failed when waiting for transaction");
        //println!("tx {:?}", tx.unwrap().inner().transaction_info());
    }

    for a in &mut accounts {
        match rest_client.get_account(a.address()).await {
            Ok(r) => {
                let a_info = rest_client.get_account(a.address()).await.unwrap();
                *a.sequence_number_mut() = a_info.inner().sequence_number;
            },
            Err(e) => {
                println!("seed {}, account error {:?}", seed, e);
                panic!("account creation");
            },
        }
    }

    let mut addresses: Vec<AccountAddress> = Vec::new();
    let mut txns: Vec<aptos_types::transaction::SignedTransaction> = Vec::new();
    for i in 0..accounts.len() {
        for i in 1..PERSPAWN {
            let bob = LocalAccount::generate(&mut rng);
            txns.push(coin_client.create_and_pay(&mut accounts[i as usize], bob.address(), PERACCOUNT, 4, None));
            addresses.push(bob.address());
        }
    }
    let mut results: Vec<_> = Vec::new();
    for tx in &mut txns {
        results.push(rest_client.submit(tx));
    }
    let mut results = join_all(results).await;
    for r in results {
        let tx = r.unwrap().into_inner();
        let tx = rest_client
            .wait_for_transaction(&tx)
            .await
            .context("Failed when waiting for transaction");
    }
    for a in addresses {
        coin_client.get_account_balance(&a).await.unwrap();
    }

    println!("tx done, seed {}", seed);
}

/**
 * By manage the seeds to random number generators, multiple client processes
 * can create the same accounts without communicate.
 */

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    assert_eq!(args.len(), 3);
    println!("rest_url entered {} {} {}", args[0], args[1], args[2]);
    let node_url = Url::from_str(args[1].as_str()).unwrap();
    let fauc_url = Url::from_str(args[2].as_str()).unwrap();
    let mut m: u64 = 0;
    for r in 0..ROUNDS {
        let start = Instant::now();

        let mut handles: Vec<_> = Vec::new();
        for i in 0..FANOUT {
            let handle = tokio::task::spawn(fanout(m + MASTER_SEED, node_url.clone(), fauc_url.clone()));
            m += 1;
            handles.push(handle);
        }

        for h in handles {
            tokio::join!(h);
        }
        let duration_wait = start.elapsed();
        println!("Round {} time: {:?}", r, duration_wait);
    }
    Ok(())
}

// join_all(results).await;
// for i in 0..20 {
//     let a = &accounts[i * 50];
//     println!(
//         "{} {} {:?}",
//         i,
//         a.address().to_hex_literal(),
//         coin_client
//             .get_account_balance(&a.address())
//             .await
//             .context("Failed to get account balance")?
//     );
// }

// for i in 0..20 {
//     let mut results: Vec<_> = Vec::new();
//     for j in 1..50 {
//         let n1 = rng.next_u32() % NUM_ACCOUNTS;
//         let mut n2 = rng.next_u32() % NUM_ACCOUNTS;
//         while n1 == n2 {
//             n2 = rng.next_u32() % NUM_ACCOUNTS;
//         }
//         let n1: usize = n1.try_into().unwrap();
//         let n2: usize = n2.try_into().unwrap();
//         let bob = accounts[n2].address();
//         let alice = &mut accounts[n1];
//         results.append(coin_client.transfer(alice, bob, 1_000, None));
//         let txn_hash = coin_client
//             .transfer(alice, bob, 1_000, None)
//             .await
//             .context("Failed to submit transaction to transfer coins")?;
//         rest_client
//             .wait_for_transaction(&txn_hash)
//             .await
//             .context("Failed when waiting for the transfer transaction")?;
//         // let bob = accounts[i * 20 + j].address();
//         // let alice = &mut accounts[i];
//         // results.push(coin_client.transfer(alice, bob, 100_000_000, None));
//     }
// }

// // for a in &mut accounts {
// //     //println!("{}", a.address().to_hex_literal());
// //     results.push(faucet_client.fund(a.address(), 100_000_000));
// //     cc += 1;
// //     if cc % 20 == 0 {
// //         join_all(&results).await;
// //     }
// //     //println!("results len {}", results.len());
// //     // .await
// //     // .context("Failed to fund account")?;
// //     // println!(
// //     //     "{} {:?}",
// //     //     a.address().to_hex_literal(),
// //     //     coin_client
// //     //         .get_account_balance(&a.address())
// //     //         .await
// //     //         .context("Failed to get account balance")?
// //     // );
// // }
// // if !results.is_empty() {
// //     join_all(results).await;
// //     results.clear();
// // }
// // let duration_send = start.elapsed();
// // join_all(results).await;
// // let duration_wait = start.elapsed();
// // let mut i = 0;
// // for a in &mut accounts {
// //     println!(
// //         "{} {} {:?}",
// //         i,
// //         a.address().to_hex_literal(),
// //         coin_client
// //             .get_account_balance(&a.address())
// //             .await
// //             .context("Failed to get account balance")?
// //     );
// //     i += 1;
// // }

// let duration_wait = start.elapsed();
// println!("Time: {:?}", duration_wait);

// // for a in &mut accounts {
// //     let ac = rest_client
// //         .get_account(a.address())
// //         .await
// //         .context("Failed to get account")
// //         .unwrap();
// //     let acc = ac.inner();
// //     println!("{} {:?}", a.address().to_hex_literal(), acc.sequence_number);
// // }

// // let start = Instant::now();
// for i in (0..NUM_TXNS) {
//     let n1 = rng.next_u32() % NUM_ACCOUNTS;
//     let mut n2 = rng.next_u32() % NUM_ACCOUNTS;
//     while n1 == n2 {
//         n2 = rng.next_u32() % NUM_ACCOUNTS;
//     }
//     let n1: usize = n1.try_into().unwrap();
//     let n2: usize = n2.try_into().unwrap();
//     let bob = accounts[n2].address();
//     let alice = &mut accounts[n1];
//     let txn_hash = coin_client
//         .transfer(alice, bob, 1_000, None)
//         .await
//         .context("Failed to submit transaction to transfer coins")?;
//     rest_client
//         .wait_for_transaction(&txn_hash)
//         .await
//         .context("Failed when waiting for the transfer transaction")?;
// }
// // let duration = start.elapsed();
// // for a in &mut accounts {
// //     println!(
// //         "{} {:?}",
// //         a.address().to_hex_literal(),
// //         coin_client
// //             .get_account_balance(&a.address())
// //             .await
// //             .context("Failed to get account balance")?
// //     );
// // }
// // println!("Time: {:?}", duration);

// println!(
//     "{} {:?}",
//     a.to_hex_literal(),
//     coin_client
//         .get_account_balance(&a)
//         .await
//         .context("Failed to get account balance")
// );
// let ac = rest_client
//     .get_account(a)
//     .await
//     .context("Failed to get account")
//     .unwrap();
// let acc = ac.inner();
// println!("{} {:?}", a.to_hex_literal(), acc); //.sequence_number
