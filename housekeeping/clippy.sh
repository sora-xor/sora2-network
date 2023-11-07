#!/bin/bash
set -e

if [ "$pr" = true ] && [ "$prBranch" != "master" ]; then
    printf "👷‍♂️ starting clippy \n"
    SKIP_WASM_BUILD=1 cargo clippy --features private-net,ready-to-test,runtime-benchmarks -- -D warnings
    SKIP_WASM_BUILD=1 cargo clippy --features private-net,ready-to-test,wip,runtime-benchmarks -- -D warnings
else
    printf "👷‍♂️ starting a regular clippy \n"
    cargo clippy
fi
