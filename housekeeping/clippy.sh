#!/bin/bash
set -e

if [ "$pr" = true ] && [ "$prBranch" != "master" ]; then
    printf "👷‍♂️ starting clippy \n"
    SKIP_WASM_BUILD=1 cargo clippy
    SKIP_WASM_BUILD=1 cargo clippy --features private-net,ready-to-test,runtime-benchmarks
    SKIP_WASM_BUILD=1 cargo clippy --features private-net,ready-to-test,wip,runtime-benchmarks
else
    printf "👷‍♂️ starting a regular clippy \n"
    cargo clippy -- -D warnings || exit 0
fi
