// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/*! Lottery Rounds Application - Re-exports from lottery-abi */

// Re-export everything from lottery-abi for this crate
pub use lottery_abi::{
    // Lottery Rounds types
    LotteryRoundsAbi,
    LotteryRoundsOperation,
    LotteryRoundsResponse,
    LotteryRoundsParameters,
    LotteryRoundsMessage as Message,
    RoundStatus,
    WinnerPool,
    LotteryRound,
    TicketPurchase,
    TicketPurchaseInfo,
    LotteryWinnerInfo,
    // Lottery App types (for cross-app calls)
    LotteryAppAbi,
    LotteryAppOperation,
    LotteryAppResponse,
};
