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


async fn fanout_register(sc_addr: AccountAddress, mut users: &mut Vec<LocalAccount>,
                         submit_batch_size: usize, chain_id: u8, url: Url)
{
    let count = users.len();
    let mut txns = Vec::new();
    for alice in users {
        txns.push(dex_utils::register_coin_tx(sc_addr, alice, "moon_coin", chain_id));
        txns.push(dex_utils::register_coin_tx(sc_addr, alice, "xrp_coin", chain_id));
    }
    let (good, bad, _, _, dur) = dex_utils::batch_submit(url, txns, submit_batch_size, true).await;
    println!("fanout_register, num {}, after waiting txns {:?}, good: {}, bad: {}", count, dur, good, bad);
}

async fn fanout_deposit(sc_addr: AccountAddress, mut users: &mut Vec<LocalAccount>, amount: u64,
                        submit_batch_size: usize, chain_id: u8, url: Url)
{
    let count = users.len();
    let mut txns = Vec::new();
    let moon = dex_utils::create_type_tag("moon_coin", "MoonCoin", sc_addr);
    let xrp = dex_utils::create_type_tag("xrp_coin", "XRPCoin", sc_addr);
    for alice in users {
        txns.push(dex_utils::deposit_tx(sc_addr, alice, moon.clone(), amount, chain_id));
        txns.push(dex_utils::deposit_tx(sc_addr, alice, xrp.clone(), amount, chain_id));
    }
    let (good, bad, _, _, dur) = dex_utils::batch_submit(url, txns, submit_batch_size, true).await;
    println!("fanout_deposit: amount {}, num {}, after waiting txns {:?}, good: {}, bad: {}", amount, count, dur, good, bad);
}

async fn fanout_transfer(sc_addr: AccountAddress, mut sender: &mut LocalAccount, mut receivers: &mut Vec<LocalAccount>,
                         amount: u64,
                         submit_batch_size: usize, chain_id: u8, url: Url)
{
    let count = receivers.len();
    let mut txns = Vec::new();
    let moon = dex_utils::create_type_tag("moon_coin", "MoonCoin", sc_addr.clone());
    let xrp = dex_utils::create_type_tag("xrp_coin", "XRPCoin", sc_addr);
    for alice in receivers {
        txns.push(dex_utils::transfer_coin_tx(&mut sender, &alice.address(), moon.clone(), amount, chain_id));
        txns.push(dex_utils::transfer_coin_tx(&mut sender, &alice.address(), xrp.clone(), amount, chain_id));
    }
    let (good, bad, _, _, dur) = dex_utils::batch_submit(url, txns, submit_batch_size, true).await;
    println!("fanout_transfer: amount {}, num {}, after waiting txns {:?}, good: {}, bad: {}", amount, count, dur, good, bad);
}

async fn fanout_multi(sc_addr: AccountAddress, mut sender: LocalAccount, mut receivers: Vec<LocalAccount>,
                      transfer_amount: u64, deposit_amount: u64,
                      submit_batch_size: usize, chain_id: u8, url: Url) -> (LocalAccount, Vec<LocalAccount>)
{
    fanout_register(sc_addr.clone(), &mut receivers,
                    submit_batch_size, chain_id, url.clone()).await;
    fanout_transfer(sc_addr.clone(), &mut sender, &mut receivers, transfer_amount,
                    submit_batch_size, chain_id, url.clone()).await;
    fanout_deposit(sc_addr.clone(), &mut receivers, deposit_amount,
                   submit_batch_size, chain_id, url).await;
    (sender, receivers)
}

async fn self_fund_coins(sc_owner: &mut LocalAccount, amount: u64, chain_id: u8, url: Url)
{
    let sc_addr = sc_owner.address();
    let mut txns = Vec::new();
    txns.push(dex_utils::fund_coin_tx(sc_owner, &sc_addr, "moon_coin", amount, chain_id));
    txns.push(dex_utils::fund_coin_tx(sc_owner, &sc_addr, "xrp_coin", amount, chain_id));
    let (good, bad, _, _, dur) = dex_utils::batch_submit(url, txns, 2, true).await;
    println!("self_fund_coins: amount {}, after waiting txns {:?}, good: {}, bad: {}", amount, dur, good, bad);
}

async fn create_book(sc_owner: &mut LocalAccount, chain_id: u8, url: Url)
{
    let sc_addr = sc_owner.address();
    let moon = dex_utils::create_type_tag("moon_coin", "MoonCoin", sc_addr.clone());
    let xrp = dex_utils::create_type_tag("xrp_coin", "XRPCoin", sc_addr);
    dex_utils::create_book_tx_send(url, sc_owner, moon, xrp, chain_id).await;
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
    let per_account: u64 = args[7].parse::<u64>().unwrap();
    let create_bk = args[8].parse::<bool>().unwrap();
    let owner_rg = args[9].parse::<bool>().unwrap();
    assert!(num_seeds % fanout == 0 && num_seeds / fanout > 1);
    assert!(per_account > 0 && submit_batch_size > 0);
    let per_spawn = num_seeds / fanout - 1;

    let start = Instant::now();
    let rest_client = Client::new(url.clone());
    let chain_id = rest_client.get_index().await.context("Failed to get chain ID")?.inner().chain_id;
    let (mut accounts, _) = dex_utils::recreate_accounts(
        url.clone(), start_seed, num_seeds).await;
    println!("total number of accounts {}, time: {:?}", accounts.len(), start.elapsed());
    let mut deputies = accounts.drain(accounts.len() - fanout as usize..).collect();
    let mut sc_owner = dex_utils::fill_sc_owner(url.clone(), &sk_str, chain_id, owner_rg).await;
    let sc_addr = sc_owner.address();
    self_fund_coins(&mut sc_owner, per_account * num_seeds, chain_id, url.clone()).await;
    if create_bk {
        create_book(&mut sc_owner, chain_id, url.clone()).await;
    }
    println!("funding deputies total {}, per_account {}", per_account * num_seeds / fanout, per_account);
    let (sc_owner, deputies) = fanout_multi(sc_addr.clone(), sc_owner, deputies,
                                            per_account * num_seeds / fanout, per_account,
                                            submit_batch_size, chain_id, url.clone()).await;

    let mut handles = Vec::new();
    for deputy in deputies {
        let mut herd = accounts.drain(accounts.len() - per_spawn as usize..).collect();
        //let mut deputy = deputies.pop().unwrap();
        let handle = tokio::task::spawn(
            fanout_multi(sc_addr.clone(), deputy, herd,
                         per_account, per_account,
                         submit_batch_size, chain_id, url.clone()));
        handles.push(handle);
    }

    assert!(accounts.is_empty());
    for h in handles {
        tokio::join!(h);
    }

    println!("Total time: {:?}", start.elapsed());
    Ok(())
}

// async fn fanout_fund_coins(mut sc_owner: LocalAccount, mut users: &mut Vec<LocalAccount>, amount: u64,
//                            submit_batch_size: usize, chain_id: u8, url: Url)
// {
//     let mut txns = Vec::new();
//     for alice in &mut users {
//         txns.push(dex_utils::fund_coin_tx(&mut sc_owner, &alice.address(), "moon_coin", amount, chain_id));
//         txns.push(dex_utils::fund_coin_tx(&mut sc_owner, &alice.address(), "xrp_coin", amount, chain_id));
//     }
//     let (good, bad, dur) = dex_utils::batch_submit(url, txns, submit_batch_size).await;
//     println!("after waiting txns {:?}, good: {}, bad: {}", dur, good, bad);
// }
