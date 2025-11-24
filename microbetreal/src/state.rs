// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use linera_sdk::views::{linera_views, RegisterView, RootView, ViewStorageContext};
use linera_sdk::linera_base_types::ApplicationId;

/// Minimal state for Microbetreal - just stores app IDs for coordination
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct MicrobetState {
    /// ApplicationId of the Native token app
    pub native_app_id: RegisterView<Option<ApplicationId<native::NativeAbi>>>,
    /// ApplicationId of the Rounds game app
    pub rounds_app_id: RegisterView<Option<ApplicationId<rounds::RoundsAbi>>>,
}
