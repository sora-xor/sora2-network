#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"

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
require_option_value() {
  local flag="$1"
  local value="${2-}"
  if [[ -z "${value}" || "${value}" == --* ]]; then
    echo "missing value for ${flag}" >&2
    echo "usage: scripts/fuzz_echidna.sh [--timeout-secs N] [--foundry-out-dir DIR]" >&2
    exit 1
  fi
}

resolve_path() {
  node -e 'const path = require("node:path"); console.log(path.resolve(process.argv[1]));' "$1"
}

validate_foundry_out_dir() {
  local raw="$1"
  local resolved
  resolved="$(resolve_path "${raw}")"
  if [[ "${resolved}" == "/" || "${resolved}" == "${REPO_ROOT}" ]]; then
    echo "[sccp-fuzz-echidna] refusing destructive foundry out dir: ${raw}" >&2
    exit 1
  fi
  case "${resolved}" in
    "${REPO_ROOT}"/*)
      printf '%s\n' "${resolved}"
      ;;
    *)
      echo "[sccp-fuzz-echidna] foundry out dir must resolve inside repo: ${raw}" >&2
      exit 1
      ;;
  esac
}

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --timeout-secs)
      require_option_value "$1" "${2-}"
      TIMEOUT_SECS="${2:-}"
      shift 2
      ;;
    --foundry-out-dir)
      require_option_value "$1" "${2-}"
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

FOUNDRY_OUT_DIR="$(validate_foundry_out_dir "${FOUNDRY_OUT_DIR}")"

echo "[sccp-fuzz-echidna] precompiling foundry build-info (out=${FOUNDRY_OUT_DIR})"
rm -rf "${FOUNDRY_OUT_DIR}"
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
  --crytic-args
  "--ignore-compile --foundry-out-directory ${FOUNDRY_OUT_DIR}"
)

echo "[sccp-fuzz-echidna] timeout=${TIMEOUT_SECS}s"
if command -v timeout >/dev/null 2>&1; then
  timeout "${TIMEOUT_SECS}" "${ECHIDNA_CMD[@]}"
elif command -v gtimeout >/dev/null 2>&1; then
  gtimeout "${TIMEOUT_SECS}" "${ECHIDNA_CMD[@]}"
else
  "${ECHIDNA_CMD[@]}"
fi
