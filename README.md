# Microbet Architecture & Deployment

This project consists of four interacting Linera applications:

1.  **Native**: A pure fungible token application (used for betting).
2.  **Leaderboard**: Tracks player statistics (wins, losses, total amounts).
3.  **Rounds**: Manages prediction game rounds, bets (Up/Down), and resolution.
4.  **Microbetreal**: A generic entry point / wrapper that coordinates between Native and Rounds.

## Features

- **Dual-Side Betting**: Users can bet on both UP and DOWN in the same round. Bets on the winning side are paid out, while bets on the losing side are considered lost.
- **Leaderboard**: Automatically tracks user performance across all games.
- **Cross-Chain**: Supports betting from different chains.

## Deployment

The project includes an automated deployment script `deploy.sh` that handles the order of deployment and linking.

### Quick Start

```bash
./deploy.sh
```

This script will:
1. Deploy `native` app.
2. Deploy `leaderboard` app.
3. Deploy `rounds` app (linked to native and leaderboard).
4. Deploy `microbetreal` app (linked to native and rounds).
5. Perform the final handshake to link `rounds` back to `microbetreal`.
6. Output all Application IDs to `app_ids.txt`.

### Manual Deployment Steps (Reference)

If you need to deploy manually, here is the order:

1.  **Deploy Native**
    *   Save `NATIVE_ID`.
2.  **Deploy Leaderboard**
    *   Save `LEADERBOARD_ID`.
3.  **Deploy Rounds**
    *   Parameters: `{"native_app_id": "...", "leaderboard_app_id": "..."}`
    *   Save `ROUNDS_ID`.
4.  **Deploy Microbetreal**
    *   Parameters: `{"native_app_id": "...", "rounds_app_id": "..."}`
    *   Required IDs: `native_app_id`, `rounds_app_id`
    *   Save `MICROBETREAL_ID`.
5.  **Link Rounds**
    *   Call `set_microbet_app_id` mutation on `rounds` with `MICROBETREAL_ID`.

## How Cross-Application Calls Work

### 1. Placing a Bet
**Flow:** `User` -> `Microbetreal` -> `Native` & `Rounds`

1.  **User** calls `transferWithPrediction` on **Microbetreal**.
2.  **Microbetreal** calls **Native** to transfer tokens to escrow.
3.  If successful, **Microbetreal** calls **Rounds** (`PlaceBet`) to record the prediction.
    *   Supports multiple bets per round (can bet UP and DOWN).

### 2. Resolving a Round
**Flow:** `Admin` -> `Rounds` -> `Microbetreal` & `Leaderboard`

1.  **Admin** calls `ResolveRound`.
2.  **Rounds** determines the winner.
3.  **Rounds** distributes rewards via **Microbetreal** -> **Native**.
4.  **Rounds** updates **Leaderboard** with stats for all participants (winners and losers).
