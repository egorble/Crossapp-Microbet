// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use linera_sdk::{
    linera_base_types::{AccountOwner, ChainId, WithContractAbi},
    views::{RootView, View},
    Contract, ContractRuntime,
};
use lottery_abi::{
    LotteryAppAbi, LotteryAppOperation, LotteryAppResponse, LotteryAppMessage as Message,
    LotteryAppParameters, LotteryRoundsAbi, LotteryRoundsOperation, LotteryRoundsResponse,
};
use self::state::LotteryAppState;

pub struct LotteryAppContract {
    state: LotteryAppState,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(LotteryAppContract);

impl WithContractAbi for LotteryAppContract {
    type Abi = LotteryAppAbi;
}

impl Contract for LotteryAppContract {
    type Message = Message;
    type Parameters = LotteryAppParameters;
    type InstantiationArgument = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = LotteryAppState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        LotteryAppContract { state, runtime }
    }

    async fn instantiate(&mut self, _arg: Self::InstantiationArgument) {
        // Validate params access
        let _ = self.runtime.application_parameters();
        self.state.initialized.set(true);
    }

    async fn execute_operation(&mut self, operation: Self::Operation) -> Self::Response {
        match operation {
            LotteryAppOperation::Transfer {
                owner,
                amount,
                target_account,
                purchase_tickets,
            } => {
                eprintln!("LotteryApp::Transfer - owner: {:?}, amount: {:?}, target: {:?}, purchase_tickets: {}", 
                    owner, amount, target_account, purchase_tickets);
                
                self.runtime
                    .check_account_permission(owner)
                    .expect("Permission for Transfer operation");

                let params = self.runtime.application_parameters();
                eprintln!("LotteryApp::Transfer - native_app_id from params: {:?}", params.native_app_id);
                
                let native_app_id = params.native_app_id.with_abi::<native::NativeAbi>();
                let lottery_rounds_app_id = params.lottery_rounds_app_id.with_abi::<LotteryRoundsAbi>();

                eprintln!("LotteryApp::Transfer - Calling native::Transfer...");
                
                // Step 1: Call Native app to transfer tokens
                let native_response: native::NativeResponse = self.runtime.call_application(
                    true,
                    native_app_id,
                    &native::NativeOperation::Transfer {
                        owner,
                        amount,
                        target_account,
                    },
                );
                
                eprintln!("LotteryApp::Transfer - native response: {:?}", native_response);

                // Step 2: If purchase_tickets, register tickets in lottery-rounds
                if purchase_tickets {
                    eprintln!("LotteryApp::Transfer - Calling lottery-rounds::PurchaseTickets...");
                    if target_account.chain_id == self.runtime.chain_id() {
                        // Same chain - call lottery-rounds directly
                        let rounds_response: LotteryRoundsResponse = self.runtime.call_application(
                            true,
                            lottery_rounds_app_id,
                            &LotteryRoundsOperation::PurchaseTickets {
                                owner: target_account.owner,
                                amount,
                                ticket_price: linera_sdk::linera_base_types::Amount::from_tokens(1), // Default ticket price
                                source_chain_id: None,
                            },
                        );
                        eprintln!("LotteryApp::Transfer - rounds response: {:?}", rounds_response);
                    } else {
                        // Cross-chain - send message with SENDER'S chain_id
                        let message = Message::TransferForTickets {
                            owner: target_account.owner,
                            amount,
                            source_chain_id: self.runtime.chain_id(),
                            source_owner: owner,
                        };
                        self.runtime
                            .prepare_message(message)
                            .with_authentication()
                            .send_to(target_account.chain_id);
                        eprintln!("LotteryApp::Transfer - Cross-chain message sent");
                    }
                }

                eprintln!("LotteryApp::Transfer - Complete");
                LotteryAppResponse::Ok
            }

            LotteryAppOperation::Claim {
                source_account,
                amount,
                target_account,
                purchase_tickets,
            } => {
                self.runtime
                    .check_account_permission(source_account.owner)
                    .expect("Permission for Claim operation");

                let params = self.runtime.application_parameters();
                let native_app_id = params.native_app_id.with_abi::<native::NativeAbi>();
                let lottery_rounds_app_id = params.lottery_rounds_app_id.with_abi::<LotteryRoundsAbi>();

                // Step 1: Call Native app to claim tokens
                let _native_response: native::NativeResponse = self.runtime.call_application(
                    true,
                    native_app_id,
                    &native::NativeOperation::Claim {
                        source_account,
                        amount,
                        target_account,
                    },
                );

                // Step 2: If purchase_tickets, register tickets in lottery-rounds
                if purchase_tickets {
                    let _rounds_response: LotteryRoundsResponse = self.runtime.call_application(
                        true,
                        lottery_rounds_app_id,
                        &LotteryRoundsOperation::PurchaseTickets {
                            owner: target_account.owner,
                            amount,
                            ticket_price: linera_sdk::linera_base_types::Amount::from_tokens(1), // Default ticket price
                            source_chain_id: None,
                        },
                    );
                }

                LotteryAppResponse::Ok
            }

            LotteryAppOperation::SendPrize { recipient, amount, source_chain_id } => {
                // Called by lottery-rounds to distribute prize
                let params = self.runtime.application_parameters();
                let native_app_id = params.native_app_id.with_abi::<native::NativeAbi>();

                let target_chain = if let Some(source_chain_id_str) = &source_chain_id {
                    source_chain_id_str.parse::<ChainId>().unwrap_or_else(|_| self.runtime.chain_id())
                } else {
                    self.runtime.chain_id()
                };

                let target_account = linera_sdk::abis::fungible::Account {
                    chain_id: target_chain,
                    owner: recipient,
                };

                let payer = self.runtime.authenticated_signer()
                    .expect("Authentication required for prize distribution");

                let _native_response: native::NativeResponse = self.runtime.call_application(
                    true,
                    native_app_id,
                    &native::NativeOperation::Transfer {
                        owner: payer,
                        amount,
                        target_account,
                    },
                );

                LotteryAppResponse::Ok
            }
        }
    }

    async fn execute_message(&mut self, message: Self::Message) {
        match message {
            Message::Notify => {
                eprintln!("LotteryApp::execute_message - Notify received");
            }
            Message::TransferForTickets { owner: _, amount, source_chain_id, source_owner } => {
                eprintln!("==== LotteryApp::execute_message - TransferForTickets ====");
                eprintln!("  source_owner: {:?}", source_owner);
                eprintln!("  source_chain: {:?}", source_chain_id);
                eprintln!("  amount: {:?}", amount);
                eprintln!("  current chain: {:?}", self.runtime.chain_id());
                
                let params = self.runtime.application_parameters();
                eprintln!("  lottery_rounds_app_id from params: {:?}", params.lottery_rounds_app_id);
                
                let lottery_rounds_app_id = params.lottery_rounds_app_id.with_abi::<LotteryRoundsAbi>();

                eprintln!("  Calling lottery-rounds::PurchaseTickets...");

                // Register tickets in lottery-rounds
                let rounds_response: LotteryRoundsResponse = self.runtime.call_application(
                    true,
                    lottery_rounds_app_id,
                    &LotteryRoundsOperation::PurchaseTickets {
                        owner: source_owner,
                        amount,
                        ticket_price: linera_sdk::linera_base_types::Amount::from_tokens(1), // Default ticket price
                        source_chain_id: Some(source_chain_id.to_string()),
                    },
                );
                
                eprintln!("  rounds_response: {:?}", rounds_response);
                eprintln!("==== LotteryApp::execute_message - Complete ====");
            }
        }
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}
