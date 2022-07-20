#!/bin/sh

export RUSTFLAGS="-Cinstrument-coverage"
export SKIP_WASM_BUILD=1
export LLVM_PROFILE_FILE="sora2-%p-%m.profraw"

mold --run cargo test --features private-net

grcov . --binary-path /app/target/debug -s . -t cobertura --branch --ignore-not-existing -o ./cobertura_report
