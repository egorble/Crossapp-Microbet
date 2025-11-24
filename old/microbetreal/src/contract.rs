// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use linera_sdk::{
    abis::fungible::{
        Account as FungibleAccount, InitialState, Parameters,
    },
    linera_base_types::{Account, AccountOwner, Amount, ChainId, WithContractAbi},
    views::{RootView, View},
    Contract, ContractRuntime,
};
use native_fungible::{Message, TICKER_SYMBOL, ExtendedNativeFungibleTokenAbi, ExtendedOperation, ExtendedResponse, Prediction as LibPrediction, PredictionRound as LibPredictionRound, RoundStatus as LibRoundStatus, ActiveBetInfo as LibActiveBetInfo};
use self::state::{NativeFungibleTokenState, Prediction, PredictionRound, RoundStatus};

// Conversion functions between lib types and state types
fn prediction_from_lib(lib_prediction: LibPrediction) -> Prediction {
    match lib_prediction {
        LibPrediction::Up => Prediction::Up,
        LibPrediction::Down => Prediction::Down,
    }
}

fn prediction_to_lib(prediction: Prediction) -> LibPrediction {
    match prediction {
        Prediction::Up => LibPrediction::Up,
        Prediction::Down => LibPrediction::Down,
    }
}

fn round_status_to_lib(status: RoundStatus) -> LibRoundStatus {
    match status {
        RoundStatus::Active => LibRoundStatus::Active,
        RoundStatus::Closed => LibRoundStatus::Closed,
        RoundStatus::Resolved => LibRoundStatus::Resolved,
    }
}

fn prediction_round_to_lib(round: PredictionRound) -> LibPredictionRound {
    LibPredictionRound {
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

fn prediction_round_option_to_lib(round: Option<PredictionRound>) -> Option<LibPredictionRound> {
    round.map(prediction_round_to_lib)
}

fn prediction_rounds_to_lib(rounds: Vec<PredictionRound>) -> Vec<LibPredictionRound> {
    rounds.into_iter().map(prediction_round_to_lib).collect()
}

pub struct NativeFungibleTokenContract {
    state: NativeFungibleTokenState,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(NativeFungibleTokenContract);

impl WithContractAbi for NativeFungibleTokenContract {
    type Abi = ExtendedNativeFungibleTokenAbi;
}

impl Contract for NativeFungibleTokenContract {
    type Message = Message;
    type Parameters = Parameters;
    type InstantiationArgument = InitialState;
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = NativeFungibleTokenState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        NativeFungibleTokenContract { state, runtime }
    }

    async fn instantiate(&mut self, state: Self::InstantiationArgument) {
        // Validate that the application parameters were configured correctly.
        assert!(
            self.runtime.application_parameters().ticker_symbol == "NAT",
            "Only NAT is accepted as ticker symbol"
        );
        for (owner, amount) in state.accounts {
            let account = Account {
                chain_id: self.runtime.chain_id(),
                owner,
            };
            self.runtime.transfer(AccountOwner::CHAIN, account, amount);
        }
    }

    async fn execute_operation(&mut self, operation: Self::Operation) -> Self::Response {
        match operation {
            ExtendedOperation::Balance { owner } => {
                let balance = self.runtime.owner_balance(owner);
                ExtendedResponse::Balance(balance)
            }

            ExtendedOperation::ChainBalance => {
                let balance = self.runtime.chain_balance();
                ExtendedResponse::ChainBalance(balance)
            }

            ExtendedOperation::TickerSymbol => {
                ExtendedResponse::TickerSymbol(String::from(TICKER_SYMBOL))
            }

            ExtendedOperation::Transfer {
                owner,
                amount,
                target_account,
                prediction,
            } => {
                self.runtime
                    .check_account_permission(owner)
                    .expect("Permission for Transfer operation");

                let fungible_target_account = target_account;
                let target_account = self.normalize_account(target_account);

                self.runtime.transfer(owner, target_account, amount);

                // If prediction is provided and target is on different chain, send prediction info in message
                if let Some(pred) = prediction {
                    if target_account.chain_id != self.runtime.chain_id() {
                        // Cross-chain transfer with prediction - send message with prediction info
                        let message = Message::TransferWithPrediction {
                            owner: target_account.owner,
                            amount,
                            prediction: Some(pred),  // Use the lib Prediction type directly
                            source_chain_id: self.runtime.chain_id(),  // Include source chain ID
                            source_owner: owner,  // Include source owner
                        };
                        self.runtime
                            .prepare_message(message)
                            .with_authentication()
                            .send_to(target_account.chain_id);
                    } else {
                        // Same chain transfer - place bet directly
                        let state_prediction = prediction_from_lib(pred);
                        // Use the runtime balance for the target owner since the transfer just happened
                        let target_balance = self.runtime.owner_balance(target_account.owner);
                        eprintln!("Placing bet from transfer operation for owner {:?}, amount {:?}, prediction {:?}, balance {:?}", 
                            target_account.owner, amount, state_prediction, target_balance);
                        if let Err(e) = self.state.place_bet_with_balance(target_account.owner, amount, state_prediction, target_balance, None).await {
                            eprintln!("Failed to place bet: {}", e);
                            // We don't panic here as the transfer was successful
                        }
                    }

                } else {
                    // No prediction - just send notify message for cross-chain
                    self.transfer(fungible_target_account.chain_id);
                }

                ExtendedResponse::Ok
            }

            ExtendedOperation::Claim {
                source_account,
                amount,
                target_account,
                prediction,
            } => {
                self.runtime
                    .check_account_permission(source_account.owner)
                    .expect("Permission for Claim operation");

                let fungible_source_account = source_account;
                let fungible_target_account = target_account;

                let source_account = self.normalize_account(source_account);
                let target_account = self.normalize_account(target_account);

                self.runtime.claim(source_account, target_account, amount);
                
                // Check if there's a pending cross-chain bet for this transfer
                let source_chain_id_str = source_account.chain_id.to_string();
                let pending_bet = self.state.get_pending_cross_chain_bet(source_chain_id_str.clone(), source_account.owner.clone()).await
                    .expect("Failed to get pending cross-chain bet");
                
                // If there's a pending bet or explicit prediction parameter, place a bet
                let final_prediction = prediction.or_else(|| {
                    pending_bet.as_ref().map(|(_, _, pred)| prediction_to_lib(*pred))
                });
                
                if let Some(pred) = final_prediction {
                    let state_prediction = prediction_from_lib(pred);
                    // Place bet using the claimed amount for the target owner
                    // Use the runtime balance for the target owner since the claim just happened
                    let target_balance = self.runtime.owner_balance(target_account.owner);
                    eprintln!("Placing bet from claim operation for owner {:?}, amount {:?}, prediction {:?}, balance {:?}", 
                        target_account.owner, amount, state_prediction, target_balance);
                    if let Err(e) = self.state.place_bet_with_balance(target_account.owner, amount, state_prediction, target_balance, None).await {
                        eprintln!("Failed to place bet: {}", e);
                        // We don't panic here as the claim was successful
                    }
                    
                    // Remove the pending bet since it's been used
                    if pending_bet.is_some() {
                        self.state.remove_pending_cross_chain_bet(source_chain_id_str, source_account.owner).await
                            .expect("Failed to remove pending cross-chain bet");
                    }
                }

                self.claim(
                    fungible_source_account.chain_id,
                    fungible_target_account.chain_id,
                );
                ExtendedResponse::Ok
            }

            ExtendedOperation::Withdraw => {
                // Get the current owner (authenticated signer)
                let owner = self.runtime.authenticated_signer().unwrap();
                // Get the balance for this owner
                let balance = self.runtime.owner_balance(owner);
                // Create target account (chain account)
                let target_account = Account {
                    chain_id: self.runtime.chain_id(),
                    owner: AccountOwner::CHAIN,
                };
                // Transfer all tokens to the chain account
                self.runtime.transfer(owner, target_account, balance);
                ExtendedResponse::Ok
            }

            ExtendedOperation::Mint { owner, amount } => {
                // Create target account for the owner
                let target_account = Account {
                    chain_id: self.runtime.chain_id(),
                    owner,
                };
                // Mint tokens by transferring from chain account to the target account
                self.runtime.transfer(AccountOwner::CHAIN, target_account, amount);
                ExtendedResponse::Ok
            }
            
            // Prediction game operations
            ExtendedOperation::CreateRound => {
                let timestamp = self.runtime.system_time().micros();
                match self.state.create_round(timestamp).await {
                    Ok(round_id) => ExtendedResponse::RoundId(round_id),
                    Err(e) => panic!("Failed to create round: {}", e),
                }
            }
            
            ExtendedOperation::CloseRound { closing_price } => {
                let timestamp = self.runtime.system_time().micros();
                match self.state.close_round(closing_price, timestamp).await {
                    Ok(new_round_id) => ExtendedResponse::RoundId(new_round_id),
                    Err(e) => panic!("Failed to close round: {}", e),
                }
            }
            
            ExtendedOperation::ResolveRound { resolution_price } => {
                // We need to resolve a closed round, not an active round
                // Let's find the last closed round to resolve
                let timestamp = self.runtime.system_time().micros();
                
                // Get the authenticated signer (owner of the chain where resolution is happening)
                let resolver_owner = self.runtime.authenticated_signer().expect("Authentication required for round resolution");
                
                // Get all rounds and find the last closed one
                match self.state.get_all_rounds().await {
                    Ok(rounds) => {
                        // Find the last closed round that is not yet resolved
                        let closed_round = rounds.into_iter().filter(|round| {
                            round.status == self::state::RoundStatus::Closed && round.resolved_at.is_none()
                        }).max_by_key(|round| round.id);
                        
                        match closed_round {
                            Some(round) => {
                                // Resolve the round and get winners for automatic reward distribution
                                match self.state.resolve_round_and_distribute_rewards(round.id, resolution_price, timestamp).await {
                                    Ok(winners) => {
                                        // Automatically distribute rewards to all winners
                                        let mut source_chains = std::collections::HashSet::new();
                                        
                                        // Send rewards to winners
                                        for (owner, _bet_amount, winnings, source_chain_id) in winners {
                                            if winnings > Amount::ZERO {
                                                // Check if this is a cross-chain winner
                                                if let Some(source_chain_id_str) = source_chain_id {
                                                    // Try to parse the source chain ID (parse only once and reuse)
                                                    match source_chain_id_str.parse::<ChainId>() {
                                                        Ok(source_chain_id) => {
                                                            // This is a cross-chain winner, send tokens via cross-chain transfer
                                                            let target_account = Account {
                                                                chain_id: source_chain_id,
                                                                owner: owner.clone(),
                                                            };
                                                            
                                                            // Transfer reward from resolver's owner balance to winner on source chain
                                                            self.runtime.transfer(resolver_owner, target_account, winnings);
                                                            
                                                            // Collect unique source chains for notify messages
                                                            source_chains.insert(source_chain_id);
                                                        }
                                                        Err(_) => {
                                                            // If we can't parse the source chain ID, send to local owner
                                                            let target_account = Account {
                                                                chain_id: self.runtime.chain_id(),
                                                                owner: owner.clone(),
                                                            };
                                                            self.runtime.transfer(resolver_owner, target_account, winnings);
                                                        }
                                                    }
                                                } else {
                                                    // This is a local winner, send tokens directly
                                                    let target_account = Account {
                                                        chain_id: self.runtime.chain_id(),
                                                        owner: owner.clone(),
                                                    };
                                                    self.runtime.transfer(resolver_owner, target_account, winnings);
                                                }
                                                
                                                // Mark bet as claimed
                                                let bet_key = (round.id, owner.clone());
                                                if let Some(mut bet) = self.state.resolved_bets.get(&bet_key).await
                                                    .map_err(|e| format!("Failed to get bet: {:?}", e)).unwrap() {
                                                    bet.claimed = true;
                                                    self.state.resolved_bets.insert(&bet_key, bet)
                                                        .map_err(|e| format!("Failed to update bet: {:?}", e)).unwrap();
                                                }
                                            }
                                        }
                                        
                                        // Send notify messages to all unique source chains only once
                                        for source_chain_id in source_chains {
                                            let message = Message::Notify;
                                            self.runtime
                                                .prepare_message(message)
                                                .with_authentication()
                                                .send_to(source_chain_id);
                                        }
                                        
                                        ExtendedResponse::Ok
                                    },
                                    Err(e) => panic!("Failed to resolve round and distribute rewards: {}", e),
                                }
                            },
                            None => panic!("No closed round to resolve"),
                        }
                    },
                    Err(e) => panic!("Failed to get all rounds: {}", e),
                }
            }

            ExtendedOperation::PlaceBet { amount, prediction } => {
                let owner = self.runtime.authenticated_signer().expect("Authentication required");
                let state_prediction = prediction_from_lib(prediction);
                // For direct bet placement, use the runtime balance to ensure we have up-to-date balance
                let current_balance = self.runtime.owner_balance(owner);
                match self.state.place_bet_with_balance(owner, amount, state_prediction, current_balance, None).await {
                    Ok(()) => ExtendedResponse::Ok,
                    Err(e) => panic!("Failed to place bet: {}", e),
                }
            }
            
            ExtendedOperation::ClaimWinnings { round_id } => {
                let owner = self.runtime.authenticated_signer().expect("Authentication required");
                match self.state.claim_winnings(round_id, owner).await {
                    Ok(winnings) => ExtendedResponse::Balance(winnings),
                    Err(e) => panic!("Failed to claim winnings: {}", e),
                }
            }

            // Query operations for prediction game state
            ExtendedOperation::GetActiveRound => {
                match self.state.get_active_round().await {
                    Ok(Some(round_id)) => {
                        match self.state.get_round(round_id).await {
                            Ok(Some(round)) => ExtendedResponse::PredictionRound(prediction_round_option_to_lib(Some(round))),
                            Ok(None) => ExtendedResponse::PredictionRound(None),
                            Err(e) => panic!("Failed to get round: {}", e),
                        }
                    },
                    Ok(None) => ExtendedResponse::PredictionRound(None),
                    Err(e) => panic!("Failed to get active round: {}", e),
                }
            }
            
            ExtendedOperation::GetRound { id } => {
                match self.state.get_round(id).await {
                    Ok(Some(round)) => ExtendedResponse::PredictionRound(prediction_round_option_to_lib(Some(round))),
                    Ok(None) => ExtendedResponse::PredictionRound(None),
                    Err(e) => panic!("Failed to get round: {}", e),
                }
            }
            
            ExtendedOperation::GetAllRounds => {
                match self.state.get_all_rounds().await {
                    Ok(rounds) => ExtendedResponse::PredictionRounds(prediction_rounds_to_lib(rounds)),
                    Err(e) => panic!("Failed to get all rounds: {}", e),
                }
            }
            
            ExtendedOperation::GetActiveBets => {
                match self.state.get_active_bets().await {
                    Ok(bets) => {
                        let active_bets: Vec<_> = bets.into_iter().map(|(owner, bet)| {
                            LibActiveBetInfo {
                                chain_id: self.runtime.chain_id(),
                                owner,
                                amount: bet.amount,
                                prediction: prediction_to_lib(bet.prediction),
                            }
                        }).collect();
                        ExtendedResponse::ActiveBets(active_bets)
                    },
                    Err(e) => panic!("Failed to get active bets: {}", e),
                }
            }
            
            ExtendedOperation::GetRoundWinners { round_id } => {
                match self.state.get_round_winners(round_id).await {
                    Ok(winners) => {
                        let winner_info: Vec<_> = winners.into_iter().map(|(owner, bet_amount, winnings, source_chain_id)| {
                            native_fungible::RoundWinnerInfo {
                                chain_id: self.runtime.chain_id(),
                                owner,
                                bet_amount,
                                winnings,
                                source_chain_id,
                            }
                        }).collect();
                        ExtendedResponse::RoundWinners(winner_info)
                    },
                    Err(e) => panic!("Failed to get round winners: {}", e),
                }
            }
        }
    }

    async fn execute_message(&mut self, message: Self::Message) {
        match message {
            Message::Notify => {
                // Auto-deploy the application on this chain if it's not already deployed
                // This ensures the application is available for cross-chain operations
                // Note: Application is now available on chain
            }
            Message::TransferWithPrediction { owner: _, amount, prediction, source_chain_id, source_owner } => {
                // Handle cross-chain transfer with prediction
                // Immediately place the bet for the source owner since they made the bet
                if let Some(lib_prediction) = prediction {
                    let state_prediction = prediction_from_lib(lib_prediction);
                    eprintln!("Placing cross-chain bet immediately for source owner {:?} from chain {:?}, amount {:?}, prediction {:?}", 
                        source_owner, source_chain_id, amount, state_prediction);
                    
                    // Place the bet immediately without checking balance
                    // The tokens will arrive with the transfer, so we bypass the balance check
                    // We use a dummy balance equal to the amount to bypass the balance check
                    let source_chain_id_str = source_chain_id.to_string();
                    if let Err(e) = self.state.place_bet_with_balance(source_owner, amount, state_prediction, amount, Some(source_chain_id_str)).await {
                        eprintln!("Failed to place cross-chain bet: {}", e);
                    } else {
                        eprintln!("Successfully placed cross-chain bet for source owner {:?}", source_owner);
                    }
                }
            }
        }
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}

impl NativeFungibleTokenContract {

    
    fn transfer(&mut self, chain_id: ChainId) {
        if chain_id != self.runtime.chain_id() {
            let message = Message::Notify;
            self.runtime
                .prepare_message(message)
                .with_authentication()
                .send_to(chain_id);
        }
    }

    fn claim(&mut self, source_chain_id: ChainId, target_chain_id: ChainId) {
        if source_chain_id == self.runtime.chain_id() {
            self.transfer(target_chain_id);
        } else {
            // If different chain, send notify message so the app gets auto-deployed
            let message = Message::Notify;
            self.runtime
                .prepare_message(message)
                .with_authentication()
                .send_to(source_chain_id);
        }
    }

    fn normalize_account(&self, account: FungibleAccount) -> Account {
        Account {
            chain_id: account.chain_id,
            owner: account.owner,
        }
    }
}