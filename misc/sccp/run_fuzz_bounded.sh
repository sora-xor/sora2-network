#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FUZZ_DIR="${ROOT_DIR}/pallets/sccp/fuzz"

PROFILE="${SCCP_FUZZ_PROFILE:-full}"
AUTO_INSTALL="${SCCP_FUZZ_AUTO_INSTALL:-0}"
SCCP_RUSTUP_TOOLCHAIN="${SCCP_RUSTUP_TOOLCHAIN:-${RUSTUP_TOOLCHAIN:-nightly-2025-05-08}}"
export RUSTUP_TOOLCHAIN="${SCCP_RUSTUP_TOOLCHAIN}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile)
      PROFILE="$2"
      shift 2
      ;;
    --auto-install)
      AUTO_INSTALL="1"
      shift
      ;;
    *)
      echo "unknown argument: $1" >&2
      echo "usage: misc/sccp/run_fuzz_bounded.sh [--profile fast|full] [--auto-install]" >&2
      exit 1
      ;;
  esac
done

require_positive_int() {
  local name="$1"
  local value="$2"
  if [[ ! "${value}" =~ ^[1-9][0-9]*$ ]]; then
    echo "${name} must be a positive integer (got: ${value})" >&2
    exit 1
  fi
}

case "${PROFILE}" in
  fast)
    DEFAULT_EVM_SECONDS=60
    DEFAULT_TRON_SECONDS=60
    DEFAULT_ATTEST_SECONDS=60
    DEFAULT_BSC_SECONDS=60
    ;;
  full)
    DEFAULT_EVM_SECONDS=300
    DEFAULT_TRON_SECONDS=300
    DEFAULT_ATTEST_SECONDS=300
    DEFAULT_BSC_SECONDS=300
    ;;
  *)
    echo "unsupported profile: ${PROFILE} (expected: fast|full)" >&2
    exit 1
    ;;
esac

EVM_SECONDS="${SCCP_FUZZ_EVM_SECONDS:-${DEFAULT_EVM_SECONDS}}"
TRON_SECONDS="${SCCP_FUZZ_TRON_SECONDS:-${DEFAULT_TRON_SECONDS}}"
ATTEST_SECONDS="${SCCP_FUZZ_ATTEST_SECONDS:-${DEFAULT_ATTEST_SECONDS}}"
BSC_SECONDS="${SCCP_FUZZ_BSC_SECONDS:-${DEFAULT_BSC_SECONDS}}"

require_positive_int "SCCP_FUZZ_EVM_SECONDS" "${EVM_SECONDS}"
require_positive_int "SCCP_FUZZ_TRON_SECONDS" "${TRON_SECONDS}"
require_positive_int "SCCP_FUZZ_ATTEST_SECONDS" "${ATTEST_SECONDS}"
require_positive_int "SCCP_FUZZ_BSC_SECONDS" "${BSC_SECONDS}"

if ! cargo fuzz --help >/dev/null 2>&1; then
  if [[ "${AUTO_INSTALL}" == "1" ]]; then
    echo "[sccp-fuzz] cargo-fuzz is missing; installing because --auto-install was requested"
    cargo install cargo-fuzz
  else
    echo "[sccp-fuzz] cargo-fuzz is required but not installed." >&2
    echo "[sccp-fuzz] install with: cargo install cargo-fuzz" >&2
    echo "[sccp-fuzz] or rerun with: misc/sccp/run_fuzz_bounded.sh --auto-install" >&2
    exit 1
  fi
fi

echo "[sccp-fuzz] profile=${PROFILE} evm_seconds=${EVM_SECONDS} tron_seconds=${TRON_SECONDS} attest_seconds=${ATTEST_SECONDS} bsc_seconds=${BSC_SECONDS}"
echo "[sccp-fuzz] RUSTUP_TOOLCHAIN=${RUSTUP_TOOLCHAIN}"

echo "[sccp-fuzz] cargo fuzz run evm_proof_helpers"
(cd "${FUZZ_DIR}" && cargo fuzz run evm_proof_helpers -- "-max_total_time=${EVM_SECONDS}")

echo "[sccp-fuzz] cargo fuzz run tron_proof_helpers"
(cd "${FUZZ_DIR}" && cargo fuzz run tron_proof_helpers -- "-max_total_time=${TRON_SECONDS}")

echo "[sccp-fuzz] cargo fuzz run attester_quorum_helpers"
(cd "${FUZZ_DIR}" && cargo fuzz run attester_quorum_helpers -- "-max_total_time=${ATTEST_SECONDS}")

echo "[sccp-fuzz] cargo fuzz run bsc_header_helpers"
(cd "${FUZZ_DIR}" && cargo fuzz run bsc_header_helpers -- "-max_total_time=${BSC_SECONDS}")

echo "[sccp-fuzz] OK"
