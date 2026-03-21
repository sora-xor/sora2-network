#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCCP_REPOS_DIR="${ROOT_DIR}/sccp/chains"
SCCP_ETH_DIR="${SCCP_REPOS_DIR}/eth"
SCCP_BSC_DIR="${SCCP_REPOS_DIR}/bsc"
SCCP_TRON_DIR="${SCCP_REPOS_DIR}/tron"
SCCP_SOL_DIR="${SCCP_REPOS_DIR}/sol"
SCCP_SOL_PROGRAM_DIR="${SCCP_SOL_DIR}/program"
SCCP_TON_DIR="${SCCP_REPOS_DIR}/ton"
SOLANA_TEST_RUST_LOG="${SOLANA_TEST_RUST_LOG:-warn}"
SCCP_SOL_PROGRAM_RETRIES="${SCCP_SOL_PROGRAM_RETRIES:-2}"
SCCP_SOL_PROGRAM_TEST_THREADS="${SCCP_SOL_PROGRAM_TEST_THREADS:-1}"
SCCP_SOL_PROGRAM_ALLOW_FAILURE="${SCCP_SOL_PROGRAM_ALLOW_FAILURE:-0}"
SCCP_SOL_PROGRAM_RETRY_DELAY_SECS="${SCCP_SOL_PROGRAM_RETRY_DELAY_SECS:-3}"
SCCP_SOL_PROGRAM_TIMEOUT_SECS="${SCCP_SOL_PROGRAM_TIMEOUT_SECS:-0}"
SCCP_SOL_PROGRAM_LOG_DIR="${SCCP_SOL_PROGRAM_LOG_DIR:-${ROOT_DIR}/misc/sccp/logs}"
SCCP_SOL_PROGRAM_LOG_TAIL_LINES="${SCCP_SOL_PROGRAM_LOG_TAIL_LINES:-120}"
SCCP_SOL_PROGRAM_PRESERVE_LOGS="${SCCP_SOL_PROGRAM_PRESERVE_LOGS:-1}"
SCCP_SOL_PROGRAM_NOCAPTURE="${SCCP_SOL_PROGRAM_NOCAPTURE:-0}"
SCCP_RUSTUP_TOOLCHAIN="${SCCP_RUSTUP_TOOLCHAIN:-${RUSTUP_TOOLCHAIN:-nightly-2025-05-08}}"
export RUSTUP_TOOLCHAIN="${SCCP_RUSTUP_TOOLCHAIN}"

require_positive_int() {
  local name="$1"
  local value="$2"
  if [[ ! "${value}" =~ ^[1-9][0-9]*$ ]]; then
    echo "${name} must be a positive integer (got: ${value})" >&2
    exit 1
  fi
}

require_bool_01() {
  local name="$1"
  local value="$2"
  if [[ "${value}" != "0" && "${value}" != "1" ]]; then
    echo "${name} must be 0 or 1 (got: ${value})" >&2
    exit 1
  fi
}

require_non_negative_int() {
  local name="$1"
  local value="$2"
  if [[ ! "${value}" =~ ^[0-9]+$ ]]; then
    echo "${name} must be a non-negative integer (got: ${value})" >&2
    exit 1
  fi
}

require_dir() {
  local dir="$1"
  if [[ ! -d "${dir}" ]]; then
    echo "missing required repo directory: ${dir}" >&2
    exit 1
  fi
}

require_positive_int "SCCP_SOL_PROGRAM_RETRIES" "${SCCP_SOL_PROGRAM_RETRIES}"
require_positive_int "SCCP_SOL_PROGRAM_TEST_THREADS" "${SCCP_SOL_PROGRAM_TEST_THREADS}"
require_bool_01 "SCCP_SOL_PROGRAM_ALLOW_FAILURE" "${SCCP_SOL_PROGRAM_ALLOW_FAILURE}"
require_bool_01 "SCCP_SOL_PROGRAM_PRESERVE_LOGS" "${SCCP_SOL_PROGRAM_PRESERVE_LOGS}"
require_bool_01 "SCCP_SOL_PROGRAM_NOCAPTURE" "${SCCP_SOL_PROGRAM_NOCAPTURE}"
require_non_negative_int "SCCP_SOL_PROGRAM_RETRY_DELAY_SECS" "${SCCP_SOL_PROGRAM_RETRY_DELAY_SECS}"
require_non_negative_int "SCCP_SOL_PROGRAM_TIMEOUT_SECS" "${SCCP_SOL_PROGRAM_TIMEOUT_SECS}"
require_positive_int "SCCP_SOL_PROGRAM_LOG_TAIL_LINES" "${SCCP_SOL_PROGRAM_LOG_TAIL_LINES}"

if [[ "${CI:-}" == "1" || "${CI:-}" == "true" ]]; then
  if [[ "${SCCP_SOL_PROGRAM_ALLOW_FAILURE}" == "1" ]]; then
    echo "SCCP_SOL_PROGRAM_ALLOW_FAILURE=1 is not allowed when CI=${CI}" >&2
    exit 1
  fi
fi

echo "[sccp-tests] RUSTUP_TOOLCHAIN=${RUSTUP_TOOLCHAIN}"

run_sccp_sol_program_tests() {
  local attempt=1
  local run_id
  run_id="$(date +%Y%m%d-%H%M%S)"
  local attempt_log=""
  local timeout_bin=""
  if (( SCCP_SOL_PROGRAM_TIMEOUT_SECS > 0 )); then
    if command -v timeout >/dev/null 2>&1; then
      timeout_bin="timeout"
    elif command -v gtimeout >/dev/null 2>&1; then
      timeout_bin="gtimeout"
    else
      echo "[sccp-sol/program] WARNING: timeout requested (${SCCP_SOL_PROGRAM_TIMEOUT_SECS}s) but neither timeout nor gtimeout is installed; continuing without timeout" >&2
    fi
  fi

  local -a cargo_args
  cargo_args=(cargo test -- --test-threads="${SCCP_SOL_PROGRAM_TEST_THREADS}")
  if [[ "${SCCP_SOL_PROGRAM_NOCAPTURE}" == "1" ]]; then
    cargo_args+=(--nocapture)
  fi

  local -a test_cmd
  if [[ -n "${timeout_bin}" ]]; then
    test_cmd=("${timeout_bin}" "${SCCP_SOL_PROGRAM_TIMEOUT_SECS}" "${cargo_args[@]}")
  else
    test_cmd=("${cargo_args[@]}")
  fi

  local repro_cmd_prefix=""
  if [[ -n "${timeout_bin}" ]]; then
    repro_cmd_prefix="${timeout_bin} ${SCCP_SOL_PROGRAM_TIMEOUT_SECS} "
  fi
  local nocapture_arg=""
  if [[ "${SCCP_SOL_PROGRAM_NOCAPTURE}" == "1" ]]; then
    nocapture_arg=" --nocapture"
  fi
  local repro_cmd="cd \"${SCCP_SOL_PROGRAM_DIR}\" && RUST_LOG=\"${SOLANA_TEST_RUST_LOG}\" ${repro_cmd_prefix}cargo test -- --test-threads=\"${SCCP_SOL_PROGRAM_TEST_THREADS}\"${nocapture_arg}"
  mkdir -p "${SCCP_SOL_PROGRAM_LOG_DIR}"
  while true; do
    attempt_log="${SCCP_SOL_PROGRAM_LOG_DIR}/sccp-sol-program.${run_id}.attempt-${attempt}.log"
    echo "[sccp-sol/program] cargo test (attempt ${attempt}/${SCCP_SOL_PROGRAM_RETRIES})"
    echo "[sccp-sol/program] logging to ${attempt_log}"
    if (cd "${SCCP_SOL_PROGRAM_DIR}" && RUST_LOG="${SOLANA_TEST_RUST_LOG}" "${test_cmd[@]}") 2>&1 | tee "${attempt_log}"; then
      if [[ "${SCCP_SOL_PROGRAM_PRESERVE_LOGS}" == "0" ]]; then
        rm -f "${attempt_log}"
      fi
      return 0
    else
      local status=$?
      if [[ "${status}" -eq 124 && -n "${timeout_bin}" ]]; then
        echo "[sccp-sol/program] attempt ${attempt} timed out after ${SCCP_SOL_PROGRAM_TIMEOUT_SECS}s" >&2
      fi
      echo "[sccp-sol/program] attempt ${attempt} failed; log: ${attempt_log}" >&2
      echo "[sccp-sol/program] last ${SCCP_SOL_PROGRAM_LOG_TAIL_LINES} lines from attempt ${attempt}:" >&2
      tail -n "${SCCP_SOL_PROGRAM_LOG_TAIL_LINES}" "${attempt_log}" >&2 || true
      local failed_test_name=""
      failed_test_name="$(sed -n 's/^---- \(.*\) stdout ----$/\1/p' "${attempt_log}" | head -n 1)"
      if [[ -n "${failed_test_name}" ]]; then
        local focused_repro_cmd="cd \"${SCCP_SOL_PROGRAM_DIR}\" && RUST_LOG=\"${SOLANA_TEST_RUST_LOG}\" cargo test \"${failed_test_name}\" -- --exact --test-threads=\"${SCCP_SOL_PROGRAM_TEST_THREADS}\"${nocapture_arg}"
        echo "[sccp-sol/program] focused rerun for first failed test: ${focused_repro_cmd}" >&2
      fi
    fi
    if (( attempt >= SCCP_SOL_PROGRAM_RETRIES )); then
      echo "[sccp-sol/program] reproduce with: ${repro_cmd}" >&2
      return 1
    fi
    attempt=$((attempt + 1))
    if (( SCCP_SOL_PROGRAM_RETRY_DELAY_SECS > 0 )); then
      echo "[sccp-sol/program] retrying after failure (sleep ${SCCP_SOL_PROGRAM_RETRY_DELAY_SECS}s)..."
      sleep "${SCCP_SOL_PROGRAM_RETRY_DELAY_SECS}"
    else
      echo "[sccp-sol/program] retrying after failure..."
    fi
  done
}

echo "[sora2-network] cargo test -p sccp"
(cd "${ROOT_DIR}" && cargo test -p sccp)

echo "[sora2-network] cargo test -p bridge-proxy"
(cd "${ROOT_DIR}" && cargo test -p bridge-proxy)

echo "[sora2-network] cargo test -p eth-bridge"
(cd "${ROOT_DIR}" && cargo test -p eth-bridge)

echo "[sora2-network] cargo test -p framenode-runtime sccp_ -- --nocapture"
(cd "${ROOT_DIR}" && cargo test -p framenode-runtime sccp_ -- --nocapture)

require_dir "${SCCP_ETH_DIR}"
echo "[sccp-eth] npm test"
(cd "${SCCP_ETH_DIR}" && npm test)

require_dir "${SCCP_BSC_DIR}"
echo "[sccp-bsc] npm test"
(cd "${SCCP_BSC_DIR}" && npm test)

require_dir "${SCCP_TRON_DIR}"
echo "[sccp-tron] npm test"
(cd "${SCCP_TRON_DIR}" && npm test)

require_dir "${SCCP_SOL_DIR}"
echo "[sccp-sol] cargo test"
(cd "${SCCP_SOL_DIR}" && cargo test)

require_dir "${SCCP_SOL_PROGRAM_DIR}"
if ! run_sccp_sol_program_tests; then
  if [[ "${SCCP_SOL_PROGRAM_ALLOW_FAILURE}" == "1" ]]; then
    echo "[sccp-sol/program] WARNING: tests failed after ${SCCP_SOL_PROGRAM_RETRIES} attempts; continuing because SCCP_SOL_PROGRAM_ALLOW_FAILURE=1" >&2
  else
    echo "[sccp-sol/program] ERROR: tests failed after ${SCCP_SOL_PROGRAM_RETRIES} attempts" >&2
    exit 1
  fi
fi

require_dir "${SCCP_TON_DIR}"
echo "[sccp-ton] npm test"
(cd "${SCCP_TON_DIR}" && npm test)

echo "OK"
