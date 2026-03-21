#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

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

echo "[repo-hygiene] OK"
