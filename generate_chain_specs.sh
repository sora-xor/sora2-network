#!/bin/sh

set -eu

arg=${1:-}
build_main=0
build_test=0
build_stage=0
build_bridge_stage=0
llvm_wrapper="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)/scripts/with_llvm_env.sh"

case $arg in
  [yYpP] ) build_test=1; build_stage=1; build_bridge_stage=1;;
  [mM] ) build_main=1;;
  [tT] ) build_test=1;;
  [sS] ) build_stage=1;;
  [bB] ) build_bridge_stage=1;;
  [nN] ) ;;
  * ) echo "Please provide network. y, p - test & stage, t - test, s - stage, b - bridge stage, m - master";;
esac

if [ "$build_main" = 1 ]; then
  "$llvm_wrapper" cargo run --bin framenode --release --features "main-net-coded runtime-wasm" -- build-spec --chain main-coded --raw > node/chain_spec/src/bytes/chain_spec_main.json || exit 1
fi

if [ "$build_test" = 1 ]; then
  "$llvm_wrapper" cargo run --bin framenode --release --features "private-net wip stage include-real-files reduced-pswap-reward-periods runtime-wasm" -- build-spec --chain test-coded --raw > node/chain_spec/src/bytes/chain_spec_test.json || exit 1
fi

if [ "$build_stage" = 1 ]; then
  "$llvm_wrapper" cargo run --bin framenode --release --features "private-net stage include-real-files runtime-wasm" -- build-spec --chain staging-coded --raw > node/chain_spec/src/bytes/chain_spec_staging.json || exit 1
fi

if [ "$build_bridge_stage" = 1 ]; then
  "$llvm_wrapper" cargo run --bin framenode --release --features "private-net stage include-real-files runtime-wasm" -- build-spec --chain bridge-staging-coded --raw > node/chain_spec/src/bytes/chain_spec_bridge_staging.json || exit 1
fi
