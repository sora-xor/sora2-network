#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FUZZ_DIR="${ROOT_DIR}/fuzz"

PROFILE="full"
AUTO_INSTALL="0"
TOOLCHAIN="nightly"
BURN_SECONDS=""
ATTEST_SECONDS=""
VERIFIER_SECONDS=""

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
    --toolchain)
      TOOLCHAIN="$2"
      shift 2
      ;;
    --burn-seconds)
      BURN_SECONDS="$2"
      shift 2
      ;;
    --attest-seconds)
      ATTEST_SECONDS="$2"
      shift 2
      ;;
    --verifier-seconds)
      VERIFIER_SECONDS="$2"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      echo "usage: scripts/run_fuzz_bounded.sh [--profile fast|full] [--auto-install] [--toolchain name] [--burn-seconds N] [--attest-seconds N] [--verifier-seconds N]" >&2
      exit 1
      ;;
  esac
done

case "${PROFILE}" in
  fast)
    DEFAULT_BURN_SECONDS=60
    DEFAULT_ATTEST_SECONDS=60
    DEFAULT_VERIFIER_SECONDS=60
    ;;
  full)
    DEFAULT_BURN_SECONDS=1200
    DEFAULT_ATTEST_SECONDS=1200
    DEFAULT_VERIFIER_SECONDS=1200
    ;;
  *)
    echo "unsupported profile: ${PROFILE} (expected: fast|full)" >&2
    exit 1
    ;;
esac

if [[ -z "${BURN_SECONDS}" ]]; then
  BURN_SECONDS="${DEFAULT_BURN_SECONDS}"
fi
if [[ -z "${ATTEST_SECONDS}" ]]; then
  ATTEST_SECONDS="${DEFAULT_ATTEST_SECONDS}"
fi
if [[ -z "${VERIFIER_SECONDS}" ]]; then
  VERIFIER_SECONDS="${DEFAULT_VERIFIER_SECONDS}"
fi

require_positive_int() {
  local name="$1"
  local value="$2"
  if [[ ! "${value}" =~ ^[1-9][0-9]*$ ]]; then
    echo "${name} must be a positive integer (got: ${value})" >&2
    exit 1
  fi
}

require_positive_int "burn-seconds" "${BURN_SECONDS}"
require_positive_int "attest-seconds" "${ATTEST_SECONDS}"
require_positive_int "verifier-seconds" "${VERIFIER_SECONDS}"

if ! command -v rustup >/dev/null 2>&1; then
  echo "[sccp-sol-fuzz] rustup is required to run cargo with an explicit toolchain" >&2
  exit 1
fi

if [[ "${AUTO_INSTALL}" == "1" ]]; then
  rustup toolchain install "${TOOLCHAIN}" --profile minimal
fi

CARGO_CMD=(rustup run "${TOOLCHAIN}" cargo)

if ! "${CARGO_CMD[@]}" --version >/dev/null 2>&1; then
  echo "[sccp-sol-fuzz] cargo is unavailable for toolchain: ${TOOLCHAIN}" >&2
  exit 1
fi

if ! "${CARGO_CMD[@]}" fuzz --help >/dev/null 2>&1; then
  if [[ "${AUTO_INSTALL}" == "1" ]]; then
    echo "[sccp-sol-fuzz] cargo-fuzz is missing; installing because --auto-install was requested"
    "${CARGO_CMD[@]}" install cargo-fuzz
  else
    echo "[sccp-sol-fuzz] cargo-fuzz is required but not installed for toolchain ${TOOLCHAIN}" >&2
    echo "[sccp-sol-fuzz] rerun with: scripts/run_fuzz_bounded.sh --auto-install" >&2
    exit 1
  fi
fi

echo "[sccp-sol-fuzz] profile=${PROFILE} burn=${BURN_SECONDS}s attest=${ATTEST_SECONDS}s verifier=${VERIFIER_SECONDS}s"
echo "[sccp-sol-fuzz] toolchain=${TOOLCHAIN}"

echo "[sccp-sol-fuzz] cargo fuzz run burn_codec"
(cd "${FUZZ_DIR}" && "${CARGO_CMD[@]}" fuzz run burn_codec -- "-max_total_time=${BURN_SECONDS}")

echo "[sccp-sol-fuzz] cargo fuzz run attest_hash"
(cd "${FUZZ_DIR}" && "${CARGO_CMD[@]}" fuzz run attest_hash -- "-max_total_time=${ATTEST_SECONDS}")

echo "[sccp-sol-fuzz] cargo fuzz run verifier_borsh_decode"
(cd "${FUZZ_DIR}" && "${CARGO_CMD[@]}" fuzz run verifier_borsh_decode -- "-max_total_time=${VERIFIER_SECONDS}")

echo "[sccp-sol-fuzz] OK"
