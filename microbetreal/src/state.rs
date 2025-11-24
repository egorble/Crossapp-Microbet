// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use linera_sdk::views::{linera_views, RegisterView, RootView, ViewStorageContext};
use linera_sdk::linera_base_types::ApplicationId;

/// The application state for the Native Fungible Token.
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct NativeFungibleTokenState {
    /// ApplicationId of the Rounds app (for cross-app calls)
    /// Using generic ApplicationId with rounds::RoundsAbi type parameter
    pub rounds_app_id: RegisterView<Option<ApplicationId<rounds::RoundsAbi>>>,
}
