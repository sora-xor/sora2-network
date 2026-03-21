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

chain_root=""
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --chain-root)
      require_value "$1" "${2-}"
      chain_root="${2}"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      echo "usage: test_ci_fuzz.sh [--chain-root <path>]" >&2
      exit 1
      ;;
  esac
done

if [[ -n "${chain_root}" ]]; then
  chain_root="$(cd "${chain_root}" && pwd)"
else
  chain_root="$(pwd)"
fi

cd "${chain_root}"

echo "[sccp-ci-fuzz] check repository hygiene"
npm run check:repo-hygiene

echo "[sccp-ci-fuzz] run nightly fuzz suite"
npm run test:fuzz:nightly

echo "[sccp-ci-fuzz] OK"
