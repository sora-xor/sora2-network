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