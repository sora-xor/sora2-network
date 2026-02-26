#!/usr/bin/env bash
set -euo pipefail

TOOLS_RS="misc/sccp-mcp/src/tools.rs"
CONFIG_RS="misc/sccp-mcp/src/config.rs"
CONFIG_EXAMPLE="misc/sccp-mcp/config.example.toml"
GUARDRAILS_DOC="docs/security/sccp_mcp_deployment_guardrails.md"

for required in "${TOOLS_RS}" "${CONFIG_RS}" "${CONFIG_EXAMPLE}" "${GUARDRAILS_DOC}"; do
  if [[ ! -f "${required}" ]]; then
    echo "[check_sccp_mcp_preflight_guard] missing ${required}" >&2
    exit 1
  fi
done

if ! rg -q "\"sccp_preflight_activation\"" "${TOOLS_RS}"; then
  echo "[check_sccp_mcp_preflight_guard] tool definition missing sccp_preflight_activation" >&2
  exit 1
fi

if ! rg -q "\"sccp_preflight_activation\" => sccp_preflight_activation" "${TOOLS_RS}"; then
  echo "[check_sccp_mcp_preflight_guard] dispatch missing sccp_preflight_activation handler" >&2
  exit 1
fi

if ! rg -q "fn sccp_preflight_activation\\(" "${TOOLS_RS}"; then
  echo "[check_sccp_mcp_preflight_guard] missing sccp_preflight_activation implementation" >&2
  exit 1
fi

if ! rg -q "\"sccp_preflight_activation\"" "${CONFIG_RS}"; then
  echo "[check_sccp_mcp_preflight_guard] read-only default allowlist missing preflight tool" >&2
  exit 1
fi

if ! rg -q "\"sccp_preflight_activation\"" "${CONFIG_EXAMPLE}"; then
  echo "[check_sccp_mcp_preflight_guard] config.example read-only allowlist missing preflight tool" >&2
  exit 1
fi

if ! rg -q "sccp_preflight_activation" "${GUARDRAILS_DOC}"; then
  echo "[check_sccp_mcp_preflight_guard] deployment guardrails doc missing preflight checklist guidance" >&2
  exit 1
fi

echo "[check_sccp_mcp_preflight_guard] PASS"
