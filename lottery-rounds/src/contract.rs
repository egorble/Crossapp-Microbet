// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use linera_sdk::{
    linera_base_types::{Amount, ApplicationId, WithContractAbi},
    views::{RootView, View},
    Contract, ContractRuntime,
};
use lottery_abi::{
    LotteryRoundsAbi, LotteryRoundsOperation, LotteryRoundsResponse, LotteryRoundsMessage as Message,
    LotteryRound as LibLotteryRound, RoundStatus as LibRoundStatus, WinnerPool as LibWinnerPool,
    TicketPurchase as LibTicketPurchase, TicketPurchaseInfo as LibTicketPurchaseInfo,
    LotteryWinnerInfo as LibLotteryWinnerInfo,
    LotteryAppAbi, LotteryAppOperation, LotteryAppResponse,
};
use self::state::{LotteryRoundsState, LotteryRound, RoundStatus, WinnerPool, TicketPurchase};


// Conversion functions between lib types and state types
fn round_status_to_lib(status: RoundStatus) -> LibRoundStatus {
    match status {
        RoundStatus::Active => LibRoundStatus::Active,
        RoundStatus::Closed => LibRoundStatus::Closed,
        RoundStatus::Complete => LibRoundStatus::Complete,
    }
}

fn winner_pool_to_lib(pool: WinnerPool) -> LibWinnerPool {
    match pool {
        WinnerPool::Pool1 => LibWinnerPool::Pool1,
        WinnerPool::Pool2 => LibWinnerPool::Pool2,
        WinnerPool::Pool3 => LibWinnerPool::Pool3,
        WinnerPool::Pool4 => LibWinnerPool::Pool4,
        WinnerPool::Complete => LibWinnerPool::Complete,
    }
}

fn lottery_round_to_lib(round: LotteryRound) -> LibLotteryRound {
    LibLotteryRound {
        id: round.id,
        created_at: round.created_at,
        closed_at: round.closed_at,
        status: round_status_to_lib(round.status),
        ticket_price: round.ticket_price,
        total_tickets_sold: round.total_tickets_sold,
        next_ticket_number: round.next_ticket_number,
        prize_pool: round.prize_pool,
        current_winner_pool: winner_pool_to_lib(round.current_winner_pool),
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

fn lottery_round_option_to_lib(round: Option<LotteryRound>) -> Option<LibLotteryRound> {
    round.map(lottery_round_to_lib)
}

fn lottery_rounds_to_lib(rounds: Vec<LotteryRound>) -> Vec<LibLotteryRound> {
    rounds.into_iter().map(lottery_round_to_lib).collect()
}

fn ticket_purchase_to_lib(purchase: TicketPurchase) -> LibTicketPurchase {
    LibTicketPurchase {
        owner: purchase.owner,
        first_ticket: purchase.first_ticket,
        last_ticket: purchase.last_ticket,
        total_tickets: purchase.total_tickets,
        amount_paid: purchase.amount_paid,
        source_chain_id: purchase.source_chain_id,
    }
}

pub struct LotteryRoundsContract {
    state: LotteryRoundsState,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(LotteryRoundsContract);

impl WithContractAbi for LotteryRoundsContract {
    type Abi = LotteryRoundsAbi;
}

impl Contract for LotteryRoundsContract {
    type Message = Message;
    type Parameters = lottery_abi::LotteryRoundsParameters;
    type InstantiationArgument = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = LotteryRoundsState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        LotteryRoundsContract { state, runtime }
    }

    async fn instantiate(&mut self, _arg: Self::InstantiationArgument) {
        // Validate params access
        let _ = self.runtime.application_parameters();
    }

    async fn execute_operation(&mut self, operation: Self::Operation) -> Self::Response {
        match operation {
            LotteryRoundsOperation::SetLotteryAppId { lottery_app_id } => {
                match lottery_app_id.parse::<ApplicationId>() {
                    Ok(app_id) => {
                        let typed_app_id: ApplicationId<LotteryAppAbi> = app_id.with_abi();
                        self.state.lottery_app_id.set(Some(typed_app_id));
                    }
                    Err(e) => panic!("Failed to parse LotteryApp ApplicationId: {:?}", e),
                }
                LotteryRoundsResponse::Ok
            }


            LotteryRoundsOperation::CreateRound { ticket_price } => {
                let timestamp = self.runtime.system_time().micros();
                match self.state.create_lottery_round(ticket_price, timestamp).await {
                    Ok(round_id) => LotteryRoundsResponse::RoundId(round_id),
                    Err(e) => panic!("Failed to create lottery round: {}", e),
                }
            }
            
            LotteryRoundsOperation::CloseRound => {
                let timestamp = self.runtime.system_time().micros();
                match self.state.close_lottery_round(timestamp).await {
                    Ok(round_id) => LotteryRoundsResponse::RoundId(round_id),
                    Err(e) => panic!("Failed to close lottery round: {}", e),
                }
            }
            
            LotteryRoundsOperation::GenerateWinner { round_id } => {
                // Generate VRF value automatically from timestamp + block height
                let timestamp = self.runtime.system_time().micros();
                let block_height = self.runtime.block_height();
                let vrf_value = timestamp.wrapping_add(block_height.into());
                
                eprintln!("GenerateWinner: round_id={}, vrf_value={} (timestamp={}, block={})", 
                    round_id, vrf_value, timestamp, block_height);
                
                // Get default ticket price for new rounds
                let default_ticket_price = self.state.get_current_ticket_price().await
                    .unwrap_or(Amount::from_tokens(1));
                
                // Generate one winner using VRF
                match self.state.generate_winner(vrf_value, round_id, timestamp, default_ticket_price).await {
                    Ok((round_id, ticket_number, owner, prize_amount, new_round_created, source_chain_id)) => {
                        // Get lottery app ID from state (set via SetLotteryAppId operation)
                        let lottery_app_id = self.state.lottery_app_id.get()
                            .expect("Lottery app ID not set - run SetLotteryAppId first");
                        
                        // Call lottery-app to send prize
                        if prize_amount > Amount::ZERO {
                            let _response: LotteryAppResponse = self.runtime.call_application(
                                true, // authenticated
                                lottery_app_id,
                                &LotteryAppOperation::SendPrize {
                                    recipient: owner.clone(),
                                    amount: prize_amount,
                                    source_chain_id: source_chain_id.clone(),
                                },
                            );
                            
                            // Mark prize as claimed
                            if let Err(e) = self.state.mark_prize_claimed(round_id, ticket_number).await {
                                eprintln!("Failed to mark prize as claimed: {}", e);
                            }
                        }

                        
                        LotteryRoundsResponse::WinnerGenerated {
                            round_id,
                            ticket_number,
                            owner,
                            prize_amount,
                            new_round_created,
                            source_chain_id,
                        }
                    }
                    Err(e) => panic!("Failed to generate winner: {}", e),
                }
            }

            LotteryRoundsOperation::PurchaseTickets { owner, amount, ticket_price, source_chain_id } => {
                match self.state.purchase_tickets(owner, amount, ticket_price, source_chain_id).await {
                    Ok(purchase) => LotteryRoundsResponse::TicketPurchase(ticket_purchase_to_lib(purchase)),
                    Err(e) => panic!("Failed to purchase tickets: {}", e),
                }
            }

            // Query operations
            LotteryRoundsOperation::GetActiveRound => {
                match self.state.get_active_round().await {
                    Ok(Some(round_id)) => {
                        match self.state.get_round(round_id).await {
                            Ok(Some(round)) => LotteryRoundsResponse::LotteryRound(lottery_round_option_to_lib(Some(round))),
                            Ok(None) => LotteryRoundsResponse::LotteryRound(None),
                            Err(e) => panic!("Failed to get round: {}", e),
                        }
                    },
                    Ok(None) => LotteryRoundsResponse::LotteryRound(None),
                    Err(e) => panic!("Failed to get active round: {}", e),
                }
            }
            
            LotteryRoundsOperation::GetRound { id } => {
                match self.state.get_round(id).await {
                    Ok(Some(round)) => LotteryRoundsResponse::LotteryRound(lottery_round_option_to_lib(Some(round))),
                    Ok(None) => LotteryRoundsResponse::LotteryRound(None),
                    Err(e) => panic!("Failed to get round: {}", e),
                }
            }
            
            LotteryRoundsOperation::GetAllRounds => {
                match self.state.get_all_rounds().await {
                    Ok(rounds) => LotteryRoundsResponse::LotteryRounds(lottery_rounds_to_lib(rounds)),
                    Err(e) => panic!("Failed to get all rounds: {}", e),
                }
            }
            
            LotteryRoundsOperation::GetRoundTicketPurchases { round_id } => {
                match self.state.get_round_ticket_purchases(round_id).await {
                    Ok(purchases) => {
                        let purchase_info: Vec<_> = purchases.into_iter().map(|(owner, purchase)| {
                            LibTicketPurchaseInfo {
                                owner,
                                first_ticket: purchase.first_ticket,
                                last_ticket: purchase.last_ticket,
                                total_tickets: purchase.total_tickets,
                                amount_paid: purchase.amount_paid,
                                source_chain_id: purchase.source_chain_id,
                            }
                        }).collect();
                        LotteryRoundsResponse::TicketPurchases(purchase_info)
                    },
                    Err(e) => panic!("Failed to get round ticket purchases: {}", e),
                }
            }
            
            LotteryRoundsOperation::GetUserTickets { round_id, owner } => {
                match self.state.get_user_tickets(round_id, owner).await {
                    Ok(Some(purchase)) => LotteryRoundsResponse::TicketPurchase(ticket_purchase_to_lib(purchase)),
                    Ok(None) => panic!("No tickets found for user"),
                    Err(e) => panic!("Failed to get user tickets: {}", e),
                }
            }
            
            LotteryRoundsOperation::GetRoundWinners { round_id } => {
                match self.state.get_round_winners(round_id).await {
                    Ok(winners) => {
                        let winner_info: Vec<_> = winners.into_iter().map(|(ticket_number, owner, prize, claimed, source_chain_id)| {
                            LibLotteryWinnerInfo {
                                ticket_number,
                                owner,
                                prize_amount: prize,
                                claimed,
                                source_chain_id,
                            }
                        }).collect();
                        LotteryRoundsResponse::LotteryWinners(winner_info)
                    },
                    Err(e) => panic!("Failed to get round winners: {}", e),
                }
            }
        }
    }

    async fn execute_message(&mut self, _message: Self::Message) {
        // No messages expected for LotteryRounds app currently
        // All communication happens via cross-app calls (operations)
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}
