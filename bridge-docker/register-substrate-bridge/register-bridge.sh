#!/bin/sh

# Wait for parachain to start
sleep 30

relayer \
	--substrate-url ws://bridge-sora-alice:9944 \
	--substrate-key //Alice \
	--parachain-url ws://bridge-parachain-alice:9844 \
	--parachain-key //Alice \
	bridge register-substrate-bridge --both