// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use linera_sdk::views::{linera_views, RegisterView, RootView, ViewStorageContext};

/// Pure token state - no game logic
/// Note: We need at least one field for RootView, so we use a dummy field
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct NativeState {
    /// Dummy field (RootView requires at least one field)
    pub _dummy: RegisterView<()>,
}
