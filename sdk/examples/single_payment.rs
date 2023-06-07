// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use aptos_sdk::{
    coin_client::CoinClient,
    rest_client::{Client, FaucetClient},
    types::LocalAccount,
};
use once_cell::sync::Lazy;
use std::str::FromStr;
use url::Url;

#[tokio::main]
async fn main() -> Result<()> {
    let rest_client = Client::new(Url::from_str("http://127.0.0.1:8080").unwrap());
    let coin_client = CoinClient::new(&rest_client);
    let key = bcs::from_bytes(&std::fs::read(PathBuf::from_str("/home/pwang/aptos/test_configs/single/mint.key").unwrap().as_path()).unwrap()).unwrap();

    let mut alice = LocalAccount::new(AuthenticationKey::ed25519(&Ed25519PublicKey::from(&key)).derived_address(), key, 0);
    let bob = LocalAccount::generate(&mut rand::rngs::OsRng);
    match rest_client.get_account(alice.address()).await {
        Ok(r) => {
            let a_info = rest_client.get_account(alice.address()).await.unwrap();
            *alice.sequence_number_mut() = a_info.inner().sequence_number;
            println!("Alice {}, info {:?}", alice.address(), a_info.inner());
        },
        Err(e) => {
            println!("alice account {:?} error {:?}", alice.address(), e);
            panic!("account creation");
        },
    }

    // Print account addresses.
    println!("\n=== Addresses ===");
    println!("Alice: {}", alice.address().to_hex_literal());
    println!("Bob: {}", bob.address().to_hex_literal());

    // Have Alice send Bob some coins.
    let start = Instant::now();
    let tx = coin_client.create_and_pay(&mut alice, bob.address(), 1_000, 4, None);
    let tx = rest_client.submit(&tx).await
        .context("Failed to submit transaction to transfer coins")?;
    rest_client
        .wait_for_transaction(tx.inner())
        .await
        .context("Failed when waiting for the transfer transaction")?;
    println!("Total time: {:?}", start.elapsed());

    Ok(());
}
