use async_graphql::{Request, Response};
use linera_sdk::{
    linera_base_types::{ApplicationId, Amount, ContractAbi, ServiceAbi, AccountOwner},
    graphql::GraphQLMutationRoot,
};
use serde::{Deserialize, Serialize};

pub struct LotteryAppAbi;

impl ContractAbi for LotteryAppAbi {
    type Operation = Operation;
    type Response = LotteryAppResponse;
}

impl ServiceAbi for LotteryAppAbi {
    type Query = Request;
    type QueryResponse = Response;
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LotteryAppParameters {
    pub native_app_id: ApplicationId,
    pub lottery_rounds_app_id: ApplicationId,
}

#[derive(Debug, Deserialize, Serialize, GraphQLMutationRoot)]
pub enum Operation {
    /// Buy tickets. Transfers tokens to this app, then calls Rounds to record purchase.
    BuyTickets { amount: Amount },
    
    /// Receive reward distribution request from Rounds.
    /// Transfers tokens from this app (escrow) to the winner.
    DistributePrize { 
        winner: AccountOwner, 
        amount: Amount,
        source_chain_id: Option<String> 
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub enum LotteryAppResponse {
    Ok,
    TicketPurchased { round_id: u64, ticket_count: u64 },
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Message {
    /// Notify for cross-chain sync
    Notify,
}
