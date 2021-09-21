#!/bin/sh

arg=$1

case $arg in
  [mM] ) cargo run --bin framenode --release --features include-real-files -- build-spec --chain main-coded --raw > node/chain_spec/src/bytes/chain_spec_main.json;;
  [tT] ) cargo run --bin framenode --release --features "private-net include-real-files reduced-pswap-reward-periods" -- build-spec --chain test-coded --raw > node/chain_spec/src/bytes/chain_spec_test.json;;
  [sS] ) cargo run --bin framenode --release --features "private-net include-real-files" -- build-spec --chain staging-coded --raw > node/chain_spec/src/bytes/chain_spec_staging.json;;
  [nN] ) ;;
  * ) echo "Please provide network. t - test, s - stage, m - master";;
esac
