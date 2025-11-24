// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/*! ABI of the Native Fungible Token Application */

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
        prediction: Prediction,
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

// Prediction direction for the Up/Down game (still needed for transfers)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, async_graphql::Enum)]
pub enum Prediction {
    Up,
    Down,
}

// Власний ABI для розширеного контракту
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
    /// Transfer tokens between accounts with optional prediction
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
    
    // Rounds-related operations (cross-app calls to Rounds app)
    /// Set the Rounds app ApplicationId (must be called after both apps are deployed)
    SetRoundsAppId { rounds_app_id: String },
    /// Resolve a round and distribute rewards
    ResolveRound { resolution_price: Amount },
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ExtendedResponse {
    Balance(Amount),
    ChainBalance(Amount),
    TickerSymbol(String),
    Ok,
}