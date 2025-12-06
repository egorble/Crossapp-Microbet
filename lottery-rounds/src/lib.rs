use async_graphql::{Request, Response, SimpleObject};
use linera_sdk::{
    linera_base_types::{AccountOwner, Amount, ContractAbi, ServiceAbi},
    graphql::GraphQLMutationRoot,
};
use serde::{Deserialize, Serialize};

pub struct LotteryRoundsAbi;

impl ContractAbi for LotteryRoundsAbi {
    type Operation = Operation;
    type Response = LotteryResponse;
}

impl ServiceAbi for LotteryRoundsAbi {
    type Query = Request;
    type QueryResponse = Response;
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LotteryRoundsParameters {
    pub native_app_id: linera_sdk::linera_base_types::ApplicationId,
}

#[derive(Debug, Deserialize, Serialize, GraphQLMutationRoot)]
pub enum Operation {
    /// Create a new lottery round manually (usually handled automatically)
    CreateRound { ticket_price: Amount },
    /// Record a ticket purchase (Internal call from Controller)
    RecordTicketPurchase {
        owner: AccountOwner,
        amount: Amount,
        source_chain_id: Option<String>,
    },
    /// Close the current round (Admin or Automation)
    CloseRound,
    /// Generate a winner for the closed round
    GenerateWinner { round_id: u64 },
    /// Set the Lottery Controller App ID (for linking)
    SetLotteryAppId { lottery_app_id: String },
}

#[derive(Debug, Deserialize, Serialize)]
pub enum LotteryResponse {
    Ok,
    RoundId(u64),
    TicketPurchase(TicketPurchase),
    WinnerGenerated {
        round_id: u64,
        ticket_number: u64,
        owner: AccountOwner,
        prize_amount: Amount,
        new_round_created: bool,
    },
    LotteryRound(Option<LotteryRound>),
    LotteryRounds(Vec<LotteryRound>),
    TicketPurchases(Vec<TicketPurchase>),
    LotteryWinners(Vec<LotteryWinnerInfo>),
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Message {
    /// Notify across chains
    Notify,
}

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct LotteryRound {
    pub id: u64,
    pub created_at: u64,
    pub closed_at: Option<u64>,
    pub status: RoundStatus,
    pub ticket_price: Amount,
    pub total_tickets_sold: u64,
    pub next_ticket_number: u64,
    pub prize_pool: Amount,
    pub current_winner_pool: WinnerPool,
    pub pool1_count: u64,
    pub pool2_count: u64,
    pub pool3_count: u64,
    pub pool4_count: u64,
    pub pool1_winners_drawn: u64,
    pub pool2_winners_drawn: u64,
    pub pool3_winners_drawn: u64,
    pub pool4_winners_drawn: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, async_graphql::Enum)]
pub enum RoundStatus {
    Active,
    Closed,
    Complete,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, async_graphql::Enum)]
pub enum WinnerPool {
    Pool1,
    Pool2,
    Pool3,
    Pool4,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct TicketPurchase {
    pub owner: AccountOwner,
    pub first_ticket: u64,
    pub last_ticket: u64,
    pub total_tickets: u64,
    pub amount_paid: Amount,
    pub source_chain_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct LotteryWinnerInfo {
    pub chain_id: linera_sdk::linera_base_types::ChainId,
    pub ticket_number: u64,
    pub owner: AccountOwner,
    pub prize_amount: Amount,
    pub claimed: bool,
    pub source_chain_id: Option<String>,
}
