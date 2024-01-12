#!/bin/sh

export RUSTFLAGS="-Cinstrument-coverage"
export SKIP_WASM_BUILD=1
export LLVM_PROFILE_FILE="sora2-%p-%m.profraw"
profraw=$(find . -name 'sora2-*.profraw'

cargo test --features private-net,ready-to-test,wip

echo "Profraw files here: $profraw"

grcov . --binary-path ./target/debug -s . -t html --branch -o ./cobertura_report --ignore-not-existing --ignore  "/opt/cargo/**" "target/debug" "node/src"

# Check coverage errors
if [ $? -eq 1 ]; then
    exit 1
fi

find . -type f -name '*.profraw' -delete
