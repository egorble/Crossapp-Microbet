use linera_sdk::views::{RootView, ViewStorageContext, RegisterView, linera_views};


#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct LotteryAppState {
    // Dummy field to satisfy RootView macro requirements for non-empty struct
    pub dummy: RegisterView<u8>,
}
