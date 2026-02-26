#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HARNESS_DIR="${ROOT_DIR}/misc/sccp-e2e"

CONFIG_PATH="${HARNESS_DIR}/config.local.json"
EXTRA_ARGS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --config|-c)
      CONFIG_PATH="$2"
      shift 2
      ;;
    *)
      EXTRA_ARGS+=("$1")
      shift
      ;;
  esac
done

if [[ ! -f "${CONFIG_PATH}" ]]; then
  echo "config file not found: ${CONFIG_PATH}" >&2
  exit 1
fi

node "${HARNESS_DIR}/src/run_hub_matrix.js" --config "${CONFIG_PATH}" "${EXTRA_ARGS[@]}"
