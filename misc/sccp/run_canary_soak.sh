#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

PROFILE="${SCCP_CANARY_SOAK_PROFILE:-full}"
HUB_CONFIG="${SCCP_CANARY_SOAK_HUB_CONFIG:-${ROOT_DIR}/misc/sccp-e2e/config.release-shadow.json}"
HUB_MODE="${SCCP_CANARY_SOAK_HUB_MODE:-release}"
CANARY_SCENARIO="${SCCP_CANARY_SCENARIO:-sora:eth}"
SOAK_MATRIX="${SCCP_CANARY_SOAK_MATRIX:-sora-core-pairs}"
MAX_MINUTES="${SCCP_CANARY_SOAK_MAX_MINUTES:-45}"
STRICT_ADAPTERS="${SCCP_CANARY_SOAK_STRICT_ADAPTERS:-1}"
DISABLE_HUB_CACHE="${SCCP_CANARY_SOAK_DISABLE_CACHE:-1}"
SKIP_PREFLIGHT="${SCCP_CANARY_SOAK_SKIP_PREFLIGHT:-1}"
INCLUDE_NEGATIVE="${SCCP_CANARY_SOAK_INCLUDE_NEGATIVE:-0}"
ARTIFACTS_DIR="${SCCP_CANARY_SOAK_ARTIFACTS_DIR:-${ROOT_DIR}/misc/sccp/artifacts/canary-soak-$(date -u +%Y%m%dT%H%M%SZ)}"

SOAK_ITERATIONS_ENV="${SCCP_CANARY_SOAK_ITERATIONS:-}"
if [[ -n "${SOAK_ITERATIONS_ENV}" ]]; then
  SOAK_ITERATIONS="${SOAK_ITERATIONS_ENV}"
elif [[ "${PROFILE}" == "fast" ]]; then
  SOAK_ITERATIONS=1
else
  SOAK_ITERATIONS=2
fi

while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile)
      PROFILE="$2"
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
    --scenario)
      CANARY_SCENARIO="$2"
      shift 2
      ;;
    --soak-iterations)
      SOAK_ITERATIONS="$2"
      shift 2
      ;;
    --soak-matrix)
      SOAK_MATRIX="$2"
      shift 2
      ;;
    --max-minutes)
      MAX_MINUTES="$2"
      shift 2
      ;;
    --strict-adapters)
      STRICT_ADAPTERS=1
      shift
      ;;
    --no-strict-adapters)
      STRICT_ADAPTERS=0
      shift
      ;;
    --disable-hub-cache)
      DISABLE_HUB_CACHE=1
      shift
      ;;
    --enable-hub-cache)
      DISABLE_HUB_CACHE=0
      shift
      ;;
    --skip-preflight)
      SKIP_PREFLIGHT=1
      shift
      ;;
    --run-preflight)
      SKIP_PREFLIGHT=0
      shift
      ;;
    --include-negative)
      INCLUDE_NEGATIVE=1
      shift
      ;;
    --exclude-negative)
      INCLUDE_NEGATIVE=0
      shift
      ;;
    --artifacts-dir)
      ARTIFACTS_DIR="$2"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      echo "usage: misc/sccp/run_canary_soak.sh [--profile fast|full] [--hub-config PATH] [--hub-mode MODE] [--scenario SRC:DST] [--soak-iterations N] [--soak-matrix sora-core-pairs|sora-pairs|full] [--max-minutes N] [--strict-adapters|--no-strict-adapters] [--disable-hub-cache|--enable-hub-cache] [--skip-preflight|--run-preflight] [--include-negative|--exclude-negative] [--artifacts-dir PATH]" >&2
      exit 1
      ;;
  esac
done

for bool_name in STRICT_ADAPTERS DISABLE_HUB_CACHE SKIP_PREFLIGHT INCLUDE_NEGATIVE; do
  value="${!bool_name}"
  if [[ "${value}" != "0" && "${value}" != "1" ]]; then
    echo "${bool_name} must be 0 or 1 (got: ${value})" >&2
    exit 1
  fi
done

if [[ ! "${SOAK_ITERATIONS}" =~ ^[1-9][0-9]*$ ]]; then
  echo "soak iterations must be a positive integer (got: ${SOAK_ITERATIONS})" >&2
  exit 1
fi

if [[ ! "${MAX_MINUTES}" =~ ^[1-9][0-9]*$ ]]; then
  echo "max minutes must be a positive integer (got: ${MAX_MINUTES})" >&2
  exit 1
fi

if [[ ! -f "${HUB_CONFIG}" ]]; then
  echo "hub config not found: ${HUB_CONFIG}" >&2
  exit 1
fi

mkdir -p "${ARTIFACTS_DIR}"

echo "[sccp-canary-soak] profile=${PROFILE}"
echo "[sccp-canary-soak] hub_config=${HUB_CONFIG} hub_mode=${HUB_MODE}"
echo "[sccp-canary-soak] canary=${CANARY_SCENARIO} soak_matrix=${SOAK_MATRIX} soak_iterations=${SOAK_ITERATIONS}"
echo "[sccp-canary-soak] artifacts=${ARTIFACTS_DIR}"

run_matrix() {
  local name="$1"
  shift
  echo "[sccp-canary-soak] ${name}"
  (
    cd "${ROOT_DIR}"
    misc/sccp-e2e/run_hub_matrix.sh "$@"
  )
}

common_args=(
  --config "${HUB_CONFIG}"
  --mode "${HUB_MODE}"
  --max-minutes "${MAX_MINUTES}"
)

if [[ "${STRICT_ADAPTERS}" == "1" ]]; then
  common_args+=(--strict-adapters)
else
  common_args+=(--no-strict-adapters)
fi

if [[ "${DISABLE_HUB_CACHE}" == "1" ]]; then
  common_args+=(--disable-command-cache)
else
  common_args+=(--enable-command-cache)
fi

if [[ "${SKIP_PREFLIGHT}" == "1" ]]; then
  common_args+=(--skip-preflight)
fi

# Canary run: fast single-path signal.
canary_args=(
  "${common_args[@]}"
  --scenario "${CANARY_SCENARIO}"
  --artifacts-dir "${ARTIFACTS_DIR}/01-canary"
)
if [[ "${INCLUDE_NEGATIVE}" == "1" ]]; then
  canary_args+=(--include-negative)
else
  canary_args+=(--exclude-negative)
fi
run_matrix "canary scenario ${CANARY_SCENARIO}" "${canary_args[@]}"

# Soak run(s): repeated matrix execution for stability.
for iter in $(seq 1 "${SOAK_ITERATIONS}"); do
  soak_args=(
    "${common_args[@]}"
    --matrix "${SOAK_MATRIX}"
    --artifacts-dir "${ARTIFACTS_DIR}/$(printf '%02d' "$((iter + 1))")-soak-${iter}"
  )
  if [[ "${INCLUDE_NEGATIVE}" == "1" ]]; then
    soak_args+=(--include-negative)
  else
    soak_args+=(--exclude-negative)
  fi
  run_matrix "soak iteration ${iter}/${SOAK_ITERATIONS}" "${soak_args[@]}"
done

echo "[sccp-canary-soak] OK"
