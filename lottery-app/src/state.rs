// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use linera_sdk::views::{linera_views, RegisterView, RootView, ViewStorageContext};

/// The application state for Lottery App (minimal state, mostly a wrapper).
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct LotteryAppState {
    /// Placeholder - lottery app is mostly stateless, delegates to native and lottery-rounds
    pub initialized: RegisterView<bool>,
}
