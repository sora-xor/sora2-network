#!/bin/bash

# Runs all benchmarks for all pallets, for a given runtime, provided by $1
# Should be run on a reference machine to gain accurate benchmarks
# current reference machine: https://github.com/paritytech/substrate/pull/5848

# Load all pallet names in an array.
PALLETS=($(
  /usr/local/bin/framenode benchmark pallet --list --chain="local" |\
    tail -n+2 |\
    cut -d',' -f1 |\
    sort |\
    uniq
))

declare -A PATH_OVERRIDES=(
    [bridge-inbound-channel]=./pallets/trustless-bridge/bridge-inbound-channel/src/weights.rs
    [bridge-outbound-channel]=./pallets/trustless-bridge/bridge-outbound-channel/src/weights.rs
    [erc20-app]=./pallets/trustless-bridge/erc20-app/src/weights.rs
    [eth-app]=./pallets/trustless-bridge/eth-app/src/weights.rs
    [ethereum-light-client]=./pallets/trustless-bridge/ethereum-light-client/src/weights.rs
    [evm-bridge-proxy]=./pallets/trustless-bridge/bridge-proxy/src/weights.rs
    [migration-app]=./pallets/trustless-bridge/migration-app/src/weights.rs
    [substrate-bridge-app]=./runtime/src/weights/substrate_bridge_app.rs
    [substrate-bridge-channel-inbound]=./runtime/src/weights/substrate_inbound_channel.rs
    [substrate-bridge-channel-outbound]=./runtime/src/weights/substrate_outbound_channel.rs
    [dispatch]=./runtime/src/weights/dispatch.rs
    [multisig-verifier]=./runtime/src/weights/multisig_verifier.rs
    [bridge-data-signer]=./runtime/src/weights/bridge_data_signer.rs
)

declare -A WITHOUT_TEMPLATE=(
    [multisig-verifier]=1
    [bridge-data-signer]=1
    [dispatch]=1
    [substrate-bridge-app]=1
    [substrate-bridge-channel-inbound]=1
    [substrate-bridge-channel-outbound]=1
)

echo "[+] Benchmarking ${#PALLETS[@]} pallets for runtime local"

# Define the error file.
ERR_FILE="benchmarking_errors.txt"
# Delete the error file before each run.
rm -f $ERR_FILE

# Benchmark each pallet.
for PALLET in "${PALLETS[@]}"; do
  pallet_dir=""
  if [[ $PALLET == *"::"* ]]; then
    # translates e.g. "pallet_foo::bar" to "pallet_foo_bar"
    pallet_dir="${PALLET//::/-}"
  else
    pallet_dir="$PALLET"
  fi
  pallet_dir="${pallet_dir//_/-}"
  
  weight_path=""
  if [[ -v "PATH_OVERRIDES[$pallet_dir]" ]]; then 
    weight_path="${PATH_OVERRIDES[$pallet_dir]}"
  else
    weight_path="./pallets/$pallet_dir/src/weights.rs"
  fi
  pallet_path="$(dirname $(dirname $weight_path))"

  BENCHMARK_ARGS=""
  if [[ -v "WITHOUT_TEMPLATE[$pallet_dir]" ]]; then 
    echo "[*] Don't use template for $pallet_dir"
  else
    BENCHMARK_ARGS+="--template=./misc/pallet-weight-template.hbs"
  fi
  
  if [ -d "$pallet_path" ]; then
    echo "[+] Benchmarking $PALLET in $pallet_path";

    OUTPUT=$(
      /usr/local/bin/framenode benchmark pallet \
      --chain="local" \
      --steps=50 \
      --repeat=20 \
      --pallet="$PALLET" \
      --extrinsic="*" \
      --execution=wasm \
      --wasm-execution=compiled \
      --header=./misc/file_header.txt \
      $BENCHMARK_ARGS \
      --output="$weight_path" 2>&1
    )
    if [ $? -ne 0 ]; then
      echo "$OUTPUT" >> "$ERR_FILE"
      echo "[-] Failed to benchmark $PALLET. Error written to $ERR_FILE; continuing..."
    fi
  else
    echo "[-] $PALLET in $pallet_path not found, skipping..."
  fi
done

# Update the block and extrinsic overhead weights.
# echo "[+] Benchmarking block and extrinsic overheads..."
# OUTPUT=$(
#   ./target/release/framenode benchmark overhead \
#   --chain="local" \
#   --execution=wasm \
#   --wasm-execution=compiled \
#   --weight-path="runtime/src/constants/" \
#   --warmup=10 \
#   --repeat=100 \
#   --header=./file_header.txt
# )
# if [ $? -ne 0 ]; then
#   echo "$OUTPUT" >> "$ERR_FILE"
#   echo "[-] Failed to benchmark the block and extrinsic overheads. Error written to $ERR_FILE; continuing..."
# fi

# Check if the error file exists.
if [ -f "$ERR_FILE" ]; then
  echo "[-] Some benchmarks failed. See: $ERR_FILE"
else
  echo "[+] All benchmarks passed."
fi