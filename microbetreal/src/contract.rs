// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use linera_sdk::{
    abis::fungible::{
        Account as FungibleAccount, InitialState, Parameters,
    },
    linera_base_types::{Account, AccountOwner, Amount, ChainId, WithContractAbi, ApplicationId},
    views::{RootView, View},
    Contract, ContractRuntime,
};
use native_fungible::{Message, TICKER_SYMBOL, ExtendedNativeFungibleTokenAbi, ExtendedOperation, ExtendedResponse, Prediction as NativePrediction};
use self::state::NativeFungibleTokenState;

// Conversion between native_fungible::Prediction and rounds::Prediction
fn to_rounds_prediction(pred: NativePrediction) -> rounds::Prediction {
    match pred {
        NativePrediction::Up => rounds::Prediction::Up,
        NativePrediction::Down => rounds::Prediction::Down,
    }
}

pub struct NativeFungibleTokenContract {
    state: NativeFungibleTokenState,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(NativeFungibleTokenContract);

impl WithContractAbi for NativeFungibleTokenContract {
    type Abi = ExtendedNativeFungibleTokenAbi;
}

impl Contract for NativeFungibleTokenContract {
    type Message = Message;
    type Parameters = Parameters;
    type InstantiationArgument = InitialState; // Just InitialState, rounds_app_id comes from Parameters
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = NativeFungibleTokenState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        NativeFungibleTokenContract { state, runtime }
    }

    async fn instantiate(&mut self, initial_state: Self::InstantiationArgument) {
        // Validate that the application parameters were configured correctly.
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
        
        // Note: rounds_app_id will be set manually via a SetRoundsAppId operation after deployment
    }

    async fn execute_operation(&mut self, operation: Self::Operation) -> Self::Response {
        match operation {
            ExtendedOperation::Balance { owner } => {
                let balance = self.runtime.owner_balance(owner);
                ExtendedResponse::Balance(balance)
            }

            ExtendedOperation::ChainBalance => {
                let balance = self.runtime.chain_balance();
                ExtendedResponse::ChainBalance(balance)
            }

            ExtendedOperation::TickerSymbol => {
                ExtendedResponse::TickerSymbol(String::from(TICKER_SYMBOL))
            }

            ExtendedOperation::Transfer {
                owner,
                amount,
                target_account,
                prediction,
            } => {
                self.runtime
                    .check_account_permission(owner)
                    .expect("Permission for Transfer operation");

                let fungible_target_account = target_account;
                let target_account = self.normalize_account(target_account);

                self.runtime.transfer(owner, target_account, amount);

                // If prediction is provided, call Rounds app to place bet
                if let Some(pred) = prediction {
                    if let Some(rounds_app_id) = *self.state.rounds_app_id.get() {
                        if target_account.chain_id == self.runtime.chain_id() {
                            // Same chain - make cross-app call to Rounds app (FIXED: proper call_application)
                            let _response: rounds::RoundsResponse = self.runtime.call_application(
                                true, // authenticated
                                rounds_app_id,
                                &rounds::RoundsOperation::PlaceBet {
                                    owner: target_account.owner,
                                    amount,
                                    prediction: to_rounds_prediction(pred),
                                    source_chain_id: None, // Same chain
                                },
                            );
                        } else {
                            // Cross-chain transfer - send message with prediction info
                            let message = Message::TransferWithPrediction {
                                owner: target_account.owner,
                                amount,
                                prediction: pred,
                                source_chain_id: self.runtime.chain_id(),
                                source_owner: owner,
                            };
                            self.runtime
                                .prepare_message(message)
                                .with_authentication()
                                .send_to(target_account.chain_id);
                        }
                    }
                } else {
                    // No prediction - just send notify message for cross-chain
                    self.transfer(fungible_target_account.chain_id);
                }

                ExtendedResponse::Ok
            }

            ExtendedOperation::Claim {
                source_account,
                amount,
                target_account,
                prediction,
            } => {
                self.runtime
                    .check_account_permission(source_account.owner)
                    .expect("Permission for Claim operation");

                let fungible_source_account = source_account;
                let fungible_target_account = target_account;

                let source_account = self.normalize_account(source_account);
                let target_account = self.normalize_account(target_account);

                self.runtime.claim(source_account, target_account, amount);
                
                // If prediction is provided, call Rounds app to place bet
                if let Some(pred) = prediction {
                    if let Some(rounds_app_id) = *self.state.rounds_app_id.get() {
                        let _response: rounds::RoundsResponse = self.runtime.call_application(
                            true, // authenticated
                            rounds_app_id,
                            &rounds::RoundsOperation::PlaceBet {
                                owner: target_account.owner,
                                amount,
                                prediction: to_rounds_prediction(pred),
                                source_chain_id: Some(source_account.chain_id.to_string()),
                            },
                        );
                    }
                }

                self.claim(
                    fungible_source_account.chain_id,
                    fungible_target_account.chain_id,
                );
                ExtendedResponse::Ok
            }

            ExtendedOperation::Withdraw => {
                // Get the current owner (authenticated signer)
                let owner = self.runtime.authenticated_signer().unwrap();
                // Get the balance for this owner
                let balance = self.runtime.owner_balance(owner);
                // Create target account (chain account)
                let target_account = Account {
                    chain_id: self.runtime.chain_id(),
                    owner: AccountOwner::CHAIN,
                };
                // Transfer all tokens to the chain account
                self.runtime.transfer(owner, target_account, balance);
                ExtendedResponse::Ok
            }

            ExtendedOperation::Mint { owner, amount } => {
                // Create target account for the owner
                let target_account = Account {
                    chain_id: self.runtime.chain_id(),
                    owner,
                };
                // Mint tokens by transferring from chain account to the target account
                self.runtime.transfer(AccountOwner::CHAIN, target_account, amount);
                ExtendedResponse::Ok
            }

            ExtendedOperation::SetRoundsAppId { rounds_app_id } => {
                // Parse ApplicationId from string (as generic ApplicationId)
                match rounds_app_id.parse::<ApplicationId>() {
                    Ok(app_id) => {
                        // Convert to typed ApplicationId using with_abi
                        let typed_app_id: ApplicationId<rounds::RoundsAbi> = app_id.with_abi();
                        self.state.rounds_app_id.set(Some(typed_app_id));
                        ExtendedResponse::Ok
                    }
                    Err(e) => {
                        panic!("Failed to parse ApplicationId: {:?}", e);
                    }
                }
            }

            ExtendedOperation::ResolveRound { resolution_price } => {
                // Call Rounds app to resolve the round and get winners
                if let Some(rounds_app_id) = *self.state.rounds_app_id.get() {
                    let response: rounds::RoundsResponse = self.runtime.call_application(
                        true, // authenticated
                        rounds_app_id,
                        &rounds::RoundsOperation::ResolveRound { resolution_price },
                    );
                    
                    // Extract winners from response
                    if let rounds::RoundsResponse::Winners(winners) = response {
                        // Distribute rewards to all winners
                        for winner_info in winners {
                            if winner_info.winnings > Amount::ZERO {
                                if let Some(source_chain_id_str) = winner_info.source_chain_id {
                                    // Cross-chain winner - send to their original chain
                                    match source_chain_id_str.parse::<ChainId>() {
                                        Ok(chain_id) => {
                                            let target_account = Account {
                                                chain_id,
                                                owner: winner_info.owner,
                                            };
                                            // Transfer from chain account
                                            self.runtime.transfer(AccountOwner::CHAIN, target_account, winner_info.winnings);
                                            
                                            // Send notify message
                                            let message = Message::Notify;
                                            self.runtime
                                                .prepare_message(message)
                                                .with_authentication()
                                                .send_to(chain_id);
                                        }
                                        Err(_) => {
                                            // If parsing fails, send to local chain
                                            let target_account = Account {
                                                chain_id: self.runtime.chain_id(),
                                                owner: winner_info.owner,
                                            };
                                            self.runtime.transfer(AccountOwner::CHAIN, target_account, winner_info.winnings);
                                        }
                                    }
                                } else {
                                    // Local winner
                                    let target_account = Account {
                                        chain_id: self.runtime.chain_id(),
                                        owner: winner_info.owner,
                                    };
                                    self.runtime.transfer(AccountOwner::CHAIN, target_account, winner_info.winnings);
                                }
                            }
                        }
                        ExtendedResponse::Ok
                    } else {
                        panic!("Unexpected response from Rounds::ResolveRound");
                    }
                } else {
                    panic!("Rounds app ID not configured");
                }
            }
        }
    }

    async fn execute_message(&mut self, message: Self::Message) {
        match message {
            Message::Notify => {
                // Auto-deploy the application on this chain if it's not already deployed
            }
            Message::TransferWithPrediction { owner: _, amount, prediction, source_chain_id, source_owner } => {
                // Handle cross-chain transfer with prediction
                // Make cross-app call to Rounds app to place bet
                if let Some(rounds_app_id) = *self.state.rounds_app_id.get() {
                    let _response: rounds::RoundsResponse = self.runtime.call_application(
                        true, // authenticated
                        rounds_app_id,
                        &rounds::RoundsOperation::PlaceBet {
                            owner: source_owner,
                            amount,
                            prediction: to_rounds_prediction(prediction),
                            source_chain_id: Some(source_chain_id.to_string()),
                        },
                    );
                }
            }
        }
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}

impl NativeFungibleTokenContract {

    
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
            // If different chain, send notify message so the app gets auto-deployed
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