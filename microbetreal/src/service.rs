// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

use std::sync::Arc;

use async_graphql::{EmptySubscription, Object, Request, Response, Schema};
use linera_sdk::{
    linera_base_types::{AccountOwner, Amount, WithServiceAbi},
    Service, ServiceRuntime,
};
use microbetreal::{MicrobetAbi, ExtendedOperation, Prediction};
use native::AccountInput;

linera_sdk::service!(MicrobetService);

pub struct MicrobetService {
    runtime: Arc<ServiceRuntime<Self>>,
}

impl WithServiceAbi for MicrobetService {
    type Abi = MicrobetAbi;
}

impl Service for MicrobetService {
    type Parameters = microbetreal::MicrobetParameters;

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        MicrobetService {
            runtime: Arc::new(runtime),
        }
    }

    async fn handle_query(&self, request: Request) -> Response {
        use linera_sdk::views::View;
        use crate::state::MicrobetState;
        
        // Load state for queries
        let state = MicrobetState::load(self.runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        
        let schema = Schema::build(
            QueryRoot { 
                state,
                runtime: self.runtime.clone(),
            },
            MutationRoot {
                runtime: self.runtime.clone(),
            },
            EmptySubscription,
        )
        .finish();
        schema.execute(request).await
    }
}

mod state;

struct QueryRoot {
    state: state::MicrobetState,
    runtime: Arc<ServiceRuntime<MicrobetService>>,
}

#[Object]
impl QueryRoot {
    /// Get the configured Native app ID
    async fn native_app_id(&self) -> Option<String> {
        let params = self.runtime.application_parameters();
        Some(format!("{}", params.native_app_id))
    }
    
    /// Get the configured Rounds app ID
    async fn rounds_app_id(&self) -> Option<String> {
        let params = self.runtime.application_parameters();
        Some(format!("{}", params.rounds_app_id))
    }
    
    /// Check if app IDs are configured (always true with parameters)
    async fn is_configured(&self) -> bool {
        true
    }
    
    /// Get version
    async fn version(&self) -> String {
        "1.0.0".to_string()
    }
}

struct MutationRoot {
    runtime: Arc<ServiceRuntime<MicrobetService>>,
}

#[Object]
impl MutationRoot {
    /// Set the Native token app ApplicationId
    async fn set_native_app_id(&self, native_app_id: String) -> String {
        self.runtime.schedule_operation(&ExtendedOperation::SetNativeAppId { native_app_id: native_app_id.clone() });
        format!("SetNativeAppId operation scheduled with ID: {}", native_app_id)
    }

    /// Set the Rounds app ApplicationId
    async fn set_rounds_app_id(&self, rounds_app_id: String) -> String {
        self.runtime.schedule_operation(&ExtendedOperation::SetRoundsAppId { rounds_app_id: rounds_app_id.clone() });
        format!("SetRoundsAppId operation scheduled with ID: {}", rounds_app_id)
    }

    /// Transfer tokens with prediction (betting)
    /// Optionally set app IDs on-the-fly
    async fn transfer_with_prediction(
        &self,
        owner: AccountOwner,
        amount: String,
        target_account: AccountInput,
        prediction: Prediction,
    ) -> String {
        let fungible_account = linera_sdk::abis::fungible::Account {
            chain_id: target_account.chain_id,
            owner: target_account.owner,
        };
        
        // Check if we're updating app IDs
        self.runtime.schedule_operation(&ExtendedOperation::Transfer {
            owner,
            amount: amount.parse::<Amount>().unwrap_or_default(),
            target_account: fungible_account,
            prediction: Some(prediction),
        });
        
        "TransferWithPrediction operation scheduled - bet will be placed".to_string()
    }
}