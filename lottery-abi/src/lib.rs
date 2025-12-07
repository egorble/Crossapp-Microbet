// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/*! Shared ABI definitions for Lottery Applications */

use async_graphql::{Request, Response, SimpleObject};
use linera_sdk::linera_base_types::{AccountOwner, Amount, ChainId, ContractAbi, ServiceAbi};
use serde::{Deserialize, Serialize};

// ========================================
// Lottery App ABI (ticket purchase wrapper)
// ========================================

#[derive(Debug, Deserialize, Serialize)]
pub enum LotteryAppMessage {
    /// Notify message for cross-chain operations
    Notify,
    /// Cross-chain transfer for ticket purchase
    TransferForTickets {
        owner: AccountOwner,
        amount: Amount,
        source_chain_id: ChainId,
        source_owner: AccountOwner,
    },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LotteryAppParameters {
    pub native_app_id: ::linera_sdk::linera_base_types::ApplicationId,
    pub lottery_rounds_app_id: ::linera_sdk::linera_base_types::ApplicationId,
}

pub struct LotteryAppAbi;

impl ContractAbi for LotteryAppAbi {
    type Operation = LotteryAppOperation;
    type Response = LotteryAppResponse;
}

impl ServiceAbi for LotteryAppAbi {
    type Query = Request;
    type QueryResponse = Response;
}

#[derive(Debug, Deserialize, Serialize)]
pub enum LotteryAppOperation {
    /// Transfer tokens with optional ticket purchase
    Transfer {
        owner: AccountOwner,
        amount: Amount,
        target_account: linera_sdk::abis::fungible::Account,
        purchase_tickets: bool,
    },
    /// Claim tokens from another chain
    Claim {
        source_account: linera_sdk::abis::fungible::Account,
        amount: Amount,
        target_account: linera_sdk::abis::fungible::Account,
        purchase_tickets: bool,
    },
    /// Send prize to winner (called by lottery-rounds)
    SendPrize {
        recipient: AccountOwner,
        amount: Amount,
        source_chain_id: Option<String>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub enum LotteryAppResponse {
    Ok,
}

// ========================================
// Lottery Rounds ABI (round management)
// ========================================

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
    
    // Winner pool sizes
    pub pool1_count: u64,
    pub pool2_count: u64,
    pub pool3_count: u64,
    pub pool4_count: u64,
    
    // Winner pool progress
    pub pool1_winners_drawn: u64,
    pub pool2_winners_drawn: u64,
    pub pool3_winners_drawn: u64,
    pub pool4_winners_drawn: u64,
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

/// Ticket purchase info for queries
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct TicketPurchaseInfo {
    pub owner: AccountOwner,
    pub first_ticket: u64,
    pub last_ticket: u64,
    pub total_tickets: u64,
    pub amount_paid: Amount,
    pub source_chain_id: Option<String>,
}

/// Winner information
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct LotteryWinnerInfo {
    pub ticket_number: u64,
    pub owner: AccountOwner,
    pub prize_amount: Amount,
    pub claimed: bool,
    pub source_chain_id: Option<String>,
}

pub struct LotteryRoundsAbi;

impl ContractAbi for LotteryRoundsAbi {
    type Operation = LotteryRoundsOperation;
    type Response = LotteryRoundsResponse;
}

impl ServiceAbi for LotteryRoundsAbi {
    type Query = Request;
    type QueryResponse = Response;
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LotteryRoundsParameters {
    pub native_app_id: ::linera_sdk::linera_base_types::ApplicationId,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum LotteryRoundsOperation {
    // Round management
    CreateRound { ticket_price: Amount },
    CloseRound,
    GenerateWinner { round_id: u64 },
    
    // Ticket purchase (called by lottery-app)
    PurchaseTickets {
        owner: AccountOwner,
        amount: Amount,
        ticket_price: Amount,
        source_chain_id: Option<String>,
    },
    
    // Configuration
    SetLotteryAppId { lottery_app_id: String },

    // Queries
    GetActiveRound,
    GetRound { id: u64 },
    GetAllRounds,
    GetRoundTicketPurchases { round_id: u64 },
    GetUserTickets { round_id: u64, owner: AccountOwner },
    GetRoundWinners { round_id: u64 },
}

#[derive(Debug, Deserialize, Serialize)]
pub enum LotteryRoundsResponse {
    Ok,
    RoundId(u64),
    RoundStatus(RoundStatus),
    LotteryRound(Option<LotteryRound>),
    LotteryRounds(Vec<LotteryRound>),
    TicketPurchase(TicketPurchase),
    TicketPurchases(Vec<TicketPurchaseInfo>),
    LotteryWinners(Vec<LotteryWinnerInfo>),
    WinnerGenerated {
        round_id: u64,
        ticket_number: u64,
        owner: AccountOwner,
        prize_amount: Amount,
        new_round_created: bool,
        source_chain_id: Option<String>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub enum LotteryRoundsMessage {
    Notify,
}
