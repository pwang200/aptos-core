// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0
#![allow(unused)]

use std::clone::Clone;
use anyhow::{Context, Result};
use aptos_sdk::{
    coin_client::CoinClient,
    rest_client::{Client, FaucetClient},
    types::LocalAccount,
};
use once_cell::sync::Lazy;
use std::str::FromStr;
use std::string::ToString;
use rand_core::SeedableRng;
use url::Url;
use aptos_crypto::ed25519::Ed25519PrivateKey;
use aptos_crypto::{SigningKey, ValidCryptoMaterialStringExt};
use aptos_sdk::types::AccountKey;
use move_core_types::account_address::AccountAddress;
use move_core_types::identifier::Identifier;
use move_core_types::language_storage::{StructTag, TypeTag};

static NODE_URL: Lazy<Url> = Lazy::new(|| {
    Url::from_str(
        std::env::var("APTOS_NODE_URL")
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("https://fullnode.devnet.aptoslabs.com"),
    )
        .unwrap()
});

static FAUCET_URL: Lazy<Url> = Lazy::new(|| {
    Url::from_str(
        std::env::var("APTOS_FAUCET_URL")
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("https://faucet.devnet.aptoslabs.com"),
    )
        .unwrap()
});

const SC_ADDRESS_STR: &str = "0xc7209866b9d94175efdd575a1b5b54d6184d5e39f3f4bcb59d4c9a32453c8a32";

const SC_ADDRESS: Lazy<AccountAddress> = Lazy::new(|| {
    AccountAddress::from_hex_literal(SC_ADDRESS_STR)        .unwrap()
});

const DOG_COIN: Lazy<TypeTag> = Lazy::new(|| {
    TypeTag::Struct(Box::new(StructTag {
        address: *SC_ADDRESS,
        module: Identifier::new("dog_coin").unwrap(),
        name: Identifier::new("DogCoin").unwrap(),
        type_params: vec![],
    }))
});

const MOON_COIN: Lazy<TypeTag> = Lazy::new(|| {
    TypeTag::Struct(Box::new(StructTag {
        address: *SC_ADDRESS,
        module: Identifier::new("moon_coin").unwrap(),
        name: Identifier::new("MoonCoin").unwrap(),
        type_params: vec![],
    }))
});

const XRP_COIN: Lazy<TypeTag> = Lazy::new(|| {
    TypeTag::Struct(Box::new(StructTag {
        address: *SC_ADDRESS,
        module: Identifier::new("xrp_coin").unwrap(),
        name: Identifier::new("XRPCoin").unwrap(),
        type_params: vec![],
    }))
});

const DOG_COIN_STR: &str = "::dog_coin::DogCoin";
const MOON_COIN_STR: &str = "::moon_coin::MoonCoin";
const XRP_COIN_STR: &str  = "::xrp_coin::XRPCoin";

const FUND_AMOUNT: u64 = 1000_000_000;
const LIMIT_ORDER: u64 = 100;

async fn register_coin<'a>(coin_client: &'a CoinClient<'a>,
                                sc_owner: & mut LocalAccount,
                                user: & mut LocalAccount,
                                coin_name: & str,
                                chain_id: u8) {
    coin_client.build_simple_sc_call_tx_send(
        user,
        sc_owner.address(),
        coin_name,
        "register",
        vec![],
        vec![],
        chain_id,
        None).await;
}
async fn fund_coin<'a>(coin_client: &'a CoinClient<'a>,
                                sc_owner: & mut LocalAccount,
                                user: & mut LocalAccount,
                                coin_name: & str,
                                chain_id: u8) {
    coin_client.build_simple_sc_call_tx_send(
        sc_owner,
        sc_owner.address(),
        coin_name,
        "mint",
        vec![],
        vec![bcs::to_bytes(&user.address()).unwrap(), bcs::to_bytes(&FUND_AMOUNT).unwrap()],
        chain_id,
        None).await;
}
async fn deposit<'a>(coin_client: &'a CoinClient<'a>,
                     sc_owner: &mut LocalAccount,
                     user: &mut LocalAccount,
                     coin: TypeTag,
                     chain_id: u8,) {
    coin_client.build_simple_sc_call_tx_send(
        user,
        sc_owner.address(),
        "vault",
        "deposit",
        vec![coin],
        vec![bcs::to_bytes(&FUND_AMOUNT).unwrap()],
        chain_id,
        None).await;
}

async fn create_trader<'a>(faucet_client: & FaucetClient,
                           coin_client: &'a CoinClient<'a>,
                           sc_owner: &mut LocalAccount,
                           chain_id: u8,) -> LocalAccount {
    let mut alice = LocalAccount::generate(&mut rand::rngs::OsRng);
    println!("created account: {}", alice.address().to_hex_literal());
    faucet_client
        .fund(alice.address(), 1000_000_000)
        .await
        .context("Failed to fund account");

    // //register_fund_coin(coin_client, sc_owner, &mut alice, "dog_coin", chain_id).await;
    // register_fund_coin(coin_client, sc_owner, &mut alice, "moon_coin", chain_id).await;
    // register_fund_coin(coin_client, sc_owner, &mut alice, "xrp_coin", chain_id).await;
    // //deposit(coin_client, sc_owner, &mut alice, *DOG_COIN, chain_id).await;
    // deposit(coin_client, sc_owner, &mut alice, (*MOON_COIN).clone(), chain_id).await;
    // deposit(coin_client, sc_owner, &mut alice, (*XRP_COIN).clone(), chain_id).await;
    alice
}

async fn create_book<'a>(coin_client: &'a CoinClient<'a>,
                         sc_owner: &mut LocalAccount,
                         base_coin: TypeTag,
                         quote_coin: TypeTag,
                         chain_id: u8,
) {
    let sc_lot: u64 = 100;
    let sc_tick: u64 = 1;
    coin_client.build_simple_sc_call_tx_send(
        sc_owner,
        *SC_ADDRESS,
        "clob_market",
        "create_market",
        vec![base_coin, quote_coin],
        vec![bcs::to_bytes(&sc_lot).unwrap(), bcs::to_bytes(&sc_tick).unwrap()],
        chain_id,
        None).await;
}

async fn fill_sc_owner<'a>(rest_client: & Client,
                           coin_client: &'a CoinClient<'a>,
                           chain_id: u8,
) -> LocalAccount {
    let sc_ss = Ed25519PrivateKey::from_encoded_string(
        "0xe15ba4a3b0317f045b4461f3b1578db8041f50d2259372501ad7829d4d0b6f46").unwrap();
    let sc_ak = AccountKey::from_private_key(sc_ss);
    let sc_sqn = rest_client.get_account(*SC_ADDRESS).
        await.context("Failed to get account").unwrap().inner().sequence_number;
    let mut sc_owner = LocalAccount::new(*SC_ADDRESS, sc_ak, sc_sqn);

    let sc_owner_addr = sc_owner.address();

    coin_client.build_simple_sc_call_tx_send(
        &mut sc_owner,
        sc_owner_addr.clone(),
        "moon_coin",
        "register",
        vec![],
        vec![],
        chain_id,
        None).await;

    coin_client.build_simple_sc_call_tx_send(
        &mut sc_owner,
        sc_owner_addr.clone(),
        "xrp_coin",
        "register",
        vec![],
        vec![],
        chain_id,
        None).await;

    sc_owner
}

async fn trade<'a>(coin_client: &'a CoinClient<'a>,
                   trader: &mut LocalAccount,
                   base_coin: TypeTag,
                   quote_coin: TypeTag,
                   is_bid: bool,
                   limit_price: u64,
                   quantity: u64,
                   chain_id: u8,
) {
    coin_client.build_simple_sc_call_tx_send(
        trader,
        *SC_ADDRESS,
        "clob_market",
        "place_order",
        vec![base_coin, quote_coin],
        vec![
            bcs::to_bytes(&is_bid).unwrap(),
            bcs::to_bytes(&limit_price).unwrap(),
            bcs::to_bytes(&quantity).unwrap(),
            bcs::to_bytes(&LIMIT_ORDER).unwrap()],
        chain_id,
        None).await;
}

async fn cancel_order<'a>(coin_client: &'a CoinClient<'a>,
                          trader: &mut LocalAccount,
                          base_coin: TypeTag,
                          quote_coin: TypeTag,
                          order_id: u128,
                          chain_id: u8,
) {
    coin_client.build_simple_sc_call_tx_send(
        trader,
        *SC_ADDRESS,
        "clob_market",
        "cancel_order",
        vec![base_coin, quote_coin],
        vec![bcs::to_bytes(&order_id).unwrap()],
        chain_id,
        None).await;
}

async fn withdraw_all<'a>(coin_client: &'a CoinClient<'a>,
                          trader: &mut LocalAccount,
                          coin: TypeTag,
                          chain_id: u8,
) {
    coin_client.build_simple_sc_call_tx_send(
        trader,
        *SC_ADDRESS,
        "vault",
        "withdraw_all_available",
        vec![coin],
        vec![],
        chain_id,
        None).await;
}

async fn account_balance(rest_client: & Client,
                         trader: &mut LocalAccount,
                         coin_str: &str) {//-> u64 {
    let coin_string= ["0x1::coin::CoinStore<", SC_ADDRESS_STR, coin_str, ">"].concat();
    let v = rest_client.get_account_resource(trader.address(), coin_string.as_str())
        .await.unwrap().inner().clone();
    let vo = v.as_ref().unwrap().data.as_object().unwrap().get("coin").unwrap()
        .as_object().unwrap().get("value").unwrap().as_str().unwrap();
    println!("coin_balance: {:?}", vo);
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut rest_client = Client::new(NODE_URL.clone());
    let mut faucet_client = FaucetClient::new(FAUCET_URL.clone(), NODE_URL.clone());
    let mut coin_client = CoinClient::new(&rest_client);
    let chain_id = rest_client.get_index().await.context("Failed to get chain ID")?.inner().chain_id;

    let mut sc_owner = fill_sc_owner(& rest_client, & coin_client, chain_id).await;
    //TODO create_book if needed, e.g. new coin pair, new contracts
    //create_book(&mut coin_client, &mut sc_owner, (*MOON_COIN).clone(), (*XRP_COIN).clone(), chain_id).await;

    println!("0");
    let mut alice = create_trader(& faucet_client, & coin_client, &mut sc_owner, chain_id).await;
    register_coin(& coin_client, &mut sc_owner, &mut alice, "moon_coin", chain_id).await;
    register_coin(& coin_client, &mut sc_owner, &mut alice, "xrp_coin", chain_id).await;
    fund_coin(& coin_client, &mut sc_owner, &mut alice, "moon_coin", chain_id).await;
    account_balance(& rest_client, &mut alice, MOON_COIN_STR).await;
    account_balance(& rest_client, &mut alice, XRP_COIN_STR).await;
    deposit(& coin_client, &mut sc_owner, &mut alice, (*MOON_COIN).clone(), chain_id).await;
    account_balance(& rest_client, &mut alice, MOON_COIN_STR).await;
    account_balance(& rest_client, &mut alice, XRP_COIN_STR).await;

    println!("1");
    let mut bob = create_trader(& faucet_client, & coin_client, &mut sc_owner, chain_id).await;
    register_coin(& coin_client, &mut sc_owner, &mut bob, "moon_coin", chain_id).await;
    register_coin(& coin_client, &mut sc_owner, &mut bob, "xrp_coin", chain_id).await;
    fund_coin(& coin_client, &mut sc_owner, &mut bob, "xrp_coin", chain_id).await;
    account_balance(& rest_client, &mut bob, MOON_COIN_STR).await;
    account_balance(& rest_client, &mut bob, XRP_COIN_STR).await;
    deposit(& coin_client, &mut sc_owner, &mut bob, (*XRP_COIN).clone(), chain_id).await;
    account_balance(& rest_client, &mut bob, MOON_COIN_STR).await;
    account_balance(& rest_client, &mut bob, XRP_COIN_STR).await;

    println!("2");
    trade(&mut coin_client,
          &mut alice,
          (*MOON_COIN).clone(),
           (*XRP_COIN).clone(),
          false,
          10000,
          200,
          chain_id,
    ).await;
    account_balance(& rest_client, &mut alice, MOON_COIN_STR).await;
    account_balance(& rest_client, &mut alice, XRP_COIN_STR).await;

    println!("3");
    trade(&mut coin_client,
          &mut bob,
          (*MOON_COIN).clone(),
           (*XRP_COIN).clone(),
          true,
          10000,
          100000,
          chain_id,
    ).await;
    account_balance(& rest_client, &mut alice, MOON_COIN_STR).await;
    account_balance(& rest_client, &mut alice, XRP_COIN_STR).await;

    withdraw_all(& coin_client, &mut alice, (*MOON_COIN).clone(), chain_id).await;
    account_balance(& rest_client, &mut alice, MOON_COIN_STR).await;
    account_balance(& rest_client, &mut alice, XRP_COIN_STR).await;

    withdraw_all(& coin_client, &mut alice, (*XRP_COIN).clone(), chain_id).await;
    account_balance(& rest_client, &mut alice, MOON_COIN_STR).await;
    account_balance(& rest_client, &mut alice, XRP_COIN_STR).await;

    println!("4");
    withdraw_all(& coin_client, &mut bob, (*MOON_COIN).clone(), chain_id).await;
    withdraw_all(& coin_client, &mut bob, (*XRP_COIN).clone(), chain_id).await;
    account_balance(& rest_client, &mut bob, MOON_COIN_STR).await;
    account_balance(& rest_client, &mut bob, XRP_COIN_STR).await;


    // cancel_order(&mut coin_client,
    //              &mut alice,
    //              (*MOON_COIN).clone(),
    //              (*XRP_COIN).clone(),
    //              0u128,
    //              chain_id,
    // ).await;
    //
    // withdraw_all(& coin_client, &mut alice, (*MOON_COIN).clone(), chain_id).await;
    // account_balance(& rest_client, &mut alice, MOON_COIN_STR).await;
    // account_balance(& rest_client, &mut alice, XRP_COIN_STR).await;
    //
    // withdraw_all(& coin_client, &mut alice, (*XRP_COIN).clone(), chain_id).await;
    // account_balance(& rest_client, &mut alice, MOON_COIN_STR).await;
    // account_balance(& rest_client, &mut alice, XRP_COIN_STR).await;

    Ok(())
}
