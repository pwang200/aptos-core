// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use aptos_sdk::{
    coin_client::CoinClient,
    rest_client::{Client, FaucetClient},
    types::LocalAccount,
};
use std::str::FromStr;
use std::time::Instant;
use url::Url;

#[tokio::main]
async fn main() -> Result<()> {
    // :!:>section_1a
    let node_url = Url::from_str("http://127.0.0.1:8080").unwrap();
    let fauc_url = Url::from_str("http://127.0.0.1:8081").unwrap();
    let rest_client = Client::new(node_url.clone());
    let coin_client = CoinClient::new(&rest_client);
    let faucet_client = FaucetClient::new(fauc_url.clone(), node_url.clone());

    let mut alice = LocalAccount::generate(&mut rand::rngs::OsRng);
    let bob = LocalAccount::generate(&mut rand::rngs::OsRng);

    // Print account addresses.
    println!("\n=== Addresses ===");
    println!("Alice: {}", alice.address().to_hex_literal());
    println!("Bob: {}", bob.address().to_hex_literal());

    // Create the accounts on chain, but only fund Alice.
    // :!:>section_3
    faucet_client
        .fund(alice.address(), 1000_000_000)
        .await
        .context("Failed to fund Alice's account")?;
    faucet_client
        .create_account(bob.address())
        .await
        .context("Failed to fund Bob's account")?; // <:!:section_3

    // Print initial balances.
    println!("\n=== Initial Balances ===");
    println!(
        "Alice: {:?}",
        coin_client
            .get_account_balance(&alice.address())
            .await
            .context("Failed to get Alice's account balance")?
    );
    println!(
        "Bob: {:?}",
        coin_client
            .get_account_balance(&bob.address())
            .await
            .context("Failed to get Bob's account balance")?
    );

    // Have Alice send Bob some coins.
    let start = Instant::now();
    let txn_hash = coin_client
        .transfer(&mut alice, bob.address(), 1_000, None)
        .await
        .context("Failed to submit transaction to transfer coins")?;
    rest_client
        .wait_for_transaction(&txn_hash)
        .await
        .context("Failed when waiting for the transfer transaction")?;
    println!("Total time: {:?}", start.elapsed());

    // Print intermediate balances.
    println!("\n=== End Balances ===");
    // :!:>section_4
    println!(
        "Alice: {:?}",
        coin_client
            .get_account_balance(&alice.address())
            .await
            .context("Failed to get Alice's account balance the second time")?
    );
    println!(
        "Bob: {:?}",
        coin_client
            .get_account_balance(&bob.address())
            .await
            .context("Failed to get Bob's account balance the second time")?
    ); // <:!:section_4



    Ok(())
}
