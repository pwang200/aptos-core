// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::{
    bcs,
    move_types::{
        identifier::Identifier,
        language_storage::{ModuleId, TypeTag},
    },
    rest_client::{Client as ApiClient, PendingTransaction},
    transaction_builder::{TransactionBuilder, TransactionFactory},
    types::{
        account_address::AccountAddress,
        chain_id::ChainId,
        transaction::{EntryFunction, TransactionPayload},
        LocalAccount,
    },
};
use anyhow::{Context, Result};
use std::{
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug)]
pub struct CoinClient<'a> {
    api_client: &'a ApiClient,
}

impl<'a> CoinClient<'a> {
    pub fn new(api_client: &'a ApiClient) -> Self {
        Self { api_client }
    }

    pub async fn transfer(
        &self,
        from_account: &mut LocalAccount,
        to_account: AccountAddress,
        amount: u64,
        options: Option<TransferOptions<'_>>,
    ) -> Result<PendingTransaction> {
        let options = options.unwrap_or_default();

        // :!:>section_1
        let chain_id = self
            .api_client
            .get_index()
            .await
            .context("Failed to get chain ID")?
            .inner()
            .chain_id;

        println!("chain-id {}", chain_id);

        let transaction_builder = TransactionBuilder::new(
            TransactionPayload::EntryFunction(EntryFunction::new(
                ModuleId::new(AccountAddress::ONE, Identifier::new("coin").unwrap()),
                Identifier::new("transfer").unwrap(),
                vec![TypeTag::from_str(options.coin_type).unwrap()],
                vec![
                    bcs::to_bytes(&to_account).unwrap(),
                    bcs::to_bytes(&amount).unwrap(),
                ],
            )),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + options.timeout_secs,
            ChainId::new(chain_id),
        )
        .sender(from_account.address())
        .sequence_number(from_account.sequence_number())
        .max_gas_amount(options.max_gas_amount)
        .gas_unit_price(options.gas_unit_price);
        let signed_txn = from_account.sign_with_transaction_builder(transaction_builder);
        Ok(self
            .api_client
            .submit(&signed_txn)
            .await
            .context("Failed to submit transfer transaction")?
            .into_inner())
        // <:!:section_1
    }

    pub fn create_and_pay(
        &self,
        from_account: &mut LocalAccount,
        to_account: AccountAddress,
        amount: u64,
        chain_id: u8,
        options: Option<TransferOptions<'_>>,
    ) -> aptos_types::transaction::SignedTransaction {
        let options = options.unwrap_or_default();

        let tx_factory = TransactionFactory::new(ChainId::new(chain_id))
            .with_max_gas_amount(options.max_gas_amount)
            .with_gas_unit_price(options.gas_unit_price)
            .with_transaction_expiration_time(options.timeout_secs);
        let tx_builder = tx_factory
            .account_transfer(to_account, amount)
            .sender(from_account.address())
            .sequence_number(from_account.sequence_number());
        from_account.sign_with_transaction_builder(tx_builder)
    }

    pub fn build(
        &self,
        from_account: &mut LocalAccount,
        to_account: AccountAddress,
        amount: u64,
        chain_id: u8,
        options: Option<TransferOptions<'_>>,
    ) -> aptos_types::transaction::SignedTransaction {
        let options = options.unwrap_or_default();

        let transaction_builder = TransactionBuilder::new(
            TransactionPayload::EntryFunction(EntryFunction::new(
                ModuleId::new(AccountAddress::ONE, Identifier::new("coin").unwrap()),
                Identifier::new("transfer").unwrap(),
                vec![TypeTag::from_str(options.coin_type).unwrap()],
                vec![
                    bcs::to_bytes(&to_account).unwrap(),
                    bcs::to_bytes(&amount).unwrap(),
                ],
            )),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + options.timeout_secs,
            ChainId::new(chain_id),
        )
        .sender(from_account.address())
        .sequence_number(from_account.sequence_number())
        .max_gas_amount(options.max_gas_amount)
        .gas_unit_price(options.gas_unit_price);
        from_account.sign_with_transaction_builder(transaction_builder)
    }

    pub fn build_simple_sc_call_tx(
        &self,
        tx_signer_account: &mut LocalAccount,
        sc_addr: AccountAddress,
        sc_name: Identifier,
        func_name: Identifier,
        ty_args: Vec<TypeTag>,
        args: Vec<Vec<u8>>,
        chain_id: u8,
        options: Option<TransferOptions<'_>>,
    ) -> aptos_types::transaction::SignedTransaction {
        let options = options.unwrap_or_default();

        let transaction_builder = TransactionBuilder::new(
            TransactionPayload::EntryFunction(EntryFunction::new(
                ModuleId::new(sc_addr, sc_name),
                func_name,
                ty_args,
                args,
            )),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + options.timeout_secs,
            ChainId::new(chain_id),
        )
            .sender(tx_signer_account.address())
            .sequence_number(tx_signer_account.sequence_number())
            .max_gas_amount(options.max_gas_amount)
            .gas_unit_price(options.gas_unit_price);
        tx_signer_account.sign_with_transaction_builder(transaction_builder)
    }

    pub async fn build_simple_sc_call_tx_send(
        &self,
        tx_signer_account: &mut LocalAccount,
        sc_addr: AccountAddress,
        sc_module_name: &str,
        sc_func_name: &str,
        ty_args: Vec<TypeTag>,
        args: Vec<Vec<u8>>,
        chain_id: u8,
        options: Option<TransferOptions<'_>>,
    ) {
        let sc_module = Identifier::from_str(sc_module_name).unwrap();
        let sc_func = Identifier::from_str(sc_func_name).unwrap();
        let signed_tx = self.build_simple_sc_call_tx(
            tx_signer_account,
            sc_addr,
            sc_module,
            sc_func,
            ty_args,
            args,
            chain_id,
            options);
        let pending_tx = self.api_client.submit(&signed_tx)
            .await.context("Failed to submit the create_market transaction").unwrap().inner().clone();

        self.api_client
            .wait_for_transaction(&pending_tx)
            .await.unwrap();
            //.context("Failed when waiting for the create_market transaction");
    }

    pub async fn get_account_balance(&self, account: &AccountAddress) -> Result<u64> {
        let response = self
            .api_client
            .get_account_balance(*account)
            .await
            .context("Failed to get account balance")?;
        Ok(response.inner().get())
    }
}

pub struct TransferOptions<'a> {
    pub max_gas_amount: u64,

    pub gas_unit_price: u64,

    /// This is the number of seconds from now you're willing to wait for the
    /// transaction to be committed.
    pub timeout_secs: u64,

    /// This is the coin type to transfer.
    pub coin_type: &'a str,
}

impl<'a> Default for TransferOptions<'a> {
    fn default() -> Self {
        Self {
            max_gas_amount: 5_000,
            gas_unit_price: 100,
            timeout_secs: 30000,
            coin_type: "0x1::aptos_coin::AptosCoin",
        }
    }
}
