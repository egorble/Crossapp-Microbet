#![cfg_attr(target_arch = "wasm32", no_main)]

use async_graphql::{EmptySubscription, Object, Schema, Request, Response};
use linera_sdk::{
    graphql::GraphQLMutationRoot, linera_base_types::WithServiceAbi, views::View, Service, ServiceRuntime,
};
use lottery_rounds::{LotteryRoundsAbi, LotteryRoundsParameters, Operation, LotteryRound, TicketPurchase, LotteryWinnerInfo};
use std::sync::Arc;
use crate::state::LotteryRoundsState;

mod state;

pub struct LotteryRoundsService {
    state: Arc<LotteryRoundsState>,
    runtime: Arc<ServiceRuntime<Self>>,
}

linera_sdk::service!(LotteryRoundsService);

impl WithServiceAbi for LotteryRoundsService {
    type Abi = LotteryRoundsAbi;
}

impl Service for LotteryRoundsService {
    type Parameters = LotteryRoundsParameters;

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = LotteryRoundsState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        LotteryRoundsService {
            state: Arc::new(state),
            runtime: Arc::new(runtime),
        }
    }

    async fn handle_query(&self, query: Request) -> Response {
        let schema = Schema::build(
            QueryRoot {
                state: self.state.clone(),
                runtime: self.runtime.clone(),
            },
            Operation::mutation_root(self.runtime.clone()),
            EmptySubscription,
        )
        .finish();
        schema.execute(query).await
    }
}

pub struct QueryRoot {
    state: Arc<LotteryRoundsState>,
    runtime: Arc<ServiceRuntime<LotteryRoundsService>>,
}

#[Object]
impl QueryRoot {
    /// Get the active round ID
    async fn active_round_id(&self) -> Option<u64> {
        self.state.get_active_round().await.unwrap_or(None)
    }

    /// Get round by ID
    async fn round(&self, round_id: u64) -> Option<LotteryRound> {
        self.state.get_round(round_id).await.unwrap_or(None)
    }

    /// Get all rounds (Last 50 for performance safety)
    async fn all_rounds(&self) -> Vec<LotteryRound> {
        self.state.get_all_rounds().await.unwrap_or_default()
    }

    /// Get ticket purchases for a round
    async fn round_ticket_purchases(&self, round_id: u64) -> Vec<TicketPurchase> {
        self.state.get_round_ticket_purchases(round_id).await
            .map(|list| list.into_iter().map(|(_, purchase)| purchase).collect())
            .unwrap_or_default()
    }

    /// Get winners for a round
    async fn round_winners(&self, round_id: u64) -> Vec<LotteryWinnerInfo> {
        self.state.get_round_winners(round_id).await
            .map(|list| list.into_iter().map(|(ticket_number, owner, prize_amount, claimed)| {
                LotteryWinnerInfo {
                    chain_id: self.runtime.chain_id(),
                    ticket_number,
                    owner,
                    prize_amount,
                    claimed,
                    source_chain_id: None,
                }
            }).collect())
            .unwrap_or_default()
    }
}
