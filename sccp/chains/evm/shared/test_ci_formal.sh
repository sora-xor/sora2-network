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
run_compile_deploy=0
run_cli_helpers=0
run_final_repo_hygiene=0

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --chain-root)
      require_value "$1" "${2-}"
      chain_root="${2}"
      shift 2
      ;;
    --compile-deploy)
      run_compile_deploy=1
      shift
      ;;
    --cli-helpers)
      run_cli_helpers=1
      shift
      ;;
    --final-repo-hygiene)
      run_final_repo_hygiene=1
      shift
      ;;
    *)
      echo "unknown argument: $1" >&2
      echo "usage: test_ci_formal.sh [--chain-root <path>] [--compile-deploy] [--cli-helpers] [--final-repo-hygiene]" >&2
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

echo "[sccp-ci-formal] check repository hygiene"
npm run check:repo-hygiene

echo "[sccp-ci-formal] compile + unit checks"
npm test

if [[ "${run_compile_deploy}" == "1" ]]; then
  echo "[sccp-ci-formal] deploy-target compile set"
  npm run compile:deploy
fi

echo "[sccp-ci-formal] deployment script checks"
npm run test:deploy-scripts

if [[ "${run_cli_helpers}" == "1" ]]; then
  echo "[sccp-ci-formal] cli helper checks"
  npm run test:cli-helpers
fi

echo "[sccp-ci-formal] formal-assisted checks"
npm run test:formal-assisted:ci

if [[ "${run_final_repo_hygiene}" == "1" ]]; then
  echo "[sccp-ci-formal] final repository hygiene check"
  npm run check:repo-hygiene
fi

echo "[sccp-ci-formal] OK"
