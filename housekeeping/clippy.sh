
#!/bin/bash
set -e

printf "starting clippy \n"

SKIP_WASM_BUILD=1 cargo clippy
SKIP_WASM_BUILD=1 cargo clippy --features private-net,ready-to-test,runtime-benchmarks
SKIP_WASM_BUILD=1 cargo clippy --features private-net,ready-to-test,wip,runtime-benchmarks