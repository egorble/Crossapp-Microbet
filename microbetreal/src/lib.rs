// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/*! Microbetreal - Betting Wrapper Application */

use async_graphql::{Request, Response};
use linera_sdk::linera_base_types::{AccountOwner, Amount, ContractAbi, ServiceAbi};
use serde::{Deserialize, Serialize};

// Re-export from native-fungible-abi
pub use native_fungible_abi::{Prediction, ExtendedOperation, ExtendedResponse, ExtendedNativeFungibleTokenAbi};

#[derive(Debug, Deserialize, Serialize)]
pub enum Message {
    // Cross-chain transfer with prediction
    TransferWithPrediction {
        owner: AccountOwner,
        amount: Amount,
        prediction: Prediction,
        source_chain_id: String, // Chain ID of the sender
        source_owner: AccountOwner,
    },
}

// Microbetreal implements the same ABI as NativeFungible (ExtendedNativeFungibleTokenAbi)
// This allows Rounds to call operations on Microbetreal using the shared ABI
// Microbetreal handles: TransferWithPrediction, SendReward, SetNativeAppId, SetRoundsAppId
// Other operations are passed through to Native app

pub struct MicrobetAbi;

impl ContractAbi for MicrobetAbi {
    type Operation = ExtendedOperation;
    type Response = ExtendedResponse;
}

impl ServiceAbi for MicrobetAbi {
    type Query = Request;
    type QueryResponse = Response;
}