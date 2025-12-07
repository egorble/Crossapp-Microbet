// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use std::sync::Arc;
use async_graphql::{EmptySubscription, Object, Request, Response, Schema};
use linera_sdk::{
    linera_base_types::{AccountOwner, Amount, WithServiceAbi},
    views::View,
    Service, ServiceRuntime,
};
use lottery_abi::{
    LotteryRoundsAbi, LotteryRound, RoundStatus, WinnerPool, TicketPurchaseInfo, LotteryWinnerInfo,
    LotteryRoundsParameters, LotteryRoundsOperation,
};
use self::state::{LotteryRoundsState, LotteryRound as StateLotteryRound, RoundStatus as StateRoundStatus, WinnerPool as StateWinnerPool};

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

    async fn handle_query(&self, request: Request) -> Response {
        let schema = Schema::build(
            QueryRoot {
                state: self.state.clone(),
                runtime: self.runtime.clone(),
            },
            MutationRoot {
                runtime: self.runtime.clone(),
            },
            EmptySubscription,
        )
        .finish();
        schema.execute(request).await
    }
}

struct QueryRoot {
    state: Arc<LotteryRoundsState>,
    runtime: Arc<ServiceRuntime<LotteryRoundsService>>,
}

// Conversion functions
fn convert_round_status(status: StateRoundStatus) -> RoundStatus {
    match status {
        StateRoundStatus::Active => RoundStatus::Active,
        StateRoundStatus::Closed => RoundStatus::Closed,
        StateRoundStatus::Complete => RoundStatus::Complete,
    }
}

fn convert_winner_pool(pool: StateWinnerPool) -> WinnerPool {
    match pool {
        StateWinnerPool::Pool1 => WinnerPool::Pool1,
        StateWinnerPool::Pool2 => WinnerPool::Pool2,
        StateWinnerPool::Pool3 => WinnerPool::Pool3,
        StateWinnerPool::Pool4 => WinnerPool::Pool4,
        StateWinnerPool::Complete => WinnerPool::Complete,
    }
}

fn convert_round(round: StateLotteryRound) -> LotteryRound {
    LotteryRound {
        id: round.id,
        created_at: round.created_at,
        closed_at: round.closed_at,
        status: convert_round_status(round.status),
        ticket_price: round.ticket_price,
        total_tickets_sold: round.total_tickets_sold,
        next_ticket_number: round.next_ticket_number,
        prize_pool: round.prize_pool,
        current_winner_pool: convert_winner_pool(round.current_winner_pool),
        pool1_count: round.pool1_count,
        pool2_count: round.pool2_count,
        pool3_count: round.pool3_count,
        pool4_count: round.pool4_count,
        pool1_winners_drawn: round.pool1_winners_drawn,
        pool2_winners_drawn: round.pool2_winners_drawn,
        pool3_winners_drawn: round.pool3_winners_drawn,
        pool4_winners_drawn: round.pool4_winners_drawn,
    }
}

#[Object]
impl QueryRoot {
    /// Get the active lottery round
    async fn active_round(&self) -> Option<LotteryRound> {
        let round_id = self.state.get_active_round().await.ok()??;
        let round = self.state.get_round(round_id).await.ok()??;
        Some(convert_round(round))
    }

    /// Get a specific round by ID
    async fn round(&self, id: u64) -> Option<LotteryRound> {
        let round = self.state.get_round(id).await.ok()??;
        Some(convert_round(round))
    }

    /// Get all rounds
    async fn all_rounds(&self) -> Vec<LotteryRound> {
        self.state.get_all_rounds().await
            .unwrap_or_default()
            .into_iter()
            .map(convert_round)
            .collect()
    }

    /// Get ticket purchases for a round
    async fn round_ticket_purchases(&self, round_id: u64) -> Vec<TicketPurchaseInfo> {
        self.state.get_round_ticket_purchases(round_id).await
            .unwrap_or_default()
            .into_iter()
            .map(|(owner, purchase)| TicketPurchaseInfo {
                owner,
                first_ticket: purchase.first_ticket,
                last_ticket: purchase.last_ticket,
                total_tickets: purchase.total_tickets,
                amount_paid: purchase.amount_paid,
                source_chain_id: purchase.source_chain_id,
            })
            .collect()
    }

    /// Get user tickets for a round
    async fn user_tickets(&self, round_id: u64, owner: AccountOwner) -> Option<TicketPurchaseInfo> {
        let purchase = self.state.get_user_tickets(round_id, owner.clone()).await.ok()??;
        Some(TicketPurchaseInfo {
            owner,
            first_ticket: purchase.first_ticket,
            last_ticket: purchase.last_ticket,
            total_tickets: purchase.total_tickets,
            amount_paid: purchase.amount_paid,
            source_chain_id: purchase.source_chain_id,
        })
    }

    /// Get winners for a round
    async fn round_winners(&self, round_id: u64) -> Vec<LotteryWinnerInfo> {
        self.state.get_round_winners(round_id).await
            .unwrap_or_default()
            .into_iter()
            .map(|(ticket_number, owner, prize_amount, claimed, source_chain_id)| LotteryWinnerInfo {
                ticket_number,
                owner,
                prize_amount,
                claimed,
                source_chain_id,
            })
            .collect()
    }
    
    /// Get the configured Native app ID
    async fn native_app_id(&self) -> String {
        let params = self.runtime.application_parameters();
        format!("{}", params.native_app_id)
    }
    
    /// Get version
    async fn version(&self) -> String {
        "1.0.0".to_string()
    }
}

struct MutationRoot {
    runtime: Arc<ServiceRuntime<LotteryRoundsService>>,
}

#[Object]
impl MutationRoot {
    /// Create a new lottery round with the specified ticket price
    async fn create_round(&self, ticket_price: String) -> String {
        self.runtime.schedule_operation(&LotteryRoundsOperation::CreateRound {
            ticket_price: ticket_price.parse::<Amount>().unwrap_or_default(),
        });
        "CreateRound operation scheduled".to_string()
    }
    
    /// Close the active round (stops accepting purchases, prepares for drawing)
    async fn close_round(&self) -> String {
        self.runtime.schedule_operation(&LotteryRoundsOperation::CloseRound);
        "CloseRound operation scheduled".to_string()
    }
    
    /// Generate one winner for a closed round using VRF
    async fn generate_winner(&self, round_id: u64) -> String {
        self.runtime.schedule_operation(&LotteryRoundsOperation::GenerateWinner {
            round_id,
        });
        format!("GenerateWinner operation scheduled for round {}", round_id)
    }
    
    /// Set the Lottery App ID for cross-app calls
    async fn set_lottery_app_id(&self, lottery_app_id: String) -> String {
        self.runtime.schedule_operation(&LotteryRoundsOperation::SetLotteryAppId {
            lottery_app_id: lottery_app_id.clone(),
        });
        format!("SetLotteryAppId operation scheduled: {}", lottery_app_id)
    }
}
