#!/usr/bin/env bash
set -euo pipefail

PROFILE="${SCCP_FORMAL_PROFILE:-full}"
WITH_KANI="${SCCP_FORMAL_WITH_KANI:-0}"
INCLUDE_SIBLINGS="${SCCP_FORMAL_INCLUDE_SIBLINGS:-1}"
SCCP_RUSTUP_TOOLCHAIN="${SCCP_RUSTUP_TOOLCHAIN:-${RUSTUP_TOOLCHAIN:-nightly-2025-05-08}}"
export RUSTUP_TOOLCHAIN="${SCCP_RUSTUP_TOOLCHAIN}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile)
      PROFILE="$2"
      shift 2
      ;;
    --with-kani)
      WITH_KANI="1"
      shift
      ;;
    --include-siblings)
      INCLUDE_SIBLINGS="1"
      shift
      ;;
    --exclude-siblings)
      INCLUDE_SIBLINGS="0"
      shift
      ;;
    *)
      echo "unknown argument: $1" >&2
      echo "usage: misc/sccp/run_formal_assisted.sh [--profile fast|full] [--with-kani] [--include-siblings|--exclude-siblings]" >&2
      exit 1
      ;;
  esac
done

run_cmd() {
  echo "[sccp-formal] $*"
  "$@"
}

require_bool_01() {
  local name="$1"
  local value="$2"
  if [[ "${value}" != "0" && "${value}" != "1" ]]; then
    echo "${name} must be 0 or 1 (got: ${value})" >&2
    exit 1
  fi
}

require_bool_01 "SCCP_FORMAL_INCLUDE_SIBLINGS" "${INCLUDE_SIBLINGS}"

echo "[sccp-formal] RUSTUP_TOOLCHAIN=${RUSTUP_TOOLCHAIN}"

case "${PROFILE}" in
  fast)
    run_cmd cargo test -p sccp formal_assisted_ -- --nocapture
    run_cmd cargo test -p sccp evm_proof_helpers_fail_closed_on_fuzzed_inputs -- --nocapture
    run_cmd cargo test -p sccp tron_proof_helpers_fail_closed_on_fuzzed_inputs -- --nocapture
    ;;
  full)
    run_cmd cargo test -p sccp formal_assisted_ -- --nocapture
    run_cmd cargo test -p sccp evm_proof_helpers_fail_closed_on_fuzzed_inputs -- --nocapture
    run_cmd cargo test -p sccp tron_proof_helpers_fail_closed_on_fuzzed_inputs -- --nocapture
    run_cmd cargo test -p sccp evm_proof_helpers_property_no_panic_on_arbitrary_bytes -- --nocapture
    run_cmd cargo test -p sccp tron_proof_helpers_property_no_panic_on_arbitrary_bytes -- --nocapture
    ;;
  *)
    echo "unsupported profile: ${PROFILE} (expected: fast|full)" >&2
    exit 1
    ;;
esac

if [[ "${WITH_KANI}" == "1" ]]; then
  if cargo kani --help >/dev/null 2>&1; then
    echo "[sccp-formal] cargo kani available; running bounded proof helpers"
    run_cmd cargo kani -p sccp --harness kani_burn_payload_roundtrip_bounded
    run_cmd cargo kani -p sccp --harness kani_burn_message_id_nonce_sensitivity_bounded
    run_cmd cargo kani -p sccp --harness kani_domain_separator_prefixes_bounded
  else
    echo "[sccp-formal] --with-kani requested but cargo kani is unavailable" >&2
    exit 1
  fi
fi

if [[ "${INCLUDE_SIBLINGS}" == "1" ]]; then
  run_cmd misc/sccp/run_formal_assisted_siblings.sh --profile "${PROFILE}"
fi

echo "[sccp-formal] OK"
