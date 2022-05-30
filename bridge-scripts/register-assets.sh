#!/usr/bin/bash -v

DEPLOYMENTS=${BASE_DIR:-ethereum-bridge-contracts}/.deployments/${NETWORK:-geth}
BASIC_OUTBOUND=$(jq '.address' $DEPLOYMENTS/BasicOutboundChannel.json | tr -d '"')
INCENTIVIZED_OUTBOUND=$(jq '.address' $DEPLOYMENTS/IncentivizedOutboundChannel.json | tr -d '"')
ETH_APP=$(jq '.address' $DEPLOYMENTS/ETHApp.json | tr -d '"')
SIDECHAIN_APP=$(jq '.address' $DEPLOYMENTS/SidechainApp.json | tr -d '"')
ERC20_APP=$(jq '.address' $DEPLOYMENTS/ERC20App.json | tr -d '"')
DAI=$(jq '.address' $DEPLOYMENTS/DAI.json | tr -d '"')
USDT=$(jq '.address' $DEPLOYMENTS/USDT.json | tr -d '"')
echo "Use deployments from $DEPLOYMENTS"

cargo run --bin relayer --release -- \
	--ethereum-url ws://localhost:8546 \
	--substrate-url ws://localhost:9944 \
	--substrate-key //Alice \
	bridge register-asset existing-erc20 \
	--address $DAI \
	--asset-id 0x0200060000000000000000000000000000000000000000000000000000000000

cargo run --bin relayer --release -- \
	--ethereum-url ws://localhost:8546 \
	--substrate-url ws://localhost:9944 \
	--substrate-key //Alice \
	bridge register-asset erc20 \
	--address $USDT \
	--name "Tether USD" \
	--symbol "USDT"

sleep 60

cargo run --bin relayer --release -- \
	--ethereum-url ws://localhost:8546 \
	--substrate-url ws://localhost:9944 \
	--substrate-key //Alice \
	bridge register-asset native \
	--asset-id 0x0200000000000000000000000000000000000000000000000000000000000000

cargo run --bin relayer --release -- \
	--ethereum-url ws://localhost:8546 \
	--substrate-url ws://localhost:9944 \
	--substrate-key //Alice \
	bridge register-asset native \
	--asset-id 0x0200040000000000000000000000000000000000000000000000000000000000

sleep 60

cargo run --bin relayer --release -- \
	--ethereum-url ws://localhost:8546 \
	--substrate-url ws://localhost:9944 \
	--substrate-key //Alice \
	bridge register-asset native \
	--asset-id 0x0200050000000000000000000000000000000000000000000000000000000000
