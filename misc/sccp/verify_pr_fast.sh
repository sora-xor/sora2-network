#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

ARTIFACTS_BASE="${SCCP_VERIFY_PR_ARTIFACTS_BASE:-${ROOT_DIR}/misc/sccp/artifacts/pr-fast}"
RUN_ID="${SCCP_VERIFY_PR_RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
SMOKE_CONFIG="${SCCP_VERIFY_PR_SMOKE_CONFIG:-}"
SMOKE_MODE="${SCCP_VERIFY_PR_SMOKE_MODE:-}"
FAIL_FAST="${SCCP_VERIFY_PR_FAIL_FAST:-0}"
SCCP_RUSTUP_TOOLCHAIN="${SCCP_VERIFY_PR_RUSTUP_TOOLCHAIN:-${SCCP_RUSTUP_TOOLCHAIN:-${RUSTUP_TOOLCHAIN:-nightly-2025-05-08}}}"
export RUSTUP_TOOLCHAIN="${SCCP_RUSTUP_TOOLCHAIN}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --artifacts-base)
      ARTIFACTS_BASE="$2"
      shift 2
      ;;
    --run-id)
      RUN_ID="$2"
      shift 2
      ;;
    --smoke-config)
      SMOKE_CONFIG="$2"
      shift 2
      ;;
    --smoke-mode)
      SMOKE_MODE="$2"
      shift 2
      ;;
    --fail-fast)
      FAIL_FAST="1"
      shift
      ;;
    *)
      echo "unknown argument: $1" >&2
      echo "usage: misc/sccp/verify_pr_fast.sh [--artifacts-base PATH] [--run-id ID] [--smoke-config PATH] [--smoke-mode pr|local] [--fail-fast]" >&2
      exit 1
      ;;
  esac
done

require_bool_01() {
  local name="$1"
  local value="$2"
  if [[ "${value}" != "0" && "${value}" != "1" ]]; then
    echo "${name} must be 0 or 1 (got: ${value})" >&2
    exit 1
  fi
}

json_escape() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//$'\n'/\\n}"
  value="${value//$'\r'/\\r}"
  value="${value//$'\t'/\\t}"
  printf '%s' "${value}"
}

xml_escape() {
  local value="$1"
  value="${value//&/&amp;}"
  value="${value//</&lt;}"
  value="${value//>/&gt;}"
  value="${value//\"/&quot;}"
  value="${value//\'/&apos;}"
  printf '%s' "${value}"
}

require_bool_01 "SCCP_VERIFY_PR_FAIL_FAST" "${FAIL_FAST}"

if [[ -z "${SMOKE_CONFIG}" ]]; then
  if [[ -d "${ROOT_DIR}/sccp/chains/eth" ]]; then
    SMOKE_CONFIG="${ROOT_DIR}/misc/sccp-e2e/config.ci.json"
    : "${SMOKE_MODE:=pr}"
  else
    SMOKE_CONFIG="${ROOT_DIR}/misc/sccp-e2e/config.local.json"
    : "${SMOKE_MODE:=local}"
  fi
fi

if [[ "${SMOKE_MODE}" != "pr" && "${SMOKE_MODE}" != "local" ]]; then
  echo "SMOKE_MODE must be 'pr' or 'local' (got: ${SMOKE_MODE})" >&2
  exit 1
fi

if [[ ! -f "${SMOKE_CONFIG}" ]]; then
  echo "smoke config not found: ${SMOKE_CONFIG}" >&2
  exit 1
fi

RUN_DIR="${ARTIFACTS_BASE}/${RUN_ID}"
LOG_DIR="${RUN_DIR}/logs"
HUB_ARTIFACTS_DIR="${RUN_DIR}/hub-smoke"
mkdir -p "${LOG_DIR}"

RUN_STARTED_EPOCH="$(date +%s)"
RUN_STARTED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

declare -a STAGE_NAMES
declare -a STAGE_COMMANDS
declare -a STAGE_STATUSES
declare -a STAGE_EXIT_CODES
declare -a STAGE_STARTED_ATS
declare -a STAGE_FINISHED_ATS
declare -a STAGE_DURATIONS
declare -a STAGE_LOGS

run_stage() {
  local stage_name="$1"
  shift
  local -a cmd=("$@")

  local idx="${#STAGE_NAMES[@]}"
  local display_idx
  display_idx="$(printf "%02d" "$((idx + 1))")"
  local stage_log="${LOG_DIR}/${display_idx}-${stage_name}.log"

  local cmd_repr=""
  printf -v cmd_repr '%q ' "${cmd[@]}"
  cmd_repr="${cmd_repr% }"

  local stage_started_epoch
  stage_started_epoch="$(date +%s)"
  local stage_started_at
  stage_started_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

  echo "[verify-pr-fast] stage=${stage_name} command=${cmd_repr}"

  set +e
  (
    set -o pipefail
    cd "${ROOT_DIR}"
    "${cmd[@]}"
  ) 2>&1 | tee "${stage_log}"
  local stage_exit="${PIPESTATUS[0]}"
  set -e

  local stage_finished_epoch
  stage_finished_epoch="$(date +%s)"
  local stage_finished_at
  stage_finished_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  local stage_duration="$((stage_finished_epoch - stage_started_epoch))"

  local stage_status="passed"
  if [[ "${stage_exit}" -ne 0 ]]; then
    stage_status="failed"
  fi

  STAGE_NAMES+=("${stage_name}")
  STAGE_COMMANDS+=("${cmd_repr}")
  STAGE_STATUSES+=("${stage_status}")
  STAGE_EXIT_CODES+=("${stage_exit}")
  STAGE_STARTED_ATS+=("${stage_started_at}")
  STAGE_FINISHED_ATS+=("${stage_finished_at}")
  STAGE_DURATIONS+=("${stage_duration}")
  STAGE_LOGS+=("${stage_log}")

  if [[ "${stage_status}" == "failed" ]]; then
    echo "[verify-pr-fast] stage=${stage_name} failed (exit=${stage_exit})"
    if [[ "${FAIL_FAST}" == "1" ]]; then
      return 1
    fi
  else
    echo "[verify-pr-fast] stage=${stage_name} passed"
  fi

  return 0
}

run_stage "sccp_critical_tests" bash -lc "cargo test -p sccp && cargo test -p bridge-proxy && cargo test -p eth-bridge && cargo test -p framenode-runtime sccp_ -- --nocapture"
run_stage "formal_fast" bash -lc "SCCP_FORMAL_INCLUDE_SIBLINGS=0 misc/sccp/run_formal_assisted.sh --profile fast"
run_stage "sibling_smoke" "misc/sccp-e2e/run_hub_matrix.sh" "--config" "${SMOKE_CONFIG}" "--mode" "${SMOKE_MODE}" "--artifacts-dir" "${HUB_ARTIFACTS_DIR}" "--skip-preflight" "--scenario" "sora:eth" "--exclude-negative"

RUN_FINISHED_EPOCH="$(date +%s)"
RUN_FINISHED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
RUN_DURATION="$((RUN_FINISHED_EPOCH - RUN_STARTED_EPOCH))"

OVERALL_STATUS="passed"
FAILED_STAGE_COUNT=0
for st in "${STAGE_STATUSES[@]}"; do
  if [[ "${st}" != "passed" ]]; then
    OVERALL_STATUS="failed"
    FAILED_STAGE_COUNT="$((FAILED_STAGE_COUNT + 1))"
  fi
done

SUMMARY_PATH="${RUN_DIR}/summary.json"
JUNIT_PATH="${RUN_DIR}/junit.xml"

{
  echo "{"
  echo "  \"run_id\": \"$(json_escape "${RUN_ID}")\","
  echo "  \"status\": \"${OVERALL_STATUS}\","
  echo "  \"started_at\": \"${RUN_STARTED_AT}\","
  echo "  \"finished_at\": \"${RUN_FINISHED_AT}\","
  echo "  \"duration_seconds\": ${RUN_DURATION},"
  echo "  \"artifacts_dir\": \"$(json_escape "${RUN_DIR}")\","
  echo "  \"config\": {"
  echo "    \"smoke_config\": \"$(json_escape "${SMOKE_CONFIG}")\","
  echo "    \"smoke_mode\": \"$(json_escape "${SMOKE_MODE}")\","
  echo "    \"fail_fast\": ${FAIL_FAST}"
  echo "  },"
  echo "  \"summary\": {"
  echo "    \"total_stages\": ${#STAGE_NAMES[@]},"
  echo "    \"failed_stages\": ${FAILED_STAGE_COUNT},"
  echo "    \"passed_stages\": $(( ${#STAGE_NAMES[@]} - FAILED_STAGE_COUNT ))"
  echo "  },"
  echo "  \"stages\": ["
  for i in "${!STAGE_NAMES[@]}"; do
    comma=","
    if [[ "${i}" -eq "$(( ${#STAGE_NAMES[@]} - 1 ))" ]]; then
      comma=""
    fi
    echo "    {"
    echo "      \"name\": \"$(json_escape "${STAGE_NAMES[$i]}")\","
    echo "      \"status\": \"$(json_escape "${STAGE_STATUSES[$i]}")\","
    echo "      \"exit_code\": ${STAGE_EXIT_CODES[$i]},"
    echo "      \"command\": \"$(json_escape "${STAGE_COMMANDS[$i]}")\","
    echo "      \"started_at\": \"$(json_escape "${STAGE_STARTED_ATS[$i]}")\","
    echo "      \"finished_at\": \"$(json_escape "${STAGE_FINISHED_ATS[$i]}")\","
    echo "      \"duration_seconds\": ${STAGE_DURATIONS[$i]},"
    echo "      \"log\": \"$(json_escape "${STAGE_LOGS[$i]}")\""
    echo "    }${comma}"
  done
  echo "  ]"
  echo "}"
} > "${SUMMARY_PATH}"

{
  echo '<?xml version="1.0" encoding="UTF-8"?>'
  echo "<testsuite name=\"sccp-pr-fast-verify\" tests=\"${#STAGE_NAMES[@]}\" failures=\"${FAILED_STAGE_COUNT}\">"
  for i in "${!STAGE_NAMES[@]}"; do
    stage_name_escaped="$(xml_escape "${STAGE_NAMES[$i]}")"
    echo "  <testcase classname=\"sccp.pr_fast\" name=\"${stage_name_escaped}\">"
    if [[ "${STAGE_STATUSES[$i]}" != "passed" ]]; then
      failure_message="stage ${STAGE_NAMES[$i]} failed (exit=${STAGE_EXIT_CODES[$i]})"
      failure_details="command=${STAGE_COMMANDS[$i]} log=${STAGE_LOGS[$i]}"
      echo "    <failure message=\"$(xml_escape "${failure_message}")\">$(xml_escape "${failure_details}")</failure>"
    fi
    echo "  </testcase>"
  done
  echo "</testsuite>"
} > "${JUNIT_PATH}"

echo "[verify-pr-fast] summary: ${SUMMARY_PATH}"
echo "[verify-pr-fast] junit: ${JUNIT_PATH}"
echo "[verify-pr-fast] artifacts: ${RUN_DIR}"

if [[ "${OVERALL_STATUS}" != "passed" ]]; then
  exit 1
fi

echo "[verify-pr-fast] OK"
