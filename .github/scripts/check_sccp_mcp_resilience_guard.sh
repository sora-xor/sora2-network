#!/usr/bin/env bash
set -euo pipefail

RPC_CLIENT_RS="misc/sccp-mcp/src/rpc_client.rs"
README_MD="misc/sccp-mcp/README.md"
GUARDRAILS_MD="docs/security/sccp_mcp_deployment_guardrails.md"

if [[ ! -f "${RPC_CLIENT_RS}" || ! -f "${README_MD}" || ! -f "${GUARDRAILS_MD}" ]]; then
  echo "[check_sccp_mcp_resilience_guard] missing required files" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_RPC_CONNECT_TIMEOUT_MS" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_resilience_guard] missing connect-timeout control" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_RPC_MAX_RETRIES" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_resilience_guard] missing bounded retry control" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_RPC_CIRCUIT_BREAKER_THRESHOLD" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_resilience_guard] missing circuit-breaker threshold control" >&2
  exit 1
fi

if ! rg -q "method_is_retry_safe" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_resilience_guard] submit/read retry safety gate missing" >&2
  exit 1
fi

if ! rg -q "rpc circuit breaker open" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_resilience_guard] fail-fast circuit-open behavior missing" >&2
  exit 1
fi

if ! rg -q "SCCP_MCP_RPC_CIRCUIT_BREAKER_THRESHOLD" "${README_MD}"; then
  echo "[check_sccp_mcp_resilience_guard] README missing circuit-breaker env docs" >&2
  exit 1
fi

for queue_env in \
  "SCCP_MCP_RPC_QUEUE_ENABLE" \
  "SCCP_MCP_RPC_QUEUE_MAX_PENDING" \
  "SCCP_MCP_RPC_QUEUE_MAX_PENDING_PER_PRINCIPAL" \
  "SCCP_MCP_RPC_QUEUE_WAIT_TIMEOUT_MS" \
  "SCCP_MCP_RPC_QUEUE_DRR_QUANTUM" \
  "SCCP_MCP_RPC_PRINCIPAL_WEIGHT_DEFAULT" \
  "SCCP_MCP_RPC_PRINCIPAL_WEIGHTS"; do
  if ! rg -q "${queue_env}" "${RPC_CLIENT_RS}"; then
    echo "[check_sccp_mcp_resilience_guard] missing queue env ${queue_env} in rpc client" >&2
    exit 1
  fi
  if ! rg -q "${queue_env}" "${README_MD}"; then
    echo "[check_sccp_mcp_resilience_guard] missing queue env ${queue_env} in README" >&2
    exit 1
  fi
  if ! rg -q "${queue_env}" "${GUARDRAILS_MD}"; then
    echo "[check_sccp_mcp_resilience_guard] missing queue env ${queue_env} in deployment guardrails" >&2
    exit 1
  fi
done

if ! rg -q "SECURITY_RPC_QUEUE_BACKPRESSURE" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_resilience_guard] missing queue backpressure security event" >&2
  exit 1
fi

if ! rg -q "SECURITY_RPC_QUEUE_ADMIT" "${RPC_CLIENT_RS}"; then
  echo "[check_sccp_mcp_resilience_guard] missing queue admit security event" >&2
  exit 1
fi

if ! rg -q "SECURITY_RPC_QUEUE_BACKPRESSURE" "${README_MD}"; then
  echo "[check_sccp_mcp_resilience_guard] README missing queue backpressure event docs" >&2
  exit 1
fi

echo "[check_sccp_mcp_resilience_guard] PASS"
