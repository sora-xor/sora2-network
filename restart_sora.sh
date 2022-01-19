#!/bin/bash -v 

rm -rf db*
cargo run --bin relayer --release fetch-ethereum-header --ethereum-url ws://localhost:8546 -d 3 > node/chain_spec/src/bytes/ethereum_header.json
cargo b --features private-net --release --bin framenode
./run_script.sh

