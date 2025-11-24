// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use linera_sdk::{
    linera_base_types::{ApplicationId, ChainId, WithContractAbi},
    views::{RootView, View},
    Contract, ContractRuntime,
};
use microbetreal::{Message, MicrobetAbi, ExtendedOperation, ExtendedResponse, Prediction};
use self::state::MicrobetState;

// Conversion function
fn to_rounds_prediction(pred: Prediction) -> rounds::Prediction {
    match pred {
        Prediction::Up => rounds::Prediction::Up,
        Prediction::Down => rounds::Prediction::Down,
    }
}

pub struct MicrobetContract {
    state: MicrobetState,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(MicrobetContract);

impl WithContractAbi for MicrobetContract {
    type Abi = MicrobetAbi;
}

impl Contract for MicrobetContract {
    type Message = Message;
    type Parameters = ();
    type InstantiationArgument = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = MicrobetState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        MicrobetContract { state, runtime }
    }

    async fn instantiate(&mut self, _argument: Self::InstantiationArgument) {
        // No initialization needed - apps configured via operations
    }

    async fn execute_operation(&mut self, operation: Self::Operation) -> Self::Response {
        match operation {
            // Microbetreal-specific operations
            ExtendedOperation::SetNativeAppId { native_app_id } => {
                match native_app_id.parse::<ApplicationId>() {
                    Ok(app_id) => {
                        let typed_app_id: ApplicationId<native::NativeAbi> = app_id.with_abi();
                        self.state.native_app_id.set(Some(typed_app_id));
                        ExtendedResponse::Ok
                    }
                    Err(e) => {
                        panic!("Failed to parse Native ApplicationId: {:?}", e);
                    }
                }
            }

            ExtendedOperation::SetRoundsAppId { rounds_app_id } => {
                match rounds_app_id.parse::<ApplicationId>() {
                    Ok(app_id) => {
                        let typed_app_id: ApplicationId<rounds::RoundsAbi> = app_id.with_abi();
                        self.state.rounds_app_id.set(Some(typed_app_id));
                        ExtendedResponse::Ok
                    }
                    Err(e) => {
                        panic!("Failed to parse Rounds ApplicationId: {:?}", e);
                    }
                }
            }

            ExtendedOperation::Transfer {
                owner,
                amount,
                target_account,
                prediction: Some(prediction),
                native_app_id,
                rounds_app_id,
            } => {
                // Transfer with prediction - this is our main betting operation
                
                // Save app IDs if provided (flexible configuration)
                if let Some(nat_id) = native_app_id {
                    match nat_id.parse::<ApplicationId>() {
                        Ok(app_id) => {
                            let typed_app_id: ApplicationId<native::NativeAbi> = app_id.with_abi();
                            self.state.native_app_id.set(Some(typed_app_id));
                        }
                        Err(e) => eprintln!("Failed to parse Native ApplicationId: {:?}", e),
                    }
                }
                
                if let Some(rnd_id) = rounds_app_id {
                    match rnd_id.parse::<ApplicationId>() {
                        Ok(app_id) => {
                            let typed_app_id: ApplicationId<rounds::RoundsAbi> = app_id.with_abi();
                            self.state.rounds_app_id.set(Some(typed_app_id));
                        }
                        Err(e) => eprintln!("Failed to parse Rounds ApplicationId: {:?}", e),
                    }
                }
                
                let native_app_id = self.state.native_app_id.get()
                    .expect("Native app ID not configured");
                let rounds_app_id = self.state.rounds_app_id.get()
                    .expect("Rounds app ID not configured");

                // Step 1: Call Native app to transfer tokens
                let _native_response: native::NativeResponse = self.runtime.call_application(
                    true,
                    native_app_id,
                    &native::NativeOperation::Transfer {
                        owner,
                        amount,
                        target_account,
                    },
                );

                // Step 2: Place bet in Rounds app
                if target_account.chain_id == self.runtime.chain_id() {
                    // Same chain - no source_chain_id needed
                    let _rounds_response: rounds::RoundsResponse = self.runtime.call_application(
                        true,
                        rounds_app_id,
                        &rounds::RoundsOperation::PlaceBet {
                            owner, // Sender makes the bet
                            amount,
                            prediction: to_rounds_prediction(prediction),
                            source_chain_id: None,
                        },
                    );
                } else {
                    // Cross-chain - send message with SENDER'S chain_id
                    let message = Message::TransferWithPrediction {
                        owner: target_account.owner,
                        amount,
                        prediction,
                        source_chain_id: self.runtime.chain_id().to_string(), // SENDER'S chain!
                        source_owner: owner,
                    };
                    self.runtime
                        .prepare_message(message)
                        .with_authentication()
                        .send_to(target_account.chain_id);
                }

                ExtendedResponse::Ok
            }

            ExtendedOperation::SendReward { recipient, amount, source_chain_id } => {
                // Called by Rounds to distribute rewards
                let native_app_id = self.state.native_app_id.get()
                    .expect("Native app ID not configured");

                let target_chain = if let Some(source_chain_id_str) = &source_chain_id {
                    source_chain_id_str.parse::<ChainId>().unwrap_or_else(|_| self.runtime.chain_id())
                } else {
                    self.runtime.chain_id()
                };

                let target_account = linera_sdk::abis::fungible::Account {
                    chain_id: target_chain,
                    owner: recipient,
                };

                let resolver_owner = self.runtime.authenticated_signer()
                    .expect("Authentication required for reward distribution");

                let _native_response: native::NativeResponse = self.runtime.call_application(
                    true,
                    native_app_id,
                    &native::NativeOperation::Transfer {
                        owner: resolver_owner,
                        amount,
                        target_account,
                    },
                );

                ExtendedResponse::Ok
            }

            // Pass-through operations to Native app
            ExtendedOperation::Transfer { owner, amount, target_account, prediction: None, native_app_id: None, rounds_app_id: None } => {
                // Regular transfer without prediction - pass to Native
                let native_app_id = self.state.native_app_id.get()
                    .expect("Native app ID not configured");

                let _response: native::NativeResponse = self.runtime.call_application(
                    true,
                    native_app_id,
                    &native::NativeOperation::Transfer { owner, amount, target_account },
                );
                ExtendedResponse::Ok
            }

            ExtendedOperation::Claim { source_account, amount, target_account, prediction: None } => {
                // Regular claim without prediction - pass to Native
                let native_app_id = self.state.native_app_id.get()
                    .expect("Native app ID not configured");

                let _response: native::NativeResponse = self.runtime.call_application(
                    true,
                    native_app_id,
                    &native::NativeOperation::Claim { source_account, amount, target_account },
                );
                ExtendedResponse::Ok
            }

            _ => {
                // All other operations: pass through to Native
                panic!("Operation not supported by Microbetreal - use Native app directly for: {:?}", std::any::type_name_of_val(&operation));
            }
        }
    }

    async fn execute_message(&mut self, message: Self::Message) {
        match message {
            Message::TransferWithPrediction { owner: _, amount, prediction, source_chain_id, source_owner } => {
                // Handle cross-chain transfer with prediction
                // Place bet for source owner with SENDER'S chain_id
                if let Some(rounds_app_id) = *self.state.rounds_app_id.get() {
                    let _response: rounds::RoundsResponse = self.runtime.call_application(
                        true,
                        rounds_app_id,
                        &rounds::RoundsOperation::PlaceBet {
                            owner: source_owner,
                            amount,
                            prediction: to_rounds_prediction(prediction),
                            source_chain_id: Some(source_chain_id), // Use SENDER'S chain from message!
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