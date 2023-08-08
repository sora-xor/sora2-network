
#!/bin/bash
set -e

echo $pr
echo $prBranch


if [ "$pr" == true ] && [ "$prBranch" != "master" ]; then
    printf "👷‍♂️ Starting clippy \n"
    SKIP_WASM_BUILD=1 cargo clippy
    SKIP_WASM_BUILD=1 cargo clippy --features private-net,ready-to-test,runtime-benchmarks
    SKIP_WASM_BUILD=1 cargo clippy --features private-net,ready-to-test,wip,runtime-benchmarks
else
    printf "👷‍♂️ Starting regular clippy \n"
    cargo clippy -- -D warnings || exit 0
fi
