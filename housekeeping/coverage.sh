#!/bin/sh
pofrw=$(find . -name 'sora2-*.profraw')
export RUSTFLAGS="-Cinstrument-coverage"
export SKIP_WASM_BUILD=1
export LLVM_PROFILE_FILE="$pofrw"
whereis llvm-profdata
cargo test --features private-net,ready-to-test,wip

grcov . --binary-path ./target/debug -s . -t html --branch -o ./cobertura_report --ignore-not-existing --ignore  "/opt/cargo/**" "target/debug" "node/src"

# Check coverage errors
if [ $? -eq 1 ]; then
    exit 1
fi

find . -type f -name '*.profraw' -delete
