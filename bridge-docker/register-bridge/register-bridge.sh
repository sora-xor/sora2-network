#!/bin/sh

DEPLOYMENTS=/data/deploy.json

while ! test -f "$DEPLOYMENTS"; do
  sleep 10
  echo "Waiting for deploy.json to be created..."
done

ETH_APP=$(jq '.contracts.ETHApp.address' $DEPLOYMENTS | tr -d '"')
SIDECHAIN_APP=$(jq '.contracts.SidechainApp.address' $DEPLOYMENTS | tr -d '"')
ERC20_APP=$(jq '.contracts.ERC20App.address' $DEPLOYMENTS | tr -d '"')
INBOUND=$(jq '.contracts.InboundChannel.address' $DEPLOYMENTS | tr -d '"')
OUTBOUND=$(jq '.contracts.OutboundChannel.address' $DEPLOYMENTS | tr -d '"')
USDT=$(jq '.contracts.USDT.address' $DEPLOYMENTS | tr -d '"')
DAI=$(jq '.contracts.DAI.address' $DEPLOYMENTS | tr -d '"')
PRIVATE_NET_CONFIG="/register-bridge/local_net_config.json"
echo "Use deployments from $DEPLOYMENTS"

REGISTER_ADDITIONAL_ARGS="--custom $PRIVATE_NET_CONFIG"
if [ $# -gt 0 ]; then
	REGISTER_ADDITIONAL_ARGS="$@"
fi

RUST_LOG=info,relayer=debug

# Wait for geth connection
sleep 10

relayer \
	--ethereum-url ws://bridge-geth:8545 \
	--substrate-url ws://bridge-sora-alice:9944 \
	--substrate-key //Alice \
	bridge register sora evm ethash \
	--descendants-until-final 10 \
  $REGISTER_ADDITIONAL_ARGS

relayer \
	--ethereum-url ws://bridge-geth:8545 \
	--substrate-url ws://bridge-sora-alice:9944 \
	--substrate-key //Alice \
	bridge register sora evm channels \
	--inbound-channel $INBOUND \
	--outbound-channel $OUTBOUND

relayer \
	--ethereum-url ws://bridge-geth:8545 \
	--substrate-url ws://bridge-sora-alice:9944 \
	--substrate-key //Alice \
	bridge register sora evm app eth-app-predefined \
	--contract $ETH_APP \
	--precision 18

relayer \
	--ethereum-url ws://bridge-geth:8545 \
	--substrate-url ws://bridge-sora-alice:9944 \
	--substrate-key //Alice \
	bridge register sora evm app native-app \
	--contract $SIDECHAIN_APP

relayer \
	--ethereum-url ws://bridge-geth:8545 \
	--substrate-url ws://bridge-sora-alice:9944 \
	--substrate-key //Alice \
	bridge register sora evm app erc20-app \
	--contract $ERC20_APP

sleep 60

relayer \
	--ethereum-url ws://bridge-geth:8545 \
	--substrate-url ws://bridge-sora-alice:9944 \
	--substrate-key //Alice \
	bridge register sora evm asset existing-erc20 \
	--address $DAI \
	--asset-id 0x0200060000000000000000000000000000000000000000000000000000000000 \
	--decimals 18

relayer \
	--ethereum-url ws://bridge-geth:8545 \
	--substrate-url ws://bridge-sora-alice:9944 \
	--substrate-key //Alice \
	bridge register sora evm asset erc20 \
	--address $USDT \
	--name "Tether USD" \
	--symbol "USDT" \
	--decimals 18

sleep 60

relayer \
	--ethereum-url ws://bridge-geth:8545 \
	--substrate-url ws://bridge-sora-alice:9944 \
	--substrate-key //Alice \
	bridge register sora evm asset native \
	--asset-id 0x0200000000000000000000000000000000000000000000000000000000000000

relayer \
	--ethereum-url ws://bridge-geth:8545 \
	--substrate-url ws://bridge-sora-alice:9944 \
	--substrate-key //Alice \
	bridge register sora evm asset native \
	--asset-id 0x0200040000000000000000000000000000000000000000000000000000000000

sleep 60

relayer \
	--ethereum-url ws://bridge-geth:8545 \
	--substrate-url ws://bridge-sora-alice:9944 \
	--substrate-key //Alice \
	bridge register sora evm asset native \
	--asset-id 0x0200050000000000000000000000000000000000000000000000000000000000
