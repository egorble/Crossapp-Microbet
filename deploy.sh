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

# Deploy rounds
echo "Deploying rounds..."
# Points to 'rounds' directory
ROUNDS_ID=$(linera project publish-and-create rounds \
    --json-parameters "{\"native_app_id\":\"$NATIVE_ID\"}")

echo "Rounds deployed with ID: $ROUNDS_ID"

# Deploy microbetreal
echo "Deploying microbetreal..."
# Points to 'microbetreal' directory
MICROBETREAL_ID=$(linera project publish-and-create microbetreal \
    --json-parameters "{\"native_app_id\":\"$NATIVE_ID\", \"rounds_app_id\":\"$ROUNDS_ID\"}")

echo "Microbetreal deployed with ID: $MICROBETREAL_ID"

# Link microbetreal to rounds
echo "Linking rounds to microbetreal..."

# Construct the operation JSON
OPERATION_JSON="{\"SetMicrobetAppId\":{\"microbet_app_id\":\"$MICROBETREAL_ID\"}}"

echo "IMPORTANT: Execute the following operation on Rounds app ($ROUNDS_ID) to finish linking:"
echo ""
echo "$OPERATION_JSON"
echo ""
echo "You can use the 'linera service' and GraphQL or a client command if available."
