#!/bin/sh

export RUSTFLAGS="-Cinstrument-coverage"
export SKIP_WASM_BUILD=1
export LLVM_PROFILE_FILE="sora2-%p-%m.profraw"

echo 'running tests'
cargo test --features private-net

echo 'running coverage'
grcov . --binary-path ./target/debug -s . -t cobertura --branch -o cobertura_report --ignore-not-existing --ignore  "/opt/cargo/**" "target/debug" "node/src" "node/src" --llvm-path /usr/lib/llvm-14/bin

find . -type f -name '*.profraw' -delete
