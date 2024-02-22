#!/bin/bash
set -e

# environment
palletListFile='pallet_list.txt'
wasmReportFile='subwasm_report.json'
PACKAGE='framenode-runtime'
RUSTFLAGS='-Dwarnings'
RUNTIME_DIR='runtime'
allfeatures='private-net,wip,ready-to-test'

test
if [[ $buildTag != null ]] && [[ ${TAG_NAME} != null || ${TAG_NAME} != '' ]]; then
    if [[ ${TAG_NAME} =~ 'benchmarking'* ]]; then
        build 'private-net runtime-benchmarks' 0
    elif [[ ${TAG_NAME} =~ 'stage'* ]]; then
        build 'private-net include-real-files ready-to-test' 0
    elif [[ ${TAG_NAME} =~ 'test'* ]]; then
        build 'private-net include-real-files reduced-pswap-reward-periods ready-to-test' 0
    elif [[ -n ${TAG_NAME} && ${TAG_NAME} != 'predev' ]]; then
        build 'include-real-files' 101
    fi
else
    if [ $prBranch = 'master' ]; then
        printf "⚡️ Running tests and migrations %s\n"
        RUST_LOG="debug cargo test --features try-runtime -- run_migrations"
    fi
fi

# build func
test() {
    if [[ $buildTag != null ]] && [[ ${TAG_NAME} != null || ${TAG_NAME} != '' ]]; then
        printf "⚡️ Tag is %s\n" $buildTag ${TAG_NAME}
        printf "⚡️ Testing with features: private-net runtime-benchmarks\n"
        cargo test --release --features "private-net runtime-benchmarks"
    else
        printf "⚡️ Running Tests for code coverage only\n"
        export RUSTFLAGS="-Cinstrument-coverage"
        export SKIP_WASM_BUILD=1
        export LLVM_PROFILE_FILE="sora2-%p-%m.profraw"
        rm -rf ~/.cargo/.package-cache
        cargo fmt -- --check > /dev/null
        cargo test --features $allfeatures
    fi
}

build() {
    featureList=$1
    sudoCheckStatus=$2
    printf "⚡️ Building with features: %s\n" "$featureList"
    printf "⚡️ Checking sudo pallet: %s\n" "$sudoCheckStatus"
    rm -rf target
    cargo build --release --features "$featureList"
    mv ./target/release/framenode .
    mv ./target/release/wbuild/framenode-runtime/framenode_runtime.compact.compressed.wasm ./framenode_runtime.compact.compressed.wasm
    subwasm --json info framenode_runtime.compact.compressed.wasm > $wasmReportFile
    subwasm metadata framenode_runtime.compact.compressed.wasm > $palletListFile
    set +e
    subwasm metadata -m Sudo framenode_runtime.compact.compressed.wasm
    if [[ $? -eq $sudoCheckStatus ]]; then 
        echo "✅ sudo check is successful!"
    else 
        echo "❌ sudo check is failed!"
        exit 1
    fi
}