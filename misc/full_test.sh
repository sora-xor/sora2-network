#!/usr/bin/bash -v
set -e
export SKIP_WASM_BUILD=1

cargo clippy -- -D warnings
cargo clippy --features stage -- -D warnings
cargo clippy --features stage,wip -- -D warnings
cargo clippy --features private-net -- -D warnings
cargo clippy --features private-net,stage -- -D warnings
cargo clippy --features private-net,stage,wip -- -D warnings
cargo clippy --features runtime-benchmarks -- -D warnings
cargo clippy --features runtime-benchmarks,private-net -- -D warnings
cargo clippy --features runtime-benchmarks,private-net,stage -- -D warnings
cargo clippy --features runtime-benchmarks,private-net,stage,wip -- -D warnings

cargo t
cargo t --features stage
cargo t --features stage,wip
cargo t --features private-net
cargo t --features private-net,stage
cargo t --features private-net,stage,wip
cargo t --features runtime-benchmarks
cargo t --features runtime-benchmarks,private-net
cargo t --features runtime-benchmarks,private-net,stage
cargo t --features runtime-benchmarks,private-net,stage,wip
