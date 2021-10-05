#!/bin/sh

export RUSTFLAGS="-Zinstrument-coverage"
export SKIP_WASM_BUILD=1
export LLVM_PROFILE_FILE="sora2-%p-%m.profraw"

cargo test --features private-net --target-dir /app/target

grcov . --binary-path /app/target/debug -s . -t cobertura --branch --ignore-not-existing -o ./cobertura_report
