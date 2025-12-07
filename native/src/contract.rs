// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use linera_sdk::{
    abis::fungible::{
        Account as FungibleAccount, InitialState, Parameters,
    },
    linera_base_types::{Account, AccountOwner, ChainId, WithContractAbi},
    views::{RootView, View},
    Contract, ContractRuntime,
};
use native::{Message, TICKER_SYMBOL, NativeAbi, NativeOperation, NativeResponse};
use self::state::NativeState;

pub struct NativeContract {
    state: NativeState,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(NativeContract);

impl WithContractAbi for NativeContract {
    type Abi = NativeAbi;
}

impl Contract for NativeContract {
    type Message = Message;
    type Parameters = Parameters;
    type InstantiationArgument = InitialState;
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = NativeState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        NativeContract { state, runtime }
    }

    async fn instantiate(&mut self, initial_state: Self::InstantiationArgument) {
        // Validate ticker symbol
        assert!(
            self.runtime.application_parameters().ticker_symbol == "NAT",
            "Only NAT is accepted as ticker symbol"
        );
        
        // Initialize balances
        for (owner, amount) in initial_state.accounts {
            let account = Account {
                chain_id: self.runtime.chain_id(),
                owner,
            };
            self.runtime.transfer(AccountOwner::CHAIN, account, amount);
        }
    }

    async fn execute_operation(&mut self, operation: Self::Operation) -> Self::Response {
        match operation {
            NativeOperation::Balance { owner } => {
                let balance = self.runtime.owner_balance(owner);
                NativeResponse::Balance(balance)
            }

            NativeOperation::ChainBalance => {
                let balance = self.runtime.chain_balance();
                NativeResponse::ChainBalance(balance)
            }

            NativeOperation::TickerSymbol => {
                NativeResponse::TickerSymbol(String::from(TICKER_SYMBOL))
            }

            NativeOperation::Transfer {
                owner,
                amount,
                target_account,
            } => {
                self.runtime
                    .check_account_permission(owner)
                    .expect("Permission for Transfer operation");

                let target_account = self.normalize_account(target_account);
                self.runtime.transfer(owner, target_account, amount);
                
                // Send notify message for cross-chain transfers
                if target_account.chain_id != self.runtime.chain_id() {
                    self.transfer(target_account.chain_id);
                }

                NativeResponse::Ok
            }

            NativeOperation::Claim {
                source_account,
                amount,
                target_account,
            } => {
                self.runtime
                    .check_account_permission(source_account.owner)
                    .expect("Permission for Claim operation");

                let source_account = self.normalize_account(source_account);
                let target_account = self.normalize_account(target_account);

                self.runtime.claim(source_account, target_account, amount);
                
                self.claim(source_account.chain_id, target_account.chain_id);
                NativeResponse::Ok
            }

            NativeOperation::Withdraw => {
                let owner = self.runtime.authenticated_signer().unwrap();
                let balance = self.runtime.owner_balance(owner);
                let target_account = Account {
                    chain_id: self.runtime.chain_id(),
                    owner: AccountOwner::CHAIN,
                };
                self.runtime.transfer(owner, target_account, balance);
                NativeResponse::Ok
            }

            NativeOperation::Mint { owner, amount } => {
                let target_account = Account {
                    chain_id: self.runtime.chain_id(),
                    owner,
                };
                self.runtime.transfer(AccountOwner::CHAIN, target_account, amount);
                NativeResponse::Ok
            }
        }
    }

    async fn execute_message(&mut self, _message: Self::Message) {
        // Auto-deploy on notify
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}

impl NativeContract {
    fn transfer(&mut self, chain_id: ChainId) {
        if chain_id != self.runtime.chain_id() {
            let message = Message::Notify;
            self.runtime
                .prepare_message(message)
                .with_authentication()
                .send_to(chain_id);
        }
    }

    fn claim(&mut self, source_chain_id: ChainId, target_chain_id: ChainId) {
        if source_chain_id == self.runtime.chain_id() {
            self.transfer(target_chain_id);
        } else {
            let message = Message::Notify;
            self.runtime
                .prepare_message(message)
                .with_authentication()
                .send_to(source_chain_id);
        }
    }

    fn normalize_account(&self, account: FungibleAccount) -> Account {
        Account {
            chain_id: account.chain_id,
            owner: account.owner,
        }
    }
}