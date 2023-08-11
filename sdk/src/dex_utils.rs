#![allow(unused)]

// Copyright © Aptos Foundation

use std::collections::HashMap;
use std::mem;
use std::time::{Duration, Instant};
use anyhow::Context;
use futures::future::join_all;
use rand_core::SeedableRng;
use url::Url;
use aptos_crypto::ed25519::Ed25519PrivateKey;
use aptos_crypto::ValidCryptoMaterialStringExt;
use aptos_rest_client::Client;
use aptos_types::transaction::SignedTransaction;
use move_core_types::account_address::AccountAddress;
use move_core_types::identifier::Identifier;
use move_core_types::language_storage::{StructTag, TypeTag};
use crate::coin_client;
use crate::coin_client::CoinClient;
use crate::types::{AccountKey, LocalAccount};

const SQN_BATCH: u64 = 20;
// const MOON_COIN_STR: &str = "::moon_coin::MoonCoin";
// const XRP_COIN_STR: &str = "::xrp_coin::XRPCoin";
const APTO_BATCH: usize = 10;
const LIMIT_ORDER: u64 = 100;
pub const LOT: u64 = 10;
const TICK: u64 = 1;

pub fn create_type_tag(module_str: &str, name_str: &str, publisher_addr: AccountAddress) -> TypeTag {
    TypeTag::Struct(Box::new(StructTag {
        address: publisher_addr,
        module: Identifier::new(module_str).unwrap(),
        name: Identifier::new(name_str).unwrap(),
        type_params: vec![],
    }))
}

pub fn register_coin_tx(//coin_client: &'a CoinClient<'a>,
                        sc_addr: AccountAddress,
                        user: &mut LocalAccount,
                        coin_name: &str,
                        chain_id: u8) -> SignedTransaction {
    coin_client::build_simple_sc_call_tx(
        user,
        sc_addr,
        coin_name,
        "register",
        vec![],
        vec![],
        chain_id,
        None)
}

pub fn fund_coin_tx(//coin_client: &'a CoinClient<'a>,
                    sc_owner: &mut LocalAccount,
                    user: &AccountAddress,
                    coin_name: &str,
                    amount: u64,
                    chain_id: u8) -> SignedTransaction {
    coin_client::build_simple_sc_call_tx(
        sc_owner,
        sc_owner.address(),
        coin_name,
        "mint",
        vec![],
        vec![bcs::to_bytes(&user).unwrap(), bcs::to_bytes(&amount).unwrap()],
        chain_id,
        None)
}

pub fn transfer_coin_tx(sender: &mut LocalAccount,
                        receiver_addr: &AccountAddress,
                        coin_type: TypeTag,
                        amount: u64,
                        chain_id: u8) -> SignedTransaction {
    coin_client::build_simple_sc_call_tx(
        sender,
        AccountAddress::ONE,
        "coin",
        "transfer",
        vec![coin_type],
        vec![bcs::to_bytes(receiver_addr).unwrap(), bcs::to_bytes(&amount).unwrap()],
        chain_id,
        None)
}

pub fn deposit_tx(sc_addr: AccountAddress,
                  user: &mut LocalAccount,
                  coin: TypeTag,
                  amount: u64,
                  chain_id: u8, ) -> SignedTransaction {
    coin_client::build_simple_sc_call_tx(
        user,
        sc_addr,
        "vault",
        "deposit",
        vec![coin],
        vec![bcs::to_bytes(&amount).unwrap()],
        chain_id,
        None)
}

pub async fn create_book_tx_send(url: Url,
                                 sc_owner: &mut LocalAccount,
                                 base_coin: TypeTag,
                                 quote_coin: TypeTag,
                                 chain_id: u8,
)  {
    let rest_client = Client::new(url.clone());
    let coin_client = CoinClient::new(&rest_client);
    coin_client.build_simple_sc_call_tx_send(
        sc_owner,
        sc_owner.address(),
        "clob_market",
        "create_market",
        vec![base_coin, quote_coin],
        vec![bcs::to_bytes(&LOT).unwrap(), bcs::to_bytes(&TICK).unwrap()],
        chain_id,
        None).await;
}

pub fn trade_tx(//coin_client: &'a CoinClient<'a>,
                sc_addr: AccountAddress,
                trader: &mut LocalAccount,
                base_coin: TypeTag,
                quote_coin: TypeTag,
                is_bid: bool,
                limit_price: u64,
                quantity: u64,
                chain_id: u8,
) -> SignedTransaction {
    coin_client::build_simple_sc_call_tx(
        trader,
        sc_addr,
        "clob_market",
        "place_order",
        vec![base_coin, quote_coin],
        vec![
            bcs::to_bytes(&is_bid).unwrap(),
            bcs::to_bytes(&limit_price).unwrap(),
            bcs::to_bytes(&quantity).unwrap(),
            bcs::to_bytes(&LIMIT_ORDER).unwrap()],
        chain_id,
        None)
}

fn cancel_order_tx(//coin_client: &'a CoinClient<'a>,
                   trader: &mut LocalAccount,
                   sc_addr: AccountAddress,
                   base_coin: TypeTag,
                   quote_coin: TypeTag,
                   order_id: u128,
                   chain_id: u8,
) -> SignedTransaction {
    coin_client::build_simple_sc_call_tx(
        trader,
        sc_addr,
        "clob_market",
        "cancel_order",
        vec![base_coin, quote_coin],
        vec![bcs::to_bytes(&order_id).unwrap()],
        chain_id,
        None)
}

fn withdraw_all_tx(//coin_client: &'a CoinClient<'a>,
                   trader: &mut LocalAccount,
                   sc_addr: AccountAddress,
                   coin: TypeTag,
                   chain_id: u8,
) -> SignedTransaction {
    coin_client::build_simple_sc_call_tx(
        trader,
        sc_addr,
        "vault",
        "withdraw_all_available",
        vec![coin],
        vec![],
        chain_id,
        None)
}

async fn account_balance(rest_client: &Client,
                         trader: &mut LocalAccount,
                         sc_addr: AccountAddress,
                         coin_str: &str) {//-> u64 {
    let coin_string = ["0x1::coin::CoinStore<", sc_addr.to_hex_literal().as_str(), coin_str, ">"].concat();
    let v = rest_client.get_account_resource(trader.address(), coin_string.as_str())
        .await.unwrap().inner().clone();
    let vo = v.as_ref().unwrap().data.as_object().unwrap().get("coin").unwrap()
        .as_object().unwrap().get("value").unwrap().as_str().unwrap();
    println!("coin_balance: {:?}", vo);
}

pub async fn fill_sc_owner(url: Url,
                           sk_str: &str,
                           chain_id: u8,
                           register: bool
) -> LocalAccount {
    let rest_client = Client::new(url.clone());
    let coin_client = CoinClient::new(&rest_client);
    let sc_ss = Ed25519PrivateKey::from_encoded_string(sk_str).unwrap();
    let sc_ak = AccountKey::from_private_key(sc_ss);
    let sc_addr = sc_ak.authentication_key().derived_address();
    let sc_sqn = rest_client.get_account(sc_addr.clone()).
        await.context("Failed to get account").unwrap().inner().sequence_number;
    let mut sc_owner = LocalAccount::new(sc_addr.clone(), sc_ak, sc_sqn);

    if register {
        coin_client.build_simple_sc_call_tx_send(
            &mut sc_owner,
            sc_addr.clone(),
            "moon_coin",
            "register",
            vec![],
            vec![],
            chain_id,
            None).await;

        coin_client.build_simple_sc_call_tx_send(
            &mut sc_owner,
            sc_addr.clone(),
            "xrp_coin",
            "register",
            vec![],
            vec![],
            chain_id,
            None).await;
    }

    sc_owner
}

pub async fn batch_submit(url: Url, txns: Vec<SignedTransaction>, submit_batch_size: usize, wait_valid: bool)
                          -> (u32, u32, u32, u32, Duration) {
    let rest_client = Client::new(url);
    let start = Instant::now();

    let mut txns_results = Vec::new();
    let num_txns = txns.len();
    assert!(num_txns > 0);
    let mut tx_idx = 0usize;
    let mut batch = Vec::new();
    let mut batches = Vec::new();
    let mut tx_hashes = Vec::new();
    while tx_idx < num_txns {
        batch.push(txns[tx_idx].clone());
//        let h = txns[tx_idx].committed_hash();
        tx_hashes.push(txns[tx_idx].clone().committed_hash());
        tx_idx += 1;
        let finished = tx_idx == num_txns;
        if tx_idx % APTO_BATCH == 0 || finished
        {
            let mut temp = Vec::new();
            mem::swap(&mut temp, &mut batch);
            batches.push(temp);
            if batches.len() % submit_batch_size == 0 || finished
            {
                let mut results: Vec<_> = Vec::new();
                for tx_batch in &mut batches {
                    results.push(rest_client.submit_batch(tx_batch));
                }
                let mut round_txns_results = join_all(results).await;
                txns_results.append(&mut round_txns_results);
                if finished {
                    break;
                }
            }
        }
    }
    //println!("before waiting txns {:?}", start.elapsed());

    let mut submit_failures: u32 = 0;
    let mut submit_successes: u32 = 0;
    for r in txns_results {
        match r {
            Ok(tx) => {
                submit_successes += 1;
            }
            Err(_) => {
                submit_failures += 1;
            }
        }
    }
    let mut wait_successes = 0;
    let mut wait_failures = 0;
    let timeout_secs = 30000u64;
    if submit_failures == 0 && wait_valid {
        for h in tx_hashes {
            if rest_client.wait_for_transaction_by_hash(h, timeout_secs,
                                                        None, None)
                .await.unwrap().inner().success(){
                wait_successes += 1;
            }
            else{
                wait_failures += 1;
            }
        }
    }

    (submit_successes, submit_failures, wait_successes, wait_failures, start.elapsed())
}


/**
 * create account array, segment by segment.
 * query chain for sqn
 */
pub async fn recreate_accounts(url: Url, start_seed: u64, num_seeds: u64) -> (Vec<LocalAccount>, Vec<AccountAddress>) {
    let start: Instant = Instant::now();
    let rest_client = Client::new(url);
    let mut accounts: Vec<LocalAccount> = Vec::new();
    let mut addresses: Vec<AccountAddress> = Vec::new();
    let last_seed = start_seed + num_seeds - 1;
    let mut seed = start_seed;

    loop {
        let mut account_map = HashMap::new();
        let mut results: Vec<_> = Vec::new();
        for _ in 0..SQN_BATCH {
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
