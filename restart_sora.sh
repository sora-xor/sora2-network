#!/bin/bash -v 

rm -rf db*
cd relayer
go run . dump-block -u http://localhost:8545 | tee ../node/chain_spec/src/bytes/ethereum_header.json
cd ..
cargo b --features private-net --release
./run_script.sh

