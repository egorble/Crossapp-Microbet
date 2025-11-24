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
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        MicrobetService {
            runtime: Arc::new(runtime),
        }
    }

    async fn handle_query(&self, request: Request) -> Response {
        let schema = Schema::build(
            QueryRoot {},
            MutationRoot {
                runtime: self.runtime.clone(),
            },
            EmptySubscription,
        )
        .finish();
        schema.execute(request).await
    }
}

struct QueryRoot {}

#[Object]
impl QueryRoot {
    /// Placeholder query
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
        
        self.runtime.schedule_operation(&ExtendedOperation::Transfer {
            owner,
            amount: amount.parse::<Amount>().unwrap_or_default(),
            target_account: fungible_account,
            prediction: Some(prediction),
        });
        "TransferWithPrediction operation scheduled - tokens will be transferred and bet placed".to_string()
    }
}