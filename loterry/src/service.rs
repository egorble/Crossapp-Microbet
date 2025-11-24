// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use std::sync::Arc;

use async_graphql::{EmptySubscription, Object, Request, Response, Schema};
use linera_sdk::{
    linera_base_types::{AccountOwner, WithServiceAbi},
    views::View,
    Service, ServiceRuntime,
};
use native_fungible::{
    AccountEntry, TICKER_SYMBOL, ExtendedNativeFungibleTokenAbi, ExtendedOperation, AccountInput,
    LotteryRound as LibLotteryRound, RoundStatus as LibRoundStatus, WinnerPool as LibWinnerPool,
    TicketPurchaseInfo as LibTicketPurchaseInfo, LotteryWinnerInfo as LibLotteryWinnerInfo,
};
use self::state::{NativeFungibleTokenState, LotteryRound, RoundStatus, WinnerPool};

linera_sdk::service!(NativeFungibleTokenService);

pub struct NativeFungibleTokenService {
    state: NativeFungibleTokenState,
    runtime: Arc<ServiceRuntime<Self>>,
}

impl WithServiceAbi for NativeFungibleTokenService {
    type Abi = ExtendedNativeFungibleTokenAbi;
}

impl Service for NativeFungibleTokenService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = NativeFungibleTokenState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        NativeFungibleTokenService {
            state,
            runtime: Arc::new(runtime),
        }
    }

    async fn handle_query(&self, request: Request) -> Response {
        // Collect all rounds from the loaded state
        let mut all_rounds = Vec::new();
        match self.state.rounds.indices().await {
            Ok(round_ids) => {
                for round_id in round_ids {
                    if let Ok(Some(round)) = self.state.rounds.get(&round_id).await {
                        all_rounds.push(round);
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to get round indices: {:?}", e);
            }
        }
        
        let schema = Schema::build(
            QueryRoot {
                all_rounds,
                runtime: self.runtime.clone(),
                storage_context: self.runtime.root_view_storage_context(),
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

struct Accounts {
    runtime: Arc<ServiceRuntime<NativeFungibleTokenService>>,
}

#[Object]
impl Accounts {
    async fn entry(&self, key: AccountOwner) -> AccountEntry {
        let value = self.runtime.owner_balance(key);
        AccountEntry { key, value }
    }

    async fn entries(&self) -> Vec<AccountEntry> {
        self.runtime
            .owner_balances()
            .into_iter()
            .map(|(owner, amount)| AccountEntry {
                key: owner,
                value: amount,
            })
            .collect()
    }

    async fn keys(&self) -> Vec<AccountOwner> {
        self.runtime.balance_owners()
    }
    
    async fn chain_balance(&self) -> String {
        let balance = self.runtime.chain_balance();
        balance.to_string()
    }
}

// Query root for GraphQL queries
struct QueryRoot {
    all_rounds: Vec<LotteryRound>,
    runtime: Arc<ServiceRuntime<NativeFungibleTokenService>>,
    storage_context: linera_sdk::views::ViewStorageContext,
}

// Helper functions to convert types
fn convert_status(status: RoundStatus) -> LibRoundStatus {
    match status {
        RoundStatus::Active => LibRoundStatus::Active,
        RoundStatus::Closed => LibRoundStatus::Closed,
        RoundStatus::Complete => LibRoundStatus::Complete,
    }
}

fn convert_winner_pool(pool: WinnerPool) -> LibWinnerPool {
    match pool {
        WinnerPool::Pool1 => LibWinnerPool::Pool1,
        WinnerPool::Pool2 => LibWinnerPool::Pool2,
        WinnerPool::Pool3 => LibWinnerPool::Pool3,
        WinnerPool::Pool4 => LibWinnerPool::Pool4,
        WinnerPool::Complete => LibWinnerPool::Complete,
    }
}

fn convert_round(round: &LotteryRound) -> LibLotteryRound {
    LibLotteryRound {
        id: round.id,
        created_at: round.created_at,
        closed_at: round.closed_at,
        status: convert_status(round.status),
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
    async fn ticker_symbol(&self) -> Result<String, async_graphql::Error> {
        Ok(String::from(TICKER_SYMBOL))
    }

    async fn accounts(&self) -> Result<Accounts, async_graphql::Error> {
        Ok(Accounts {
            runtime: self.runtime.clone(),
        })
    }
    
    async fn chain_balance(&self) -> Result<String, async_graphql::Error> {
        let balance = self.runtime.chain_balance();
        Ok(balance.to_string())
    }
    
    // Lottery queries
    async fn active_round(&self) -> Option<LibLotteryRound> {
        match NativeFungibleTokenState::load(self.storage_context.clone()).await {
            Ok(state) => {
                match state.get_active_round().await {
                    Ok(Some(round_id)) => {
                        match state.get_round(round_id).await {
                            Ok(Some(round)) => Some(convert_round(&round)),
                            _ => None,
                        }
                    },
                    _ => None,
                }
            },
            Err(_) => None,
        }
    }
    
    async fn round(&self, id: u64) -> Option<LibLotteryRound> {
        self.all_rounds.iter()
            .find(|round| round.id == id)
            .map(convert_round)
    }
    
    async fn all_rounds(&self) -> Vec<LibLotteryRound> {
        self.all_rounds.iter().map(convert_round).collect()
    }
    
    async fn round_ticket_purchases(&self, round_id: u64) -> Vec<LibTicketPurchaseInfo> {
        match NativeFungibleTokenState::load(self.storage_context.clone()).await {
            Ok(state) => {
                match state.get_round_ticket_purchases(round_id).await {
                    Ok(purchases) => {
                        purchases.into_iter().map(|(owner, purchase)| {
                            LibTicketPurchaseInfo {
                                chain_id: self.runtime.chain_id(),
                                owner,
                                first_ticket: purchase.first_ticket,
                                last_ticket: purchase.last_ticket,
                                total_tickets: purchase.total_tickets,
                                amount_paid: purchase.amount_paid,
                            }
                        }).collect()
                    },
                    Err(_) => Vec::new(),
                }
            },
            Err(_) => Vec::new(),
        }
    }
    
    async fn round_winners(&self, round_id: u64) -> Vec<LibLotteryWinnerInfo> {
        match NativeFungibleTokenState::load(self.storage_context.clone()).await {
            Ok(state) => {
                match state.get_round_winners(round_id).await {
                    Ok(winners) => {
                        winners.into_iter().map(|(ticket_number, owner, prize, claimed)| {
                            // Try to get source chain ID from ticket purchase
                            let source_chain_id = futures::executor::block_on(async {
                                state.ticket_purchases.get(&(round_id, owner.clone())).await
                                    .ok()
                                    .flatten()
                                    .and_then(|purchase| purchase.source_chain_id)
                            });
                            
                            LibLotteryWinnerInfo {
                                chain_id: self.runtime.chain_id(),
                                ticket_number,
                                owner,
                                prize_amount: prize,
                                claimed,
                                source_chain_id,
                            }
                        }).collect()
                    },
                    Err(_) => Vec::new(),
                }
            },
            Err(_) => Vec::new(),
        }
    }
}

struct MutationRoot {
    runtime: Arc<ServiceRuntime<NativeFungibleTokenService>>,
}

#[Object]
impl MutationRoot {
    async fn balance(&self, owner: AccountOwner) -> String {
        self.runtime.schedule_operation(&ExtendedOperation::Balance { owner });
        "Balance operation scheduled".to_string()
    }

    async fn chain_balance(&self) -> String {
        self.runtime.schedule_operation(&ExtendedOperation::ChainBalance);
        "ChainBalance operation scheduled".to_string()
    }

    async fn ticker_symbol(&self) -> String {
        self.runtime.schedule_operation(&ExtendedOperation::TickerSymbol);
        "TickerSymbol operation scheduled".to_string()
    }

    async fn transfer(
        &self,
        owner: AccountOwner,
        amount: String,
        target_account: AccountInput,
        purchase_tickets: bool,
    ) -> String {
        use linera_sdk::linera_base_types::Amount;
        let fungible_account = linera_sdk::abis::fungible::Account {
            chain_id: target_account.chain_id,
            owner: target_account.owner,
        };
        
        self.runtime.schedule_operation(&ExtendedOperation::Transfer {
            owner,
            amount: amount.parse::<Amount>().unwrap_or_default(),
            target_account: fungible_account,
            purchase_tickets,
        });
        "Transfer operation scheduled".to_string()
    }

    async fn claim(
        &self,
        source_account: AccountInput,
        amount: String,
        target_account: AccountInput,
        purchase_tickets: bool,
    ) -> String {
        use linera_sdk::linera_base_types::Amount;
        let fungible_source_account = linera_sdk::abis::fungible::Account {
            chain_id: source_account.chain_id,
            owner: source_account.owner,
        };
        let fungible_target_account = linera_sdk::abis::fungible::Account {
            chain_id: target_account.chain_id,
            owner: target_account.owner,
        };
        
        self.runtime.schedule_operation(&ExtendedOperation::Claim {
            source_account: fungible_source_account,
            amount: amount.parse::<Amount>().unwrap_or_default(),
            target_account: fungible_target_account,
            purchase_tickets,
        });
        "Claim operation scheduled".to_string()
    }

    async fn withdraw(&self) -> String {
        self.runtime.schedule_operation(&ExtendedOperation::Withdraw);
        "Withdraw operation scheduled successfully".to_string()
    }

    async fn mint(&self, owner: AccountOwner, amount: String) -> String {
        use linera_sdk::linera_base_types::Amount;
        self.runtime.schedule_operation(&ExtendedOperation::Mint {
            owner,
            amount: amount.parse::<Amount>().unwrap_or_default(),
        });
        "Mint operation scheduled successfully".to_string()
    }
    
    // Lottery mutations
    async fn create_lottery_round(&self, ticket_price: String) -> String {
        use linera_sdk::linera_base_types::Amount;
        self.runtime.schedule_operation(&ExtendedOperation::CreateLotteryRound {
            ticket_price: ticket_price.parse::<Amount>().unwrap_or_default(),
        });
        "CreateLotteryRound operation scheduled".to_string()
    }
    
    async fn purchase_tickets(&self, amount: String) -> String {
        use linera_sdk::linera_base_types::Amount;
        self.runtime.schedule_operation(&ExtendedOperation::PurchaseTickets {
            amount: amount.parse::<Amount>().unwrap_or_default(),
        });
        "PurchaseTickets operation scheduled".to_string()
    }
    
    async fn close_lottery_round(&self) -> String {
        self.runtime.schedule_operation(&ExtendedOperation::CloseLotteryRound);
        "CloseLotteryRound operation scheduled".to_string()
    }
    
    async fn generate_winner(&self, round_id: u64) -> String {
        self.runtime.schedule_operation(&ExtendedOperation::GenerateWinner {
            round_id,
        });
        "GenerateWinner operation scheduled (VRF auto-generated)".to_string()
    }
}