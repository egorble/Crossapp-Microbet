// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use std::sync::Arc;

use async_graphql::{EmptySubscription, Object, Request, Response, Schema};
use linera_sdk::{
    linera_base_types::{AccountOwner, Amount, WithServiceAbi},
    views::View,
    Service, ServiceRuntime,
};
use rounds::{
    RoundsAbi, RoundsOperation, Prediction, 
    PredictionRound as LibPredictionRound, RoundStatus as LibRoundStatus, 
    ActiveBetInfo as LibActiveBetInfo, RoundWinnerInfo as LibRoundWinnerInfo
};
use self::state::{RoundsState, PredictionRound};

linera_sdk::service!(RoundsService);

pub struct RoundsService {
    state: RoundsState,
    runtime: Arc<ServiceRuntime<Self>>,
}

impl WithServiceAbi for RoundsService {
    type Abi = RoundsAbi;
}

impl Service for RoundsService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = RoundsState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        RoundsService {
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

// Query root for GraphQL queries
struct QueryRoot {
    all_rounds: Vec<PredictionRound>,
    runtime: Arc<ServiceRuntime<RoundsService>>,
    storage_context: linera_sdk::views::ViewStorageContext,
}

#[Object]
impl QueryRoot {
    /// Get the active round
    async fn active_round(&self) -> Option<LibPredictionRound> {
        // Load a fresh state to query the active round
        match RoundsState::load(self.storage_context.clone()).await {
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
    
    /// Get a specific round by ID
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
    
    /// Get all rounds
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
        match RoundsState::load(self.storage_context.clone()).await {
            Ok(state) => {
                match state.get_active_bets().await {
                    Ok(bets) => {

                        bets.into_iter().flat_map(|(owner, bet)| {
                            let mut list = Vec::new();
                            if !bet.amount_up.is_zero() {
                                list.push(LibActiveBetInfo {
                                    owner,
                                    amount: bet.amount_up,
                                    prediction: Prediction::Up,
                                });
                            }
                            if !bet.amount_down.is_zero() {
                                list.push(LibActiveBetInfo {
                                    owner,
                                    amount: bet.amount_down,
                                    prediction: Prediction::Down,
                                });
                            }
                            list
                        }).collect()
                    },
                    Err(_) => Vec::new(),
                }
            },
            Err(_) => Vec::new(),
        }
    }
    
    /// Get winners for a resolved round
    async fn round_winners(&self, round_id: u64) -> Vec<LibRoundWinnerInfo> {
        // Load a fresh state to query round winners
        match RoundsState::load(self.storage_context.clone()).await {
            Ok(state) => {
                match state.get_round_winners(round_id).await {
                    Ok(winners) => {
                        winners.into_iter().map(|(owner, bet_amount, winnings, source_chain_id)| {
                            LibRoundWinnerInfo {
                                owner,
                                bet_amount,
                                winnings,
                                source_chain_id,
                            }
                        }).collect()
                    },
                    Err(_) => Vec::new(),
                }
            },
            Err(_) => Vec::new(),
        }
    }
}

struct MutationRoot {
    runtime: Arc<ServiceRuntime<RoundsService>>,
}

#[Object]
impl MutationRoot {
    /// Set the Microbetreal Application Id
    async fn set_microbet_app_id(&self, microbet_app_id: String) -> String {
        self.runtime.schedule_operation(&RoundsOperation::SetMicrobetAppId { microbet_app_id });
        "SetMicrobetAppId operation scheduled".to_string()
    }

    /// Create a new prediction round
    async fn create_round(&self) -> String {
        self.runtime.schedule_operation(&RoundsOperation::CreateRound);
        "CreateRound operation scheduled".to_string()
    }

    /// Close the current round
    async fn close_round(&self, closing_price: String) -> String {
        let amount = closing_price.parse::<Amount>().unwrap_or_default();
        self.runtime.schedule_operation(&RoundsOperation::CloseRound { closing_price: amount });
        "CloseRound operation scheduled".to_string()
    }

    /// Resolve a round and distribute rewards (calls NativeFungible to send rewards)
    async fn resolve_round(&self, resolution_price: String) -> String {
        let amount = resolution_price.parse::<Amount>().unwrap_or_default();
        self.runtime.schedule_operation(&RoundsOperation::ResolveRound { resolution_price: amount });
        "ResolveRound operation scheduled - will call NativeFungible for reward distribution".to_string()
    }
    
    /// Place a bet in the active round (typically called via cross-app call from NativeFungible)
    async fn place_bet(&self, owner: AccountOwner, amount: String, prediction: Prediction, source_chain_id: Option<String>) -> String {
        self.runtime.schedule_operation(&RoundsOperation::PlaceBet {
            owner,
            amount: amount.parse::<Amount>().unwrap_or_default(),
            prediction,
            source_chain_id,
        });
        "PlaceBet operation scheduled".to_string()
    }
}
