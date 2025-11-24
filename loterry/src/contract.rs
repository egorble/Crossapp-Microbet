// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use linera_sdk::{
    abis::fungible::{
        Account as FungibleAccount, InitialState, Parameters,
    },
    linera_base_types::{Account, AccountOwner, Amount, ChainId, WithContractAbi},
    views::{RootView, View},
    Contract, ContractRuntime,
};
use native_fungible::{
    Message, TICKER_SYMBOL, ExtendedNativeFungibleTokenAbi, ExtendedOperation, ExtendedResponse,
    LotteryRound as LibLotteryRound, RoundStatus as LibRoundStatus, WinnerPool as LibWinnerPool,
    TicketPurchase as LibTicketPurchase, TicketPurchaseInfo as LibTicketPurchaseInfo,
    LotteryWinnerInfo as LibLotteryWinnerInfo,
};
use self::state::{NativeFungibleTokenState, LotteryRound, RoundStatus, WinnerPool, TicketPurchase};

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

pub struct NativeFungibleTokenContract {
    state: NativeFungibleTokenState,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(NativeFungibleTokenContract);

impl WithContractAbi for NativeFungibleTokenContract {
    type Abi = ExtendedNativeFungibleTokenAbi;
}

impl Contract for NativeFungibleTokenContract {
    type Message = Message;
    type Parameters = Parameters;
    type InstantiationArgument = InitialState;
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = NativeFungibleTokenState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        NativeFungibleTokenContract { state, runtime }
    }

    async fn instantiate(&mut self, state: Self::InstantiationArgument) {
        // Validate that the application parameters were configured correctly.
        assert!(
            self.runtime.application_parameters().ticker_symbol == "NAT",
            "Only NAT is accepted as ticker symbol"
        );
        for (owner, amount) in state.accounts {
            let account = Account {
                chain_id: self.runtime.chain_id(),
                owner,
            };
            self.runtime.transfer(AccountOwner::CHAIN, account, amount);
        }
    }

    async fn execute_operation(&mut self, operation: Self::Operation) -> Self::Response {
        match operation {
            ExtendedOperation::Balance { owner } => {
                let balance = self.runtime.owner_balance(owner);
                ExtendedResponse::Balance(balance)
            }

            ExtendedOperation::ChainBalance => {
                let balance = self.runtime.chain_balance();
                ExtendedResponse::ChainBalance(balance)
            }

            ExtendedOperation::TickerSymbol => {
                ExtendedResponse::TickerSymbol(String::from(TICKER_SYMBOL))
            }

            ExtendedOperation::Transfer {
                owner,
                amount,
                target_account,
                purchase_tickets,
            } => {
                self.runtime
                    .check_account_permission(owner)
                    .expect("Permission for Transfer operation");

                let fungible_target_account = target_account;
                let target_account = self.normalize_account(target_account);

                self.runtime.transfer(owner, target_account, amount);

                // If purchase_tickets flag is set
                if purchase_tickets {
                    if target_account.chain_id != self.runtime.chain_id() {
                        // Cross-chain transfer for tickets - send message
                        let message = Message::TransferForTickets {
                            owner: target_account.owner,
                            amount,
                            source_chain_id: self.runtime.chain_id(),
                            source_owner: owner,
                        };
                        self.runtime
                            .prepare_message(message)
                            .with_authentication()
                            .send_to(target_account.chain_id);
                    } else {
                        // Same chain transfer - purchase tickets directly
                        let target_balance = self.runtime.owner_balance(target_account.owner);
                        eprintln!("Purchasing tickets from transfer operation for owner {:?}, amount {:?}, balance {:?}", 
                            target_account.owner, amount, target_balance);
                        if let Err(e) = self.state.purchase_tickets(target_account.owner, amount, target_balance, None).await {
                            eprintln!("Failed to purchase tickets: {}", e);
                            // We don't panic here as the transfer was successful
                        }
                    }
                } else {
                    // No ticket purchase - just send notify message for cross-chain
                    self.transfer(fungible_target_account.chain_id);
                }

                ExtendedResponse::Ok
            }

            ExtendedOperation::Claim {
                source_account,
                amount,
                target_account,
                purchase_tickets,
            } => {
                self.runtime
                    .check_account_permission(source_account.owner)
                    .expect("Permission for Claim operation");

                let fungible_source_account = source_account;
                let fungible_target_account = target_account;

                let source_account = self.normalize_account(source_account);
                let target_account = self.normalize_account(target_account);

                self.runtime.claim(source_account, target_account, amount);
                
                // If purchase_tickets flag is set, purchase tickets
                if purchase_tickets {
                    let target_balance = self.runtime.owner_balance(target_account.owner);
                    eprintln!("Purchasing tickets from claim operation for owner {:?}, amount {:?}, balance {:?}", 
                        target_account.owner, amount, target_balance);
                    if let Err(e) = self.state.purchase_tickets(target_account.owner, amount, target_balance, None).await {
                        eprintln!("Failed to purchase tickets: {}", e);
                        // We don't panic here as the claim was successful
                    }
                }

                self.claim(
                    fungible_source_account.chain_id,
                    fungible_target_account.chain_id,
                );
                ExtendedResponse::Ok
            }

            ExtendedOperation::Withdraw => {
                // Get the current owner (authenticated signer)
                let owner = self.runtime.authenticated_signer().unwrap();
                // Get the balance for this owner
                let balance = self.runtime.owner_balance(owner);
                // Create target account (chain account)
                let target_account = Account {
                    chain_id: self.runtime.chain_id(),
                    owner: AccountOwner::CHAIN,
                };
                // Transfer all tokens to the chain account
                self.runtime.transfer(owner, target_account, balance);
                ExtendedResponse::Ok
            }

            ExtendedOperation::Mint { owner, amount } => {
                // Create target account for the owner
                let target_account = Account {
                    chain_id: self.runtime.chain_id(),
                    owner,
                };
                // Mint tokens by transferring from chain account to the target account
                self.runtime.transfer(AccountOwner::CHAIN, target_account, amount);
                ExtendedResponse::Ok
            }
            
            // Lottery operations
            ExtendedOperation::CreateLotteryRound { ticket_price } => {
                let timestamp = self.runtime.system_time().micros();
                match self.state.create_lottery_round(ticket_price, timestamp).await {
                    Ok(round_id) => ExtendedResponse::RoundId(round_id),
                    Err(e) => panic!("Failed to create lottery round: {}", e),
                }
            }
            
            ExtendedOperation::PurchaseTickets { amount } => {
                let owner = self.runtime.authenticated_signer().expect("Authentication required");
                let current_balance = self.runtime.owner_balance(owner);
                match self.state.purchase_tickets(owner, amount, current_balance, None).await {
                    Ok(purchase) => ExtendedResponse::TicketPurchase(ticket_purchase_to_lib(purchase)),
                    Err(e) => panic!("Failed to purchase tickets: {}", e),
                }
            }
            
            ExtendedOperation::CloseLotteryRound => {
                let timestamp = self.runtime.system_time().micros();
                match self.state.close_lottery_round(timestamp).await {
                    Ok(round_id) => ExtendedResponse::RoundId(round_id),
                    Err(e) => panic!("Failed to close lottery round: {}", e),
                }
            }
            
            ExtendedOperation::GenerateWinner { round_id } => {
                // Generate VRF value automatically from timestamp + block height
                // This is more secure than manual input as it cannot be manipulated
                let timestamp = self.runtime.system_time().micros();
                let block_height = self.runtime.block_height();
                let vrf_value = timestamp.wrapping_add(block_height.into());
                
                eprintln!("GenerateWinner: round_id={}, vrf_value={} (timestamp={}, block={})", 
                    round_id, vrf_value, timestamp, block_height);
                
                // Get the authenticated signer (who pays the winnings)
                let payer = self.runtime.authenticated_signer().expect("Authentication required");
                
                // Generate one winner using VRF and automatically distribute prize
                match self.state.generate_winner(vrf_value, round_id, timestamp).await {
                    Ok((round_id, ticket_number, owner, prize_amount, new_round_created)) => {
                        // Get source chain ID for cross-chain prize distribution
                        let source_chain_id = self.state.ticket_purchases.get(&(round_id, owner.clone())).await
                            .map_err(|e| format!("Failed to get ticket purchase: {:?}", e))
                            .ok()
                            .flatten()
                            .and_then(|purchase| purchase.source_chain_id.clone());
                        
                        // Distribute prize immediately
                        if prize_amount > Amount::ZERO {
                            if let Some(source_chain_id_str) = source_chain_id.clone() {
                                // Cross-chain winner - send prize to source chain
                                match source_chain_id_str.parse::<ChainId>() {
                                    Ok(source_chain_id) => {
                                        let target_account = Account {
                                            chain_id: source_chain_id,
                                            owner: owner.clone(),
                                        };
                                        
                                        // Transfer prize from payer to winner
                                        self.runtime.transfer(payer, target_account, prize_amount);
                                        
                                        // Send notify message
                                        let message = Message::Notify;
                                        self.runtime
                                            .prepare_message(message)
                                            .with_authentication()
                                            .send_to(source_chain_id);
                                    }
                                    Err(_) => {
                                        // If can't parse, send to local owner
                                        let target_account = Account {
                                            chain_id: self.runtime.chain_id(),
                                            owner: owner.clone(),
                                        };
                                        self.runtime.transfer(payer, target_account, prize_amount);
                                    }
                                }
                            } else {
                                // Local winner - send prize directly
                                let target_account = Account {
                                    chain_id: self.runtime.chain_id(),
                                    owner: owner.clone(),
                                };
                                self.runtime.transfer(payer, target_account, prize_amount);
                            }
                        }
                        
                        ExtendedResponse::WinnerGenerated {
                            round_id,
                            ticket_number,
                            owner,
                            prize_amount,
                            new_round_created,
                        }
                    }
                    Err(e) => panic!("Failed to generate winner: {}", e),
                }
            }

            // Query operations for lottery state
            ExtendedOperation::GetActiveRound => {
                match self.state.get_active_round().await {
                    Ok(Some(round_id)) => {
                        match self.state.get_round(round_id).await {
                            Ok(Some(round)) => ExtendedResponse::LotteryRound(lottery_round_option_to_lib(Some(round))),
                            Ok(None) => ExtendedResponse::LotteryRound(None),
                            Err(e) => panic!("Failed to get round: {}", e),
                        }
                    },
                    Ok(None) => ExtendedResponse::LotteryRound(None),
                    Err(e) => panic!("Failed to get active round: {}", e),
                }
            }
            
            ExtendedOperation::GetRound { id } => {
                match self.state.get_round(id).await {
                    Ok(Some(round)) => ExtendedResponse::LotteryRound(lottery_round_option_to_lib(Some(round))),
                    Ok(None) => ExtendedResponse::LotteryRound(None),
                    Err(e) => panic!("Failed to get round: {}", e),
                }
            }
            
            ExtendedOperation::GetAllRounds => {
                match self.state.get_all_rounds().await {
                    Ok(rounds) => ExtendedResponse::LotteryRounds(lottery_rounds_to_lib(rounds)),
                    Err(e) => panic!("Failed to get all rounds: {}", e),
                }
            }
            
            ExtendedOperation::GetRoundTicketPurchases { round_id } => {
                match self.state.get_round_ticket_purchases(round_id).await {
                    Ok(purchases) => {
                        let purchase_info: Vec<_> = purchases.into_iter().map(|(owner, purchase)| {
                            LibTicketPurchaseInfo {
                                chain_id: self.runtime.chain_id(),
                                owner,
                                first_ticket: purchase.first_ticket,
                                last_ticket: purchase.last_ticket,
                                total_tickets: purchase.total_tickets,
                                amount_paid: purchase.amount_paid,
                            }
                        }).collect();
                        ExtendedResponse::TicketPurchases(purchase_info)
                    },
                    Err(e) => panic!("Failed to get round ticket purchases: {}", e),
                }
            }
            
            ExtendedOperation::GetUserTickets { round_id, owner } => {
                match self.state.get_user_tickets(round_id, owner).await {
                    Ok(Some(purchase)) => ExtendedResponse::TicketPurchase(ticket_purchase_to_lib(purchase)),
                    Ok(None) => panic!("No tickets found for user"),
                    Err(e) => panic!("Failed to get user tickets: {}", e),
                }
            }
            
            ExtendedOperation::GetRoundWinners { round_id } => {
                match self.state.get_round_winners(round_id).await {
                    Ok(winners) => {
                        let winner_info: Vec<_> = winners.into_iter().map(|(ticket_number, owner, prize, claimed)| {
                            // Get source chain ID from ticket purchase
                            let source_chain_id = futures::executor::block_on(async {
                                self.state.ticket_purchases.get(&(round_id, owner.clone())).await
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
                        }).collect();
                        ExtendedResponse::LotteryWinners(winner_info)
                    },
                    Err(e) => panic!("Failed to get round winners: {}", e),
                }
            }
        }
    }

    async fn execute_message(&mut self, message: Self::Message) {
        match message {
            Message::Notify => {
                // Auto-deploy the application on this chain if it's not already deployed
                // This ensures the application is available for cross-chain operations
                // Note: Application is now available on chain
            }
            Message::TransferForTickets { owner: _, amount, source_chain_id, source_owner } => {
                // Handle cross-chain transfer for ticket purchase
                // Immediately purchase tickets for the source owner since they initiated the transfer
                eprintln!("Purchasing cross-chain tickets for source owner {:?} from chain {:?}, amount {:?}", 
                    source_owner, source_chain_id, amount);
                
                // Purchase tickets immediately
                // The tokens arrive with the transfer, so we use amount as balance
                let source_chain_id_str = source_chain_id.to_string();
                if let Err(e) = self.state.purchase_tickets(source_owner, amount, amount, Some(source_chain_id_str)).await {
                    eprintln!("Failed to purchase cross-chain tickets: {}", e);
                } else {
                    eprintln!("Successfully purchased cross-chain tickets for source owner {:?}", source_owner);
                }
            }
        }
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}

impl NativeFungibleTokenContract {

    
    fn transfer(&mut self, chain_id: ChainId) {
        if chain_id != self.runtime.chain_id() {
            let message = Message::Notify;
            self.runtime
                .prepare_message(message)
                .with_authentication()
                .send_to(chain_id);
        }
    }

    fn claim(&mut self, source_chain_id: ChainId, target_chain_id: ChainId) {
        if source_chain_id == self.runtime.chain_id() {
            self.transfer(target_chain_id);
        } else {
            // If different chain, send notify message so the app gets auto-deployed
            let message = Message::Notify;
            self.runtime
                .prepare_message(message)
                .with_authentication()
                .send_to(source_chain_id);
        }
    }

    fn normalize_account(&self, account: FungibleAccount) -> Account {
        Account {
            chain_id: account.chain_id,
            owner: account.owner,
        }
    }
}