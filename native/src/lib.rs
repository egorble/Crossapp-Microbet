// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/*! Pure Native Fungible Token Application - No Game Logic */

use async_graphql::{Request, Response, SimpleObject, InputObject};
use linera_sdk::linera_base_types::{AccountOwner, Amount, ContractAbi, ServiceAbi, ChainId};
use serde::{Deserialize, Serialize};

pub const TICKER_SYMBOL: &str = "NAT";

#[derive(Debug, Deserialize, Serialize)]
pub enum Message {
    Notify,
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

// Pure token operations - NO prediction/betting logic
#[derive(Debug, Deserialize, Serialize)]
pub enum NativeOperation {
    /// Get balance for an account owner
    Balance { owner: AccountOwner },
    /// Get the chain balance
    ChainBalance,
    /// Get the ticker symbol
    TickerSymbol,
    /// Transfer tokens between accounts (pure transfer, no prediction)
    Transfer {
        owner: AccountOwner,
        amount: Amount,
        target_account: linera_sdk::abis::fungible::Account,
    },
    /// Claim tokens from another chain
    Claim {
        source_account: linera_sdk::abis::fungible::Account,
        amount: Amount,
        target_account: linera_sdk::abis::fungible::Account,
    },
    /// Withdraw all tokens to chain account
    Withdraw,
    /// Mint new tokens to an account
    Mint {
        owner: AccountOwner,
        amount: Amount,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub enum NativeResponse {
    Ok,
    Balance(Amount),
    ChainBalance(Amount),
    TickerSymbol(String),
}

pub struct NativeAbi;

impl ContractAbi for NativeAbi {
    type Operation = NativeOperation;
    type Response = NativeResponse;
}

impl ServiceAbi for NativeAbi {
    type Query = Request;
    type QueryResponse = Response;
}