#![allow(unused)]
// Copyright © Aptos Foundation

use anyhow::{Context, Result};
use aptos_sdk::{
    coin_client::CoinClient,
    crypto::ed25519::{Ed25519PrivateKey, Ed25519PublicKey},
    rest_client::{Client, FaucetClient},
    types::LocalAccount,
};
use futures::executor::block_on;
use once_cell::sync::Lazy;
use rand::SeedableRng;
use rand::{rngs, RngCore};
use std::str::FromStr;
use std::time::{Duration, Instant};
use url::Url;

static NODE_URL: Lazy<Url> = Lazy::new(|| Url::from_str("http://127.0.0.1:8080").unwrap());
static FAUCET_URL: Lazy<Url> = Lazy::new(|| Url::from_str("http://127.0.0.1:8081").unwrap());

const NUM_ACCOUNTS: u32 = 2;
const NUM_TXNS: u32 = 2;

/**
 * create account array. note multiple client processes will create the same accounts
 * fund the accounts. note ok to fund multiple times, don't need new chain
 * random select account pairs and submit tx, note time measure
 */

#[tokio::main]
async fn main() -> Result<()> {
    let rest_client = Client::new(NODE_URL.clone());
    let faucet_client = FaucetClient::new(FAUCET_URL.clone(), NODE_URL.clone());
    let coin_client = CoinClient::new(&rest_client);

    println!("faucet_url {}", FAUCET_URL.clone());
    let mut accounts: Vec<LocalAccount> = Vec::new();
    let mut rng = rand::rngs::StdRng::seed_from_u64(9);

    for i in (0..NUM_ACCOUNTS) {
        accounts.push(LocalAccount::generate(&mut rng));
    }

    for a in &mut accounts {
        println!("{}", a.address().to_hex_literal());
        faucet_client
            .fund(a.address(), 100_000_000)
            .await
            .context("Failed to fund Alice's account")?;
        println!(
            "{} {:?}",
            a.address().to_hex_literal(),
            coin_client
                .get_account_balance(&a.address())
                .await
                .context("Failed to get account balance")?
        );
    }
    for a in &mut accounts {
        let ac = rest_client
            .get_account(a.address())
            .await
            .context("Failed to get account")
            .unwrap();
        let acc = ac.inner();
        println!("{} {:?}", a.address().to_hex_literal(), acc.sequence_number);
        let sqn = a.sequence_number_mut();
        *sqn = acc.sequence_number;
    }

    let start = Instant::now();
    for i in (0..NUM_TXNS) {
        let n1 = rng.next_u32() % NUM_ACCOUNTS;
        let mut n2 = rng.next_u32() % NUM_ACCOUNTS;
        while n1 == n2 {
            n2 = rng.next_u32() % NUM_ACCOUNTS;
        }
        let n1: usize = n1.try_into().unwrap();
        let n2: usize = n2.try_into().unwrap();
        let bob = accounts[n2].address();
        let alice = &mut accounts[n1];
        let txn_hash = coin_client
            .transfer(alice, bob, 1_000, None)
            .await
            .context("Failed to submit transaction to transfer coins")?;
        rest_client
            .wait_for_transaction(&txn_hash)
            .await
            .context("Failed when waiting for the transfer transaction")?;
    }
    let duration = start.elapsed();
    for a in &mut accounts {
        println!(
            "{} {:?}",
            a.address().to_hex_literal(),
            coin_client
                .get_account_balance(&a.address())
                .await
                .context("Failed to get account balance")?
        );
    }
    println!("Time: {:?}", duration);
    Ok(())
}
