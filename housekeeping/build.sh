#!/bin/bash

# environment
palletListFile='pallet_list.txt'
wasmReportFile='subwasm_report.json'
PACKAGE='framenode-runtime'
RUSTFLAGS='-Dwarnings'
RUNTIME_DIR='runtime'

printf "Tag is %s\n" ${TAG_NAME}
printf "Tag2 is %s\n" $buildTag

# build
# If TAG_NAME is defined, build for a specific tag
if [[ ${TAG_NAME} != '' || $buildTag != '' ]]; then
    if [[ ${TAG_NAME} =~ 'benchmarking'* ]]; then
        featureList='private-net runtime-benchmarks'
        sudoCheckStatus=0
    elif [[ ${TAG_NAME} =~ 'stage'* ]]; then
        featureList='private-net include-real-files ready-to-test'
        sudoCheckStatus=0
    elif [[ ${TAG_NAME} =~ 'test'* ]]; then
        featureList='private-net include-real-files reduced-pswap-reward-periods ready-to-test'
        sudoCheckStatus=0
    elif [[ -v ${TAG_NAME} ]]; then
        featureList='include-real-files'
        sudoCheckStatus=101
    fi
    printf "Building with features: %s\n" "$featureList"
    printf "Checking sudo pallet: %s\n" "$sudoCheckStatus"
    cargo test --release --features "private-net runtime-benchmarks"
    rm -rf target
    cargo build --release --features "$featureList"
    mv ./target/release/framenode .
    mv ./target/release/relayer ./relayer.bin
    mv ./target/release/wbuild/framenode-runtime/framenode_runtime.compact.compressed.wasm ./framenode_runtime.compact.compressed.wasm
    wasm-opt -Os -o ./framenode_runtime.compact.wasm ./target/release/wbuild/framenode-runtime/framenode_runtime.compact.wasm
    subwasm --json info framenode_runtime.compact.wasm > $wasmReportFile
    subwasm metadata framenode_runtime.compact.wasm > $palletListFile
    set +e
    subwasm metadata -m Sudo target/release/wbuild/framenode-runtime/framenode_runtime.compact.wasm
    echo $?
    if [[ $(echo $?) -eq $sudoCheckStatus ]]; then echo "✅ sudo check is successful!"; else echo "❌ sudo check is failed!"; exit 1; fi
else
    # If TAG_NAME is not defined, run tests and checks
    echo 'build without tag'
    rm -rf ~/.cargo/.package-cache
    rm Cargo.lock
    cargo fmt -- --check > /dev/null
    SKIP_WASM_BUILD=1 cargo check
    SKIP_WASM_BUILD=1 cargo check --features private-net,ready-to-test
    SKIP_WASM_BUILD=1 cargo check --features private-net,ready-to-test,wip
    cargo test
    cargo test --features "private-net wip ready-to-test runtime-benchmarks"
fi
