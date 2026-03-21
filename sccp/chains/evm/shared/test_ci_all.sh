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
skip_fuzz=0
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --chain-root)
      require_value "$1" "${2-}"
      chain_root="${2}"
      shift 2
      ;;
    --skip-fuzz)
      skip_fuzz=1
      shift
      ;;
    *)
      echo "unknown argument: $1" >&2
      echo "usage: test_ci_all.sh [--chain-root <path>] [--skip-fuzz]" >&2
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

echo "[sccp-ci-all] run formal suite"
npm run test:ci-formal

if [[ "${skip_fuzz}" == "1" ]]; then
  echo "[sccp-ci-all] skip fuzz suite (--skip-fuzz)"
  echo "[sccp-ci-all] OK"
  exit 0
fi

echo "[sccp-ci-all] run fuzz suite"
npm run test:ci-fuzz

echo "[sccp-ci-all] OK"
