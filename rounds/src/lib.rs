// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/*! ABI of the Rounds Application for Prediction Game */

use async_graphql::{Request, Response, SimpleObject};
use linera_sdk::linera_base_types::{AccountOwner, Amount, ContractAbi, ServiceAbi};
use serde::{Deserialize, Serialize};

// Prediction direction for the Up/Down game
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, async_graphql::Enum)]
pub enum Prediction {
    Up,
    Down,
}

// Status of a prediction round
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, async_graphql::Enum)]
pub enum RoundStatus {
    Active,    // Round is accepting bets
    Closed,    // Round is closed, awaiting resolution
    Resolved,  // Round has been resolved with a result
}

// A prediction round for the Up/Down game
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct PredictionRound {
    pub id: u64,
    pub created_at: u64,
    pub closed_at: Option<u64>,
    pub resolved_at: Option<u64>,
    pub status: RoundStatus,
    pub closing_price: Option<Amount>,    // Price at which round was closed (fractional)
    pub resolution_price: Option<Amount>, // Price used to resolve the round (fractional)
    pub up_bets: u64,                     // Number of up bets
    pub down_bets: u64,                   // Number of down bets
    pub up_bets_pool: Amount,             // Total amount of up bets
    pub down_bets_pool: Amount,           // Total amount of down bets
    pub prize_pool: Amount,               // Total amount of tokens bet in this round
    pub result: Option<Prediction>,       // Result of the round (Up, Down, or None if not resolved)
}

// A user's bet in a prediction round
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct PredictionBet {
    pub owner: AccountOwner,
    pub amount: Amount,
    pub prediction: Prediction,
    pub claimed: bool, // Whether the reward has been claimed
}

// Active bet information for GraphQL queries
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct ActiveBetInfo {
    pub owner: AccountOwner,
    pub amount: Amount,
    pub prediction: Prediction,
}

// Winner information for a resolved round
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct RoundWinnerInfo {
    pub owner: AccountOwner,
    pub bet_amount: Amount,
    pub winnings: Amount,
    pub source_chain_id: Option<String>, // Add source chain ID for cross-chain winners
}

// Rounds Application ABI
pub struct RoundsAbi;

impl ContractAbi for RoundsAbi {
    type Operation = RoundsOperation;
    type Response = RoundsResponse;
}

impl ServiceAbi for RoundsAbi {
    type Query = Request;
    type QueryResponse = Response;
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RoundsParameters {
    pub native_app_id: ::linera_sdk::linera_base_types::ApplicationId,
    pub leaderboard_app_id: ::linera_sdk::linera_base_types::ApplicationId,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum RoundsOperation {
    // Round management operations
    /// Create a new prediction round
    CreateRound,
    /// Close the active round with a closing price
    CloseRound { closing_price: Amount },
    /// Resolve a closed round with a resolution price and distribute rewards
    ResolveRound { resolution_price: Amount },
    
    // Betting operations (called by NativeFungible app)
    /// Place a bet in the active round
    PlaceBet {
        owner: AccountOwner,
        amount: Amount,
        prediction: Prediction,
        source_chain_id: Option<String>, // For cross-chain attribution
    },
    /// Claim winnings from a resolved round (called by user directly)
    ClaimWinnings { round_id: u64 },
    
    // Configuration operations
    /// Set the Microbetreal app ID (called after deployment)
    SetMicrobetAppId { microbet_app_id: String },
    /// Set the chain ID where Leaderboard app is deployed (for cross-chain updates)
    /// If None, leaderboard is on the same chain as rounds
    SetLeaderboardChainId { chain_id: Option<String> },

    // Query operations for prediction game state
    /// Get the active round
    GetActiveRound,
    /// Get a specific round by ID
    GetRound { id: u64 },
    /// Get all rounds
    GetAllRounds,
    /// Get all active bets
    GetActiveBets,
    /// Get winners for a resolved round
    GetRoundWinners { round_id: u64 },
}

#[derive(Debug, Deserialize, Serialize)]
pub enum RoundsResponse {
    Ok,
    RoundId(u64),
    RoundStatus(RoundStatus),
    PredictionRound(Option<PredictionRound>),
    PredictionRounds(Vec<PredictionRound>),
    ActiveBets(Vec<ActiveBetInfo>),
    RoundWinners(Vec<RoundWinnerInfo>),
    // Add Winners response for ResolveRound to return winners list
    Winners(Vec<RoundWinnerInfo>),
}

// Message for cross-application communication
#[derive(Debug, Deserialize, Serialize)]
pub enum Message {
    /// Placeholder for future cross-chain messages
    Notify,
    /// Cross-chain leaderboard update
    LeaderboardUpdate {
        owner: AccountOwner,
        chain_id: String,
        is_win: bool,
        amount: Amount,
    },
}
