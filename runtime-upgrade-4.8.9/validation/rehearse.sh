#!/usr/bin/env bash
set -euo pipefail
UPGRADE_VALIDATION_DIR="$(cd "$(dirname "$0")" && pwd)"
# Pass the extracted release source directory, or default to the enclosing checkout.
UPGRADE_SOURCE_DIR="${1:-$UPGRADE_VALIDATION_DIR/../..}"
cd "$UPGRADE_SOURCE_DIR"
test -f runtime/Cargo.toml || { echo "Pass the extracted release source directory as argument 1" >&2; exit 1; }
export SKIP_WASM_BUILD=1 REQUIRE_REMOTE=1 REMOTE_RPC_URL=wss://mof2.sora.org
export REMOTE_BLOCK_HASH=0xedc2506edcb75c79c12d26a8c89a2473677a8aff67b3e05eb310573bdbf2a049
export REMOTE_CHILD_TRIE=0
export REMOTE_PALLETS=System,MultiBlockMigrations,XorFee,Staking,Offences,Session,Grandpa,ImOnline,PoolXYK,PswapDistribution,VestedRewards,Identity,Farming,Kensetsu,Band,Polkamarkt,OracleProxy,BridgeInboundChannel,SubstrateBridgeInboundChannel,SubstrateBridgeOutboundChannel
export REMOTE_HASHED_PREFIXES=0x8e8f4d4777099074419cbb4597dac1d3d58d7b3d264ccf1905be36bf3f4f7040,0x8e8f4d4777099074419cbb4597dac1d32455672c652ba1a1af2a4ce9614f541b,0x8e8f4d4777099074419cbb4597dac1d32db2d969c1d2247f7efebb578ef18d2b,0x8e8f4d4777099074419cbb4597dac1d3b94b71c7bb97c63db43b451b46f31b09,0x8e8f4d4777099074419cbb4597dac1d317cd2ab6bb58708f5903f65e8ace2fb0,0x8e8f4d4777099074419cbb4597dac1d34f1c48c3cf3da900a7dac823371f7a9f,0x8e8f4d4777099074419cbb4597dac1d35c63ad4ba9b87d8a9938e57f2391a87b,0x8e8f4d4777099074419cbb4597dac1d3e973821932a99de497c2362ed9f9be34
export REMOTE_HASHED_KEYS=0x8e8f4d4777099074419cbb4597dac1d34e7b9012096b41c4eb3aaf947f6ea429,0x72756e74696d653a6d6967726174696f6e733a7374616b696e675f7265776172645f706f696e74735f73746173685f72656d6170706564
export SNAP="$UPGRADE_VALIDATION_DIR/mainnet-27620167.snap"
export LIBCLANG_PATH="${LIBCLANG_PATH:-/opt/homebrew/opt/llvm@21/lib}"
export LLVM_CONFIG_PATH="${LLVM_CONFIG_PATH:-/opt/homebrew/opt/llvm@21/bin/llvm-config}"
scripts/with_llvm_env.sh cargo test --locked -p framenode-runtime --features try-runtime --lib remote_try_runtime_upgrade_rehearsal -- --exact --nocapture
scripts/with_llvm_env.sh cargo test --locked -p framenode-runtime --features try-runtime --lib remote_eth_bridge_migration_rehearsal -- --exact --nocapture
