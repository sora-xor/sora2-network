#!/bin/bash -v
export RUST_LOG=info,relayer=debug 

cargo run --release --bin relayer -- \
    --ethereum-key $1 \
    --ethereum-url ws://localhost:8546 \
    --substrate-url ws://localhost:9944 \
    bridge relay substrate
