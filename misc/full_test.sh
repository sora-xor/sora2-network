#!/usr/bin/bash -v
set -e
export SKIP_WASM_BUILD=1

cargo clippy -- -D warnings
cargo clippy --features ready-to-test -- -D warnings
cargo clippy --features ready-to-test,wip -- -D warnings
cargo clippy --features private-net -- -D warnings
cargo clippy --features private-net,ready-to-test -- -D warnings
cargo clippy --features private-net,ready-to-test,wip -- -D warnings
cargo clippy --features runtime-benchmarks -- -D warnings
cargo clippy --features runtime-benchmarks,private-net -- -D warnings
cargo clippy --features runtime-benchmarks,private-net,ready-to-test -- -D warnings
cargo clippy --features runtime-benchmarks,private-net,ready-to-test,wip -- -D warnings

cargo t
cargo t --features ready-to-test
cargo t --features ready-to-test,wip
cargo t --features private-net
cargo t --features private-net,ready-to-test
cargo t --features private-net,ready-to-test,wip
cargo t --features runtime-benchmarks
cargo t --features runtime-benchmarks,private-net
cargo t --features runtime-benchmarks,private-net,ready-to-test
cargo t --features runtime-benchmarks,private-net,ready-to-test,wip