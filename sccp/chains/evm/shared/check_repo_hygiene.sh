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
      echo "usage: check_repo_hygiene.sh [--chain-root <path>]" >&2
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

if [[ ! -f .gitignore ]]; then
  echo "[repo-hygiene] missing .gitignore" >&2
  exit 1
fi

if [[ ! -f .github/CODEOWNERS ]]; then
  echo "[repo-hygiene] missing .github/CODEOWNERS" >&2
  exit 1
fi

if ! rg -n '^\*\s+@' .github/CODEOWNERS >/dev/null 2>&1; then
  echo "[repo-hygiene] .github/CODEOWNERS must define a wildcard owner rule" >&2
  exit 1
fi

if ! rg -n '^__pycache__/$' .gitignore >/dev/null 2>&1; then
  echo "[repo-hygiene] .gitignore is missing '__pycache__/'" >&2
  exit 1
fi

if ! rg -n '^\*\*/__pycache__/$' .gitignore >/dev/null 2>&1; then
  echo "[repo-hygiene] .gitignore is missing '**/__pycache__/'" >&2
  exit 1
fi

if git ls-files -- '*.pyc' | rg -q '.'; then
  echo "[repo-hygiene] tracked .pyc files detected:" >&2
  git ls-files -- '*.pyc' >&2
  exit 1
fi

if git ls-files | rg -q '(^|/)__pycache__/'; then
  echo "[repo-hygiene] tracked __pycache__ entries detected:" >&2
  git ls-files | rg '(^|/)__pycache__/' >&2
  exit 1
fi

if find . -type d -name '__pycache__' -not -path './.git/*' | rg -q '.'; then
  echo "[repo-hygiene] workspace __pycache__ directories detected (remove before commit):" >&2
  find . -type d -name '__pycache__' -not -path './.git/*' | sort >&2
  exit 1
fi

chain_name="$(basename "${chain_root}")"
if [[ "${chain_name}" == "eth" || "${chain_name}" == "bsc" || "${chain_name}" == "tron" ]]; then
  node "${chain_root}/../evm/shared/sync_sources.mjs" --check --chain-root "${chain_root}"
fi

echo "[repo-hygiene] OK"
