#!/bin/sh

arg=$1

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
  cargo run --bin framenode --release --features include-real-files -- build-spec --chain main-coded --raw > node/chain_spec/src/bytes/chain_spec_main.json || exit 1
fi

if [ "$build_test" = 1 ]; then
  cargo run --bin framenode --release --features "private-net wip stage include-real-files reduced-pswap-reward-periods" -- build-spec --chain test-coded --raw > node/chain_spec/src/bytes/chain_spec_test.json || exit 1
fi

if [ "$build_stage" = 1 ]; then
  cargo run --bin framenode --release --features "private-net stage include-real-files" -- build-spec --chain staging-coded --raw > node/chain_spec/src/bytes/chain_spec_staging.json || exit 1
fi

if [ "$build_bridge_stage" = 1 ]; then
  cargo run --bin framenode --release --features "private-net stage include-real-files" -- build-spec --chain bridge-staging-coded --raw > node/chain_spec/src/bytes/chain_spec_bridge_staging.json || exit 1
fi
