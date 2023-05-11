#!/bin/bash

export RUST_LOG=info,relayer=debug 

cargo run --bin relayer --release -- \
    --ethereum-url ws://localhost:8546 \
    --substrate-url ws://localhost:9944 \
    --substrate-key //Relayer \
    bridge relay ethereum \
    --base-path /tmp/relayer
