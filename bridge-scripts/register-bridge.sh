#!/bin/bash -v

DEPLOYMENTS=${BASE_DIR:-ethereum-bridge-contracts}/.deployments/${NETWORK:-geth}
ETH_APP=$(jq '.address' $DEPLOYMENTS/ETHApp.json | tr -d '"')
SIDECHAIN_APP=$(jq '.address' $DEPLOYMENTS/SidechainApp.json | tr -d '"')
MIGRATION_APP=$(jq '.address' $DEPLOYMENTS/MigrationApp.json | tr -d '"')
ERC20_APP=$(jq '.address' $DEPLOYMENTS/ERC20App.json | tr -d '"')
BASIC_OUTBOUND=$(jq '.address' $DEPLOYMENTS/BasicOutboundChannel.json | tr -d '"')
INCENTIVIZED_OUTBOUND=$(jq '.address' $DEPLOYMENTS/IncentivizedOutboundChannel.json | tr -d '"')
echo "Use deployments from $DEPLOYMENTS"

cargo b --release --bin relayer

cargo run --bin relayer --release -- \
	--ethereum-url ws://localhost:8546 \
	--substrate-url ws://localhost:9944 \
	--substrate-key //Alice \
	bridge register-bridge \
	--basic-outbound $BASIC_OUTBOUND \
	--incentivized-outbound $INCENTIVIZED_OUTBOUND \
	-d 10

sleep 60

cargo run --bin relayer --release -- \
	--ethereum-url ws://localhost:8546 \
	--substrate-url ws://localhost:9944 \
	--substrate-key //Alice \
	bridge register-app eth-app-predefined \
	--contract $ETH_APP

cargo run --bin relayer --release -- \
	--ethereum-url ws://localhost:8546 \
	--substrate-url ws://localhost:9944 \
	--substrate-key //Alice \
	bridge register-app native-app \
	--contract $SIDECHAIN_APP

cargo run --bin relayer --release -- \
	--ethereum-url ws://localhost:8546 \
	--substrate-url ws://localhost:9944 \
	--substrate-key //Alice \
	bridge register-app erc20-app \
	--contract $ERC20_APP
