// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use linera_sdk::views::{linera_views, MapView, RootView, ViewStorageContext};
use linera_sdk::linera_base_types::{AccountOwner, Amount};
use leaderboard::PlayerStats;

#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct LeaderboardState {
    pub players: MapView<AccountOwner, PlayerStats>,
}

impl LeaderboardState {
    pub async fn update_score(&mut self, owner: AccountOwner, chain_id: String, is_win: bool, amount: Amount) {
        let mut stats = self.players.get(&owner).await.expect("Failed to get player").unwrap_or(PlayerStats {
            owner,
            chain_id: chain_id.clone(),
            wins: 0,
            losses: 0,
            total_won: Amount::ZERO,
            total_lost: Amount::ZERO,
        });

        // Update chain ID to latest used
        stats.chain_id = chain_id;

        if is_win {
            stats.wins += 1;
            stats.total_won = stats.total_won.saturating_add(amount);
        } else {
            stats.losses += 1;
            stats.total_lost = stats.total_lost.saturating_add(amount);
        }

        self.players.insert(&owner, stats).expect("Failed to insert player stats");
    }
}
