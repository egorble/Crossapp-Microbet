// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use linera_sdk::views::{linera_views, MapView, RegisterView, RootView, ViewStorageContext, ViewError};
use linera_sdk::linera_base_types::{AccountOwner, Amount};
use serde::{Deserialize, Serialize};
use async_graphql::SimpleObject;
use num_bigint::BigUint;
use num_traits::cast::ToPrimitive;

/// Calculate winnings proportionally based on bet amount
/// Returns bet_amount + (bet_amount / winner_pool) * total_prize_pool
/// This function performs calculations using u128 to avoid Amount type limitations
fn calculate_winnings_proportional(bet_amount: Amount, winner_pool: Amount, total_prize_pool: Amount) -> Amount {
    // Extract u128 values from Amount instances
    let bet_u128: u128 = u128::from(bet_amount);
    let winner_pool_u128: u128 = u128::from(winner_pool);
    let total_prize_pool_u128: u128 = u128::from(total_prize_pool);
    
    // Check for division by zero
    if winner_pool_u128 == 0 {
        return Amount::ZERO;
    }
    
    // Calculate (bet_amount * total_prize_pool) / winner_pool
    // Using BigUint to prevent overflow during multiplication
    let bet_big = BigUint::from(bet_u128);
    let total_big = BigUint::from(total_prize_pool_u128);
    let winner_pool_big = BigUint::from(winner_pool_u128);
    
    let numerator = bet_big * total_big;
    let winnings_big = numerator / winner_pool_big;
    
    // Convert back to u128, saturating at u128::MAX if somehow it still overflows (unlikely given the logic)
    let winnings_u128 = winnings_big.to_u128().unwrap_or(u128::MAX);
    
    // Convert back to Amount
    Amount::from_attos(winnings_u128)
}

/// The application state for the Native Fungible Token with Prediction Game.
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct NativeFungibleTokenState {
    /// The balances of accounts.
    pub accounts: MapView<AccountOwner, Amount>,
    
    /// Prediction game state
    /// Counter for generating unique round IDs
    pub round_counter: RegisterView<u64>,
    /// All prediction rounds
    pub rounds: MapView<u64, PredictionRound>,
    /// The currently active round (accepting bets)
    pub active_round: RegisterView<Option<u64>>,
    /// Bets placed in the active round
    pub active_bets: MapView<AccountOwner, Vec<PredictionBet>>,
    /// Bets placed in closed rounds (awaiting resolution)
    pub closed_bets: MapView<(u64, AccountOwner), Vec<PredictionBet>>,
    /// Bets placed in resolved rounds (awaiting claim)
    pub resolved_bets: MapView<(u64, AccountOwner), Vec<PredictionBet>>,
    
    /// Pending cross-chain predictions
    /// This stores predictions that arrived via cross-chain messages
    /// Key: (source_chain_id, source_owner) -> (target_owner, amount, prediction)
    pub pending_cross_chain_bets: MapView<(String, AccountOwner), (AccountOwner, Amount, Prediction)>,
}

/// A prediction round for the Up/Down game
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct PredictionRound {
    pub id: u64,
    pub created_at: u64,
    pub closed_at: Option<u64>,
    pub resolved_at: Option<u64>,
    pub status: RoundStatus,
    pub closing_price: Option<Amount>,
    pub resolution_price: Option<Amount>,
    pub up_bets: u64,                  // Number of up bets
    pub down_bets: u64,                // Number of down bets
    pub up_bets_pool: Amount,          // Total amount of up bets
    pub down_bets_pool: Amount,        // Total amount of down bets
    pub prize_pool: Amount,            // Total amount of tokens bet in this round
    pub result: Option<Prediction>,    // Result of the round (Up, Down, or None if not resolved)
}

/// Status of a prediction round
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, async_graphql::Enum)]
pub enum RoundStatus {
    Active,
    Closed,
    Resolved,
}

/// A user's bet in a prediction round
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct PredictionBet {
    pub owner: AccountOwner,
    pub amount: Amount,
    pub prediction: Prediction,
    pub claimed: bool,
    pub source_chain_id: Option<String>, // Add source chain ID for cross-chain bets
}

/// Prediction direction for the Up/Down game
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, async_graphql::Enum)]
pub enum Prediction {
    Up,
    Down,
}

#[allow(dead_code)]
impl NativeFungibleTokenState {
    /// Creates a new prediction round
    pub async fn create_round(&mut self, timestamp: u64) -> Result<u64, String> {
        let round_id = *self.round_counter.get() + 1;
        self.round_counter.set(round_id);
        
        let round = PredictionRound {
            id: round_id,
            created_at: timestamp,
            closed_at: None,
            resolved_at: None,
            status: RoundStatus::Active,
            closing_price: None,
            resolution_price: None,
            up_bets: 0,
            down_bets: 0,
            up_bets_pool: Amount::default(),
            down_bets_pool: Amount::default(),
            prize_pool: Amount::default(),
            result: None,
        };
        
        self.rounds.insert(&round_id, round)
            .map_err(|e: ViewError| format!("Failed to insert round: {:?}", e))?;
        self.active_round.set(Some(round_id));
        
        // Clear active bets for the new round by removing all entries
        let keys: Vec<AccountOwner> = self.active_bets.indices().await
            .map_err(|e: ViewError| format!("Failed to get active bet indices: {:?}", e))?
            .into_iter()
            .collect();
        
        for key in keys {
            self.active_bets.remove(&key)
                .map_err(|e: ViewError| format!("Failed to remove active bet: {:?}", e))?;
        }
        
        Ok(round_id)
    }
    
    /// Close the active round
    pub async fn close_round(&mut self, closing_price: Amount, timestamp: u64) -> Result<u64, String> {
        let round_id_opt = self.active_round.get();
        
        if let Some(round_id) = *round_id_opt {
            let mut round = self.rounds.get(&round_id).await
                .map_err(|e: ViewError| format!("Failed to get round: {:?}", e))?
                .ok_or("Active round not found")?
                .clone();
            
            if round.status != RoundStatus::Active {
                return Err("Round is not active".to_string());
            }
            
            // Calculate round statistics before closing
            let mut up_bets = 0u64;
            let mut down_bets = 0u64;
            let mut up_bets_pool = Amount::default();
            let mut down_bets_pool = Amount::default();
            let mut prize_pool = Amount::default();
            
            // Count bets and calculate prize pools
            let active_bet_indices = self.active_bets.indices().await
                .map_err(|e: ViewError| format!("Failed to get active bet indices: {:?}", e))?;
            
            // Pre-allocate vectors for better performance
            let mut bets_to_move = Vec::with_capacity(active_bet_indices.len());
            
            for owner in &active_bet_indices {
                if let Some(bets) = self.active_bets.get(owner).await
                    .map_err(|e: ViewError| format!("Failed to get active bets: {:?}", e))? {
                    
                    for bet in &bets {
                        match bet.prediction {
                            Prediction::Up => {
                                up_bets += 1;
                                up_bets_pool = up_bets_pool.saturating_add(bet.amount);
                            },
                            Prediction::Down => {
                                down_bets += 1;
                                down_bets_pool = down_bets_pool.saturating_add(bet.amount);
                            },
                        }
                        prize_pool = prize_pool.saturating_add(bet.amount);
                    }
                    
                    // Collect bets to move for later processing
                    bets_to_move.push((owner.clone(), bets));
                }
            }
            
            round.up_bets = up_bets;
            round.down_bets = down_bets;
            round.up_bets_pool = up_bets_pool;
            round.down_bets_pool = down_bets_pool;
            round.prize_pool = prize_pool;
            
            round.status = RoundStatus::Closed;
            round.closed_at = Some(timestamp);
            round.closing_price = Some(closing_price);
            
            self.rounds.insert(&round_id, round)
                .map_err(|e: ViewError| format!("Failed to update round: {:?}", e))?;
            self.active_round.set(None);
            
            // Move active bets to closed bets in batch
            for (owner, bets) in bets_to_move {
                let bet_key = (round_id, owner);
                self.closed_bets.insert(&bet_key, bets)
                    .map_err(|e: ViewError| format!("Failed to move bets to closed: {:?}", e))?;
                self.active_bets.remove(&bet_key.1)
                    .map_err(|e: ViewError| format!("Failed to remove active bets: {:?}", e))?;
            }
            
            // Automatically create a new round after closing the current one
            let new_round_id = *self.round_counter.get() + 1;
            self.round_counter.set(new_round_id);
            
            let new_round = PredictionRound {
                id: new_round_id,
                created_at: timestamp,
                closed_at: None,
                resolved_at: None,
                status: RoundStatus::Active,
                closing_price: None,
                resolution_price: None,
                up_bets: 0,
                down_bets: 0,
                up_bets_pool: Amount::default(),
                down_bets_pool: Amount::default(),
                prize_pool: Amount::default(),
                result: None,
            };
            
            self.rounds.insert(&new_round_id, new_round)
                .map_err(|e: ViewError| format!("Failed to insert new round: {:?}", e))?;
            self.active_round.set(Some(new_round_id));
            
            // Clear active bets for the new round by removing all entries
            let keys: Vec<AccountOwner> = self.active_bets.indices().await
                .map_err(|e: ViewError| format!("Failed to get active bet indices: {:?}", e))?
                .into_iter()
                .collect();
            
            for key in keys {
                self.active_bets.remove(&key)
                    .map_err(|e: ViewError| format!("Failed to remove active bet: {:?}", e))?;
            }
            
            Ok(new_round_id)
        } else {
            Err("No active round to close".to_string())
        }
    }
    

    
    /// Resolve a closed round and automatically distribute rewards
    pub async fn resolve_round_and_distribute_rewards(&mut self, round_id: u64, resolution_price: Amount, timestamp: u64) -> Result<Vec<(AccountOwner, Amount, Amount, Option<String>)>, String> {
        let mut round = self.rounds.get(&round_id).await
            .map_err(|e: ViewError| format!("Failed to get round: {:?}", e))?
            .ok_or("Round not found")?
            .clone();
        
        if round.status != RoundStatus::Closed {
            return Err("Round is not closed".to_string());
        }
        
        // Determine the result based on closing and resolution prices
        let closing_price = round.closing_price.ok_or("Round has no closing price")?;
        let result = if resolution_price > closing_price {
            Some(Prediction::Up)
        } else if resolution_price < closing_price {
            Some(Prediction::Down)
        } else {
            // If prices are equal, no one wins
            None
        };
        
        round.result = result;
        round.status = RoundStatus::Resolved;
        round.resolved_at = Some(timestamp);
        round.resolution_price = Some(resolution_price);
        
        self.rounds.insert(&round_id, round)
            .map_err(|e: ViewError| format!("Failed to update round: {:?}", e))?;
        
        // Move closed bets to resolved bets
        let keys: Vec<(u64, AccountOwner)> = self.closed_bets.indices().await
            .map_err(|e: ViewError| format!("Failed to get closed bet indices: {:?}", e))?
            .into_iter()
            .filter(|(id, _)| *id == round_id)
            .collect();
        
        // Pre-allocate vector for better performance
        let mut bets_to_move = Vec::with_capacity(keys.len());
        
        // Collect all bets to move
        for bet_key in keys {
            if let Some(bet) = self.closed_bets.get(&bet_key).await
                .map_err(|e: ViewError| format!("Failed to get closed bet: {:?}", e))? {
                bets_to_move.push((bet_key, bet));
            }
        }
        
        // Move bets in batch
        for (bet_key, bets) in bets_to_move {
            self.resolved_bets.insert(&bet_key, bets)
                .map_err(|e: ViewError| format!("Failed to move bets to resolved: {:?}", e))?;
            self.closed_bets.remove(&bet_key)
                .map_err(|e: ViewError| format!("Failed to remove closed bets: {:?}", e))?;
        }
        
        // Get winners for reward distribution
        let winners = self.get_round_winners(round_id).await
            .map_err(|e| format!("Failed to get round winners: {:?}", e))?;
        
        Ok(winners)
    }
    

    
    /// Place a bet in the active round with a specific balance check
    /// This variant is used when the balance might not be reflected in the state yet
    pub async fn place_bet_with_balance(&mut self, owner: AccountOwner, amount: Amount, prediction: Prediction, current_balance: Amount, source_chain_id: Option<String>) -> Result<(), String> {
        let round_id_opt = self.active_round.get();
        
        if let Some(round_id) = *round_id_opt {
            let mut round = self.rounds.get(&round_id).await
                .map_err(|e: ViewError| format!("Failed to get round: {:?}", e))?
                .ok_or("Active round not found")?
                .clone();
            
            if round.status != RoundStatus::Active {
                return Err("No active round accepting bets".to_string());
            }
            
            // Check if user already placed a bet - REMOVED limitation
            // We now allow multiple bets per user
            // let has_bet = self.active_bets.contains_key(&owner).await
            //     .map_err(|e: ViewError| format!("Failed to check bet existence: {:?}", e))?;
            // if has_bet {
            //     return Err("User already placed a bet in this round".to_string());
            // }
            
            // Check user balance from provided parameter (not from state)
            if current_balance < amount {
                return Err("Insufficient balance".to_string());
            }
            
            // Deduct bet amount from user's balance in state
            let balance = self.accounts.get(&owner).await
                .map_err(|e: ViewError| format!("Failed to get balance: {:?}", e))?
                .unwrap_or_default();
            let new_balance = balance.saturating_sub(amount);
            if new_balance.is_zero() {
                self.accounts.remove(&owner)
                    .map_err(|e: ViewError| format!("Failed to remove account: {:?}", e))?;
            } else {
                self.accounts.insert(&owner, new_balance)
                    .map_err(|e: ViewError| format!("Failed to update balance: {:?}", e))?;
            }
            
            // Record the bet
            let bet = PredictionBet {
                owner: owner.clone(),
                amount,
                prediction,
                claimed: false,
                source_chain_id, // Include source chain ID
            };
            
            // Get existing bets or create new vector
            let mut user_bets = self.active_bets.get(&owner).await
                .map_err(|e: ViewError| format!("Failed to get active bets: {:?}", e))?
                .unwrap_or_default();
            
            // Add new bet
            user_bets.push(bet);
            
            self.active_bets.insert(&owner, user_bets)
                .map_err(|e: ViewError| format!("Failed to place bet: {:?}", e))?;
            
            // Update round statistics
            match prediction {
                Prediction::Up => {
                    round.up_bets += 1;
                    round.up_bets_pool = round.up_bets_pool.saturating_add(amount);
                },
                Prediction::Down => {
                    round.down_bets += 1;
                    round.down_bets_pool = round.down_bets_pool.saturating_add(amount);
                },
            }
            round.prize_pool = round.prize_pool.saturating_add(amount);
            
            // Save updated round
            self.rounds.insert(&round_id, round)
                .map_err(|e: ViewError| format!("Failed to update round statistics: {:?}", e))?;
        } else {
            return Err("No active round".to_string());
        }
        
        Ok(())
    }
    
    /// Claim winnings from a resolved round
    pub async fn claim_winnings(&mut self, round_id: u64, owner: AccountOwner) -> Result<Amount, String> {
        let round = self.rounds.get(&round_id).await
            .map_err(|e: ViewError| format!("Failed to get round: {:?}", e))?
            .ok_or("Round not found")?;
        
        if round.status != RoundStatus::Resolved {
            return Err("Round is not resolved".to_string());
        }
        
        let closing_price = round.closing_price.ok_or("Round has no closing price")?;
        let resolution_price = round.resolution_price.ok_or("Round has no resolution price")?;
        
        // Determine the correct prediction (Up or Down)
        let correct_prediction = if resolution_price > closing_price {
            Prediction::Up
        } else if resolution_price < closing_price {
            Prediction::Down
        } else {
            // If prices are equal, no one wins
            return Err("Prices are equal, no winners".to_string());
        };
        
        // Get the bets for this round and user
        let bet_key = (round_id, owner.clone());
        let mut bets = self.resolved_bets.get(&bet_key).await
            .map_err(|e: ViewError| format!("Failed to get bets: {:?}", e))?
            .ok_or("No bets found for this user")?
            .clone();
        
        let mut total_winnings = Amount::ZERO;
        let mut any_claimed = false;
        
        for bet in &mut bets {
            if bet.claimed {
                continue;
            }
            
            if bet.prediction == correct_prediction {
                // Calculate winnings (for simplicity, we return the bet amount as winnings)
                let winnings = bet.amount;
                total_winnings = total_winnings.saturating_add(winnings);
                bet.claimed = true;
                any_claimed = true;
            }
        }
        
        if !any_claimed && total_winnings.is_zero() {
            // Check if all were already claimed or just no winning bets
            let all_claimed = bets.iter().all(|b| b.claimed);
            if all_claimed {
                return Err("Winnings already claimed".to_string());
            } else {
                 return Err("No winning bets to claim".to_string());
            }
        }
        
        // Update the bets as claimed
        self.resolved_bets.insert(&bet_key, bets)
            .map_err(|e: ViewError| format!("Failed to update bets: {:?}", e))?;
        
        // Add winnings to user's balance
        let current_balance = self.accounts.get(&owner).await
            .map_err(|e: ViewError| format!("Failed to get balance: {:?}", e))?
            .unwrap_or_default();
        let new_balance = current_balance.saturating_add(total_winnings);
        self.accounts.insert(&owner, new_balance)
            .map_err(|e: ViewError| format!("Failed to update balance: {:?}", e))?;
        
        Ok(total_winnings)
    }
    

    
    /// Get a pending cross-chain bet
    pub async fn get_pending_cross_chain_bet(&self, source_chain_id: String, source_owner: AccountOwner) -> Result<Option<(AccountOwner, Amount, Prediction)>, ViewError> {
        self.pending_cross_chain_bets.get(&(source_chain_id, source_owner)).await
    }
    
    /// Remove a pending cross-chain bet
    pub async fn remove_pending_cross_chain_bet(&mut self, source_chain_id: String, source_owner: AccountOwner) -> Result<(), ViewError> {
        self.pending_cross_chain_bets.remove(&(source_chain_id, source_owner))
    }
    
    /// Get the active round ID
    pub async fn get_active_round(&self) -> Result<Option<u64>, String> {
        Ok(*self.active_round.get())
    }
    
    /// Get all active bets
    pub async fn get_active_bets(&self) -> Result<Vec<(AccountOwner, PredictionBet)>, String> {
        let mut bets = Vec::new();
        let indices = self.active_bets.indices().await
            .map_err(|e: ViewError| format!("Failed to get active bet indices: {:?}", e))?;
        
        for owner in indices {
            if let Some(user_bets) = self.active_bets.get(&owner).await
                .map_err(|e: ViewError| format!("Failed to get active bets: {:?}", e))? {
                for bet in user_bets {
                    bets.push((owner.clone(), bet));
                }
            }
        }
        Ok(bets)
    }
    
    /// Get a round by ID
    pub async fn get_round(&self, round_id: u64) -> Result<Option<PredictionRound>, String> {
        self.rounds.get(&round_id).await
            .map_err(|e: ViewError| format!("Failed to get round: {:?}", e))
    }
    
    /// Get all rounds
    pub async fn get_all_rounds(&self) -> Result<Vec<PredictionRound>, String> {
        let indices = self.rounds.indices().await
            .map_err(|e: ViewError| format!("Failed to get round indices: {:?}", e))?;
        
        // Pre-allocate vector with known capacity for better performance
        let mut rounds = Vec::with_capacity(indices.len());
        
        for index in indices {
            if let Some(round) = self.rounds.get(&index).await
                .map_err(|e: ViewError| format!("Failed to get round: {:?}", e))? {
                rounds.push(round);
            }
        }
        Ok(rounds)
    }
    
    /// Get winners for a resolved round
    pub async fn get_round_winners(&self, round_id: u64) -> Result<Vec<(AccountOwner, Amount, Amount, Option<String>)>, String> {
        let round = self.rounds.get(&round_id).await
            .map_err(|e: ViewError| format!("Failed to get round: {:?}", e))?
            .ok_or("Round not found")?;
        
        if round.status != RoundStatus::Resolved {
            return Err("Round is not resolved".to_string());
        }
        
        let result = round.result.ok_or("Round has no result")?;
        
        // Calculate total prize pool and winner pool
        let total_prize_pool = round.prize_pool;
        let winner_pool = match result {
            Prediction::Up => round.up_bets_pool,
            Prediction::Down => round.down_bets_pool,
        };
        
        if winner_pool.is_zero() {
            return Ok(Vec::new()); // No winners
        }
        
        // Get all resolved bets for this specific round
        // Instead of iterating through all resolved bets, we can directly access bets for this round
        let bet_indices = self.resolved_bets.indices().await
            .map_err(|e: ViewError| format!("Failed to get resolved bet indices: {:?}", e))?;
        
        // Pre-filter indices to only include those matching our round_id
        let round_bet_indices: Vec<_> = bet_indices
            .into_iter()
            .filter(|(id, _)| *id == round_id)
            .collect();
        
        let mut winners = Vec::new();
        
        // Process only the bets for this specific round
        for (id, owner) in round_bet_indices {
            if let Some(bets) = self.resolved_bets.get(&(id, owner.clone())).await
                .map_err(|e: ViewError| format!("Failed to get bets: {:?}", e))? {
                
                for bet in bets {
                    // Only include winners who haven't claimed yet
                    if bet.prediction == result && !bet.claimed {
                        // Calculate winnings properly
                        // Winnings = (bet_amount / winner_pool) * total_prize_pool
                        let winnings = if !winner_pool.is_zero() {
                            calculate_winnings_proportional(bet.amount, winner_pool, total_prize_pool)
                        } else {
                            Amount::ZERO
                        };
                        
                        // Avoid cloning source_chain_id, use as_ref() instead
                        winners.push((owner.clone(), bet.amount, winnings, bet.source_chain_id.clone()));
                    }
                }
            }
        }
        
        Ok(winners)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use linera_sdk::linera_base_types::Amount;

    #[test]
    fn test_calculate_winnings_overflow() {
        // 50 tokens with 18 decimals
        let token_amount = Amount::from_attos(50_000_000_000_000_000_000);
        
        // If we have 1 winner with 50 tokens, and the total pool is 50 tokens
        let bet_amount = token_amount;
        let winner_pool = token_amount;
        let total_prize_pool = token_amount;
        
        // Expected: (50 * 50) / 50 = 50
        // Actual with overflow: saturating_mul(50, 50) / 50 < 50
        
        let winnings = calculate_winnings_proportional(bet_amount, winner_pool, total_prize_pool);
        
        println!("Winnings: {:?}", winnings);
        
        // This assertion should fail if the bug exists (or rather, pass if we assert equality to the wrong value, 
        // but we want to demonstrate it's wrong).
        // Let's assert what we EXPECT it to be, so it fails.
        assert_eq!(winnings, token_amount, "Winnings calculation overflowed!");
    }
}
