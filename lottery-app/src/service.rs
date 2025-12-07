// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use std::sync::Arc;
use async_graphql::{EmptySubscription, InputObject, Object, Request, Response, Schema};
use linera_sdk::{
    linera_base_types::{AccountOwner, Amount, ChainId, WithServiceAbi},
    views::View,
    Service, ServiceRuntime,
};
use lottery_abi::{LotteryAppAbi, LotteryAppParameters, LotteryAppOperation};
use self::state::LotteryAppState;

/// Input type for fungible account
#[derive(InputObject, Clone)]
pub struct AccountInput {
    pub chain_id: ChainId,
    pub owner: AccountOwner,
}

pub struct LotteryAppService {
    state: Arc<LotteryAppState>,
    runtime: Arc<ServiceRuntime<Self>>,
}

linera_sdk::service!(LotteryAppService);

impl WithServiceAbi for LotteryAppService {
    type Abi = LotteryAppAbi;
}

impl Service for LotteryAppService {
    type Parameters = LotteryAppParameters;

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = LotteryAppState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        LotteryAppService {
            state: Arc::new(state),
            runtime: Arc::new(runtime),
        }
    }

    async fn handle_query(&self, request: Request) -> Response {
        let schema = Schema::build(
            QueryRoot {
                state: self.state.clone(),
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

struct QueryRoot {
    state: Arc<LotteryAppState>,
    runtime: Arc<ServiceRuntime<LotteryAppService>>,
}

#[Object]
impl QueryRoot {
    /// Check if the app is initialized
    async fn initialized(&self) -> bool {
        *self.state.initialized.get()
    }
    
    /// Get the configured Native app ID
    async fn native_app_id(&self) -> String {
        let params = self.runtime.application_parameters();
        format!("{}", params.native_app_id)
    }
    
    /// Get the configured Lottery Rounds app ID
    async fn lottery_rounds_app_id(&self) -> String {
        let params = self.runtime.application_parameters();
        format!("{}", params.lottery_rounds_app_id)
    }
    
    /// Get version
    async fn version(&self) -> String {
        "1.0.0".to_string()
    }
}

struct MutationRoot {
    runtime: Arc<ServiceRuntime<LotteryAppService>>,
}

#[Object]
impl MutationRoot {
    /// Transfer tokens with optional ticket purchase
    /// If purchase_tickets is true, tickets will be registered in lottery-rounds
    async fn transfer(
        &self,
        owner: AccountOwner,
        amount: String,
        target_account: AccountInput,
        purchase_tickets: bool,
    ) -> String {
        let fungible_account = linera_sdk::abis::fungible::Account {
            chain_id: target_account.chain_id,
            owner: target_account.owner,
        };
        
        self.runtime.schedule_operation(&LotteryAppOperation::Transfer {
            owner,
            amount: amount.parse::<Amount>().unwrap_or_default(),
            target_account: fungible_account,
            purchase_tickets,
        });
        
        if purchase_tickets {
            "Transfer with ticket purchase scheduled".to_string()
        } else {
            "Transfer scheduled".to_string()
        }
    }
    
    /// Claim tokens from another chain with optional ticket purchase
    async fn claim(
        &self,
        source_account: AccountInput,
        amount: String,
        target_account: AccountInput,
        purchase_tickets: bool,
    ) -> String {
        let source_fungible_account = linera_sdk::abis::fungible::Account {
            chain_id: source_account.chain_id,
            owner: source_account.owner,
        };
        
        let target_fungible_account = linera_sdk::abis::fungible::Account {
            chain_id: target_account.chain_id,
            owner: target_account.owner,
        };
        
        self.runtime.schedule_operation(&LotteryAppOperation::Claim {
            source_account: source_fungible_account,
            amount: amount.parse::<Amount>().unwrap_or_default(),
            target_account: target_fungible_account,
            purchase_tickets,
        });
        
        if purchase_tickets {
            "Claim with ticket purchase scheduled".to_string()
        } else {
            "Claim scheduled".to_string()
        }
    }
    
    /// Send prize to winner (called by lottery-rounds via cross-app call)
    async fn send_prize(
        &self,
        recipient: AccountOwner,
        amount: String,
        source_chain_id: Option<String>,
    ) -> String {
        self.runtime.schedule_operation(&LotteryAppOperation::SendPrize {
            recipient,
            amount: amount.parse::<Amount>().unwrap_or_default(),
            source_chain_id,
        });
        
        "SendPrize operation scheduled".to_string()
    }
}
