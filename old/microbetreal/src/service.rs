// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use std::sync::Arc;

use async_graphql::{EmptySubscription, Object, Request, Response, Schema};
use linera_sdk::{
    linera_base_types::{AccountOwner, WithServiceAbi},
    views::View,
    Service, ServiceRuntime,
};
use native_fungible::{AccountEntry, TICKER_SYMBOL, ExtendedNativeFungibleTokenAbi, ExtendedOperation, AccountInput, Prediction, PredictionRound as LibPredictionRound, RoundStatus as LibRoundStatus, ActiveBetInfo as LibActiveBetInfo};
use self::state::{NativeFungibleTokenState, PredictionRound};

linera_sdk::service!(NativeFungibleTokenService);

pub struct NativeFungibleTokenService {
    state: NativeFungibleTokenState,
    runtime: Arc<ServiceRuntime<Self>>,
}

impl WithServiceAbi for NativeFungibleTokenService {
    type Abi = ExtendedNativeFungibleTokenAbi;
}

impl Service for NativeFungibleTokenService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = NativeFungibleTokenState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        NativeFungibleTokenService {
            state,
            runtime: Arc::new(runtime),
        }
    }

    async fn handle_query(&self, request: Request) -> Response {
        // Collect all rounds from the loaded state
        let mut all_rounds = Vec::new();
        match self.state.rounds.indices().await {
            Ok(round_ids) => {
                for round_id in round_ids {
                    if let Ok(Some(round)) = self.state.rounds.get(&round_id).await {
                        all_rounds.push(round);
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to get round indices: {:?}", e);
            }
        }
        
        let schema = Schema::build(
            QueryRoot {
                all_rounds,
                runtime: self.runtime.clone(),
                storage_context: self.runtime.root_view_storage_context(),
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
    all_rounds: Vec<PredictionRound>,
    runtime: Arc<ServiceRuntime<NativeFungibleTokenService>>,
    storage_context: linera_sdk::views::ViewStorageContext,
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
    
    // Prediction game queries
    async fn active_round(&self) -> Option<LibPredictionRound> {
        // Load a fresh state to query the active round
        match NativeFungibleTokenState::load(self.storage_context.clone()).await {
            Ok(state) => {
                match state.get_active_round().await {
                    Ok(Some(round_id)) => {
                        match state.get_round(round_id).await {
                            Ok(Some(round)) => {
                                // Convert RoundStatus from state to lib
                                let status = match round.status {
                                    self::state::RoundStatus::Active => LibRoundStatus::Active,
                                    self::state::RoundStatus::Closed => LibRoundStatus::Closed,
                                    self::state::RoundStatus::Resolved => LibRoundStatus::Resolved,
                                };
                                
                                Some(LibPredictionRound {
                                    id: round.id,
                                    created_at: round.created_at,
                                    closed_at: round.closed_at,
                                    resolved_at: round.resolved_at,
                                    status,
                                    closing_price: round.closing_price,
                                    resolution_price: round.resolution_price,
                                    up_bets: round.up_bets,
                                    down_bets: round.down_bets,
                                    up_bets_pool: round.up_bets_pool,
                                    down_bets_pool: round.down_bets_pool,
                                    prize_pool: round.prize_pool,
                                    result: round.result.map(|p| match p {
                                        self::state::Prediction::Up => Prediction::Up,
                                        self::state::Prediction::Down => Prediction::Down,
                                    }),
                                })
                            },
                            Ok(None) => None,
                            Err(_) => None,
                        }
                    },
                    Ok(None) => None,
                    Err(_) => None,
                }
            },
            Err(_) => None,
        }
    }
    
    async fn round(&self, id: u64) -> Option<LibPredictionRound> {
        // Find the round with the given ID
        if let Some(round) = self.all_rounds.iter().find(|round| round.id == id) {
            // Convert RoundStatus from state to lib
            let status = match round.status {
                self::state::RoundStatus::Active => LibRoundStatus::Active,
                self::state::RoundStatus::Closed => LibRoundStatus::Closed,
                self::state::RoundStatus::Resolved => LibRoundStatus::Resolved,
            };
            
            Some(LibPredictionRound {
                id: round.id,
                created_at: round.created_at,
                closed_at: round.closed_at,
                resolved_at: round.resolved_at,
                status,
                closing_price: round.closing_price,
                resolution_price: round.resolution_price,
                up_bets: round.up_bets,
                down_bets: round.down_bets,
                up_bets_pool: round.up_bets_pool,
                down_bets_pool: round.down_bets_pool,
                prize_pool: round.prize_pool,
                result: round.result.map(|p| match p {
                    self::state::Prediction::Up => Prediction::Up,
                    self::state::Prediction::Down => Prediction::Down,
                }),
            })
        } else {
            None
        }
    }
    
    async fn all_rounds(&self) -> Vec<LibPredictionRound> {
        // Convert our internal PredictionRound to the library version
        self.all_rounds.iter().map(|round| {
            // Convert RoundStatus from state to lib
            let status = match round.status {
                self::state::RoundStatus::Active => LibRoundStatus::Active,
                self::state::RoundStatus::Closed => LibRoundStatus::Closed,
                self::state::RoundStatus::Resolved => LibRoundStatus::Resolved,
            };
            
            LibPredictionRound {
                id: round.id,
                created_at: round.created_at,
                closed_at: round.closed_at,
                resolved_at: round.resolved_at,
                status,
                closing_price: round.closing_price,
                resolution_price: round.resolution_price,
                up_bets: round.up_bets,
                down_bets: round.down_bets,
                up_bets_pool: round.up_bets_pool,
                down_bets_pool: round.down_bets_pool,
                prize_pool: round.prize_pool,
                result: round.result.map(|p| match p {
                    self::state::Prediction::Up => Prediction::Up,
                    self::state::Prediction::Down => Prediction::Down,
                }),
            }
        }).collect()
    }
    
    /// Get all active bets
    async fn active_bets(&self) -> Vec<LibActiveBetInfo> {
        // Load a fresh state to query active bets
        match NativeFungibleTokenState::load(self.storage_context.clone()).await {
            Ok(state) => {
                match state.get_active_bets().await {
                    Ok(bets) => {
                        bets.into_iter().map(|(owner, bet)| {
                            // Use the source chain ID if available, otherwise use the current chain ID
                            let chain_id = match &bet.source_chain_id {
                                Some(source_chain_id_str) => {
                                    // Try to parse the source chain ID, fallback to current chain ID if parsing fails
                                    source_chain_id_str.parse().unwrap_or_else(|_| self.runtime.chain_id())
                                },
                                None => self.runtime.chain_id(),
                            };
                            
                            LibActiveBetInfo {
                                chain_id,
                                owner,
                                amount: bet.amount,
                                prediction: match bet.prediction {
                                    self::state::Prediction::Up => Prediction::Up,
                                    self::state::Prediction::Down => Prediction::Down,
                                },
                            }
                        }).collect()
                    },
                    Err(_) => Vec::new(), // Return empty vector on error
                }
            },
            Err(_) => Vec::new(), // Return empty vector on error
        }
    }
    
    /// Get winners for a resolved round
    async fn round_winners(&self, round_id: u64) -> Vec<native_fungible::RoundWinnerInfo> {
        // Load a fresh state to query round winners
        match NativeFungibleTokenState::load(self.storage_context.clone()).await {
            Ok(state) => {
                match state.get_round_winners(round_id).await {
                    Ok(winners) => {
                        winners.into_iter().map(|(owner, bet_amount, winnings, source_chain_id)| {
                            native_fungible::RoundWinnerInfo {
                                chain_id: self.runtime.chain_id(),
                                owner,
                                bet_amount,
                                winnings,
                                source_chain_id,
                            }
                        }).collect()
                    },
                    Err(_) => Vec::new(), // Return empty vector on error
                }
            },
            Err(_) => Vec::new(), // Return empty vector on error
        }
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
    
    // Prediction game mutations
    /// Create a new prediction round
    async fn create_round(&self) -> String {
        self.runtime.schedule_operation(&ExtendedOperation::CreateRound);
        "CreateRound operation scheduled".to_string()
    }
    
    /// Close the active round with a closing price
    async fn close_round(&self, closing_price: String) -> String {
        use linera_sdk::linera_base_types::Amount;
        let amount = closing_price.parse::<Amount>().unwrap_or_default();
        self.runtime.schedule_operation(&ExtendedOperation::CloseRound { closing_price: amount });
        "CloseRound operation scheduled".to_string()
    }
    
    /// Resolve a closed round with a resolution price
    async fn resolve_round(&self, resolution_price: String) -> String {
        use linera_sdk::linera_base_types::Amount;
        let amount = resolution_price.parse::<Amount>().unwrap_or_default();
        self.runtime.schedule_operation(&ExtendedOperation::ResolveRound { resolution_price: amount });
        "ResolveRound operation scheduled".to_string()
    }
    
    /// Place a bet in the active round
    async fn place_bet(&self, amount: String, prediction: Prediction) -> String {
        use linera_sdk::linera_base_types::Amount;
        self.runtime.schedule_operation(&ExtendedOperation::PlaceBet {
            amount: amount.parse::<Amount>().unwrap_or_default(),
            prediction,
        });
        "PlaceBet operation scheduled".to_string()
    }
    
    /// Claim winnings from a resolved round
    async fn claim_winnings(&self, round_id: u64) -> String {
        self.runtime.schedule_operation(&ExtendedOperation::ClaimWinnings { round_id });
        "ClaimWinnings operation scheduled".to_string()
    }
}