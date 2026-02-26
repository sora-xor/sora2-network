#!/usr/bin/env bash
set -euo pipefail

CONFIG_RS="misc/sccp-mcp/src/config.rs"
MAIN_RS="misc/sccp-mcp/src/main.rs"
EXAMPLE_TOML="misc/sccp-mcp/config.example.toml"

if [[ ! -f "${CONFIG_RS}" || ! -f "${MAIN_RS}" || ! -f "${EXAMPLE_TOML}" ]]; then
  echo "[check_sccp_mcp_auth_guard] missing required files" >&2
  exit 1
fi

if ! rg -q "resolve_auth_token_for_startup\\(" "${MAIN_RS}"; then
  echo "[check_sccp_mcp_auth_guard] runtime startup must resolve MCP auth token before serving" >&2
  exit 1
fi

if ! rg -q "missing required MCP auth token" "${CONFIG_RS}"; then
  echo "[check_sccp_mcp_auth_guard] startup policy must fail closed when MCP auth token is missing" >&2
  exit 1
fi

if ! rg -q "min_required_token_bytes" "${CONFIG_RS}"; then
  echo "[check_sccp_mcp_auth_guard] auth token minimum-length policy is missing" >&2
  exit 1
fi

if ! rg -q "max_token_bytes" "${CONFIG_RS}"; then
  echo "[check_sccp_mcp_auth_guard] auth token maximum-length policy is missing" >&2
  exit 1
fi

if ! rg -q "resolved auth token too short" "${CONFIG_RS}"; then
  echo "[check_sccp_mcp_auth_guard] startup policy must fail closed for weak auth token length" >&2
  exit 1
fi

if ! rg -q "resolved auth token too long" "${CONFIG_RS}"; then
  echo "[check_sccp_mcp_auth_guard] startup policy must fail closed for oversized auth token length" >&2
  exit 1
fi

if ! rg -q 'required_token_env = "SCCP_MCP_AUTH_TOKEN"' "${EXAMPLE_TOML}"; then
  echo "[check_sccp_mcp_auth_guard] config.example.toml must declare required_token_env" >&2
  exit 1
fi

if ! rg -q 'min_required_token_bytes = 32' "${EXAMPLE_TOML}"; then
  echo "[check_sccp_mcp_auth_guard] config.example.toml must declare strong auth token minimum length" >&2
  exit 1
fi

if ! rg -q 'max_token_bytes = 512' "${EXAMPLE_TOML}"; then
  echo "[check_sccp_mcp_auth_guard] config.example.toml must declare bounded auth token maximum length" >&2
  exit 1
fi

echo "[check_sccp_mcp_auth_guard] PASS"
