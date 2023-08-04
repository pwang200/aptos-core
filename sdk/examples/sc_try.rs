// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0
#![allow(unused)]

use std::str::FromStr;

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use url::Url;

use aptos_sdk::{
    coin_client::CoinClient,
    move_types::identifier::Identifier,
    rest_client::{Client, FaucetClient},
    types::account_address::AccountAddress,
    types::LocalAccount,
};

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

#[tokio::main]
async fn main() -> Result<()> {
    let rest_client = Client::new(NODE_URL.clone());
    let faucet_client = FaucetClient::new(FAUCET_URL.clone(), NODE_URL.clone()); // <:!:section_1a
    let coin_client = CoinClient::new(&rest_client); // <:!:section_1b

    let chain_id = rest_client
        .get_index()
        .await
        .context("Failed to get chain ID")?
        .inner()
        .chain_id;
    println!("chain-id {}", chain_id);

    let mut alice = LocalAccount::generate(&mut rand::rngs::OsRng);
    println!("Alice address: {}", alice.address().to_hex_literal());
    faucet_client
        .fund(alice.address(), 1000_000_000)
        .await
        .context("Failed to fund Alice's account")?;

    let sc_address = AccountAddress::from_hex_literal(
        "0xd20172e611f6371378204c0bbbd74ac6e31a833a1e3acb5888658d8a562d396b").unwrap();
    let sc_name = Identifier::from_str("user_info").unwrap();
    let sc_func = Identifier::from_str("set_age").unwrap();
    let sc_set_value: u8 = 11;
    let signed_tx = coin_client.build_simple_sc_call_tx(
        &mut alice,
        sc_address,
        "user_info"sc_name,
        sc_func,
        vec![],
        vec![bcs::to_bytes(&sc_set_value).unwrap()],
        chain_id,
        None);
    let pending_tx = rest_client.submit(&signed_tx)
        .await.context("Failed to submit transfer transaction")?.into_inner();
    rest_client
        .wait_for_transaction(&pending_tx)
        .await
        .context("Failed when waiting for the transfer transaction")?;

    let v = rest_client.get_account_resource(
        alice.address(),
        "0xd20172e611f6371378204c0bbbd74ac6e31a833a1e3acb5888658d8a562d396b::user_info::UserInfo")
        .await.unwrap().inner().clone().unwrap().data.as_object().unwrap().get("age").unwrap().as_u64().unwrap();
    println!("Alice age: {:?}", v);
    println!(
        "Alice balance: {:?}",
        coin_client
            .get_account_balance(&alice.address())
            .await
            .context("Failed to get Alice's account balance")?
    );

    Ok(())
}
