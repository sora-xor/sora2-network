#!/bin/bash
set -e

# environment
palletListFile='pallet_list.txt'
wasmReportFile='subwasm_report.json'
PACKAGE='framenode-runtime'
RUSTFLAGS='-Dwarnings'
RUNTIME_DIR='runtime'
allfeatures='private-net,wip,ready-to-test,runtime-benchmarks'

# build
# If TAG_NAME is defined, build for a specific tag
if [[ $buildTag != null ]] && [[ ${TAG_NAME} != null || ${TAG_NAME} != '' ]]; then
    printf "⚡️ Tag is %s\n" $buildTag ${TAG_NAME}
    build=true
    if [[ ${TAG_NAME} =~ 'benchmarking'* ]]; then
        featureList='private-net runtime-benchmarks'
        sudoCheckStatus=0
    elif [[ ${TAG_NAME} =~ 'stage'* ]]; then
        featureList='private-net include-real-files ready-to-test'
        sudoCheckStatus=0
    elif [[ ${TAG_NAME} =~ 'test'* ]]; then
        featureList='private-net include-real-files reduced-pswap-reward-periods ready-to-test'
        sudoCheckStatus=0
    elif [[ -n ${TAG_NAME} && ${TAG_NAME} != 'predev' ]]; then
        featureList='include-real-files'
        sudoCheckStatus=101
    fi
        printf "⚡️ Testing with features: private-net runtime-benchmark %s\n"
        # we should always run these tests for these features
        cargo test --release --features "private-net runtime-benchmarks"
        printf "⚡️ Building with features: %s\n" "$featureList"
        printf "⚡️ Checking sudo pallet: %s\n" "$sudoCheckStatus"
        rm -rf target
        cargo build --release --features "$featureList"
        mv ./target/release/framenode .
        mv ./target/release/relayer ./relayer.bin
        mv ./target/release/wbuild/framenode-runtime/framenode_runtime.compact.compressed.wasm ./framenode_runtime.compact.compressed.wasm
        subwasm --json info framenode_runtime.compact.compressed.wasm > $wasmReportFile
        subwasm metadata framenode_runtime.compact.compressed.wasm > $palletListFile
        set +e
        subwasm metadata -m Sudo framenode_runtime.compact.compressed.wasm
        if [[ $(echo $?) -eq $sudoCheckStatus ]]; then echo "✅ sudo check is successful!"; else echo "❌ sudo check is failed!"; exit 1; fi
else
    # If TAG_NAME is not defined, run tests and checks
    if [[ $prBranch == 'master' ]]; then
        printf "⚡️ Running tests and migrations %s\n"
        RUST_LOG="debug cargo test --features try-runtime -- run_migrations"
    else
        printf "⚡️ Running Tests for code coverage only %s\n"
        export RUSTFLAGS="-Cinstrument-coverage"
        export SKIP_WASM_BUILD=1
        export LLVM_PROFILE_FILE="sora2-%p-%m.profraw"
        rm -rf ~/.cargo/.package-cache
        rm Cargo.lock
        cargo fmt -- --check > /dev/null
        cargo test --features 'private-net,wip,ready-to-test'
    fi
fi
