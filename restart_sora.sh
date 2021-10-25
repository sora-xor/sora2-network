#!/bin/bash -v 

rm -rf db*
cd relayer
#go run . dump-block -u http://localhost:8545 | tee ../node/chain_spec/src/bytes/ethereum_header.json
go run . dump-block -f rust -u http://localhost:8545 > ../node/chain_spec/src/bytes/ethereum_header_4224.in
go run . dump-block -f rust -u http://localhost:8555 > ../node/chain_spec/src/bytes/ethereum_header_4225.in
cd ..
cargo b --features private-net --release
./run_script.sh

