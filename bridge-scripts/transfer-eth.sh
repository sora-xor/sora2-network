#!/bin/bash -v

cargo run --bin relayer --release -- \
    --substrate-url ws://localhost:9944 \
	--ethereum-url ws://localhost:8546 \
	--ethereum-key $1 \
	bridge transfer-to-sora \
	--asset-id 0x0200070000000000000000000000000000000000000000000000000000000000 \
	-r $2 \
	-a $3
