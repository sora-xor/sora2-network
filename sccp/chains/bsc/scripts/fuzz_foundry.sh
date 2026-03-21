#!/usr/bin/env bash
set -euo pipefail

if ! command -v forge >/dev/null 2>&1; then
  echo "[sccp-fuzz-foundry] forge is required but not installed" >&2
  exit 1
fi

require_value() {
  local flag="$1"
  local value="${2:-}"
  if [[ -z "${value}" || "${value}" == --* ]]; then
    echo "missing value for ${flag}" >&2
    echo "usage: scripts/fuzz_foundry.sh [--runs N]" >&2
    exit 1
  fi
}

RUNS=4096
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --runs)
      require_value "$1" "${2:-}"
      RUNS="${2:-}"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      echo "usage: scripts/fuzz_foundry.sh [--runs N]" >&2
      exit 1
      ;;
  esac
done

if [[ ! "${RUNS}" =~ ^[1-9][0-9]*$ ]]; then
  echo "runs must be a positive integer (got: ${RUNS})" >&2
  exit 1
fi

echo "[sccp-fuzz-foundry] runs=${RUNS}"
forge test --match-path "test/fuzz/*.t.sol" --fuzz-runs "${RUNS}"
