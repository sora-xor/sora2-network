#!/usr/bin/env bash
set -euo pipefail

if ! command -v node >/dev/null 2>&1; then
  exit 0
fi

node_major="$(node -p "process.versions.node.split('.')[0]")"
if [[ "${node_major}" == "22" ]]; then
  exit 0
fi

for candidate in /opt/homebrew/opt/node@22/bin /usr/local/opt/node@22/bin; do
  if [[ -x "${candidate}/node" ]]; then
    printf '%s\n' "${candidate}"
    exit 0
  fi
done
