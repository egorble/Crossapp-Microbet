// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

use std::sync::Arc;

use async_graphql::{EmptySubscription, Object, Request, Response, Schema};
use linera_sdk::{
    linera_base_types::{AccountOwner, WithServiceAbi},
    Service, ServiceRuntime,
};
use native_fungible::{AccountEntry, TICKER_SYMBOL, ExtendedNativeFungibleTokenAbi, ExtendedOperation, AccountInput, Prediction};

linera_sdk::service!(NativeFungibleTokenService);

pub struct NativeFungibleTokenService {
    runtime: Arc<ServiceRuntime<Self>>,
}

impl WithServiceAbi for NativeFungibleTokenService {
    type Abi = ExtendedNativeFungibleTokenAbi;
}

impl Service for NativeFungibleTokenService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        NativeFungibleTokenService {
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
    runtime: Arc<ServiceRuntime<NativeFungibleTokenService>>,
}

#[Object]
impl Accounts {
    // Define a field that lets you query by key
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
    
    /// Get the chain balance (total balance of the chain)
    async fn chain_balance(&self) -> String {
        let balance = self.runtime.chain_balance();
        balance.to_string()
    }
}

// Query root for GraphQL queries
struct QueryRoot {
    runtime: Arc<ServiceRuntime<NativeFungibleTokenService>>,
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
    
    /// Get the chain balance (total balance of the chain)
    async fn chain_balance(&self) -> Result<String, async_graphql::Error> {
        let balance = self.runtime.chain_balance();
        Ok(balance.to_string())
    }
}

struct MutationRoot {
    runtime: Arc<ServiceRuntime<NativeFungibleTokenService>>,
}

#[Object]
impl MutationRoot {
    /// Get balance for an account owner
    async fn balance(&self, owner: AccountOwner) -> String {
        self.runtime.schedule_operation(&ExtendedOperation::Balance { owner });
        "Balance operation scheduled".to_string()
    }

    /// Get the chain balance (total balance of the chain)
    async fn chain_balance(&self) -> String {
        self.runtime.schedule_operation(&ExtendedOperation::ChainBalance);
        "ChainBalance operation scheduled".to_string()
    }

    /// Get the ticker symbol
    async fn ticker_symbol(&self) -> String {
        self.runtime.schedule_operation(&ExtendedOperation::TickerSymbol);
        "TickerSymbol operation scheduled".to_string()
    }

    /// Transfer tokens between accounts
    async fn transfer(
        &self,
        owner: AccountOwner,
        amount: String,
        target_account: AccountInput,
        prediction: Option<Prediction>,
    ) -> String {
        use linera_sdk::linera_base_types::Amount;
        // Convert AccountInput to linera_sdk::abis::fungible::Account
        let fungible_account = linera_sdk::abis::fungible::Account {
            chain_id: target_account.chain_id,
            owner: target_account.owner,
        };
        
        self.runtime.schedule_operation(&ExtendedOperation::Transfer {
            owner,
            amount: amount.parse::<Amount>().unwrap_or_default(),
            target_account: fungible_account,
            prediction,
        });
        "Transfer operation scheduled".to_string()
    }

    /// Claim tokens from another chain
    async fn claim(
        &self,
        source_account: AccountInput,
        amount: String,
        target_account: AccountInput,
        prediction: Option<Prediction>,
    ) -> String {
        use linera_sdk::linera_base_types::Amount;
        // Convert AccountInput to linera_sdk::abis::fungible::Account
        let fungible_source_account = linera_sdk::abis::fungible::Account {
            chain_id: source_account.chain_id,
            owner: source_account.owner,
        };
        let fungible_target_account = linera_sdk::abis::fungible::Account {
            chain_id: target_account.chain_id,
            owner: target_account.owner,
        };
        
        self.runtime.schedule_operation(&ExtendedOperation::Claim {
            source_account: fungible_source_account,
            amount: amount.parse::<Amount>().unwrap_or_default(),
            target_account: fungible_target_account,
            prediction,
        });
        "Claim operation scheduled".to_string()
    }

    /// Withdraw all tokens to the chain account
    async fn withdraw(&self) -> String {
        self.runtime.schedule_operation(&ExtendedOperation::Withdraw);
        "Withdraw operation scheduled successfully".to_string()
    }

    /// Mint new tokens to an account
    async fn mint(&self, owner: AccountOwner, amount: String) -> String {
        use linera_sdk::linera_base_types::Amount;
        // Parse amount from string
        self.runtime.schedule_operation(&ExtendedOperation::Mint {
            owner,
            amount: amount.parse::<Amount>().unwrap_or_default(),
        });
        "Mint operation scheduled successfully".to_string()
    }
    
    /// Set the Rounds app ApplicationId (call this after deploying Rounds app)
    async fn set_rounds_app_id(&self, rounds_app_id: String) -> String {
        self.runtime.schedule_operation(&ExtendedOperation::SetRoundsAppId { rounds_app_id: rounds_app_id.clone() });
        format!("SetRoundsAppId operation scheduled with ID: {}", rounds_app_id)
    }
    
    /// Resolve a round and distribute rewards (calls Rounds app)
    async fn resolve_round(&self, resolution_price: String) -> String {
        use linera_sdk::linera_base_types::Amount;
        let amount = resolution_price.parse::<Amount>().unwrap_or_default();
        self.runtime.schedule_operation(&ExtendedOperation::ResolveRound { resolution_price: amount });
        "ResolveRound operation scheduled - will call Rounds app and distribute rewards".to_string()
    }
}