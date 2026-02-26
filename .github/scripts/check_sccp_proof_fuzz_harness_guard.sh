#!/usr/bin/env bash
set -euo pipefail

SCCP_CARGO="pallets/sccp/Cargo.toml"
SCCP_LIB="pallets/sccp/src/lib.rs"
FUZZ_CARGO="pallets/sccp/fuzz/Cargo.toml"
FUZZ_EVM="pallets/sccp/fuzz/fuzz_targets/evm_proof_helpers.rs"
FUZZ_TRON="pallets/sccp/fuzz/fuzz_targets/tron_proof_helpers.rs"

for required in "${SCCP_CARGO}" "${SCCP_LIB}" "${FUZZ_CARGO}" "${FUZZ_EVM}" "${FUZZ_TRON}"; do
  if [[ ! -f "${required}" ]]; then
    echo "[check_sccp_proof_fuzz_harness_guard] missing ${required}" >&2
    exit 1
  fi
done

if ! rg -q "^fuzzing\s*=\s*\[\]" "${SCCP_CARGO}"; then
  echo "[check_sccp_proof_fuzz_harness_guard] missing sccp fuzzing feature gate" >&2
  exit 1
fi

if ! rg -q "pub mod evm_proof;" "${SCCP_LIB}" || ! rg -q "feature = \"fuzzing\"" "${SCCP_LIB}"; then
  echo "[check_sccp_proof_fuzz_harness_guard] missing fuzz-gated evm_proof export" >&2
  exit 1
fi

if ! rg -q "pub mod tron_proof;" "${SCCP_LIB}" || ! rg -q "feature = \"fuzzing\"" "${SCCP_LIB}"; then
  echo "[check_sccp_proof_fuzz_harness_guard] missing fuzz-gated tron_proof export" >&2
  exit 1
fi

if ! rg -q "name = \"evm_proof_helpers\"" "${FUZZ_CARGO}"; then
  echo "[check_sccp_proof_fuzz_harness_guard] missing evm_proof_helpers fuzz target declaration" >&2
  exit 1
fi

if ! rg -q "name = \"tron_proof_helpers\"" "${FUZZ_CARGO}"; then
  echo "[check_sccp_proof_fuzz_harness_guard] missing tron_proof_helpers fuzz target declaration" >&2
  exit 1
fi

if ! rg -q "mpt_get\(" "${FUZZ_EVM}" || ! rg -q "rlp_decode\(" "${FUZZ_EVM}"; then
  echo "[check_sccp_proof_fuzz_harness_guard] EVM fuzz target missing proof helper invocations" >&2
  exit 1
fi

if ! rg -q "parse_tron_header_raw\(" "${FUZZ_TRON}" || ! rg -q "recover_eth_address_from_sig\(" "${FUZZ_TRON}"; then
  echo "[check_sccp_proof_fuzz_harness_guard] TRON fuzz target missing proof helper invocations" >&2
  exit 1
fi

echo "[check_sccp_proof_fuzz_harness_guard] PASS"
