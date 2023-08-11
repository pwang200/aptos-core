#![allow(unused)]
// Copyright © Aptos Foundation

use anyhow::{Context, Result};
use aptos_rest_client::PendingTransaction;
use aptos_sdk::{
    coin_client::CoinClient,
    crypto::ed25519::{Ed25519PrivateKey, Ed25519PublicKey},
    rest_client::{Client, FaucetClient},
    types::LocalAccount,
    dex_utils,
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
use std::collections::HashMap;
use url::Url;
use static_assertions;


async fn fanout_trade(sc_addr: AccountAddress, mut users: Vec<LocalAccount>,
                         rounds: u64, seed: u64,
                         submit_batch_size: usize, chain_id: u8, url: Url, wait_valid: bool)
{
    let price: u64 = 1;
    let mut count = 0;
    let mut good_submit = 0;
    let mut bad_submit = 0;
    let mut good_wait = 0;
    let mut bad_wait = 0;
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let moon = dex_utils::create_type_tag("moon_coin", "MoonCoin", sc_addr);
    let xrp = dex_utils::create_type_tag("xrp_coin", "XRPCoin", sc_addr);
    let start = Instant::now();
    for r in 0.. rounds{
        let mut txns = Vec::new();
        for alice in &mut users {
            let is_bid = rng.next_u32() % 2 == 1;
            count+=1;
            txns.push(dex_utils::trade_tx(sc_addr, alice, moon.clone(),xrp.clone(), is_bid, price, dex_utils::LOT, chain_id));
        }
        let (good, bad, w_good, w_bad,  dur) = dex_utils::batch_submit(url.clone(), txns, submit_batch_size, wait_valid).await;
        good_submit += good;
        bad_submit += bad;
        good_wait += w_good;
        bad_wait += w_bad;
        //println!("round {}, time {:?}, good: {}, bad: {}", r, dur, good, bad);
    }
    println!("seed {}, trades {}, good_submit {}, bad_submit {}, good_wait {}, bad_wait {}, time {:?}",
             seed, count, good_submit, bad_submit, good_wait, bad_wait, start.elapsed());
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    // assert_eq!(args.len(), 6);
    let url = Url::from_str(args[1].as_str()).unwrap();
    let sk_str = args[2].as_str();
    let start_seed: u64 = args[3].parse().unwrap();
    let num_seeds: u64 = args[4].parse().unwrap();
    let fanout: u64 = args[5].parse().unwrap();
    let submit_batch_size: usize = args[6].parse().unwrap();
    let rounds: u64 = args[7].parse().unwrap();
    let wait_valid: bool = args[8].parse().unwrap();
    assert!(num_seeds % fanout == 0 && num_seeds / fanout > 0);
    assert!(rounds > 0 && submit_batch_size > 0 );
    let per_spawn = (num_seeds / fanout) as usize;

    let start = Instant::now();
    let rest_client = Client::new(url.clone());
    let chain_id = rest_client.get_index().await.context("Failed to get chain ID")?.inner().chain_id;
    let (mut accounts, _) = dex_utils::recreate_accounts(
        url.clone(), start_seed, num_seeds).await;
    println!("total number of accounts {}, time: {:?}", accounts.len(), start.elapsed());
    let mut sc_owner = dex_utils::fill_sc_owner(url.clone(), &sk_str, chain_id, false).await;
    let sc_addr = sc_owner.address();

    let mut handles = Vec::new();
    for i in 0..fanout {
        let mut herd = accounts.drain(accounts.len() - per_spawn..).collect();
        let handle = tokio::task::spawn(
            fanout_trade(sc_addr.clone(), herd, rounds, i as u64,
                         submit_batch_size , chain_id, url.clone(), wait_valid));
        handles.push(handle);
    }
    assert!(accounts.is_empty());
    for h in handles {
        tokio::join!(h);
    }

    println!("Total time: {:?}", start.elapsed());
    Ok(())
}
