// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use linera_sdk::{
    linera_base_types::WithContractAbi,
    views::{RootView, View},
    Contract, ContractRuntime,
};
use leaderboard::{LeaderboardAbi, Operation};
use self::state::LeaderboardState;

pub struct LeaderboardContract {
    state: LeaderboardState,
    #[allow(dead_code)]
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(LeaderboardContract);

impl WithContractAbi for LeaderboardContract {
    type Abi = LeaderboardAbi;
}

impl Contract for LeaderboardContract {
    type Message = ();
    type Parameters = ();
    type InstantiationArgument = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = LeaderboardState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        LeaderboardContract { state, runtime }
    }

    async fn instantiate(&mut self, _arg: Self::InstantiationArgument) {
        // No initialization needed
    }

    async fn execute_operation(&mut self, operation: Self::Operation) -> Self::Response {
        match operation {
            Operation::UpdateScore { owner, chain_id, is_win, amount } => {
                self.state.update_score(owner, chain_id, is_win, amount).await;
            }
        }
    }

    async fn execute_message(&mut self, _message: Self::Message) {
        // No messages
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}
