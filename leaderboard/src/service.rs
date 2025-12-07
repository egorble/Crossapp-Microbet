// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use async_graphql::{EmptyMutation, EmptySubscription, Object, Request, Response, Schema};
use linera_sdk::{
    linera_base_types::{AccountOwner, WithServiceAbi},
    views::View,
    Service, ServiceRuntime,
};
use std::sync::Arc;
use leaderboard::{LeaderboardAbi, PlayerStats};
use self::state::LeaderboardState;

pub struct LeaderboardService {
    runtime: Arc<ServiceRuntime<Self>>,
}

linera_sdk::service!(LeaderboardService);

impl WithServiceAbi for LeaderboardService {
    type Abi = LeaderboardAbi;
}

impl Service for LeaderboardService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        LeaderboardService {
            runtime: Arc::new(runtime),
        }
    }

    async fn handle_query(&self, query: Request) -> Response {
        let schema = Schema::build(
            QueryRoot { 
                storage_context: self.runtime.root_view_storage_context()
            },
            EmptyMutation,
            EmptySubscription,
        )
        .finish();
        schema.execute(query).await
    }
}

struct QueryRoot {
    storage_context: linera_sdk::views::ViewStorageContext,
}

#[Object]
impl QueryRoot {
    async fn player(&self, owner: AccountOwner) -> Option<PlayerStats> {
        let state = LeaderboardState::load(self.storage_context.clone())
            .await
            .expect("Failed to load state");
        state.players.get(&owner).await.ok().flatten()
    }

    async fn top_players(&self, limit: usize) -> Vec<PlayerStats> {
        let state = LeaderboardState::load(self.storage_context.clone())
            .await
            .expect("Failed to load state");
            
        let mut players = Vec::new();
        if let Ok(indices) = state.players.indices().await {
            for owner in indices {
                if let Ok(Some(stats)) = state.players.get(&owner).await {
                    players.push(stats);
                }
            }
        }
        
        // Sort by total won (descending)
        players.sort_by(|a, b| b.total_won.cmp(&a.total_won));
        
        players.into_iter().take(limit).collect()
    }
}
