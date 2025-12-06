#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use linera_sdk::{
    linera_base_types::{ApplicationId, WithContractAbi},
    views::{RootView, View},
    Contract, ContractRuntime,
};
use lottery_rounds::{LotteryRoundsAbi, LotteryRoundsParameters, Operation, LotteryResponse, Message};
use self::state::LotteryRoundsState;

// Conversion helpers would go here if needed, but we are using shared types mostly.
// For simplicity, we assume strict type matching or re-export.
// Actually, types in lib.rs are re-used in state.rs?
// state.rs imports them from crate::... so yes.

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
    type Parameters = LotteryRoundsParameters;
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
       // Initialize app id as None
       self.state.lottery_app_id.set(None);
    }

    async fn execute_operation(&mut self, operation: Self::Operation) -> Self::Response {
        match operation {
            Operation::CreateRound { ticket_price } => {
                let timestamp = self.runtime.system_time().micros();
                match self.state.create_lottery_round(ticket_price, timestamp).await {
                    Ok(round_id) => LotteryResponse::RoundId(round_id),
                    Err(e) => panic!("Failed to create round: {}", e),
                }
            }
            
            Operation::RecordTicketPurchase { owner, amount, source_chain_id } => {
                // Ensure caller is the authorized Lottery App
                let caller = self.runtime.authenticated_caller_id();
                let authorized_app_id = self.state.lottery_app_id.get().expect("Lottery App ID not set");
                if caller != Some(authorized_app_id) {
                     panic!("Unauthorized: RecordTicketPurchase can only be called by Lottery App");
                }
                
                match self.state.record_ticket_purchase(owner, amount, source_chain_id).await {
                    Ok(purchase) => LotteryResponse::TicketPurchase(purchase),
                    Err(e) => panic!("Failed to record purchase: {}", e),
                }
            }
            
            Operation::CloseRound => {
                let timestamp = self.runtime.system_time().micros();
                match self.state.close_lottery_round(timestamp).await {
                    Ok(round_id) => LotteryResponse::RoundId(round_id),
                    Err(e) => panic!("Failed to close round: {}", e),
                }
            }
            
            Operation::GenerateWinner { round_id } => {
                // Generate VRF
                let timestamp = self.runtime.system_time().micros();
                let block_height = self.runtime.block_height();
                let vrf_value = timestamp.wrapping_add(block_height.into()); // Simple pseudo-random for demo
                
                match self.state.generate_winner(vrf_value, round_id, timestamp).await {
                    Ok((round_id, ticket_number, owner, prize_amount, new_round_created)) => {
                        // We found a winner. Now tell Lottery App to pay them.
                        // We need to know the Lottery App ID (which holds the funds/escrow).
                        // Note: In `microbetreal`, `rounds` tells `microbetreal` to pay.
                        // Here `lottery-rounds` tells `lottery-app` to pay.
                        
                        let _lottery_app_id = self.state.lottery_app_id.get().expect("Lottery App ID not set");
                        
                        // We assume Lottery App has an operation `SendReward`
                        // We need to know LotteryAppAbi to call it. 
                        // Since we are refactoring, we haven't defined LotteryAppAbi yet. 
                        // But we know we will define it.
                        // Let's assume generic call for now or use placeholder.
                        
                        // Wait, we can't compile if we reference non-existent crate/abi.
                        // We should define `lottery-app` ABI somewhere or use blind call? 
                        // Linera requires ABI for type safety usually.
                        // Strategy: We will define `lottery-app` later. 
                        // For now, we can omit the cross-call implementation and mark TODO, 
                        // OR we can define a minimal ABI structure here?
                        // Better: We should check if `leaderboard` needs update too.
                        
                        // Let's implement the response first.
                        LotteryResponse::WinnerGenerated {
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
            
            Operation::SetLotteryAppId { lottery_app_id } => {
                 match lottery_app_id.parse::<ApplicationId>() {
                    Ok(app_id) => {
                        self.state.lottery_app_id.set(Some(app_id));
                    }
                    Err(e) => panic!("Failed to parse ApplicationId: {:?}", e),
                }
                LotteryResponse::Ok
            }
        }
    }

    async fn execute_message(&mut self, _message: Self::Message) {
        // Handle cross-chain messages if any (e.g. Notify)
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}
