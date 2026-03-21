#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCCP_REPOS_DIR="${ROOT_DIR}/sccp/chains"

PROFILE="${SCCP_FUZZ_PROFILE:-full}"

# Align PATH across login/non-login shells so Homebrew and Foundry binaries are discoverable.
if [[ -d "/opt/homebrew/bin" ]]; then
  PATH="/opt/homebrew/bin:${PATH}"
fi
if [[ -d "/usr/local/bin" ]]; then
  PATH="/usr/local/bin:${PATH}"
fi
if [[ -d "${HOME}/.foundry/bin" ]]; then
  PATH="${HOME}/.foundry/bin:${PATH}"
fi
export PATH

while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile)
      PROFILE="$2"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      echo "usage: misc/sccp/run_fuzz_bounded_siblings.sh [--profile fast|full]" >&2
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
  echo "[sccp-fuzz-siblings] $*"
  "$@"
}

case "${PROFILE}" in
  fast)
    : "${SCCP_FUZZ_FASTCHECK_RUNS:=200}"
    : "${SCCP_FUZZ_FOUNDRY_RUNS:=3000}"
    : "${SCCP_FUZZ_ECHIDNA_TIMEOUT_SECS:=300}"
    : "${SCCP_SOL_FUZZ_BURN_SECONDS:=120}"
    : "${SCCP_SOL_FUZZ_ATTEST_SECONDS:=120}"
    : "${SCCP_SOL_FUZZ_VERIFIER_SECONDS:=120}"
    export SCCP_FUZZ_FASTCHECK_RUNS SCCP_FUZZ_FOUNDRY_RUNS SCCP_FUZZ_ECHIDNA_TIMEOUT_SECS
    export SCCP_SOL_FUZZ_BURN_SECONDS SCCP_SOL_FUZZ_ATTEST_SECONDS SCCP_SOL_FUZZ_VERIFIER_SECONDS
    ;;
  full)
    : "${SCCP_FUZZ_FASTCHECK_RUNS:=1000}"
    : "${SCCP_FUZZ_FOUNDRY_RUNS:=12000}"
    : "${SCCP_FUZZ_ECHIDNA_TIMEOUT_SECS:=1200}"
    : "${SCCP_SOL_FUZZ_BURN_SECONDS:=1200}"
    : "${SCCP_SOL_FUZZ_ATTEST_SECONDS:=1200}"
    : "${SCCP_SOL_FUZZ_VERIFIER_SECONDS:=1200}"
    export SCCP_FUZZ_FASTCHECK_RUNS SCCP_FUZZ_FOUNDRY_RUNS SCCP_FUZZ_ECHIDNA_TIMEOUT_SECS
    export SCCP_SOL_FUZZ_BURN_SECONDS SCCP_SOL_FUZZ_ATTEST_SECONDS SCCP_SOL_FUZZ_VERIFIER_SECONDS
    ;;
  *)
    echo "unsupported profile: ${PROFILE} (expected: fast|full)" >&2
    exit 1
    ;;
esac

echo "[sccp-fuzz-siblings] profile=${PROFILE} fastcheck_runs=${SCCP_FUZZ_FASTCHECK_RUNS} foundry_runs=${SCCP_FUZZ_FOUNDRY_RUNS} echidna_timeout=${SCCP_FUZZ_ECHIDNA_TIMEOUT_SECS}"
echo "[sccp-fuzz-siblings] sol_burn_seconds=${SCCP_SOL_FUZZ_BURN_SECONDS} sol_attest_seconds=${SCCP_SOL_FUZZ_ATTEST_SECONDS} sol_verifier_seconds=${SCCP_SOL_FUZZ_VERIFIER_SECONDS}"

require_dir "${SCCP_REPOS_DIR}/eth"
run_cmd bash -lc "cd '${SCCP_REPOS_DIR}/eth' && npm run test:fuzz"

require_dir "${SCCP_REPOS_DIR}/bsc"
run_cmd bash -lc "cd '${SCCP_REPOS_DIR}/bsc' && npm run test:fuzz"

require_dir "${SCCP_REPOS_DIR}/tron"
run_cmd bash -lc "cd '${SCCP_REPOS_DIR}/tron' && npm run test:fuzz"

require_dir "${SCCP_REPOS_DIR}/ton"
run_cmd bash -lc "cd '${SCCP_REPOS_DIR}/ton' && npm run test:fuzz"

require_dir "${SCCP_REPOS_DIR}/sol"
run_cmd bash -lc "cd '${SCCP_REPOS_DIR}/sol' && ./scripts/run_fuzz_bounded.sh --profile '${PROFILE}'"

echo "[sccp-fuzz-siblings] OK"
