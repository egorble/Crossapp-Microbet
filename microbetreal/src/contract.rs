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
    type Parameters = microbetreal::MicrobetParameters;
    type InstantiationArgument = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = MicrobetState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        MicrobetContract { state, runtime }
    }

    async fn instantiate(&mut self, _arg: Self::InstantiationArgument) {
        // Validation (optional)
        let _ = self.runtime.application_parameters();
    }

    async fn execute_operation(&mut self, operation: Self::Operation) -> Self::Response {
        match operation {
            // Microbetreal-specific operations
            ExtendedOperation::SetNativeAppId { .. } => {
                // Disabled - set during initialization
                panic!("SetNativeAppId is disabled - configured at initialization");
            }

            ExtendedOperation::SetRoundsAppId { .. } => {
                // Disabled - set during initialization
                panic!("SetRoundsAppId is disabled - configured at initialization");
            }

            ExtendedOperation::Transfer {
                owner,
                amount,
                target_account,
                prediction: Some(prediction),
            } => {
                // Transfer with prediction - this is our main betting operation
                

                
                let params = self.runtime.application_parameters();
                let native_app_id = params.native_app_id.with_abi::<native::NativeAbi>();
                let rounds_app_id = params.rounds_app_id.with_abi::<rounds::RoundsAbi>();

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
            ExtendedOperation::Transfer { owner, amount, target_account, prediction: None } => {
                // Regular transfer without prediction - pass to Native
                let params = self.runtime.application_parameters();
                let native_app_id = params.native_app_id.with_abi::<native::NativeAbi>();

                let _response: native::NativeResponse = self.runtime.call_application(
                    true,
                    native_app_id,
                    &native::NativeOperation::Transfer { owner, amount, target_account },
                );
                ExtendedResponse::Ok
            }

            ExtendedOperation::Claim { source_account, amount, target_account, prediction: None } => {
                // Regular claim without prediction - pass to Native
                let params = self.runtime.application_parameters();
                let native_app_id = params.native_app_id.with_abi::<native::NativeAbi>();

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
                let params = self.runtime.application_parameters();
                let rounds_app_id = params.rounds_app_id.with_abi::<rounds::RoundsAbi>();
                
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

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}