// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use linera_sdk::{
    linera_base_types::{Amount, ApplicationId, WithContractAbi},
    views::{RootView, View},
    Contract, ContractRuntime,
};
use rounds::{RoundsAbi, RoundsOperation, RoundsResponse, Prediction, Message};
use self::state::{RoundsState, PredictionRound, RoundStatus, Prediction as StatePrediction};

// Conversion functions between lib types and state types
fn prediction_from_lib(lib_prediction: Prediction) -> StatePrediction {
    match lib_prediction {
        Prediction::Up => StatePrediction::Up,
        Prediction::Down => StatePrediction::Down,
    }
}

fn prediction_to_lib(prediction: StatePrediction) -> Prediction {
    match prediction {
        StatePrediction::Up => Prediction::Up,
        StatePrediction::Down => Prediction::Down,
    }
}

fn round_status_to_lib(status: RoundStatus) -> rounds::RoundStatus {
    match status {
        RoundStatus::Active => rounds::RoundStatus::Active,
        RoundStatus::Closed => rounds::RoundStatus::Closed,
        RoundStatus::Resolved => rounds::RoundStatus::Resolved,
    }
}

fn prediction_round_to_lib(round: PredictionRound) -> rounds::PredictionRound {
    rounds::PredictionRound {
        id: round.id,
        created_at: round.created_at,
        closed_at: round.closed_at,
        resolved_at: round.resolved_at,
        status: round_status_to_lib(round.status),
        closing_price: round.closing_price,
        resolution_price: round.resolution_price,
        up_bets: round.up_bets,
        down_bets: round.down_bets,
        up_bets_pool: round.up_bets_pool,
        down_bets_pool: round.down_bets_pool,
        prize_pool: round.prize_pool,
        result: round.result.map(prediction_to_lib),
    }
}

fn prediction_round_option_to_lib(round: Option<PredictionRound>) -> Option<rounds::PredictionRound> {
    round.map(prediction_round_to_lib)
}

fn prediction_rounds_to_lib(rounds_vec: Vec<PredictionRound>) -> Vec<rounds::PredictionRound> {
    rounds_vec.into_iter().map(prediction_round_to_lib).collect()
}

pub struct RoundsContract {
    state: RoundsState,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(RoundsContract);

impl WithContractAbi for RoundsContract {
    type Abi = RoundsAbi;
}

impl Contract for RoundsContract {
    type Message = Message;
    type Parameters = rounds::RoundsParameters; // No parameters needed
    type InstantiationArgument = (); // Native App ID
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = RoundsState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        RoundsContract { state, runtime }
    }

    async fn instantiate(&mut self, _arg: Self::InstantiationArgument) {
        // Validate params access
        let _ = self.runtime.application_parameters();
        // Initialize Microbetreal ID as None (will be set via operation)
        self.state.microbet_app_id.set(None);
    }

    async fn execute_operation(&mut self, operation: Self::Operation) -> Self::Response {
        match operation {
            RoundsOperation::SetMicrobetAppId { microbet_app_id } => {
                match microbet_app_id.parse::<ApplicationId>() {
                    Ok(app_id) => {
                        let typed_app_id: ApplicationId<native_fungible_abi::ExtendedNativeFungibleTokenAbi> = app_id.with_abi();
                        self.state.microbet_app_id.set(Some(typed_app_id));
                    }
                    Err(e) => panic!("Failed to parse Microbetreal ApplicationId: {:?}", e),
                }
                RoundsResponse::Ok
            }
            
            RoundsOperation::SetLeaderboardChainId { chain_id } => {
                eprintln!("SetLeaderboardChainId: {:?}", chain_id);
                self.state.leaderboard_chain_id.set(chain_id);
                RoundsResponse::Ok
            }


            RoundsOperation::CreateRound => {
                let timestamp = self.runtime.system_time().micros();
                match self.state.create_round(timestamp).await {
                    Ok(round_id) => RoundsResponse::RoundId(round_id),
                    Err(e) => panic!("Failed to create round: {}", e),
                }
            }
            
            RoundsOperation::CloseRound { closing_price } => {
                let timestamp = self.runtime.system_time().micros();
                match self.state.close_round(closing_price, timestamp).await {
                    Ok(new_round_id) => RoundsResponse::RoundId(new_round_id),
                    Err(e) => panic!("Failed to close round: {}", e),
                }
            }
            
            RoundsOperation::ResolveRound { resolution_price } => {
                let timestamp = self.runtime.system_time().micros();
                
                // Get all rounds and find the last closed one
                match self.state.get_all_rounds().await {
                    Ok(rounds) => {
                        // Find the last closed round that is not yet resolved
                        let closed_round = rounds.into_iter().filter(|round| {
                            round.status == RoundStatus::Closed && round.resolved_at.is_none()
                        }).max_by_key(|round| round.id);
                        
                        match closed_round {
                            Some(round) => {
                                // Resolve the round and get winners
                                match self.state.resolve_round_and_distribute_rewards(round.id, resolution_price, timestamp).await {
                                    Ok(results) => {
                                        // Get app IDs
                                        let params = self.runtime.application_parameters();
                                        let leaderboard_app_id = params.leaderboard_app_id.with_abi::<leaderboard::LeaderboardAbi>();
                                        
                                        let microbetreal_app_id = self.state.microbet_app_id.get()
                                            .expect("Microbetreal app ID not set");
                                        
                                        for (owner, bet_amount, winnings, is_win, source_chain_id) in results {
                                            // Distribute rewards if any
                                            if winnings > Amount::ZERO {
                                                let _response: native_fungible_abi::ExtendedResponse = self.runtime.call_application(
                                                    true, // authenticated
                                                    microbetreal_app_id,
                                                    &native_fungible_abi::ExtendedOperation::SendReward {
                                                        recipient: owner,
                                                        amount: winnings,
                                                        source_chain_id: source_chain_id.clone(),
                                                    },
                                                );
                                            }

                                            // Update leaderboard stats (for everyone)
                                            let player_chain_id_str = source_chain_id.clone().unwrap_or_else(|| self.runtime.chain_id().to_string());
                                            
                                            // Check if leaderboard is on a different chain
                                            let leaderboard_target_chain = self.state.leaderboard_chain_id.get().clone();
                                            
                                            if let Some(target_chain_str) = leaderboard_target_chain {
                                                // Cross-chain: send message to target chain
                                                let target_chain_id = target_chain_str.parse::<linera_sdk::linera_base_types::ChainId>()
                                                    .expect("Invalid leaderboard_chain_id format");
                                                
                                                if target_chain_id != self.runtime.chain_id() {
                                                    // Send cross-chain message
                                                    self.runtime
                                                        .prepare_message(Message::LeaderboardUpdate {
                                                            owner,
                                                            chain_id: player_chain_id_str.clone(),
                                                            is_win,
                                                            amount: if is_win { winnings } else { bet_amount },
                                                        })
                                                        .with_authentication()
                                                        .send_to(target_chain_id);
                                                    eprintln!("Sent LeaderboardUpdate cross-chain to {:?}", target_chain_id);
                                                } else {
                                                    // Same chain, call directly  
                                                    let _response: () = self.runtime.call_application(
                                                        true,
                                                        leaderboard_app_id,
                                                        &leaderboard::Operation::UpdateScore {
                                                            owner,
                                                            chain_id: player_chain_id_str,
                                                            is_win,
                                                            amount: if is_win { winnings } else { bet_amount },
                                                        }
                                                    );
                                                }
                                            } else {
                                                // No target chain set, call leaderboard on same chain
                                                let _response: () = self.runtime.call_application(
                                                    true,
                                                    leaderboard_app_id,
                                                    &leaderboard::Operation::UpdateScore {
                                                        owner,
                                                        chain_id: player_chain_id_str,
                                                        is_win,
                                                        amount: if is_win { winnings } else { bet_amount },
                                                    }
                                                );
                                            }
                                        }
                                        
                                        RoundsResponse::Ok
                                    },
                                    Err(e) => panic!("Failed to resolve round: {}", e),
                                }
                            },
                            None => panic!("No closed round to resolve"),
                        }
                    },
                    Err(e) => panic!("Failed to get all rounds: {}", e),
                }
            }

            RoundsOperation::PlaceBet { owner, amount, prediction, source_chain_id } => {
                let state_prediction = prediction_from_lib(prediction);
                match self.state.place_bet(owner, amount, state_prediction, source_chain_id).await {
                    Ok(()) => RoundsResponse::Ok,
                    Err(e) => panic!("Failed to place bet: {}", e),
                }
            }
            
            RoundsOperation::ClaimWinnings { round_id } => {
                // This operation is not used in the current design since rewards are auto-distributed
                // But we keep it for potential future use
                let _ = round_id;
                panic!("ClaimWinnings is deprecated - rewards are automatically distributed on resolve");
            }

            // Query operations
            RoundsOperation::GetActiveRound => {
                match self.state.get_active_round().await {
                    Ok(Some(round_id)) => {
                        match self.state.get_round(round_id).await {
                            Ok(Some(round)) => RoundsResponse::PredictionRound(prediction_round_option_to_lib(Some(round))),
                            Ok(None) => RoundsResponse::PredictionRound(None),
                            Err(e) => panic!("Failed to get round: {}", e),
                        }
                    },
                    Ok(None) => RoundsResponse::PredictionRound(None),
                    Err(e) => panic!("Failed to get active round: {}", e),
                }
            }
            
            RoundsOperation::GetRound { id } => {
                match self.state.get_round(id).await {
                    Ok(Some(round)) => RoundsResponse::PredictionRound(prediction_round_option_to_lib(Some(round))),
                    Ok(None) => RoundsResponse::PredictionRound(None),
                    Err(e) => panic!("Failed to get round: {}", e),
                }
            }
            
            RoundsOperation::GetAllRounds => {
                match self.state.get_all_rounds().await {
                    Ok(rounds) => RoundsResponse::PredictionRounds(prediction_rounds_to_lib(rounds)),
                    Err(e) => panic!("Failed to get all rounds: {}", e),
                }
            }
            
            RoundsOperation::GetActiveBets => {
                match self.state.get_active_bets().await {
                    Ok(bets) => {
                        let active_bets: Vec<_> = bets.into_iter().flat_map(|(owner, bet)| {
                            let mut list = Vec::new();
                            if !bet.amount_up.is_zero() {
                                list.push(rounds::ActiveBetInfo {
                                    owner,
                                    amount: bet.amount_up,
                                    prediction: Prediction::Up,
                                });
                            }
                            if !bet.amount_down.is_zero() {
                                list.push(rounds::ActiveBetInfo {
                                    owner,
                                    amount: bet.amount_down,
                                    prediction: Prediction::Down,
                                });
                            }
                            list
                        }).collect();
                        RoundsResponse::ActiveBets(active_bets)
                    },
                    Err(e) => panic!("Failed to get active bets: {}", e),
                }
            }
            
            RoundsOperation::GetRoundWinners { round_id } => {
                match self.state.get_round_winners(round_id).await {
                    Ok(winners) => {
                        let winner_info: Vec<_> = winners.into_iter().map(|(owner, bet_amount, winnings, source_chain_id)| {
                            rounds::RoundWinnerInfo {
                                owner,
                                bet_amount,
                                winnings,
source_chain_id,
                            }
                        }).collect();
                        RoundsResponse::RoundWinners(winner_info)
                    },
                    Err(e) => panic!("Failed to get round winners: {}", e),
                }
            }
        }
    }

    async fn execute_message(&mut self, message: Self::Message) {
        match message {
            Message::Notify => {
                // Auto-deploy notification
                eprintln!("Rounds::execute_message - Notify received");
            }
            Message::LeaderboardUpdate { owner, chain_id, is_win, amount } => {
                // Cross-chain leaderboard update received
                eprintln!("Rounds::execute_message - LeaderboardUpdate: owner={:?}, chain={}, is_win={}, amount={:?}", 
                    owner, chain_id, is_win, amount);
                
                // Call leaderboard on this chain
                let params = self.runtime.application_parameters();
                let leaderboard_app_id = params.leaderboard_app_id.with_abi::<leaderboard::LeaderboardAbi>();
                
                let _response: () = self.runtime.call_application(
                    true,
                    leaderboard_app_id,
                    &leaderboard::Operation::UpdateScore {
                        owner,
                        chain_id,
                        is_win,
                        amount,
                    }
                );
                
                eprintln!("Rounds::execute_message - LeaderboardUpdate completed");
            }
        }
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}
