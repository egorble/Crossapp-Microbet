// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use async_graphql::{Request, Response, SimpleObject};
use linera_sdk::linera_base_types::{AccountOwner, Amount, ContractAbi, ServiceAbi};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct PlayerStats {
    pub owner: AccountOwner,
    pub chain_id: String,
    pub wins: u64,
    pub losses: u64,
    pub total_won: Amount,
    pub total_lost: Amount,
    // Rank will be calculated dynamically in service
}

pub struct LeaderboardAbi;

impl ContractAbi for LeaderboardAbi {
    type Operation = Operation;
    type Response = ();
}

impl ServiceAbi for LeaderboardAbi {
    type Query = Request;
    type QueryResponse = Response;
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Operation {
    UpdateScore {
        owner: AccountOwner,
        chain_id: String,
        is_win: bool,
        amount: Amount,
    }
}
