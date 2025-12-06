# Microbet Architecture & Deployment

This project consists of three interacting Linera applications:

1.  **Native**: A pure fungible token application (no betting logic).
2.  **Rounds**: Manages prediction game rounds, bets, and resolution.
3.  **Microbetreal**: A generic entry point / wrapper that coordinates between Native and Rounds.

## Deployment Instructions

### 1. Deploy Native App
The Native app requires initial accounts and a ticker symbol.

```bash
# Deploy Native
# --json-argument: {"accounts": { "UserOwner": "Amount" }}
# --json-parameters: {"ticker_symbol": "NAT"}

linera project publish-and-create \
    --json-argument '{ "accounts": { "0x32e98a772b3fddaadef05b319559067069dd6243fd649a3e77c2b9ed842f8a8b": "100" } }' \
    --json-parameters '{ "ticker_symbol": "NAT" }'
```
*Save the resulting Application ID as `NATIVE_APP_ID`.*

### 2. Deploy Rounds App
The Rounds app needs the Native App ID to know which token to use for betting. It accepts this via immutable parameters.

```bash
# Deploy Rounds
# --json-parameters: {"native_app_id": "..."}
# --json-argument: null (no state checking needed at init)

linera project publish-and-create \
    --json-parameters "{\"native_app_id\": \"$NATIVE_APP_ID\"}"
```
*Save the resulting Application ID as `ROUNDS_APP_ID`.*

### 3. Deploy Microbetreal App
The Microbetreal app acts as the user interface for betting. It needs both the Native ID (for transfers) and Rounds ID (for placing bets).

```bash
# Deploy Microbetreal
# --json-parameters: {"native_app_id": "...", "rounds_app_id": "..."}
# --json-argument: null

linera project publish-and-create \
    --json-parameters "{\"native_app_id\": \"$NATIVE_APP_ID\", \"rounds_app_id\": \"$ROUNDS_APP_ID\"}" \
    --required-application-ids $NATIVE_APP_ID $ROUNDS_APP_ID
```
*Save the resulting Application ID as `MICROBET_APP_ID`.*

### 4. Link Rounds to Microbetreal
To allow Rounds to distribute rewards back to users, it needs to know the Microbetreal App ID. Since Microbetreal depends on Rounds, we must link them *after* both are deployed.

```bash
SetMicrobetAppId(microbetAppId:"$MICROBET_APP_ID")
```

---

## How Cross-Application Calls Work

The architecture relies on Linera's **Cross-Application Calls** to separate concerns while maintaining atomic execution.

### 1. Placing a Bet
**Flow:** `User` -> `Microbetreal` -> `Native` & `Rounds`

1.  **User** calls `transferWithPrediction` on **Microbetreal**.
2.  **Microbetreal** first calls **Native** to transfer the bet amount from the User to the Microbetreal contract (escrow).
3.  If the transfer succeeds, **Microbetreal** then calls **Rounds** (`PlaceBet`) to record the prediction.
    *   If this is a cross-chain request, Microbetreal handles sending the message to the correct chain.
    *   This ensures that a bet is only recorded if the funds are successfully locked.

### 2. Resolving a Round & Distributing Rewards
**Flow:** `Admin/User` -> `Rounds` -> `Microbetreal` -> `Native`

1.  **Admin** calls `ResolveRound` on **Rounds** with the final price.
2.  **Rounds** calculates winners and their payouts.
3.  For each winner, **Rounds** calls **Microbetreal** (`SendReward`).
    *   Rounds does not hold the funds directly vs Native; it delegates the "payout" action to Microbetreal (which holds the escrowed funds or has authority).
4.  **Microbetreal**, upon receiving the `SendReward` call from the authenticated Rounds app, calls **Native** to transfer tokens from the escrow/treasury to the Winner.

This cycle (`User` -> `Funds Locked` -> `Bet Recorded` -> `Resolution` -> `Funds Released`) is secured by the fact that only the authorized Apps can call the specific sensitive operations on each other.
