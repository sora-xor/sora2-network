#!/usr/bin/bash -v

DEPLOYMENTS=${BASE_DIR:-ethereum-bridge-contracts}/.deployments/${NETWORK:-geth}
ETH_APP=$(jq '.address' $DEPLOYMENTS/ETHApp.json | tr -d '"')
echo "Use deployments from $DEPLOYMENTS"

cargo run --bin relayer --release -- \
	transfer-to-sora \
	--eth-app $ETH_APP \
	--ethereum-url ws://localhost:8546 \
	--ethereum-key $1 \
	-r 0x90b5ab205c6974c9ea841be688864633dc9ca8a357843eeacf2314649965fe22 \
	-a 1000000000000000000
