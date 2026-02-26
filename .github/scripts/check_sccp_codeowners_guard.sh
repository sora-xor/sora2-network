#!/usr/bin/env bash
set -euo pipefail

CODEOWNERS_FILE=".github/CODEOWNERS"

if [[ ! -f "${CODEOWNERS_FILE}" ]]; then
  echo "[check_sccp_codeowners_guard] missing ${CODEOWNERS_FILE}" >&2
  exit 1
fi

check_entry_two_owners() {
  local pattern="$1"
  if ! awk -v pattern="${pattern}" '
    $0 !~ /^[[:space:]]*#/ && $1 == pattern && NF >= 3 { found = 1 }
    END { exit(found ? 0 : 1) }
  ' "${CODEOWNERS_FILE}"; then
    echo "[check_sccp_codeowners_guard] expected CODEOWNERS entry with >=2 owners for ${pattern}" >&2
    exit 1
  fi
}

check_entry_two_owners "/pallets/sccp/"
check_entry_two_owners "/runtime/src/lib.rs"
check_entry_two_owners "/runtime/src/tests/sccp_runtime_integration.rs"
check_entry_two_owners "/misc/sccp-mcp/"
check_entry_two_owners "/docs/security/"

echo "[check_sccp_codeowners_guard] PASS"
