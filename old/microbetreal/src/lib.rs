// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/*! ABI of the Native Fungible Token Example Application */

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
    // Cross-chain transfer with prediction information
    TransferWithPrediction {
        owner: AccountOwner,
        amount: Amount,
        prediction: Option<Prediction>,
        source_chain_id: ChainId,  // Add source chain ID to properly track cross-chain transfers
        source_owner: AccountOwner, // Add source owner to properly attribute the bet
    },
}

// GraphQL Input type для Account
#[derive(Debug, Deserialize, Serialize, InputObject)]
pub struct AccountInput {
    pub chain_id: ChainId,
    pub owner: AccountOwner,
}

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
    pub chain_id: ChainId,
    pub owner: AccountOwner,
    pub amount: Amount,
    pub prediction: Prediction,
}

// Winner information for a resolved round
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct RoundWinnerInfo {
    pub chain_id: ChainId,
    pub owner: AccountOwner,
    pub bet_amount: Amount,
    pub winnings: Amount,
    pub source_chain_id: Option<String>, // Add source chain ID for cross-chain winners
}

// Власний ABI для розширеного контрактиту
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
        prediction: Option<Prediction>, // Optional prediction for betting
    },
    /// Claim tokens from another chain
    Claim {
        source_account: linera_sdk::abis::fungible::Account,
        amount: Amount,
        target_account: linera_sdk::abis::fungible::Account,
        prediction: Option<Prediction>, // Optional prediction for betting
    },
    /// Withdraw all tokens to chain account
    Withdraw,
    /// Mint new tokens to an account
    Mint {
        owner: AccountOwner,
        amount: Amount,
    },
    
    // Prediction game operations
    /// Create a new prediction round
    CreateRound,
    /// Close the active round with a closing price
    CloseRound { closing_price: Amount },
    /// Resolve a closed round with a resolution price
    ResolveRound { resolution_price: Amount },
    /// Place a bet in the active round
    PlaceBet { amount: Amount, prediction: Prediction },
    /// Claim winnings from a resolved round
    ClaimWinnings { round_id: u64 },

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
pub enum ExtendedResponse {
    Balance(Amount),
    ChainBalance(Amount),
    TickerSymbol(String),
    Ok,
    RoundId(u64),
    RoundStatus(RoundStatus),
    PredictionRound(Option<PredictionRound>),
    PredictionRounds(Vec<PredictionRound>),
    ActiveBets(Vec<ActiveBetInfo>),
    RoundWinners(Vec<RoundWinnerInfo>),
}