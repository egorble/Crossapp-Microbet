#!/bin/bash

# Exit on error
set -e

# Deploy native
echo "Deploying native..."
# Uses 'linera project publish-and-create' which automatically handles compilation
# Points to 'native' directory containing Cargo.toml
NATIVE_ID=$(linera project publish-and-create native \
    --json-argument "{\"accounts\":{}}" \
    --json-parameters "{\"ticker_symbol\":\"NAT\"}")

echo "Native deployed with ID: $NATIVE_ID"
echo "NATIVE=$NATIVE_ID" > app_ids.txt

# Deploy leaderboard
echo "Deploying leaderboard..."
# Leaderboard takes no init args/params (empty structs)
LEADERBOARD_ID=$(linera project publish-and-create leaderboard)

echo "Leaderboard deployed with ID: $LEADERBOARD_ID"
echo "Leaderboard=$LEADERBOARD_ID" >> app_ids.txt

# Deploy rounds
echo "Deploying rounds..."
# Points to 'rounds' directory
# Parameters: native_app_id, leaderboard_app_id
ROUNDS_ID=$(linera project publish-and-create rounds \
    --json-parameters "{\"native_app_id\":\"$NATIVE_ID\", \"leaderboard_app_id\":\"$LEADERBOARD_ID\"}")

echo "Rounds deployed with ID: $ROUNDS_ID"
echo "ROUNDS=$ROUNDS_ID" >> app_ids.txt

# Deploy microbetreal
echo "Deploying microbetreal..."
# Points to 'microbetreal' directory
MICROBETREAL_ID=$(linera project publish-and-create microbetreal \
    --json-parameters "{\"native_app_id\":\"$NATIVE_ID\", \"rounds_app_id\":\"$ROUNDS_ID\"}" \
    --required-application-ids "$NATIVE_ID" "$ROUNDS_ID")

echo "Microbetreal deployed with ID: $MICROBETREAL_ID"
echo "MICROBETREAL=$MICROBETREAL_ID" >> app_ids.txt

# Link microbetreal to rounds
echo "Linking rounds to microbetreal..."

# Construct the operation JSON
OPERATION_JSON="{\"SetMicrobetAppId\":{\"microbet_app_id\":\"$MICROBETREAL_ID\"}}"

echo "IMPORTANT: Execute the following operation on Rounds app ($ROUNDS_ID) to finish linking:"
echo "$OPERATION_JSON"

# Deploy lottery-rounds
echo "Deploying lottery-rounds..."
LOTTERY_ROUNDS_ID=$(linera project publish-and-create lottery-rounds \
    --json-parameters "{\"native_app_id\":\"$NATIVE_ID\"}")

echo "Lottery Rounds deployed with ID: $LOTTERY_ROUNDS_ID"
echo "LOTTERY_ROUNDS=$LOTTERY_ROUNDS_ID" >> app_ids.txt

# Deploy lottery-app
echo "Deploying lottery-app..."
LOTTERY_APP_ID=$(linera project publish-and-create lottery-app \
    --json-parameters "{\"native_app_id\":\"$NATIVE_ID\", \"lottery_rounds_app_id\":\"$LOTTERY_ROUNDS_ID\"}" \
    --required-application-ids "$NATIVE_ID" "$LOTTERY_ROUNDS_ID")

echo "Lottery App deployed with ID: $LOTTERY_APP_ID"
echo "LOTTERY_APP=$LOTTERY_APP_ID" >> app_ids.txt

# Link lottery-rounds to lottery-app
echo "Linking lottery-rounds to lottery-app..."
LOTTERY_LINK_OP="{\"SetLotteryAppId\":{\"lottery_app_id\":\"$LOTTERY_APP_ID\"}}"

echo ""
echo "=========================================="
echo "DEPLOYMENT COMPLETE"
echo "=========================================="
echo "Run the following GraphQL mutations to link apps:"
echo "1. On Rounds App ($ROUNDS_ID):"
echo "   mutation { setMicrobetAppId(microbetAppId: \"$MICROBETREAL_ID\") }"
echo ""
echo "2. On Lottery Rounds App ($LOTTERY_ROUNDS_ID):"
echo "   mutation { setLotteryAppId(lotteryAppId: \"$LOTTERY_APP_ID\") }"
echo "=========================================="
