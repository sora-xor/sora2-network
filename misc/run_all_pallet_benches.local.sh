#!/bin/bash

# Runs all benchmarks for all pallets, for a given runtime, provided by $1
# Should be run on a reference machine to gain accurate benchmarks
# current reference machine: https://github.com/paritytech/substrate/pull/5848

echo "[+] Compiling benchmarks..."
cargo build --release --locked --features runtime-benchmarks,private-net --bin framenode

# Load all pallet names in an array.
PALLETS=($(
  ./target/release/framenode benchmark pallet --list --chain="local" |\
    tail -n+2 |\
    cut -d',' -f1 |\
    sort |\
    uniq
))

declare -A PATH_OVERRIDES=(
    [bridge_inbound_channel]=./pallets/trustless-bridge/bridge-inbound-channel
    [bridge_outbound_channel]=./pallets/trustless-bridge/bridge-outbound-channel
    [erc20_app]=./pallets/trustless-bridge/erc20-app
    [eth_app]=./pallets/trustless-bridge/eth-app
    [ethereum_light_client]=./pallets/trustless-bridge/ethereum-light-client
    [evm_bridge_proxy]=./pallets/trustless-bridge/bridge-proxy
    [migration_app]=./pallets/trustless-bridge/migration-app
    [multisig-verifier]=./runtime/src/weights/multisig_verifier.rs
    [bridge-data-signer]=./runtime/src/weights/bridge_data_signer.rs
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
  
  pallet_path=""
  if [[ -v "PATH_OVERRIDES[$pallet_dir]" ]]; then 
    pallet_path="${PATH_OVERRIDES[$pallet_dir]}"
  else
    pallet_path="./pallets/$pallet_dir"
  fi
  
  if [ -d "$pallet_path" ]; then
    echo "[+] Benchmarking $PALLET in $pallet_path";

    OUTPUT=$(
      ./target/release/framenode benchmark pallet \
      --chain="local" \
      --steps=50 \
      --repeat=20 \
      --pallet="$PALLET" \
      --extrinsic="*" \
      --execution=wasm \
      --wasm-execution=compiled \
      --header=./misc/file_header.txt \
      --template=./misc/pallet-weight-template.hbs \
      --output="$pallet_path/src/weights.rs" 2>&1
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