#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

MANIFESTS=()
while IFS= read -r manifest; do
  MANIFESTS+=("${manifest}")
done < <(find . -path ./target -prune -o -name Cargo.toml -print | sort)

if [[ ${#MANIFESTS[@]} -eq 0 ]]; then
  echo "[check_stable_deps_no_rc] no Cargo.toml files found" >&2
  exit 1
fi

MANIFEST_RC_PATTERN='https://github\.com/(paritytech|sora-xor)/polkadot-sdk\.git".*(tag|branch)\s*=\s*"[^"]*-rc[0-9]*"'
LOCKFILE_RC_PATTERN='source = "git\+https://github\.com/(paritytech|sora-xor)/polkadot-sdk\.git\?(tag|branch)=[^"#]*-rc[0-9]*'

if rg -n --pcre2 "${MANIFEST_RC_PATTERN}" "${MANIFESTS[@]}"; then
  echo "[check_stable_deps_no_rc] RC polkadot-sdk reference detected in Cargo.toml" >&2
  exit 1
fi

if [[ -f Cargo.lock ]] && rg -n --pcre2 "${LOCKFILE_RC_PATTERN}" Cargo.lock; then
  echo "[check_stable_deps_no_rc] RC polkadot-sdk source detected in Cargo.lock" >&2
  exit 1
fi

echo "[check_stable_deps_no_rc] PASS"
