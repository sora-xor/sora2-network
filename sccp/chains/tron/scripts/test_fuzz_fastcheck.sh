#!/usr/bin/env bash
set -euo pipefail

RUNS="${SCCP_FUZZ_RUNS:-}"
extra_args=()

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --runs|--fuzz-runs)
      RUNS="${2:-}"
      if [[ -z "${RUNS}" || ! "${RUNS}" =~ ^[1-9][0-9]*$ ]]; then
        echo "usage: scripts/test_fuzz_fastcheck.sh [--runs <positive-int>] [extra hardhat args...]" >&2
        exit 1
      fi
      shift 2
      ;;
    *)
      extra_args+=("$1")
      shift
      ;;
  esac
done

if [[ -n "${RUNS}" ]]; then
  export SCCP_FUZZ_RUNS="${RUNS}"
fi

if [[ "${#extra_args[@]}" -eq 0 ]]; then
  bash ./scripts/run_hardhat.sh test test/fuzz/*.fastcheck.test.js
else
  bash ./scripts/run_hardhat.sh test test/fuzz/*.fastcheck.test.js "${extra_args[@]}"
fi
