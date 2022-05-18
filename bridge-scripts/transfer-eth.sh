#!/usr/bin/bash -v

DEPLOYMENTS=${BASE_DIR:-ethereum-bridge-contracts}/.deployments/${NETWORK:-geth}
ETH_APP=$(jq '.address' $DEPLOYMENTS/ETHApp.json | tr -d '"')
echo "Use deployments from $DEPLOYMENTS"

cargo run --bin relayer --release -- \
	transfer-to-sora \
	--ethereum-url ws://localhost:8546 \
	--ethereum-key $1 \
	--substrate-url ws://localhost:9944 \
	-r 0x90b5ab205c6974c9ea841be688864633dc9ca8a357843eeacf2314649965fe22 \
	-a 1000000000000000000

cargo run --bin relayer --release -- \
	transfer-to-sora \
	--ethereum-url ws://localhost:8546 \
	--ethereum-key $1 \
	--substrate-url ws://localhost:9944 \
	--asset-id 0x0200060000000000000000000000000000000000000000000000000000000000
	-r 0x90b5ab205c6974c9ea841be688864633dc9ca8a357843eeacf2314649965fe22 \
	-a 1000000000000000000

cargo run --bin relayer --release -- \
	transfer-to-sora \
	--ethereum-url ws://localhost:8546 \
	--ethereum-key $1 \
	--substrate-url ws://localhost:9944 \
	--asset-id 0x0200000000000000000000000000000000000000000000000000000000000000
	-r 0x90b5ab205c6974c9ea841be688864633dc9ca8a357843eeacf2314649965fe22 \
	-a 1000000000000000000