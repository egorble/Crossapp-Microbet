// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/*! Lottery App - Ticket Purchase Wrapper Application - Re-exports from lottery-abi */

// Re-export everything from lottery-abi for this crate
pub use lottery_abi::{
    // Lottery App types
    LotteryAppAbi,
    LotteryAppOperation,
    LotteryAppResponse,
    LotteryAppParameters,
    LotteryAppMessage as Message,
    // Lottery Rounds types (for cross-app calls)
    LotteryRoundsAbi,
    LotteryRoundsOperation,
    LotteryRoundsResponse,
};
