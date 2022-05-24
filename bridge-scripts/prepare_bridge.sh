#!/usr/bin/bash -v

cargo run --bin relayer --release -- \
	register-bridge \
	--ethereum-url ws://localhost:8546 \
	--substrate-url ws://localhost:9944 \
	--substrate-key //Alice \
	--incentivized-channel-inbound 0x2708Ca421cB69305831018353168727601De3e39 \
	--basic-channel-inbound 0x56a2100f161ae3df13137f65a213A9872c78c7D6 \
	--eth-app 0xC9543E78F2dDFA4a72A2E5130EC9A156D94F16aa \
	-d 10

cargo run --bin relayer --release -- \
	register-erc20-app \
	--ethereum-url ws://localhost:8546 \
	--substrate-url ws://localhost:9944 \
	--substrate-key //Alice \
	--is-native \
	--contract 0xdC0e6638015a2E4A00bc2980412e744C9aAd7056

cargo run --bin relayer --release -- \
	register-erc20-app \
	--ethereum-url ws://localhost:8546 \
	--substrate-url ws://localhost:9944 \
	--substrate-key //Alice \
	--contract 0x55E87ac6e15cfefD0E2B80c70B4A8Ab112d10f4c

sleep 60

cargo run --bin relayer --release -- \
	register-erc20-asset \
	--ethereum-url ws://localhost:8546 \
	--substrate-url ws://localhost:9944 \
	--substrate-key //Alice \
	--address 0xF02F94cc6581dD0D26A745530580A3d22dFD44F4 \
	--asset-id 0x0200060000000000000000000000000000000000000000000000000000000000

cargo run --bin relayer --release -- \
	register-erc20-asset \
	--ethereum-url ws://localhost:8546 \
	--substrate-url ws://localhost:9944 \
	--substrate-key //Alice \
	--is-native \
	--asset-id 0x0200000000000000000000000000000000000000000000000000000000000000

cargo run --bin relayer --release -- \
	register-erc20-asset \
	--ethereum-url ws://localhost:8546 \
	--substrate-url ws://localhost:9944 \
	--substrate-key //Alice \
	--is-native \
	--asset-id 0x0200040000000000000000000000000000000000000000000000000000000000

cargo run --bin relayer --release -- \
	transfer-to-sora \
	--eth-app 0xC9543E78F2dDFA4a72A2E5130EC9A156D94F16aa \
	--ethereum-url ws://localhost:8546 \
	--ethereum-key-file relayer/ethereum-key \
	-r 0x90b5ab205c6974c9ea841be688864633dc9ca8a357843eeacf2314649965fe22 \
	-a 1000000000000000000
