#!/usr/bin/env bash
set -euo pipefail

SKIP_FUZZ=0
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --skip-fuzz)
      SKIP_FUZZ=1
      shift
      ;;
    *)
      echo "unknown argument: $1" >&2
      echo "usage: scripts/test_ci_all.sh [--skip-fuzz]" >&2
      exit 1
      ;;
  esac
done

echo "[sccp-ci-all] run formal suite"
npm run test:ci-formal

if [[ "${SKIP_FUZZ}" == "1" ]]; then
  echo "[sccp-ci-all] skip fuzz suite (--skip-fuzz)"
  echo "[sccp-ci-all] OK"
  exit 0
fi

echo "[sccp-ci-all] run fuzz suite"
npm run test:ci-fuzz

echo "[sccp-ci-all] OK"
