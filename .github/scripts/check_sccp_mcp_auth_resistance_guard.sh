#!/usr/bin/env bash
set -euo pipefail

MCP_RS="misc/sccp-mcp/src/mcp.rs"
TOOLS_RS="misc/sccp-mcp/src/tools.rs"
README_MD="misc/sccp-mcp/README.md"
GUARDRAILS_MD="docs/security/sccp_mcp_deployment_guardrails.md"

for required in "${MCP_RS}" "${TOOLS_RS}" "${README_MD}" "${GUARDRAILS_MD}"; do
  if [[ ! -f "${required}" ]]; then
    echo "[check_sccp_mcp_auth_resistance_guard] missing ${required}" >&2
    exit 1
  fi
done

if ! rg -q "SCCP_MCP_AUTH_FAILURE_BASE_DELAY_MS" "${MCP_RS}"; then
  echo "[check_sccp_mcp_auth_resistance_guard] missing auth backoff base-delay control" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_AUTH_FAILURE_MAX_DELAY_MS" "${MCP_RS}"; then
  echo "[check_sccp_mcp_auth_resistance_guard] missing auth backoff max-delay control" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_AUTH_FAILURE_TRACKED_PRINCIPALS" "${MCP_RS}"; then
  echo "[check_sccp_mcp_auth_resistance_guard] missing auth backoff tracked-principals cap control" >&2
  exit 1
fi

if ! rg -q "register_auth_failure_backoff" "${MCP_RS}"; then
  echo "[check_sccp_mcp_auth_resistance_guard] missing auth backoff registration path" >&2
  exit 1
fi

if ! rg -q "SECURITY_AUTH_BACKOFF" "${MCP_RS}"; then
  echo "[check_sccp_mcp_auth_resistance_guard] missing auth backoff security log event" >&2
  exit 1
fi

if ! rg -q "SECURITY_AUDIT tool_decision" "${TOOLS_RS}"; then
  echo "[check_sccp_mcp_auth_resistance_guard] missing submit-tool security audit logging" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_AUTH_FAILURE_BASE_DELAY_MS" "${README_MD}"; then
  echo "[check_sccp_mcp_auth_resistance_guard] README missing auth backoff env docs" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_AUTH_FAILURE_TRACKED_PRINCIPALS" "${README_MD}"; then
  echo "[check_sccp_mcp_auth_resistance_guard] README missing auth tracked-principals cap docs" >&2
  exit 1
fi

if ! rg -q "SECURITY_AUDIT" "${README_MD}"; then
  echo "[check_sccp_mcp_auth_resistance_guard] README missing security audit log notes" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_AUTH_FAILURE_TRACKED_PRINCIPALS" "${GUARDRAILS_MD}"; then
  echo "[check_sccp_mcp_auth_resistance_guard] deployment guardrails missing auth tracked-principals cap guidance" >&2
  exit 1
fi

if ! rg -q "SECURITY_AUTH_BACKOFF" "${GUARDRAILS_MD}"; then
  echo "[check_sccp_mcp_auth_resistance_guard] deployment guardrails missing auth backoff monitoring guidance" >&2
  exit 1
fi

echo "[check_sccp_mcp_auth_resistance_guard] PASS"
