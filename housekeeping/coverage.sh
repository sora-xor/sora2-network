#!/bin/sh
set -e

# export RUSTFLAGS="-Cinstrument-coverage"
# export SKIP_WASM_BUILD=1
# export LLVM_PROFILE_FILE="sora2-%p-%m.profraw"
# new params
# export CARGO_INCREMENTAL=0
# export RUSTFLAGS="-Cinstrument-coverage -Zprofile -Ccodegen-units=1 -Copt-level=0 -Clink-dead-code -Coverflow-checks=off -Zpanic_abort_tests -Cpanic=abort"
# export RUSTDOCFLAGS="-Cpanic=abort"


# echo 'running tests'
# cargo test --features "private-net,ready-to-test,wip" -- --test-threads 3

echo '⚡️ Running coverage'
grcov . --binary-path ./target/debug -s . -t cobertura --branch -o cobertura.xml --ignore-not-existing --ignore  "/opt/cargo/**" "target/debug" "node/src" "node/src" --log-level "ERROR" --llvm-path /usr/lib/llvm-14/bin

find . -type f -name '*.profraw' -delete