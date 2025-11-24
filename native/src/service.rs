// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

use std::sync::Arc;

use async_graphql::{EmptySubscription, Object, Request, Response, Schema};
use linera_sdk::{
    linera_base_types::{AccountOwner, WithServiceAbi},
    Service, ServiceRuntime,
};
use native::{AccountEntry, TICKER_SYMBOL, NativeAbi, NativeOperation, AccountInput};

linera_sdk::service!(NativeService);

pub struct NativeService {
    runtime: Arc<ServiceRuntime<Self>>,
}

impl WithServiceAbi for NativeService {
    type Abi = NativeAbi;
}

impl Service for NativeService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        NativeService {
            runtime: Arc::new(runtime),
        }
    }

    async fn handle_query(&self, request: Request) -> Response {
        let schema = Schema::build(
            QueryRoot {
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

struct Accounts {
    runtime: Arc<ServiceRuntime<NativeService>>,
}

#[Object]
impl Accounts {
    async fn entry(&self, key: AccountOwner) -> AccountEntry {
        let value = self.runtime.owner_balance(key);
        AccountEntry { key, value }
    }

    async fn entries(&self) -> Vec<AccountEntry> {
        self.runtime
            .owner_balances()
            .into_iter()
            .map(|(owner, amount)| AccountEntry {
                key: owner,
                value: amount,
            })
            .collect()
    }

    async fn keys(&self) -> Vec<AccountOwner> {
        self.runtime.balance_owners()
    }
    
    async fn chain_balance(&self) -> String {
        let balance = self.runtime.chain_balance();
        balance.to_string()
    }
}

struct QueryRoot {
    runtime: Arc<ServiceRuntime<NativeService>>,
}

#[Object]
impl QueryRoot {
    async fn ticker_symbol(&self) -> Result<String, async_graphql::Error> {
        Ok(String::from(TICKER_SYMBOL))
    }

    async fn accounts(&self) -> Result<Accounts, async_graphql::Error> {
        Ok(Accounts {
            runtime: self.runtime.clone(),
        })
    }
    
    async fn chain_balance(&self) -> Result<String, async_graphql::Error> {
        let balance = self.runtime.chain_balance();
        Ok(balance.to_string())
    }
}

struct MutationRoot {
    runtime: Arc<ServiceRuntime<NativeService>>,
}

#[Object]
impl MutationRoot {
    async fn balance(&self, owner: AccountOwner) -> String {
        self.runtime.schedule_operation(&NativeOperation::Balance { owner });
        "Balance operation scheduled".to_string()
    }

    async fn chain_balance(&self) -> String {
        self.runtime.schedule_operation(&NativeOperation::ChainBalance);
        "ChainBalance operation scheduled".to_string()
    }

    async fn ticker_symbol(&self) -> String {
        self.runtime.schedule_operation(&NativeOperation::TickerSymbol);
        "TickerSymbol operation scheduled".to_string()
    }

    async fn transfer(
        &self,
        owner: AccountOwner,
        amount: String,
        target_account: AccountInput,
    ) -> String {
        use linera_sdk::linera_base_types::Amount;
        let fungible_account = linera_sdk::abis::fungible::Account {
            chain_id: target_account.chain_id,
            owner: target_account.owner,
        };
        
        self.runtime.schedule_operation(&NativeOperation::Transfer {
            owner,
            amount: amount.parse::<Amount>().unwrap_or_default(),
            target_account: fungible_account,
        });
        "Transfer operation scheduled".to_string()
    }

    async fn claim(
        &self,
        source_account: AccountInput,
        amount: String,
        target_account: AccountInput,
    ) -> String {
        use linera_sdk::linera_base_types::Amount;
        let fungible_source_account = linera_sdk::abis::fungible::Account {
            chain_id: source_account.chain_id,
            owner: source_account.owner,
        };
        let fungible_target_account = linera_sdk::abis::fungible::Account {
            chain_id: target_account.chain_id,
            owner: target_account.owner,
        };
        
        self.runtime.schedule_operation(&NativeOperation::Claim {
            source_account: fungible_source_account,
            amount: amount.parse::<Amount>().unwrap_or_default(),
            target_account: fungible_target_account,
        });
        "Claim operation scheduled".to_string()
    }

    async fn withdraw(&self) -> String {
        self.runtime.schedule_operation(&NativeOperation::Withdraw);
        "Withdraw operation scheduled successfully".to_string()
    }

    async fn mint(&self, owner: AccountOwner, amount: String) -> String {
        use linera_sdk::linera_base_types::Amount;
        self.runtime.schedule_operation(&NativeOperation::Mint {
            owner,
            amount: amount.parse::<Amount>().unwrap_or_default(),
        });
        "Mint operation scheduled successfully".to_string()
    }
}