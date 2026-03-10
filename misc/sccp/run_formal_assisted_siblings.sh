#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEV_DIR="${SCCP_DEV_DIR:-$(cd "${ROOT_DIR}/.." && pwd)}"

PROFILE="${SCCP_FORMAL_PROFILE:-full}"
SCCP_RUSTUP_TOOLCHAIN="${SCCP_RUSTUP_TOOLCHAIN:-${RUSTUP_TOOLCHAIN:-nightly-2025-05-08}}"
export RUSTUP_TOOLCHAIN="${SCCP_RUSTUP_TOOLCHAIN}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile)
      PROFILE="$2"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      echo "usage: misc/sccp/run_formal_assisted_siblings.sh [--profile fast|full]" >&2
      exit 1
      ;;
  esac
done

require_dir() {
  local dir="$1"
  if [[ ! -d "${dir}" ]]; then
    echo "missing required repo directory: ${dir}" >&2
    exit 1
  fi
}

run_cmd() {
  echo "[sccp-formal-siblings] $*"
  "$@"
}

case "${PROFILE}" in
  fast|full)
    ;;
  *)
    echo "unsupported profile: ${PROFILE} (expected: fast|full)" >&2
    exit 1
    ;;
esac

echo "[sccp-formal-siblings] RUSTUP_TOOLCHAIN=${RUSTUP_TOOLCHAIN}"
echo "[sccp-formal-siblings] profile=${PROFILE}"
export SCCP_FORMAL_PROFILE="${PROFILE}"

require_dir "${DEV_DIR}/sccp-eth"
run_cmd bash -lc "cd '${DEV_DIR}/sccp-eth' && npm run test:formal-assisted"

require_dir "${DEV_DIR}/sccp-bsc"
run_cmd bash -lc "cd '${DEV_DIR}/sccp-bsc' && npm run test:formal-assisted"

require_dir "${DEV_DIR}/sccp-tron"
run_cmd bash -lc "cd '${DEV_DIR}/sccp-tron' && npm run test:formal-assisted"

require_dir "${DEV_DIR}/sccp-ton"
run_cmd bash -lc "cd '${DEV_DIR}/sccp-ton' && npm run test:formal-assisted"

require_dir "${DEV_DIR}/sccp-sol"
if [[ "${PROFILE}" == "fast" ]]; then
  run_cmd bash -lc "cd '${DEV_DIR}/sccp-sol' && cargo test formal_assisted_burn_payload_roundtrip_bounded -- --nocapture"
  run_cmd bash -lc "cd '${DEV_DIR}/sccp-sol' && cargo test formal_assisted_message_id_and_attest_hash_sensitivity_bounded -- --nocapture"
  run_cmd bash -lc "cd '${DEV_DIR}/sccp-sol' && cargo test formal_assisted_prefix_literals_remain_stable -- --nocapture"
else
  run_cmd bash -lc "cd '${DEV_DIR}/sccp-sol' && cargo test formal_assisted_ -- --nocapture"
fi

echo "[sccp-formal-siblings] OK"
