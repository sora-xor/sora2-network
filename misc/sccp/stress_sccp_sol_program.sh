#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEV_DIR="$(cd "${ROOT_DIR}/.." && pwd)"
PROGRAM_DIR="${DEV_DIR}/sccp-sol/program"

SCCP_SOL_STRESS_RUNS="${SCCP_SOL_STRESS_RUNS:-20}"
SCCP_SOL_STRESS_TEST_THREADS="${SCCP_SOL_STRESS_TEST_THREADS:-1}"
SCCP_SOL_STRESS_DELAY_SECS="${SCCP_SOL_STRESS_DELAY_SECS:-2}"
SCCP_SOL_STRESS_TIMEOUT_SECS="${SCCP_SOL_STRESS_TIMEOUT_SECS:-0}"
SCCP_SOL_STRESS_RUST_LOG="${SCCP_SOL_STRESS_RUST_LOG:-warn}"
SCCP_SOL_STRESS_LOG_DIR="${SCCP_SOL_STRESS_LOG_DIR:-${ROOT_DIR}/misc/sccp/logs/stress}"
SCCP_SOL_STRESS_LOG_TAIL_LINES="${SCCP_SOL_STRESS_LOG_TAIL_LINES:-120}"
SCCP_SOL_STRESS_PRESERVE_PASS_LOGS="${SCCP_SOL_STRESS_PRESERVE_PASS_LOGS:-0}"
SCCP_SOL_STRESS_STOP_ON_FAILURE="${SCCP_SOL_STRESS_STOP_ON_FAILURE:-0}"
SCCP_SOL_STRESS_ALLOW_FAILURE="${SCCP_SOL_STRESS_ALLOW_FAILURE:-0}"
SCCP_SOL_STRESS_NOCAPTURE="${SCCP_SOL_STRESS_NOCAPTURE:-0}"
SCCP_SOL_STRESS_TEST_FILTER="${SCCP_SOL_STRESS_TEST_FILTER:-}"

require_positive_int() {
  local name="$1"
  local value="$2"
  if [[ ! "${value}" =~ ^[1-9][0-9]*$ ]]; then
    echo "${name} must be a positive integer (got: ${value})" >&2
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

require_bool_01() {
  local name="$1"
  local value="$2"
  if [[ "${value}" != "0" && "${value}" != "1" ]]; then
    echo "${name} must be 0 or 1 (got: ${value})" >&2
    exit 1
  fi
}

require_positive_int "SCCP_SOL_STRESS_RUNS" "${SCCP_SOL_STRESS_RUNS}"
require_positive_int "SCCP_SOL_STRESS_TEST_THREADS" "${SCCP_SOL_STRESS_TEST_THREADS}"
require_non_negative_int "SCCP_SOL_STRESS_DELAY_SECS" "${SCCP_SOL_STRESS_DELAY_SECS}"
require_non_negative_int "SCCP_SOL_STRESS_TIMEOUT_SECS" "${SCCP_SOL_STRESS_TIMEOUT_SECS}"
require_positive_int "SCCP_SOL_STRESS_LOG_TAIL_LINES" "${SCCP_SOL_STRESS_LOG_TAIL_LINES}"
require_bool_01 "SCCP_SOL_STRESS_PRESERVE_PASS_LOGS" "${SCCP_SOL_STRESS_PRESERVE_PASS_LOGS}"
require_bool_01 "SCCP_SOL_STRESS_STOP_ON_FAILURE" "${SCCP_SOL_STRESS_STOP_ON_FAILURE}"
require_bool_01 "SCCP_SOL_STRESS_ALLOW_FAILURE" "${SCCP_SOL_STRESS_ALLOW_FAILURE}"
require_bool_01 "SCCP_SOL_STRESS_NOCAPTURE" "${SCCP_SOL_STRESS_NOCAPTURE}"

if [[ ! -d "${PROGRAM_DIR}" ]]; then
  echo "missing required repo directory: ${PROGRAM_DIR}" >&2
  exit 1
fi

timeout_bin=""
if (( SCCP_SOL_STRESS_TIMEOUT_SECS > 0 )); then
  if command -v timeout >/dev/null 2>&1; then
    timeout_bin="timeout"
  elif command -v gtimeout >/dev/null 2>&1; then
    timeout_bin="gtimeout"
  else
    echo "[stress] WARNING: timeout requested (${SCCP_SOL_STRESS_TIMEOUT_SECS}s) but neither timeout nor gtimeout is installed; continuing without timeout" >&2
  fi
fi

run_id="$(date +%Y%m%d-%H%M%S)"
mkdir -p "${SCCP_SOL_STRESS_LOG_DIR}"
summary_file="${SCCP_SOL_STRESS_LOG_DIR}/sccp-sol-program-stress.${run_id}.summary.txt"

passes=0
fails=0
declare -a failed_iterations=()
declare -a failure_details=()
start_epoch="$(date +%s)"

echo "[stress] run_id=${run_id}"
echo "[stress] program=${PROGRAM_DIR}"
echo "[stress] runs=${SCCP_SOL_STRESS_RUNS} test_threads=${SCCP_SOL_STRESS_TEST_THREADS} delay_secs=${SCCP_SOL_STRESS_DELAY_SECS} timeout_secs=${SCCP_SOL_STRESS_TIMEOUT_SECS} nocapture=${SCCP_SOL_STRESS_NOCAPTURE}"
if [[ -n "${SCCP_SOL_STRESS_TEST_FILTER}" ]]; then
  echo "[stress] test_filter=${SCCP_SOL_STRESS_TEST_FILTER}"
fi

for ((iter = 1; iter <= SCCP_SOL_STRESS_RUNS; iter++)); do
  iter_log="${SCCP_SOL_STRESS_LOG_DIR}/sccp-sol-program-stress.${run_id}.iter-${iter}.log"
  echo "[stress] iteration ${iter}/${SCCP_SOL_STRESS_RUNS}"
  echo "[stress] logging to ${iter_log}"

  cargo_args=(cargo test)
  if [[ -n "${SCCP_SOL_STRESS_TEST_FILTER}" ]]; then
    cargo_args+=("${SCCP_SOL_STRESS_TEST_FILTER}")
  fi
  cargo_args+=(-- --test-threads="${SCCP_SOL_STRESS_TEST_THREADS}")
  if [[ "${SCCP_SOL_STRESS_NOCAPTURE}" == "1" ]]; then
    cargo_args+=(--nocapture)
  fi

  cmd=("${cargo_args[@]}")
  if [[ -n "${timeout_bin}" ]]; then
    cmd=("${timeout_bin}" "${SCCP_SOL_STRESS_TIMEOUT_SECS}" "${cargo_args[@]}")
  fi

  if (cd "${PROGRAM_DIR}" && RUST_LOG="${SCCP_SOL_STRESS_RUST_LOG}" "${cmd[@]}") 2>&1 | tee "${iter_log}"; then
    passes=$((passes + 1))
    if [[ "${SCCP_SOL_STRESS_PRESERVE_PASS_LOGS}" == "0" ]]; then
      rm -f "${iter_log}"
    fi
  else
    status=$?
    fails=$((fails + 1))
    failed_iterations+=("${iter}")
    echo "[stress] iteration ${iter} failed with status ${status}; log: ${iter_log}" >&2
    if [[ "${status}" -eq 124 && -n "${timeout_bin}" ]]; then
      echo "[stress] iteration ${iter} timed out after ${SCCP_SOL_STRESS_TIMEOUT_SECS}s" >&2
    fi
    echo "[stress] tail (${SCCP_SOL_STRESS_LOG_TAIL_LINES} lines) for iteration ${iter}:" >&2
    tail -n "${SCCP_SOL_STRESS_LOG_TAIL_LINES}" "${iter_log}" >&2 || true
    failed_test_name="$(sed -n 's/^---- \(.*\) stdout ----$/\1/p' "${iter_log}" | head -n 1)"
    failure_signature="$(
      (
        grep -m1 'TransactionError(' "${iter_log}" \
          || grep -m1 'panicked at' "${iter_log}" \
          || grep -m1 'error: test failed' "${iter_log}" \
          || true
      ) | sed 's/^[[:space:]]*//'
    )"
    if [[ -z "${failed_test_name}" ]]; then
      failed_test_name="unknown"
    fi
    if [[ -z "${failure_signature}" ]]; then
      failure_signature="unknown"
    fi
    failure_details+=("iter=${iter};test=${failed_test_name};signature=${failure_signature}")
    if [[ "${failed_test_name}" != "unknown" ]]; then
      focused_cmd="cd \"${PROGRAM_DIR}\" && RUST_LOG=\"${SCCP_SOL_STRESS_RUST_LOG}\" cargo test \"${failed_test_name}\" -- --exact --test-threads=\"${SCCP_SOL_STRESS_TEST_THREADS}\""
      if [[ "${SCCP_SOL_STRESS_NOCAPTURE}" == "1" ]]; then
        focused_cmd="${focused_cmd} --nocapture"
      fi
      echo "[stress] focused rerun: ${focused_cmd}" >&2
    fi
    if [[ "${SCCP_SOL_STRESS_STOP_ON_FAILURE}" == "1" ]]; then
      echo "[stress] stopping on first failure (SCCP_SOL_STRESS_STOP_ON_FAILURE=1)" >&2
      break
    fi
  fi

  if (( iter < SCCP_SOL_STRESS_RUNS )) && (( SCCP_SOL_STRESS_DELAY_SECS > 0 )); then
    sleep "${SCCP_SOL_STRESS_DELAY_SECS}"
  fi
done

end_epoch="$(date +%s)"
duration_secs=$((end_epoch - start_epoch))

{
  echo "run_id=${run_id}"
  echo "program_dir=${PROGRAM_DIR}"
  echo "runs_requested=${SCCP_SOL_STRESS_RUNS}"
  echo "passes=${passes}"
  echo "fails=${fails}"
  echo "failed_iterations=${failed_iterations[*]:-none}"
  if (( ${#failure_details[@]} > 0 )); then
    echo "failure_details:"
    for detail in "${failure_details[@]}"; do
      echo "  - ${detail}"
    done
  else
    echo "failure_details=none"
  fi
  echo "duration_secs=${duration_secs}"
  echo "log_dir=${SCCP_SOL_STRESS_LOG_DIR}"
} > "${summary_file}"

echo "[stress] summary written: ${summary_file}"
echo "[stress] passes=${passes} fails=${fails} duration_secs=${duration_secs}"
if (( fails > 0 )); then
  if [[ "${SCCP_SOL_STRESS_ALLOW_FAILURE}" == "1" ]]; then
    echo "[stress] WARNING: failures observed; continuing because SCCP_SOL_STRESS_ALLOW_FAILURE=1" >&2
    exit 0
  fi
  exit 1
fi

exit 0
