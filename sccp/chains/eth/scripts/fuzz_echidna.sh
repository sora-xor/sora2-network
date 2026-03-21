#!/usr/bin/env bash
set -euo pipefail

require_value() {
  local flag="$1"
  local value="${2-}"
  if [[ -z "${value}" || "${value}" == --* ]]; then
    echo "missing value for ${flag}" >&2
    exit 1
  fi
}

validate_foundry_out_dir() {
  local path="$1"
  local part
  if [[ "${path}" == /* ]]; then
    echo "foundry-out-dir must be a relative path under the repository (got absolute path: ${path})" >&2
    exit 1
  fi

  IFS='/' read -r -a parts <<<"${path}"
  for part in "${parts[@]}"; do
    if [[ -z "${part}" || "${part}" == "." || "${part}" == ".." ]]; then
      echo "foundry-out-dir must not contain empty, '.' , or '..' path segments (got: ${path})" >&2
      exit 1
    fi
  done
}

if ! command -v echidna >/dev/null 2>&1; then
  echo "[sccp-fuzz-echidna] echidna is required but not installed" >&2
  exit 1
fi

if ! command -v forge >/dev/null 2>&1; then
  echo "[sccp-fuzz-echidna] forge is required but not installed" >&2
  exit 1
fi

TARGET="contracts/echidna/EchidnaSccpCodec.sol"
CONTRACT="EchidnaSccpCodec"
TIMEOUT_SECS=1200
FOUNDRY_OUT_DIR="out"
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --timeout-secs)
      require_value "$1" "${2-}"
      TIMEOUT_SECS="${2:-}"
      shift 2
      ;;
    --foundry-out-dir)
      require_value "$1" "${2-}"
      FOUNDRY_OUT_DIR="${2:-}"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      echo "usage: scripts/fuzz_echidna.sh [--timeout-secs N] [--foundry-out-dir DIR]" >&2
      exit 1
      ;;
  esac
done

if [[ ! "${TIMEOUT_SECS}" =~ ^[1-9][0-9]*$ ]]; then
  echo "timeout-secs must be a positive integer (got: ${TIMEOUT_SECS})" >&2
  exit 1
fi

validate_foundry_out_dir "${FOUNDRY_OUT_DIR}"

echo "[sccp-fuzz-echidna] precompiling foundry build-info (out=${FOUNDRY_OUT_DIR})"
mkdir -p "${FOUNDRY_OUT_DIR}"
rm -rf "${FOUNDRY_OUT_DIR}/build-info"
forge build --out "${FOUNDRY_OUT_DIR}" --build-info "${TARGET}"

BUILD_INFO_DIR="${FOUNDRY_OUT_DIR}/build-info"
BUILD_INFO_FILE=""
if [[ -d "${BUILD_INFO_DIR}" ]]; then
  while IFS= read -r candidate; do
    if grep -q '"output"[[:space:]]*:' "${candidate}"; then
      BUILD_INFO_FILE="${candidate}"
      break
    fi
  done < <(find "${BUILD_INFO_DIR}" -maxdepth 1 -type f -name '*.json' | sort)
fi

if [[ -z "${BUILD_INFO_FILE}" ]]; then
  echo "[sccp-fuzz-echidna] expected foundry build-info with output key at ${BUILD_INFO_DIR}/*.json" >&2
  exit 1
fi

ECHIDNA_CMD=(
  echidna
  "${TARGET}"
  --contract
  "${CONTRACT}"
  --config
  echidna.yaml
  --timeout
  "${TIMEOUT_SECS}"
  --crytic-args
  "--ignore-compile --foundry-out-directory ${FOUNDRY_OUT_DIR}"
)

echo "[sccp-fuzz-echidna] timeout=${TIMEOUT_SECS}s"
"${ECHIDNA_CMD[@]}"
