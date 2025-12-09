// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use linera_sdk::views::{linera_views, MapView, RegisterView, RootView, ViewStorageContext, ViewError};
use linera_sdk::linera_base_types::{AccountOwner, Amount, ApplicationId};
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
    
    // Check for division by zero (and empty winner pool)
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

/// The application state for the Rounds application
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct RoundsState {
    /// ApplicationId of the Microbetreal app (for sending SendReward operations)
    pub microbet_app_id: RegisterView<Option<ApplicationId<native_fungible_abi::ExtendedNativeFungibleTokenAbi>>>,

    /// ApplicationId of the Native Fungible Token app (for reference)
    pub native_app_id: RegisterView<Option<ApplicationId>>,
    
    
    /// Chain ID where Leaderboard app is deployed (for cross-chain updates)
    /// If None, leaderboard is on the same chain
    pub leaderboard_chain_id: RegisterView<Option<String>>,
    
    /// Counter for generating unique round IDs
    pub round_counter: RegisterView<u64>,
    /// All prediction rounds
    pub rounds: MapView<u64, PredictionRound>,
    /// The currently active round (accepting bets)
    pub active_round: RegisterView<Option<u64>>,
    /// Bets placed in the active round
    pub active_bets: MapView<AccountOwner, PredictionBet>,
    /// Bets placed in closed rounds (awaiting resolution)
    pub closed_bets: MapView<(u64, AccountOwner), PredictionBet>,
    /// Bets placed in resolved rounds (awaiting claim)
    pub resolved_bets: MapView<(u64, AccountOwner), PredictionBet>,
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
    pub amount_up: Amount,
    pub amount_down: Amount,
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
impl RoundsState {
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
                if let Some(bet) = self.active_bets.get(owner).await
                    .map_err(|e: ViewError| format!("Failed to get active bet: {:?}", e))? {
                    
                    if !bet.amount_up.is_zero() {
                        up_bets += 1;
                        up_bets_pool = up_bets_pool.saturating_add(bet.amount_up);
                        prize_pool = prize_pool.saturating_add(bet.amount_up);
                    }
                    if !bet.amount_down.is_zero() {
                        down_bets += 1;
                        down_bets_pool = down_bets_pool.saturating_add(bet.amount_down);
                        prize_pool = prize_pool.saturating_add(bet.amount_down);
                    }
                    
                    // Collect bets to move for later processing
                    bets_to_move.push((owner.clone(), bet));
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
            for (owner, bet) in bets_to_move {
                let bet_key = (round_id, owner);
                self.closed_bets.insert(&bet_key, bet)
                    .map_err(|e: ViewError| format!("Failed to move bet to closed: {:?}", e))?;
                self.active_bets.remove(&bet_key.1)
                    .map_err(|e: ViewError| format!("Failed to remove active bet: {:?}", e))?;
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
    

    
    /// Resolve a closed round and return list of all bets with their outcomes for reward distribution and stats
    /// Returns: Vec<(AccountOwner, bet_amount, winnings, is_win, source_chain_id)>
    pub async fn resolve_round_and_distribute_rewards(&mut self, round_id: u64, resolution_price: Amount, timestamp: u64) -> Result<Vec<(AccountOwner, Amount, Amount, bool, Option<String>)>, String> {
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
        
        self.rounds.insert(&round_id, round.clone())
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
        for (bet_key, bet) in &bets_to_move {
            self.resolved_bets.insert(bet_key, bet.clone())
                .map_err(|e: ViewError| format!("Failed to move bet to resolved: {:?}", e))?;
            self.closed_bets.remove(&bet_key)
                .map_err(|e: ViewError| format!("Failed to remove closed bet: {:?}", e))?;
        }
        
        // Initialize results vector
        let mut results = Vec::new();
        
        // Calculate total prize pool and winner pool for calculations
        let total_prize_pool = round.prize_pool;
        let winner_pool = match result {
            Some(Prediction::Up) => round.up_bets_pool,
            Some(Prediction::Down) => round.down_bets_pool,
            None => Amount::ZERO,
        };

        // Reuse bets_to_move (which contains all bets for this round) to generate results
        for (_, bet) in &bets_to_move {
             // Calculate winnings for UP and DOWN
             let mut winnings_up = Amount::ZERO;
             let mut winnings_down = Amount::ZERO;

             if !winner_pool.is_zero() {
                 match result {
                    Some(Prediction::Up) => {
                         if !bet.amount_up.is_zero() {
                             winnings_up = calculate_winnings_proportional(bet.amount_up, winner_pool, total_prize_pool);
                         }
                    },
                    Some(Prediction::Down) => {
                         if !bet.amount_down.is_zero() {
                             winnings_down = calculate_winnings_proportional(bet.amount_down, winner_pool, total_prize_pool);
                         }
                    },
                    None => {},
                 }
             }

             let total_wagered = bet.amount_up.saturating_add(bet.amount_down);
             let total_winnings = winnings_up.saturating_add(winnings_down);
             
             // Logic for leaderboard:
             // 1. Calculate Net Profit = Total Winnings - Total Wagered
             // 2. If Net Profit > 0: Player WON. Amount = Net Profit.
             // 3. If Net Profit <= 0: Player LOST (or broke even). Amount = Total Wagered - Total Winnings (Net Loss).
             
             let is_win = total_winnings > total_wagered;
             let amount_for_leaderboard = if is_win {
                 total_winnings.saturating_sub(total_wagered)
             } else {
                 total_wagered.saturating_sub(total_winnings)
             };

             // We return a SINGLE entry per user for this round.
             // The `amount` field in the result tuple will now represent the Clean Profit (if win) or Net Loss (if loss).
             // The `bet_amount` field usually isn't used for logic downstream other than display, so we put total_wagered there.
             // The `winnings` field usually represents generic winnings, we put total_winnings there.
             
             results.push((
                 bet.owner, 
                 total_wagered, 
                 total_winnings, // This is the amount sent to user wallet
                 is_win, 
                 bet.source_chain_id.clone()
             ));
        }
        
        Ok(results)
    }
    

    
    /// Place a bet in the active round
    pub async fn place_bet(&mut self, owner: AccountOwner, amount: Amount, prediction: Prediction, source_chain_id: Option<String>) -> Result<(), String> {
        let round_id_opt = self.active_round.get();
        
        if let Some(round_id) = *round_id_opt {
            let mut round = self.rounds.get(&round_id).await
                .map_err(|e: ViewError| format!("Failed to get round: {:?}", e))?
                .ok_or("Active round not found")?
                .clone();
            
            if round.status != RoundStatus::Active {
                return Err("No active round accepting bets".to_string());
            }
            
            // Check if user already placed a bet
            let existing_bet = self.active_bets.get(&owner).await
                .map_err(|e: ViewError| format!("Failed to check bet existence: {:?}", e))?;
            
            let bet = if let Some(mut old_bet) = existing_bet {
                // Update existing bet
                match prediction {
                    Prediction::Up => {
                        // Increment counter only if this side was previously empty (new unique bettor for this side)
                        // Actually, logic is messy if we count unique bettors. Simplest is: don't increment counters on update. 
                        // But if they had 0 on Up and now bet on Up, they ARE a new Up bettor.
                        if old_bet.amount_up.is_zero() {
                            round.up_bets += 1;
                        }
                        old_bet.amount_up = old_bet.amount_up.saturating_add(amount);
                    },
                    Prediction::Down => {
                         if old_bet.amount_down.is_zero() {
                            round.down_bets += 1;
                        }
                        old_bet.amount_down = old_bet.amount_down.saturating_add(amount);
                    }
                }
                old_bet
            } else {
                // New bet
                let (amount_up, amount_down) = match prediction {
                    Prediction::Up => {
                        round.up_bets += 1;
                        (amount, Amount::ZERO)
                    },
                    Prediction::Down => {
                        round.down_bets += 1;
                        (Amount::ZERO, amount)
                    },
                };
                
                PredictionBet {
                    owner,
                    amount_up,
                    amount_down,
                    claimed: false,
                    source_chain_id,
                }
            };
            
            self.active_bets.insert(&owner, bet)
                .map_err(|e: ViewError| format!("Failed to place bet: {:?}", e))?;
            
            // Update global pools and prize pool
            match prediction {
                Prediction::Up => {
                    round.up_bets_pool = round.up_bets_pool.saturating_add(amount);
                },
                Prediction::Down => {
                    round.down_bets_pool = round.down_bets_pool.saturating_add(amount);
                },
            }
            round.prize_pool = round.prize_pool.saturating_add(amount);
            
            // Save updated round
            self.rounds.insert(&round_id, round.clone())
                .map_err(|e: ViewError| format!("Failed to update round statistics: {:?}", e))?;
        } else {
            return Err("No active round".to_string());
        }

        
        Ok(())
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
            if let Some(bet) = self.active_bets.get(&owner).await
                .map_err(|e: ViewError| format!("Failed to get active bet: {:?}", e))? {
                bets.push((owner, bet));
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
    
    /// Get winners for a resolved round (returns: owner, bet_amount, winnings, source_chain_id)
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
            if let Some(bet) = self.resolved_bets.get(&(id, owner.clone())).await
                .map_err(|e: ViewError| format!("Failed to get bet: {:?}", e))? {
                
                // Only include winners who haven't claimed yet
                // Logic check: verify they won on the winning side and haven't claimed
                // Since `claimed` is a single bool, it's global for the user in this round.
                // Assuming "claimed" means "claimed everything".
                
                if !bet.claimed { // Not yet claimed
                    let mut winnings = Amount::ZERO;
                    let mut bet_amount = Amount::ZERO;
                    
                    match result {
                        Prediction::Up => {
                            if !bet.amount_up.is_zero() {
                                bet_amount = bet.amount_up;
                                winnings = calculate_winnings_proportional(bet.amount_up, winner_pool, total_prize_pool);
                            }
                        },
                        Prediction::Down => {
                            if !bet.amount_down.is_zero() {
                                bet_amount = bet.amount_down;
                                winnings = calculate_winnings_proportional(bet.amount_down, winner_pool, total_prize_pool);
                            }
                        }
                    }
                    
                    if !winnings.is_zero() {
                        winners.push((owner, bet_amount, winnings, bet.source_chain_id.clone()));
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
        
        let bet_amount = token_amount;
        let winner_pool = token_amount;
        let total_prize_pool = token_amount;
        
        let winnings = calculate_winnings_proportional(bet_amount, winner_pool, total_prize_pool);
        
        println!("Winnings: {:?}", winnings);
        assert_eq!(winnings, token_amount, "Winnings calculation overflowed!");
    }
}
