#!/usr/bin/env bash
set -euo pipefail

# Adapter contract for SCCP hub harness.
#
# Usage:
#   ./scripts/sccp_e2e_adapter.sh burn --json '<payload>'
#   ./scripts/sccp_e2e_adapter.sh mint_verify --json '<payload>'
#   ./scripts/sccp_e2e_adapter.sh negative_verify --json '<payload>'

if [[ $# -lt 3 ]]; then
  echo "usage: $0 <burn|mint_verify|negative_verify> --json '<payload>'" >&2
  exit 2
fi

action="$1"
shift

if [[ "$1" != "--json" ]]; then
  echo "expected --json argument" >&2
  exit 2
fi

payload="$2"

case "${action}" in
  burn)
    # Replace with domain-specific burn checks.
    echo "${payload}" >/dev/null
    ;;
  mint_verify)
    # Replace with domain-specific mint verification.
    echo "${payload}" >/dev/null
    ;;
  negative_verify)
    # Replace with domain-specific negative checks.
    echo "${payload}" >/dev/null
    ;;
  *)
    echo "unsupported action: ${action}" >&2
    exit 2
    ;;
esac

echo '{"ok":true}'
