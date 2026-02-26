#!/usr/bin/env bash
set -euo pipefail

TOOLS_RS="misc/sccp-mcp/src/tools.rs"

if [[ ! -f "${TOOLS_RS}" ]]; then
  echo "[check_sccp_mcp_readonly_tool_guard] missing ${TOOLS_RS}" >&2
  exit 1
fi

if ! rg -q "fn ensure_ton_read_method\\(" "${TOOLS_RS}"; then
  echo "[check_sccp_mcp_readonly_tool_guard] missing TON read-method validator" >&2
  exit 1
fi

if ! rg -q "TON read tool only allows read-only methods" "${TOOLS_RS}"; then
  echo "[check_sccp_mcp_readonly_tool_guard] missing fail-closed TON read-method error" >&2
  exit 1
fi

if ! rg -q "ensure_ton_read_method\\(method\\)\\?;" "${TOOLS_RS}"; then
  echo "[check_sccp_mcp_readonly_tool_guard] ton_sccp_get_method must invoke read-method validator" >&2
  exit 1
fi

echo "[check_sccp_mcp_readonly_tool_guard] PASS"
