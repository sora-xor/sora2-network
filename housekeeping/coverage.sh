#!/bin/sh

cargo install grcov
rustup component add llvm-tools-preview

export RUSTFLAGS="-Zinstrument-coverage"
export SKIP_WASM_BUILD=1
export LLVM_PROFILE_FILE="sora2-%p-%m.profraw"

cargo test --features private-net

grcov . --binary-path target/debug -s . -t cobertura --branch --ignore-not-existing -o target/debug/cobertura_report
