#!/usr/bin/env bash
set -euo pipefail

require_value() {
  local flag="$1"
  local value="${2-}"
  if [[ -z "${value}" || "${value}" == --* ]]; then
    echo "missing value for ${flag}" >&2
    echo "usage: test_formal_assisted.sh [--chain-root <path>] [--profile fast|full]" >&2
    exit 1
  fi
}

chain_root=""
profile="full"
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --chain-root)
      require_value "$1" "${2-}"
      chain_root="${2}"
      shift 2
      ;;
    --profile)
      require_value "$1" "${2-}"
      profile="${2}"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      echo "usage: test_formal_assisted.sh [--chain-root <path>] [--profile fast|full]" >&2
      exit 1
      ;;
  esac
done

if [[ "${profile}" != "fast" && "${profile}" != "full" ]]; then
  echo "profile must be fast or full (got: ${profile})" >&2
  exit 1
fi

if [[ -n "${chain_root}" ]]; then
  chain_root="$(cd "${chain_root}" && pwd)"
else
  chain_root="$(pwd)"
fi

cd "${chain_root}"

echo "[sccp-formal-assisted] profile=${profile}"
npm run test:formal-assisted -- --profile "${profile}"
