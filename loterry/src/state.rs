// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use linera_sdk::views::{linera_views, MapView, RegisterView, RootView, ViewStorageContext, ViewError};
use linera_sdk::linera_base_types::{AccountOwner, Amount};
use serde::{Deserialize, Serialize};
use async_graphql::SimpleObject;
use num_bigint::BigUint;
use num_traits::cast::ToPrimitive;

/// Calculate prize amount for a specific winner pool
/// Returns the portion of prize pool allocated to this pool
fn calculate_prize_for_pool(prize_pool: Amount, pool: WinnerPool) -> Amount {
    let prize_u128: u128 = u128::from(prize_pool);
    
    let percentage = match pool {
        WinnerPool::Pool1 => 20, // 15% of tickets get 20% of prize
        WinnerPool::Pool2 => 25, // 7% of tickets get 25% of prize
        WinnerPool::Pool3 => 30, // 5% of tickets get 30% of prize
        WinnerPool::Pool4 => 25, // 3% of tickets get 25% of prize
        WinnerPool::Complete => 0,
    };
    
    let pool_prize = (prize_u128 * percentage) / 100;
    Amount::from_attos(pool_prize)
}

/// Calculate individual prize per winner in a pool
/// Returns prize_for_pool / number_of_winners_in_pool
fn calculate_prize_per_winner(prize_pool: Amount, pool: WinnerPool, winners_in_pool: u64) -> Amount {
    if winners_in_pool == 0 {
        return Amount::ZERO;
    }
    
    let pool_prize = calculate_prize_for_pool(prize_pool, pool);
    let pool_prize_u128: u128 = u128::from(pool_prize);
    
    // Using BigUint to prevent overflow
    let pool_prize_big = BigUint::from(pool_prize_u128);
    let winners_big = BigUint::from(winners_in_pool);
    
    let prize_per_winner_big = pool_prize_big / winners_big;
    let prize_per_winner_u128 = prize_per_winner_big.to_u128().unwrap_or(u128::MAX);
    
    Amount::from_attos(prize_per_winner_u128)
}

/// The application state for the Native Fungible Token with Lottery.
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct NativeFungibleTokenState {
    /// The balances of accounts.
    pub accounts: MapView<AccountOwner, Amount>,
    
    /// Lottery state
    /// Counter for generating unique round IDs
    pub round_counter: RegisterView<u64>,
    /// All lottery rounds
    pub rounds: MapView<u64, LotteryRound>,
    /// The currently active round (accepting ticket purchases)
    pub active_round: RegisterView<Option<u64>>,
    /// Ticket purchases per round and user
    pub ticket_purchases: MapView<(u64, AccountOwner), TicketPurchase>,
    /// Mapping from ticket number to owner
    pub ticket_to_owner: MapView<(u64, u64), AccountOwner>,
    /// Winning tickets with prize info: (round_id, ticket_number) -> (owner, prize_amount, claimed)
    pub winning_tickets: MapView<(u64, u64), (AccountOwner, Amount, bool)>,
}

/// A lottery round
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct LotteryRound {
    pub id: u64,
    pub created_at: u64,
    pub closed_at: Option<u64>,
    pub status: RoundStatus,
    pub ticket_price: Amount,
    pub total_tickets_sold: u64,
    pub next_ticket_number: u64,
    pub prize_pool: Amount,
    pub current_winner_pool: WinnerPool,
    
    // Winner pool sizes (calculated when round closes)
    pub pool1_count: u64,  // 15% of tickets
    pub pool2_count: u64,  // 7% of tickets
    pub pool3_count: u64,  // 5% of tickets
    pub pool4_count: u64,  // 3% of tickets
    
    // Winner pool progress
    pub pool1_winners_drawn: u64,
    pub pool2_winners_drawn: u64,
    pub pool3_winners_drawn: u64,
    pub pool4_winners_drawn: u64,
}

/// Status of a lottery round
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, async_graphql::Enum)]
pub enum RoundStatus {
    Active,   // Accepting ticket purchases
    Closed,   // Not accepting purchases, drawing winners
    Complete, // All winners drawn
}

/// Winner pool identifier
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, async_graphql::Enum)]
pub enum WinnerPool {
    Pool1,    // 15% of tickets, 20% of prize
    Pool2,    // 7% of tickets, 25% of prize
    Pool3,    // 5% of tickets, 30% of prize
    Pool4,    // 3% of tickets, 25% of prize
    Complete, // All winners drawn
}

/// A user's ticket purchase
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct TicketPurchase {
    pub owner: AccountOwner,
    pub first_ticket: u64,
    pub last_ticket: u64,
    pub total_tickets: u64,
    pub amount_paid: Amount,
    pub source_chain_id: Option<String>,
}

#[allow(dead_code)]
impl NativeFungibleTokenState {
    /// Creates a new lottery round with specified ticket price
    pub async fn create_lottery_round(&mut self, ticket_price: Amount, timestamp: u64) -> Result<u64, String> {
        let round_id = *self.round_counter.get() + 1;
        self.round_counter.set(round_id);
        
        let round = LotteryRound {
            id: round_id,
            created_at: timestamp,
            closed_at: None,
            status: RoundStatus::Active,
            ticket_price,
            total_tickets_sold: 0,
            next_ticket_number: 1, // Tickets start from 1
            prize_pool: Amount::ZERO,
            current_winner_pool: WinnerPool::Pool1,
            pool1_count: 0,
            pool2_count: 0,
            pool3_count: 0,
            pool4_count: 0,
            pool1_winners_drawn: 0,
            pool2_winners_drawn: 0,
            pool3_winners_drawn: 0,
            pool4_winners_drawn: 0,
        };
        
        self.rounds.insert(&round_id, round)
            .map_err(|e: ViewError| format!("Failed to insert round: {:?}", e))?;
        self.active_round.set(Some(round_id));
        
        Ok(round_id)
    }
    
    /// Purchase tickets in the active round
    /// Automatically calculates number of tickets from amount and ticket price
    pub async fn purchase_tickets(&mut self, owner: AccountOwner, amount: Amount, current_balance: Amount, source_chain_id: Option<String>) -> Result<TicketPurchase, String> {
        let round_id_opt = self.active_round.get();
        
        if let Some(round_id) = *round_id_opt {
            let mut round = self.rounds.get(&round_id).await
                .map_err(|e: ViewError| format!("Failed to get round: {:?}", e))?
                .ok_or("Active round not found")?
                .clone();
            
            if round.status != RoundStatus::Active {
                return Err("No active round accepting ticket purchases".to_string());
            }
            
            // Check user balance
            if current_balance < amount {
                return Err("Insufficient balance".to_string());
            }
            
            // Calculate number of tickets
            let amount_u128 = u128::from(amount);
            let ticket_price_u128 = u128::from(round.ticket_price);
            
            if ticket_price_u128 == 0 {
                return Err("Invalid ticket price".to_string());
            }
            
            let ticket_count = amount_u128 / ticket_price_u128;
            if ticket_count == 0 {
                return Err("Amount too small to purchase any tickets".to_string());
            }
            
            let ticket_count_u64 = ticket_count as u64;
            
            // Assign ticket numbers
            let first_ticket = round.next_ticket_number;
            let last_ticket = first_ticket + ticket_count_u64 - 1;
            
            // Deduct from user's balance
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
            
            // Record the purchase
            let purchase = TicketPurchase {
                owner: owner.clone(),
                first_ticket,
                last_ticket,
                total_tickets: ticket_count_u64,
                amount_paid: amount,
                source_chain_id: source_chain_id.clone(),
            };
            
            // Store ticket purchase
            self.ticket_purchases.insert(&(round_id, owner.clone()), purchase.clone())
                .map_err(|e: ViewError| format!("Failed to record purchase: {:?}", e))?;
            
            // Map each ticket to owner
            for ticket_num in first_ticket..=last_ticket {
                self.ticket_to_owner.insert(&(round_id, ticket_num), owner.clone())
                    .map_err(|e: ViewError| format!("Failed to map ticket to owner: {:?}", e))?;
            }
            
            // Update round
            round.next_ticket_number = last_ticket + 1;
            round.total_tickets_sold += ticket_count_u64;
            round.prize_pool = round.prize_pool.saturating_add(amount);
            
            self.rounds.insert(&round_id, round)
                .map_err(|e: ViewError| format!("Failed to update round: {:?}", e))?;
            
            Ok(purchase)
        } else {
            Err("No active round".to_string())
        }
    }
    
    /// Close the active lottery round and calculate winner pools
    pub async fn close_lottery_round(&mut self, timestamp: u64) -> Result<u64, String> {
        let round_id_opt = self.active_round.get();
        
        if let Some(round_id) = *round_id_opt {
            let mut round = self.rounds.get(&round_id).await
                .map_err(|e: ViewError| format!("Failed to get round: {:?}", e))?
                .ok_or("Active round not found")?
                .clone();
            
            if round.status != RoundStatus::Active {
                return Err("Round is not active".to_string());
            }
            
            if round.total_tickets_sold < 4 {
                return Err("Cannot close round with fewer than 4 tickets sold".to_string());
            }
            
            // Calculate winner pool sizes
            // 30% of tickets are winners: 15% + 7% + 5% + 3% = 30%
            let total_tickets = round.total_tickets_sold;
            round.pool1_count = (total_tickets * 15) / 100; // 15% of tickets
            round.pool2_count = (total_tickets * 7) / 100;  // 7% of tickets
            round.pool3_count = (total_tickets * 5) / 100;  // 5% of tickets
            round.pool4_count = (total_tickets * 3) / 100;  // 3% of tickets
            
            // Ensure at least 1 winner in each pool if there are tickets
            if round.pool1_count == 0 && total_tickets > 0 {
                round.pool1_count = 1;
            }
            if round.pool2_count == 0 && total_tickets > 1 {
                round.pool2_count = 1;
            }
            if round.pool3_count == 0 && total_tickets > 2 {
                round.pool3_count = 1;
            }
            if round.pool4_count == 0 && total_tickets > 3 {
                round.pool4_count = 1;
            }
            
            round.status = RoundStatus::Closed;
            round.closed_at = Some(timestamp);
            round.current_winner_pool = WinnerPool::Pool1;
            
            self.rounds.insert(&round_id, round)
                .map_err(|e: ViewError| format!("Failed to update round: {:?}", e))?;
            self.active_round.set(None);
            
            Ok(round_id)
        } else {
            Err("No active round to close".to_string())
        }
    }
    
    /// Generate one winner using VRF and automatically distribute prize
    /// Returns: (round_id, ticket_number, owner, prize_amount, new_round_created)
    pub async fn generate_winner(&mut self, vrf_value: u64, round_id: u64, current_timestamp: u64) -> Result<(u64, u64, AccountOwner, Amount, bool), String> {
        let mut round = self.rounds.get(&round_id).await
            .map_err(|e: ViewError| format!("Failed to get round: {:?}", e))?
            .ok_or("Round not found")?
            .clone();
        
        if round.status != RoundStatus::Closed {
            return Err("Round is not closed".to_string());
        }
        
        // Determine current pool and check if complete
        let (pool, winners_count, winners_drawn) = match round.current_winner_pool {
            WinnerPool::Pool1 => (WinnerPool::Pool1, round.pool1_count, round.pool1_winners_drawn),
            WinnerPool::Pool2 => (WinnerPool::Pool2, round.pool2_count, round.pool2_winners_drawn),
            WinnerPool::Pool3 => (WinnerPool::Pool3, round.pool3_count, round.pool3_winners_drawn),
            WinnerPool::Pool4 => (WinnerPool::Pool4, round.pool4_count, round.pool4_winners_drawn),
            WinnerPool::Complete => return Err("All winners already drawn".to_string()),
        };
        
        if winners_drawn >= winners_count {
            return Err("Current pool complete, should not happen".to_string());
        }
        
        // Get all winning tickets already drawn for this round
        let existing_winners = self.winning_tickets.indices().await
            .map_err(|e: ViewError| format!("Failed to get winning ticket indices: {:?}", e))?
            .into_iter()
            .filter(|(rid, _)| *rid == round_id)
            .map(|(_, ticket)| ticket)
            .collect::<std::collections::HashSet<_>>();
        
        // Select a random ticket that hasn't won yet
        let mut attempts = 0u64;
        let max_attempts = round.total_tickets_sold * 2;
        let selected_ticket;
        
        loop {
            if attempts >= max_attempts {
                return Err("Failed to find unique winning ticket after many attempts".to_string());
            }
            
            // Generate pseudo-random ticket number using VRF value + attempts
            let seed = vrf_value.wrapping_add(attempts);
            let ticket = (seed % round.total_tickets_sold) + 1; // Tickets are 1-indexed
            
            if !existing_winners.contains(&ticket) {
                selected_ticket = ticket;
                break;
            }
            
            attempts += 1;
        }
        
        // Get ticket owner
        let owner = self.ticket_to_owner.get(&(round_id, selected_ticket)).await
            .map_err(|e: ViewError| format!("Failed to get ticket owner: {:?}", e))?
            .ok_or("Ticket has no owner")?;
        
        // Calculate prize for this winner
        let prize_amount = calculate_prize_per_winner(round.prize_pool, pool, winners_count);
        
        // Record winning ticket
        self.winning_tickets.insert(&(round_id, selected_ticket), (owner.clone(), prize_amount, true))
            .map_err(|e: ViewError| format!("Failed to record winning ticket: {:?}", e))?;
        
        // Update round progress
        match pool {
            WinnerPool::Pool1 => round.pool1_winners_drawn += 1,
            WinnerPool::Pool2 => round.pool2_winners_drawn += 1,
            WinnerPool::Pool3 => round.pool3_winners_drawn += 1,
            WinnerPool::Pool4 => round.pool4_winners_drawn += 1,
            WinnerPool::Complete => {},
        }
        
        // Check if current pool is complete and advance to next pool
        let mut new_round_created = false;
        let current_pool_complete = match pool {
            WinnerPool::Pool1 => round.pool1_winners_drawn >= round.pool1_count,
            WinnerPool::Pool2 => round.pool2_winners_drawn >= round.pool2_count,
            WinnerPool::Pool3 => round.pool3_winners_drawn >= round.pool3_count,
            WinnerPool::Pool4 => round.pool4_winners_drawn >= round.pool4_count,
            WinnerPool::Complete => false,
        };
        
        if current_pool_complete {
            // Advance to next pool
            round.current_winner_pool = match pool {
                WinnerPool::Pool1 => WinnerPool::Pool2,
                WinnerPool::Pool2 => WinnerPool::Pool3,
                WinnerPool::Pool3 => WinnerPool::Pool4,
                WinnerPool::Pool4 => {
                    // All pools complete, mark round as complete
                    round.status = RoundStatus::Complete;
                    WinnerPool::Complete
                },
                WinnerPool::Complete => WinnerPool::Complete,
            };
            
            // If all pools complete, automatically create new round
            if round.current_winner_pool == WinnerPool::Complete {
                // Use current timestamp for the new round instead of the old round's timestamp
                let new_round_id = self.create_lottery_round(round.ticket_price, current_timestamp).await?;
                new_round_created = true;
                eprintln!("All winners drawn for round {}. Created new round {} at timestamp {}", round_id, new_round_id, current_timestamp);
            }
        }
        
        // Save updated round
        self.rounds.insert(&round_id, round)
            .map_err(|e: ViewError| format!("Failed to update round: {:?}", e))?;
        
        Ok((round_id, selected_ticket, owner, prize_amount, new_round_created))
    }
    
    /// Get lottery round by ID
    pub async fn get_round(&self, round_id: u64) -> Result<Option<LotteryRound>, String> {
        self.rounds.get(&round_id).await
            .map_err(|e: ViewError| format!("Failed to get round: {:?}", e))
    }
    
    /// Get all lottery rounds
    pub async fn get_all_rounds(&self) -> Result<Vec<LotteryRound>, String> {
        let indices = self.rounds.indices().await
            .map_err(|e: ViewError| format!("Failed to get round indices: {:?}", e))?;
        
        let mut rounds = Vec::with_capacity(indices.len());
        
        for index in indices {
            if let Some(round) = self.rounds.get(&index).await
                .map_err(|e: ViewError| format!("Failed to get round: {:?}", e))? {
                rounds.push(round);
            }
        }
        Ok(rounds)
    }
    
    /// Get active round ID
    pub async fn get_active_round(&self) -> Result<Option<u64>, String> {
        Ok(*self.active_round.get())
    }
    
    /// Get ticket purchases for a specific round
    pub async fn get_round_ticket_purchases(&self, round_id: u64) -> Result<Vec<(AccountOwner, TicketPurchase)>, String> {
        let indices = self.ticket_purchases.indices().await
            .map_err(|e: ViewError| format!("Failed to get ticket purchase indices: {:?}", e))?;
        
        let mut purchases = Vec::new();
        
        for (rid, owner) in indices {
            if rid == round_id {
                if let Some(purchase) = self.ticket_purchases.get(&(rid, owner.clone())).await
                    .map_err(|e: ViewError| format!("Failed to get ticket purchase: {:?}", e))? {
                    purchases.push((owner, purchase));
                }
            }
        }
        
        Ok(purchases)
    }
    
    /// Get user's tickets for a specific round
    pub async fn get_user_tickets(&self, round_id: u64, owner: AccountOwner) -> Result<Option<TicketPurchase>, String> {
        self.ticket_purchases.get(&(round_id, owner)).await
            .map_err(|e: ViewError| format!("Failed to get user tickets: {:?}", e))
    }
    
    /// Get all winning tickets for a round
    pub async fn get_round_winners(&self, round_id: u64) -> Result<Vec<(u64, AccountOwner, Amount, bool)>, String> {
        let indices = self.winning_tickets.indices().await
            .map_err(|e: ViewError| format!("Failed to get winning ticket indices: {:?}", e))?;
        
        let mut winners = Vec::new();
        
        for (rid, ticket_number) in indices {
            if rid == round_id {
                if let Some((owner, prize, claimed)) = self.winning_tickets.get(&(rid, ticket_number)).await
                    .map_err(|e: ViewError| format!("Failed to get winning ticket: {:?}", e))? {
                    winners.push((ticket_number, owner, prize, claimed));
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
