#!/bin/bash -v

DEPLOYMENTS=${BASE_DIR:-ethereum-bridge-contracts}/.deployments/${NETWORK:-geth}
ETH_APP=$(jq '.address' $DEPLOYMENTS/ETHApp.json | tr -d '"')
SIDECHAIN_APP=$(jq '.address' $DEPLOYMENTS/SidechainApp.json | tr -d '"')
MIGRATION_APP=$(jq '.address' $DEPLOYMENTS/MigrationApp.json | tr -d '"')
ERC20_APP=$(jq '.address' $DEPLOYMENTS/ERC20App.json | tr -d '"')
INBOUND=$(jq '.address' $DEPLOYMENTS/InboundChannel.json | tr -d '"')
OUTBOUND=$(jq '.address' $DEPLOYMENTS/OutboundChannel.json | tr -d '"')
PRIVATE_NET_CONFIG="bridge-scripts/local_net_config.json"
echo "Use deployments from $DEPLOYMENTS"

REGISTER_ADDITIONAL_ARGS="--custom $PRIVATE_NET_CONFIG"
if [[ $# -gt 0 ]]; then
	REGISTER_ADDITIONAL_ARGS="$@"
fi

cargo b --release --bin relayer

cargo run --bin relayer --release -- \
	--ethereum-url ws://localhost:8546 \
	--substrate-url ws://localhost:9944 \
	--substrate-key //Alice \
	bridge register sora evm ethash \
	--descendants-until-final 10 \
  $REGISTER_ADDITIONAL_ARGS

cargo run --bin relayer --release -- \
	--ethereum-url ws://localhost:8546 \
	--substrate-url ws://localhost:9944 \
	--substrate-key //Alice \
	bridge register sora evm channels \
	--inbound-channel $INBOUND \
	--outbound-channel $OUTBOUND \
	$REGISTER_ADDITIONAL_ARGS

cargo run --bin relayer --release -- \
	--ethereum-url ws://localhost:8546 \
	--substrate-url ws://localhost:9944 \
	--substrate-key //Alice \
	bridge register sora evm app eth-app-predefined \
	--contract $ETH_APP
	--precision 18

cargo run --bin relayer --release -- \
	--ethereum-url ws://localhost:8546 \
	--substrate-url ws://localhost:9944 \
	--substrate-key //Alice \
	bridge register sora evm app native-app \
	--contract $SIDECHAIN_APP

cargo run --bin relayer --release -- \
	--ethereum-url ws://localhost:8546 \
	--substrate-url ws://localhost:9944 \
	--substrate-key //Alice \
	bridge register sora evm app erc20-app \
	--contract $ERC20_APP
