#!/usr/bin/env bash
set -euo pipefail

EVM_PROOF_RS="pallets/sccp/src/evm_proof.rs"
TRON_PROOF_RS="pallets/sccp/src/tron_proof.rs"

for required in "${EVM_PROOF_RS}" "${TRON_PROOF_RS}"; do
  if [[ ! -f "${required}" ]]; then
    echo "[check_sccp_proof_parser_tests_guard] missing ${required}" >&2
    exit 1
  fi
done

if ! rg -q "fn evm_proof_helpers_fail_closed_on_fuzzed_inputs\\(" "${EVM_PROOF_RS}"; then
  echo "[check_sccp_proof_parser_tests_guard] missing EVM deterministic fuzz-style regression test" >&2
  exit 1
fi

if ! rg -q "fn evm_proof_helpers_property_no_panic_on_arbitrary_bytes\\(" "${EVM_PROOF_RS}"; then
  echo "[check_sccp_proof_parser_tests_guard] missing EVM property-based regression test" >&2
  exit 1
fi

if ! rg -q "fn tron_proof_helpers_fail_closed_on_fuzzed_inputs\\(" "${TRON_PROOF_RS}"; then
  echo "[check_sccp_proof_parser_tests_guard] missing TRON deterministic fuzz-style regression test" >&2
  exit 1
fi

if ! rg -q "fn tron_proof_helpers_property_no_panic_on_arbitrary_bytes\\(" "${TRON_PROOF_RS}"; then
  echo "[check_sccp_proof_parser_tests_guard] missing TRON property-based regression test" >&2
  exit 1
fi

echo "[check_sccp_proof_parser_tests_guard] PASS"
