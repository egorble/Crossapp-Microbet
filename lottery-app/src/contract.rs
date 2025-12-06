#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use linera_sdk::{
    linera_base_types::{AccountOwner, WithContractAbi},
    views::{RootView, View},
    Contract, ContractRuntime,
};
use linera_sdk::abis::fungible::Account;
use lottery_app::{LotteryAppAbi, LotteryAppParameters, Operation, LotteryAppResponse, Message};
use self::state::LotteryAppState;
use native_fungible_abi::{ExtendedNativeFungibleTokenAbi, ExtendedOperation};
use lottery_rounds::{LotteryRoundsAbi, Operation as RoundsOperation};

pub struct LotteryAppContract {
    state: LotteryAppState,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(LotteryAppContract);

impl WithContractAbi for LotteryAppContract {
    type Abi = LotteryAppAbi;
}

impl Contract for LotteryAppContract {
    type Message = Message;
    type Parameters = LotteryAppParameters;
    type InstantiationArgument = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = LotteryAppState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        LotteryAppContract { state, runtime }
    }

    async fn instantiate(&mut self, _arg: Self::InstantiationArgument) {
        // Validation of params
        let _ = self.runtime.application_parameters();
    }

    async fn execute_operation(&mut self, operation: Self::Operation) -> Self::Response {
        let params = self.runtime.application_parameters();
        let native_app_id = params.native_app_id;
        let lottery_rounds_app_id = params.lottery_rounds_app_id;

        match operation {
            Operation::BuyTickets { amount } => {
                let owner = self.runtime.authenticated_signer().expect("Authentication required");
                
                // 1. Transfer tokens from User to this App (Escrow)
                // We construct the separate Native App call
                let transfer_op = ExtendedOperation::Transfer {
                    owner: owner.into(),
                    amount,
                    target_account: Account {
                        chain_id: self.runtime.chain_id(),
                        owner: self.runtime.application_id().forget_abi().into(),
                    },
                    prediction: None,
                };
                
                // Call Native App
                let response = self.runtime.call_application(true, native_app_id.with_abi::<ExtendedNativeFungibleTokenAbi>(), &transfer_op);
                // In generic `call_application` we usually get bytes back, but the SDK helper might type it?
                // The new SDK `call_application` returns `Vec<u8>`. We shouldn't rely on return value if we trust it doesn't verify logic? 
                // Wait, if transfer fails (insufficient funds), Native should panic. If Native panics, this transaction fails atomically.
                
                // 2. Call Lottery Rounds to record purchase
                let record_op = RoundsOperation::RecordTicketPurchase {
                    owner: owner.into(),
                    amount,
                    source_chain_id: None, // TODO: Handle cross-chain ID
                };
                
                let result = self.runtime.call_application(true, lottery_rounds_app_id.with_abi::<LotteryRoundsAbi>(), &record_op);
                // We assume successful call means it's recorded.
                
                LotteryAppResponse::Ok
            }
            
            Operation::DistributePrize { winner, amount, source_chain_id } => {
                // Verify caller is Rounds App
                let caller = self.runtime.authenticated_caller_id();
                if caller != Some(lottery_rounds_app_id) {
                    panic!("Unauthorized: DistributePrize can only be called by Lottery Rounds App");
                }
                
                // Transfer prize from here (Escrow) to Winner
                // Using Native App Transfer
                let transfer_op = ExtendedOperation::Transfer {
                    owner: self.runtime.application_id().forget_abi().into(),
                    amount,
                    target_account: Account {
                        chain_id: self.runtime.chain_id(), // Or source_chain? Simplified for now.
                        owner: winner, // Already generic AccountOwner
                    },
                    prediction: None,
                };
                
                self.runtime.call_application(true, native_app_id.with_abi::<ExtendedNativeFungibleTokenAbi>(), &transfer_op);
                
                LotteryAppResponse::Ok
            }
        }
    }

    async fn execute_message(&mut self, _message: Self::Message) {
        // Handle incoming messages
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}
