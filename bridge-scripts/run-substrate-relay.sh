#!/usr/bin/bash -v
export RUST_LOG=info,relayer=debug 

DEPLOYMENTS=${BASE_DIR:-ethereum-bridge-contracts}/.deployments/${NETWORK:-geth}
BASIC_OUTBOUND=$(jq '.address' $DEPLOYMENTS/BasicOutboundChannel.json | tr -d '"')
INCENTIVIZED_OUTBOUND=$(jq '.address' $DEPLOYMENTS/IncentivizedOutboundChannel.json | tr -d '"')
BASIC_INBOUND=$(jq '.address' $DEPLOYMENTS/BasicInboundChannel.json | tr -d '"')
INCENTIVIZED_INBOUND=$(jq '.address' $DEPLOYMENTS/IncentivizedInboundChannel.json | tr -d '"')
ETH_APP=$(jq '.address' $DEPLOYMENTS/ETHApp.json | tr -d '"')
BEEFY=$(jq '.address' $DEPLOYMENTS/BeefyLightClient.json | tr -d '"')
echo "Use deployments from $DEPLOYMENTS"
echo "Beefy $BEEFY"

cargo run --release --bin relayer -- \
    substrate-relay \
    --basic-inbound-channel $BASIC_INBOUND \
    --incentivized-inbound-channel $INCENTIVIZED_INBOUND \
    --beefy $BEEFY \
    --ethereum-key $1 \
    --ethereum-url ws://localhost:8546 \
    --substrate-url ws://localhost:9944
