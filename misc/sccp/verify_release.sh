#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

ARTIFACTS_BASE="${SCCP_VERIFY_ARTIFACTS_BASE:-${ROOT_DIR}/misc/sccp/artifacts}"
RUN_ID="${SCCP_VERIFY_RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"

MATRIX_MODE="${SCCP_VERIFY_MATRIX_MODE:-full}"
MATRIX_MAX_MINUTES="${SCCP_VERIFY_MATRIX_MAX_MINUTES:-120}"
STRICT_ADAPTERS="${SCCP_VERIFY_STRICT_ADAPTERS:-1}"
INCLUDE_NEGATIVE="${SCCP_VERIFY_INCLUDE_NEGATIVE:-1}"
DISABLE_HUB_CACHE="${SCCP_VERIFY_DISABLE_HUB_CACHE:-1}"
HUB_CONFIG="${SCCP_VERIFY_HUB_CONFIG:-${ROOT_DIR}/misc/sccp-e2e/config.release-shadow.json}"
HUB_MODE="${SCCP_VERIFY_HUB_MODE:-release}"
SKIP_PREFLIGHT="${SCCP_VERIFY_SKIP_PREFLIGHT:-0}"
FUZZ_PROFILE="${SCCP_VERIFY_FUZZ_PROFILE:-full}"
FORMAL_PROFILE="${SCCP_VERIFY_FORMAL_PROFILE:-full}"
CANARY_SOAK_PROFILE="${SCCP_VERIFY_CANARY_SOAK_PROFILE:-full}"
INCLUDE_CANARY_SOAK="${SCCP_VERIFY_INCLUDE_CANARY_SOAK:-1}"
REQUIRE_CLEAN_TREE="${SCCP_VERIFY_REQUIRE_CLEAN_TREE:-1}"
FAIL_FAST="${SCCP_VERIFY_FAIL_FAST:-0}"
SCCP_RUSTUP_TOOLCHAIN="${SCCP_VERIFY_RUSTUP_TOOLCHAIN:-${SCCP_RUSTUP_TOOLCHAIN:-${RUSTUP_TOOLCHAIN:-nightly-2025-05-08}}}"
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
    --hub-config)
      HUB_CONFIG="$2"
      shift 2
      ;;
    --hub-mode)
      HUB_MODE="$2"
      shift 2
      ;;
    --matrix)
      MATRIX_MODE="$2"
      shift 2
      ;;
    --max-minutes)
      MATRIX_MAX_MINUTES="$2"
      shift 2
      ;;
    --fuzz-profile)
      FUZZ_PROFILE="$2"
      shift 2
      ;;
    --formal-profile)
      FORMAL_PROFILE="$2"
      shift 2
      ;;
    --canary-soak-profile)
      CANARY_SOAK_PROFILE="$2"
      shift 2
      ;;
    --strict-adapters)
      STRICT_ADAPTERS="1"
      shift
      ;;
    --no-strict-adapters)
      STRICT_ADAPTERS="0"
      shift
      ;;
    --include-negative)
      INCLUDE_NEGATIVE="1"
      shift
      ;;
    --exclude-negative)
      INCLUDE_NEGATIVE="0"
      shift
      ;;
    --disable-hub-cache)
      DISABLE_HUB_CACHE="1"
      shift
      ;;
    --enable-hub-cache)
      DISABLE_HUB_CACHE="0"
      shift
      ;;
    --fail-fast)
      FAIL_FAST="1"
      shift
      ;;
    --skip-preflight)
      SKIP_PREFLIGHT="1"
      shift
      ;;
    --run-preflight)
      SKIP_PREFLIGHT="0"
      shift
      ;;
    --include-canary-soak)
      INCLUDE_CANARY_SOAK="1"
      shift
      ;;
    --exclude-canary-soak)
      INCLUDE_CANARY_SOAK="0"
      shift
      ;;
    --require-clean-tree)
      REQUIRE_CLEAN_TREE="1"
      shift
      ;;
    --allow-dirty-tree)
      REQUIRE_CLEAN_TREE="0"
      shift
      ;;
    *)
      echo "unknown argument: $1" >&2
      echo "usage: misc/sccp/verify_release.sh [--matrix sora-core-pairs|sora-pairs|full] [--max-minutes N] [--hub-config PATH] [--hub-mode MODE] [--fuzz-profile fast|full] [--formal-profile fast|full] [--canary-soak-profile fast|full] [--strict-adapters|--no-strict-adapters] [--include-negative|--exclude-negative] [--disable-hub-cache|--enable-hub-cache] [--skip-preflight|--run-preflight] [--include-canary-soak|--exclude-canary-soak] [--require-clean-tree|--allow-dirty-tree] [--run-id ID] [--artifacts-base PATH] [--fail-fast]" >&2
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

require_positive_int() {
  local name="$1"
  local value="$2"
  if [[ ! "${value}" =~ ^[1-9][0-9]*$ ]]; then
    echo "${name} must be a positive integer (got: ${value})" >&2
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

require_bool_01 "SCCP_VERIFY_STRICT_ADAPTERS" "${STRICT_ADAPTERS}"
require_bool_01 "SCCP_VERIFY_INCLUDE_NEGATIVE" "${INCLUDE_NEGATIVE}"
require_bool_01 "SCCP_VERIFY_DISABLE_HUB_CACHE" "${DISABLE_HUB_CACHE}"
require_bool_01 "SCCP_VERIFY_SKIP_PREFLIGHT" "${SKIP_PREFLIGHT}"
require_bool_01 "SCCP_VERIFY_INCLUDE_CANARY_SOAK" "${INCLUDE_CANARY_SOAK}"
require_bool_01 "SCCP_VERIFY_REQUIRE_CLEAN_TREE" "${REQUIRE_CLEAN_TREE}"
require_bool_01 "SCCP_VERIFY_FAIL_FAST" "${FAIL_FAST}"
require_positive_int "SCCP_VERIFY_MATRIX_MAX_MINUTES" "${MATRIX_MAX_MINUTES}"

echo "[verify-release] RUSTUP_TOOLCHAIN=${RUSTUP_TOOLCHAIN}"

if [[ ! -f "${HUB_CONFIG}" ]]; then
  echo "hub config not found: ${HUB_CONFIG}" >&2
  exit 1
fi

if [[ "${REQUIRE_CLEAN_TREE}" == "1" ]]; then
  dirty_report=""
  repo_candidates=(
    "${ROOT_DIR}"
    "${ROOT_DIR}/sccp/chains/eth"
    "${ROOT_DIR}/sccp/chains/bsc"
    "${ROOT_DIR}/sccp/chains/tron"
    "${ROOT_DIR}/sccp/chains/ton"
    "${ROOT_DIR}/sccp/chains/sol"
  )
  for repo in "${repo_candidates[@]}"; do
    if [[ ! -d "${repo}" ]] || ! git -C "${repo}" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
      continue
    fi
    repo_dirty="$(git -C "${repo}" status --porcelain --untracked-files=no)"
    if [[ -n "${repo_dirty}" ]]; then
      dirty_report+="${repo}"$'\n'
      while IFS= read -r line; do
        [[ -z "${line}" ]] && continue
        dirty_report+="  ${line}"$'\n'
      done <<< "${repo_dirty}"
    fi
  done
  if [[ -n "${dirty_report}" ]]; then
    echo "[verify-release] clean-tree check failed; tracked modifications detected:" >&2
    printf "%s" "${dirty_report}" >&2
    echo "[verify-release] re-run with --allow-dirty-tree only for non-release diagnostics" >&2
    exit 1
  fi
fi

RUN_DIR="${ARTIFACTS_BASE}/${RUN_ID}"
LOG_DIR="${RUN_DIR}/logs"
HUB_ARTIFACTS_DIR="${RUN_DIR}/hub-matrix"

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

  echo "[verify-release] stage=${stage_name} command=${cmd_repr}"

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
    echo "[verify-release] stage=${stage_name} failed (exit=${stage_exit})"
    if [[ "${FAIL_FAST}" == "1" ]]; then
      return 1
    fi
  else
    echo "[verify-release] stage=${stage_name} passed"
  fi

  return 0
}

run_stage "run_all_tests" "misc/sccp/run_all_tests.sh"

HUB_ARGS=(
  "misc/sccp-e2e/run_hub_matrix.sh"
  "--config" "${HUB_CONFIG}"
  "--mode" "${HUB_MODE}"
  "--artifacts-dir" "${HUB_ARTIFACTS_DIR}"
  "--matrix" "${MATRIX_MODE}"
  "--max-minutes" "${MATRIX_MAX_MINUTES}"
)
if [[ "${SKIP_PREFLIGHT}" == "1" ]]; then
  HUB_ARGS+=("--skip-preflight")
fi
if [[ "${STRICT_ADAPTERS}" == "1" ]]; then
  HUB_ARGS+=("--strict-adapters")
fi
if [[ "${INCLUDE_NEGATIVE}" == "1" ]]; then
  HUB_ARGS+=("--include-negative")
else
  HUB_ARGS+=("--exclude-negative")
fi
if [[ "${DISABLE_HUB_CACHE}" == "1" ]]; then
  HUB_ARGS+=("--disable-command-cache")
else
  HUB_ARGS+=("--enable-command-cache")
fi
run_stage "hub_matrix" "${HUB_ARGS[@]}"

if [[ "${INCLUDE_CANARY_SOAK}" == "1" ]]; then
  run_stage "canary_soak" \
    "misc/sccp/run_canary_soak.sh" \
    "--profile" "${CANARY_SOAK_PROFILE}" \
    "--hub-config" "${HUB_CONFIG}" \
    "--hub-mode" "${HUB_MODE}" \
    "--strict-adapters" \
    "--disable-hub-cache" \
    "--skip-preflight" \
    "--exclude-negative" \
    "--artifacts-dir" "${RUN_DIR}/canary-soak"
fi

run_stage "fuzz_bounded" "misc/sccp/run_fuzz_bounded.sh" "--profile" "${FUZZ_PROFILE}"
run_stage "fuzz_bounded_siblings" "misc/sccp/run_fuzz_bounded_siblings.sh" "--profile" "${FUZZ_PROFILE}"
run_stage "formal_assisted" "misc/sccp/run_formal_assisted.sh" "--profile" "${FORMAL_PROFILE}"

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
  echo "    \"hub_config\": \"$(json_escape "${HUB_CONFIG}")\","
  echo "    \"hub_mode\": \"$(json_escape "${HUB_MODE}")\","
  echo "    \"matrix_mode\": \"$(json_escape "${MATRIX_MODE}")\","
  echo "    \"matrix_max_minutes\": ${MATRIX_MAX_MINUTES},"
  echo "    \"strict_adapters\": ${STRICT_ADAPTERS},"
  echo "    \"include_negative\": ${INCLUDE_NEGATIVE},"
  echo "    \"disable_hub_cache\": ${DISABLE_HUB_CACHE},"
  echo "    \"skip_preflight\": ${SKIP_PREFLIGHT},"
  echo "    \"include_canary_soak\": ${INCLUDE_CANARY_SOAK},"
  echo "    \"canary_soak_profile\": \"$(json_escape "${CANARY_SOAK_PROFILE}")\","
  echo "    \"require_clean_tree\": ${REQUIRE_CLEAN_TREE},"
  echo "    \"fuzz_profile\": \"$(json_escape "${FUZZ_PROFILE}")\","
  echo "    \"formal_profile\": \"$(json_escape "${FORMAL_PROFILE}")\","
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
  echo "<testsuite name=\"sccp-release-verify\" tests=\"${#STAGE_NAMES[@]}\" failures=\"${FAILED_STAGE_COUNT}\">"
  for i in "${!STAGE_NAMES[@]}"; do
    stage_name_escaped="$(xml_escape "${STAGE_NAMES[$i]}")"
    echo "  <testcase classname=\"sccp.release\" name=\"${stage_name_escaped}\">"
    if [[ "${STAGE_STATUSES[$i]}" != "passed" ]]; then
      failure_message="stage ${STAGE_NAMES[$i]} failed (exit=${STAGE_EXIT_CODES[$i]})"
      failure_details="command=${STAGE_COMMANDS[$i]} log=${STAGE_LOGS[$i]}"
      echo "    <failure message=\"$(xml_escape "${failure_message}")\">$(xml_escape "${failure_details}")</failure>"
    fi
    echo "  </testcase>"
  done
  echo "</testsuite>"
} > "${JUNIT_PATH}"

echo "[verify-release] summary: ${SUMMARY_PATH}"
echo "[verify-release] junit: ${JUNIT_PATH}"
echo "[verify-release] artifacts: ${RUN_DIR}"

if [[ "${OVERALL_STATUS}" != "passed" ]]; then
  exit 1
fi

echo "[verify-release] OK"
