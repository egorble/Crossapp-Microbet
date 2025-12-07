// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/*! ABI definitions for Native Fungible Token Application */

use async_graphql::{Request, Response, SimpleObject, InputObject};
use linera_sdk::linera_base_types::{AccountOwner, Amount, ContractAbi, ServiceAbi, ChainId};
use serde::{Deserialize, Serialize};

pub const TICKER_SYMBOL: &str = "NAT";

// Prediction direction for the Up/Down game
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, async_graphql::Enum)]
pub enum Prediction {
    Up,
    Down,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Message {
    Notify,
    // Cross-chain transfer with prediction information
    TransferWithPrediction {
        owner: AccountOwner,
        amount: Amount,
        prediction: Prediction,
        source_chain_id: ChainId,
        source_owner: AccountOwner,
    },
    // Send reward to winner (called by Rounds app)
    SendReward {
        recipient: AccountOwner,
        amount: Amount,
        source_chain_id: Option<String>,
    },
}

// GraphQL Input type для Account
#[derive(InputObject, Debug, Clone)]
pub struct AccountInput {
    pub chain_id: ChainId,
    pub owner: AccountOwner,
}

#[derive(SimpleObject)]
pub struct AccountEntry {
    pub key: AccountOwner,
    pub value: Amount,
}

// Extended operations for NativeFungible with game integration
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
        prediction: Option<Prediction>,
    },
    /// Claim tokens from another chain
    Claim {
        source_account: linera_sdk::abis::fungible::Account,
        amount: Amount,
        target_account: linera_sdk::abis::fungible::Account,
        prediction: Option<Prediction>,
    },
    /// Withdraw all tokens to chain account
    Withdraw,
    /// Mint new tokens to an account
    Mint {
        owner: AccountOwner,
        amount: Amount,
    },
    
    // Admin operations (for Microbetreal)
    /// Set the Native token app ApplicationId (Microbetreal only)
    SetNativeAppId { native_app_id: String },
    /// Set the Rounds app ApplicationId (Microbetreal only)
    SetRoundsAppId { rounds_app_id: String },
    
    // Called by Rounds app to send rewards
    /// Send reward to winner (internal - called by Rounds app)
    SendReward {
        recipient: AccountOwner,
        amount: Amount,
        source_chain_id: Option<String>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ExtendedResponse {
    Ok,
    Balance(Amount),
    ChainBalance(Amount),
    TickerSymbol(String),
}

pub struct ExtendedNativeFungibleTokenAbi;

impl ContractAbi for ExtendedNativeFungibleTokenAbi {
    type Operation = ExtendedOperation;
    type Response = ExtendedResponse;
}

impl ServiceAbi for ExtendedNativeFungibleTokenAbi {
    type Query = Request;
    type QueryResponse = Response;
}
