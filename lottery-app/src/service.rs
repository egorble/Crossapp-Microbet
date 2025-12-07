#![cfg_attr(target_arch = "wasm32", no_main)]

use async_graphql::{EmptySubscription, Object, Schema, Request, Response};
use linera_sdk::{
    graphql::GraphQLMutationRoot, linera_base_types::WithServiceAbi, views::View, Service, ServiceRuntime,
};
use lottery_app::{LotteryAppAbi, LotteryAppParameters, Operation};
use std::sync::Arc;
use crate::state::LotteryAppState;

mod state;

pub struct LotteryAppService {
    _state: Arc<LotteryAppState>,
    runtime: Arc<ServiceRuntime<Self>>,
}

linera_sdk::service!(LotteryAppService);

impl WithServiceAbi for LotteryAppService {
    type Abi = LotteryAppAbi;
}

impl Service for LotteryAppService {
    type Parameters = LotteryAppParameters;

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = LotteryAppState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        LotteryAppService {
            _state: Arc::new(state),
            runtime: Arc::new(runtime),
        }
    }

    async fn handle_query(&self, query: Request) -> Response {
        let schema = Schema::build(
            QueryRoot,
            Operation::mutation_root(self.runtime.clone()),
            EmptySubscription,
        )
        .finish();
        schema.execute(query).await
    }
}

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn version(&self) -> &str {
        "0.1.0"
    }
}
