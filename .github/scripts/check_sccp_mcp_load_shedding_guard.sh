#!/usr/bin/env bash
set -euo pipefail

RPC_CLIENT_RS="misc/sccp-mcp/src/rpc_client.rs"
MCP_RS="misc/sccp-mcp/src/mcp.rs"
README_MD="misc/sccp-mcp/README.md"
GUARDRAILS_MD="docs/security/sccp_mcp_deployment_guardrails.md"

for required in "${RPC_CLIENT_RS}" "${MCP_RS}" "${README_MD}" "${GUARDRAILS_MD}"; do
  if [[ ! -f "${required}" ]]; then
    echo "[check_sccp_mcp_load_shedding_guard] missing ${required}" >&2
    exit 1
  fi
done

if ! rg -q "SCCP_MCP_RPC_MAX_INFLIGHT" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_load_shedding_guard] missing RPC max in-flight control" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_RPC_MAX_INFLIGHT_PER_ENDPOINT" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_load_shedding_guard] missing per-endpoint max in-flight control" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_RPC_MAX_INFLIGHT_PER_PRINCIPAL" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_load_shedding_guard] missing per-principal max in-flight control" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_RPC_MAX_INFLIGHT_PER_SCOPE" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_load_shedding_guard] missing per-scope max in-flight control" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_RPC_MAX_INFLIGHT_PER_METHOD" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_load_shedding_guard] missing per-method max in-flight control" >&2
  exit 1
fi

if ! rg -q "try_acquire_rpc_inflight_slot" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_load_shedding_guard] missing in-flight acquisition gate" >&2
  exit 1
fi

if ! rg -q "try_acquire_rpc_endpoint_inflight_slot" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_load_shedding_guard] missing per-endpoint in-flight acquisition gate" >&2
  exit 1
fi

if ! rg -q "try_acquire_rpc_principal_inflight_slot" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_load_shedding_guard] missing per-principal in-flight acquisition gate" >&2
  exit 1
fi

if ! rg -q "try_acquire_rpc_scope_inflight_slot" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_load_shedding_guard] missing per-scope in-flight acquisition gate" >&2
  exit 1
fi

if ! rg -q "try_acquire_rpc_method_inflight_slot" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_load_shedding_guard] missing per-method in-flight acquisition gate" >&2
  exit 1
fi

if ! rg -q "SECURITY_RPC_BACKPRESSURE" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_load_shedding_guard] missing security backpressure event" >&2
  exit 1
fi

if ! rg -q "scope=method" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_load_shedding_guard] missing method-scope backpressure event" >&2
  exit 1
fi

if ! rg -q "scope=tool" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_load_shedding_guard] missing tool-scope backpressure event" >&2
  exit 1
fi

if ! rg -q "scope=principal" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_load_shedding_guard] missing principal-scope backpressure event" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_REQUIRE_REQUESTER_ID" "${MCP_RS}"; then
  echo "[check_sccp_mcp_load_shedding_guard] missing requester-id requirement control" >&2
  exit 1
fi

if ! rg -q "ensure_requester_id_if_required" "${MCP_RS}"; then
  echo "[check_sccp_mcp_load_shedding_guard] missing requester-id enforcement helper" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_RPC_MAX_INFLIGHT" "${README_MD}"; then
  echo "[check_sccp_mcp_load_shedding_guard] README missing RPC max in-flight documentation" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_RPC_MAX_INFLIGHT_PER_ENDPOINT" "${README_MD}"; then
  echo "[check_sccp_mcp_load_shedding_guard] README missing per-endpoint max in-flight documentation" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_RPC_MAX_INFLIGHT_PER_PRINCIPAL" "${README_MD}"; then
  echo "[check_sccp_mcp_load_shedding_guard] README missing per-principal max in-flight documentation" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_RPC_MAX_INFLIGHT_PER_SCOPE" "${README_MD}"; then
  echo "[check_sccp_mcp_load_shedding_guard] README missing per-scope max in-flight documentation" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_RPC_MAX_INFLIGHT_PER_METHOD" "${README_MD}"; then
  echo "[check_sccp_mcp_load_shedding_guard] README missing per-method max in-flight documentation" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_REQUIRE_REQUESTER_ID" "${README_MD}"; then
  echo "[check_sccp_mcp_load_shedding_guard] README missing requester-id requirement documentation" >&2
  exit 1
fi

if ! rg -q "SECURITY_RPC_BACKPRESSURE" "${README_MD}"; then
  echo "[check_sccp_mcp_load_shedding_guard] README missing backpressure event documentation" >&2
  exit 1
fi

if ! rg -q "scope=method" "${README_MD}"; then
  echo "[check_sccp_mcp_load_shedding_guard] README missing method-scope backpressure documentation" >&2
  exit 1
fi

if ! rg -q "scope=tool" "${README_MD}"; then
  echo "[check_sccp_mcp_load_shedding_guard] README missing tool-scope backpressure documentation" >&2
  exit 1
fi

if ! rg -q "scope=principal" "${README_MD}"; then
  echo "[check_sccp_mcp_load_shedding_guard] README missing principal-scope backpressure documentation" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_RPC_MAX_INFLIGHT" "${GUARDRAILS_MD}"; then
  echo "[check_sccp_mcp_load_shedding_guard] deployment guardrails missing max in-flight control" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_RPC_MAX_INFLIGHT_PER_ENDPOINT" "${GUARDRAILS_MD}"; then
  echo "[check_sccp_mcp_load_shedding_guard] deployment guardrails missing per-endpoint max in-flight control" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_RPC_MAX_INFLIGHT_PER_PRINCIPAL" "${GUARDRAILS_MD}"; then
  echo "[check_sccp_mcp_load_shedding_guard] deployment guardrails missing per-principal max in-flight control" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_RPC_MAX_INFLIGHT_PER_SCOPE" "${GUARDRAILS_MD}"; then
  echo "[check_sccp_mcp_load_shedding_guard] deployment guardrails missing per-scope max in-flight control" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_RPC_MAX_INFLIGHT_PER_METHOD" "${GUARDRAILS_MD}"; then
  echo "[check_sccp_mcp_load_shedding_guard] deployment guardrails missing per-method max in-flight control" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_REQUIRE_REQUESTER_ID" "${GUARDRAILS_MD}"; then
  echo "[check_sccp_mcp_load_shedding_guard] deployment guardrails missing requester-id requirement guidance" >&2
  exit 1
fi

if ! rg -q "SECURITY_RPC_BACKPRESSURE" "${GUARDRAILS_MD}"; then
  echo "[check_sccp_mcp_load_shedding_guard] deployment guardrails missing backpressure monitoring guidance" >&2
  exit 1
fi

if ! rg -q "scope=global|endpoint|principal|tool|method" "${GUARDRAILS_MD}"; then
  echo "[check_sccp_mcp_load_shedding_guard] deployment guardrails missing explicit backpressure scope breakdown" >&2
  exit 1
fi

echo "[check_sccp_mcp_load_shedding_guard] PASS"
