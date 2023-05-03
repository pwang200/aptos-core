#![allow(unused)]
// Copyright © Aptos Foundation

use anyhow::{Context, Result};
use aptos_sdk::{
    coin_client::CoinClient,
    crypto::ed25519::{Ed25519PrivateKey, Ed25519PublicKey},
    rest_client::{Client, FaucetClient},
    types::LocalAccount,
};
use futures::{executor::block_on, future::join_all};
use once_cell::sync::Lazy;
use rand::SeedableRng;
use rand::{rngs, RngCore};
use std::collections::HashMap;
use std::str::FromStr;
use std::time::{Duration, Instant};
use url::Url;

static NODE_URL: Lazy<Url> = Lazy::new(|| Url::from_str("http://127.0.0.1:39397").unwrap());

const CHAINID: u8 = 4;
const FANOUT: u64 = 10;
const PERSPAWN: u64 = 20;
const MASTER_SEED: u64 = 0;
const NUM_ACCOUNTS: u64 = FANOUT * PERSPAWN;
const BATCH_SIZE: u64 = 1000;
const NUM_BATCHES: u64 = 2;

/**
 * create account array, segment by segment.
 * query chain for sqn
 */

async fn recreate_accounts() -> Vec<LocalAccount> {
    let start: Instant = Instant::now();
    let rest_client = Client::new(NODE_URL.clone());
    let mut accounts: Vec<LocalAccount> = Vec::new();
    for i in (0..FANOUT) {
        let mut rng = rand::rngs::StdRng::seed_from_u64(MASTER_SEED + i);
        let mut account_map = HashMap::new();
        let mut results: Vec<_> = Vec::new();
        for i in 0..PERSPAWN {
            let a = LocalAccount::generate(&mut rng);
            results.push(rest_client.get_account(a.address()));
            account_map.insert(a.authentication_key(), a);
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
    }
    println!("accounts ready, took {:?}", start.elapsed());
    accounts
}

fn get_indexes(rng: &mut rand::rngs::StdRng) -> (usize, usize) {
    let n1 = rng.next_u64() % NUM_ACCOUNTS;
    let mut n2 = rng.next_u64() % NUM_ACCOUNTS;
    while n1 == n2 {
        n2 = rng.next_u64() % NUM_ACCOUNTS;
    }
    let n1: usize = n1.try_into().unwrap();
    let n2: usize = n2.try_into().unwrap();
    (n1, n2)
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("rest_url {}", NODE_URL.clone());

    let rest_client = Client::new(NODE_URL.clone());
    let coin_client = CoinClient::new(&rest_client);

    let mut accounts = recreate_accounts().await;
    let mut rng = rand::rngs::StdRng::seed_from_u64(MASTER_SEED + FANOUT);
    let mut txns_results = Vec::new();
    let start = Instant::now();
    for i in (0..NUM_BATCHES) {
        let mut txns: Vec<aptos_types::transaction::SignedTransaction> = Vec::new();
        for j in 0..BATCH_SIZE {
            let (n1, n2) = get_indexes(&mut rng);
            let bob = accounts[n2].address();
            let alice = &mut accounts[n1];
            txns.push(coin_client.build(alice, bob, 50, CHAINID, None));
        }
        let mut results: Vec<_> = Vec::new();
        let round_start = Instant::now();
        for tx in &mut txns {
            results.push(rest_client.submit(tx));
        }
        let mut round_txns_results = join_all(results).await;
        txns_results.append(&mut round_txns_results);
        println!("round {}, {:?}", i, round_start.elapsed());
    }
    println!("before waiting txns {:?}", start.elapsed());

    let mut submit_failures: u32 = 0;
    let mut consensus_failures: u32 = 0;
    let mut successes: u32 = 0;
    for r in txns_results {
        match r {
            Ok(re) => {
                let p_tx = re.inner();
                let tx_r = rest_client.wait_for_transaction(&p_tx).await;
                match tx_r {
                    Ok(tx_rr) => {
                        if tx_rr.inner().success() {
                            successes += 1;
                        } else {
                            println!("tx {:?}", tx_rr.inner().transaction_info());
                        }
                    },
                    Err(e) => {
                        consensus_failures += 1;
                    },
                }
            },
            Err(e) => {
                submit_failures += 1;
            },
        }
    }
    println!(
        "after waiting txns {:?}, {} {} {}",
        start.elapsed(),
        successes,
        submit_failures,
        consensus_failures
    );

    Ok(())
}

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
