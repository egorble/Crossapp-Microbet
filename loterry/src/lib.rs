// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/*! ABI of the Native Fungible Token with Lottery */

use async_graphql::{Request, Response, SimpleObject, InputObject};
use linera_sdk::linera_base_types::{AccountOwner, Amount, ContractAbi, ServiceAbi, ChainId};
use serde::{Deserialize, Serialize};

pub const TICKER_SYMBOL: &str = "NAT";

#[derive(Deserialize, SimpleObject)]
pub struct AccountEntry {
    pub key: AccountOwner,
    pub value: Amount,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Message {
    Notify,
    // Cross-chain transfer for ticket purchase
    TransferForTickets {
        owner: AccountOwner,
        amount: Amount,
        source_chain_id: ChainId,
        source_owner: AccountOwner,
    },
}

// GraphQL Input type для Account
#[derive(Debug, Deserialize, Serialize, InputObject)]
pub struct AccountInput {
    pub chain_id: ChainId,
    pub owner: AccountOwner,
}

// Status of a lottery round
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, async_graphql::Enum)]
pub enum RoundStatus {
    Active,   // Accepting ticket purchases
    Closed,   // Not accepting purchases, drawing winners
    Complete, // All winners drawn
}

// Winner pool identifier
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, async_graphql::Enum)]
pub enum WinnerPool {
    Pool1,    // 15% of tickets, 20% of prize
    Pool2,    // 7% of tickets, 25% of prize
    Pool3,    // 5% of tickets, 30% of prize
    Pool4,    // 3% of tickets, 25% of prize
    Complete, // All winners drawn
}

// A lottery round
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

// A user's ticket purchase
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct TicketPurchase {
    pub owner: AccountOwner,
    pub first_ticket: u64,
    pub last_ticket: u64,
    pub total_tickets: u64,
    pub amount_paid: Amount,
    pub source_chain_id: Option<String>,
}

// Ticket purchase information for GraphQL queries
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct TicketPurchaseInfo {
    pub chain_id: ChainId,
    pub owner: AccountOwner,
    pub first_ticket: u64,
    pub last_ticket: u64,
    pub total_tickets: u64,
    pub amount_paid: Amount,
}

// Winner information for a round
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct LotteryWinnerInfo {
    pub chain_id: ChainId,
    pub ticket_number: u64,
    pub owner: AccountOwner,
    pub prize_amount: Amount,
    pub claimed: bool,
    pub source_chain_id: Option<String>,
}

// ABI для контракту
pub struct ExtendedNativeFungibleTokenAbi;

impl ContractAbi for ExtendedNativeFungibleTokenAbi {
    type Operation = ExtendedOperation;
    type Response = ExtendedResponse;
}

impl ServiceAbi for ExtendedNativeFungibleTokenAbi {
    type Query = Request;
    type QueryResponse = Response;
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ExtendedOperation {
    /// Get balance for an account owner
    Balance { owner: AccountOwner },
    /// Get the chain balance (total balance of the chain)
    ChainBalance,
    /// Get the ticker symbol
    TickerSymbol,
    /// Transfer tokens between accounts
    Transfer {
        owner: AccountOwner,
        amount: Amount,
        target_account: linera_sdk::abis::fungible::Account,
        purchase_tickets: bool, // Flag to auto-purchase tickets
    },
    /// Claim tokens from another chain
    Claim {
        source_account: linera_sdk::abis::fungible::Account,
        amount: Amount,
        target_account: linera_sdk::abis::fungible::Account,
        purchase_tickets: bool, // Flag to auto-purchase tickets
    },
    /// Withdraw all tokens to chain account
    Withdraw,
    /// Mint new tokens to an account
    Mint {
        owner: AccountOwner,
        amount: Amount,
    },
    
    // Lottery operations
    /// Create a new lottery round with ticket price
    CreateLotteryRound { ticket_price: Amount },
    /// Purchase tickets in the active round
    PurchaseTickets { amount: Amount },
    /// Close the active round and calculate winner pools
    CloseLotteryRound,
    /// Generate one winner using VRF (automatically distributes prize)
    /// VRF value is generated automatically from timestamp + block height
    GenerateWinner { 
        round_id: u64,
    },

    // Query operations for lottery state
    /// Get the active round
    GetActiveRound,
    /// Get a specific round by ID
    GetRound { id: u64 },
    /// Get all rounds
    GetAllRounds,
    /// Get all ticket purchases for a round
    GetRoundTicketPurchases { round_id: u64 },
    /// Get user's tickets for a round
    GetUserTickets { round_id: u64, owner: AccountOwner },
    /// Get winners for a round
    GetRoundWinners { round_id: u64 },
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ExtendedResponse {
    Balance(Amount),
    ChainBalance(Amount),
    TickerSymbol(String),
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
    },
}