#!/bin/bash

# Exit on error
set -e

# Compile native
echo "Compiling native..."
cd native
cargo build --release --target wasm32-unknown-unknown
cd ..

# Compile microbetreal
echo "Compiling microbetreal..."
cd microbetreal
cargo build --release --target wasm32-unknown-unknown
cd ..

# Compile rounds
echo "Compiling rounds..."
cd rounds
cargo build --release --target wasm32-unknown-unknown
cd ..

# Deploy native
echo "Deploying native..."
# Native app usually takes some init args (e.g. initial balances). 
# For now, we'll initialize with basic args or empty state if allowed.
# Checking native/src/lib.rs for proper args would be ideal, but assuming standard map init or similar.
# We'll pass a basic initial state or null if permitted. 
# WARNING: This assumes Native app instantiation is simple.
NATIVE_ID=$(linera publish-and-create \
    native/target/wasm32-unknown-unknown/release/native_{contract,service}.wasm \
    --json-argument "{\"accounts\":{}}" \
    --json-parameters "{\"ticker_symbol\":\"NAT\"}")

echo "Native deployed with ID: $NATIVE_ID"

# Deploy rounds
echo "Deploying rounds..."
# Parameters: RoundsParameters { native_app_id }
# Instantiation argument: ()
ROUNDS_ID=$(linera publish-and-create \
    rounds/target/wasm32-unknown-unknown/release/rounds_{contract,service}.wasm \
    --json-parameters "{\"native_app_id\":\"$NATIVE_ID\"}")

echo "Rounds deployed with ID: $ROUNDS_ID"

# Deploy microbetreal
echo "Deploying microbetreal..."
# Parameters: MicrobetParameters { native_app_id, rounds_app_id }
# Instantiation argument: ()
MICROBETREAL_ID=$(linera publish-and-create \
    microbetreal/target/wasm32-unknown-unknown/release/microbetreal_{contract,service}.wasm \
    --json-parameters "{\"native_app_id\":\"$NATIVE_ID\", \"rounds_app_id\":\"$ROUNDS_ID\"}")

echo "Microbetreal deployed with ID: $MICROBETREAL_ID"

# Link microbetreal to rounds
# Call SetMicrobetAppId on Rounds
# Operation: SetMicrobetAppId { microbet_app_id: String }
echo "Linking rounds to microbetreal..."

# Construct the operation JSON
OPERATION_JSON="{\"SetMicrobetAppId\":{\"microbet_app_id\":\"$MICROBETREAL_ID\"}}"

echo "IMPORTANT: Execute the following operation on Rounds app ($ROUNDS_ID) to finish linking:"
echo ""
echo "$OPERATION_JSON"
echo ""
echo "You can use the 'linera service' and GraphQL or a client command if available."
# Note: Since we can't easily execute operations from bash without `linera service` running and using curl,
# printing instructions is the safest bet for the user context unless they have a helper script.
