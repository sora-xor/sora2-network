# SCCP MCP Deployment Guardrails

## Scope

This guardrail applies to `misc/sccp-mcp` and any service that launches it.

## Baseline Requirements

- Run `sccp-mcp` only as a local `stdio` child process of a trusted orchestrator.
- Keep all configured RPC endpoints private or loopback-only (`127.0.0.1`, RFC1918, ULA).
- Keep submit-capable tools disabled unless there is an explicit operational need.
- If submit tools are enabled, require an authenticated and authorized gateway in front of MCP access.

## Required Configuration Controls

- Keep `[policy].allow_tools` on the read-only baseline from `config.example.toml`.
- To enable a submit tool, explicitly add it to `allow_tools`; never rely on implicit defaults.
- Use per-environment config files and avoid sharing privileged production policy with dev/test.
- Keep `[deployment].allow_mutating_tools = false` and `[deployment].allow_non_private_rpc = false` unless there is a documented exception.
- Configure MCP auth token via `[auth].required_token` or `[auth].required_token_env` (default env key: `SCCP_MCP_AUTH_TOKEN`).
- Keep `[auth].min_required_token_bytes` at a strong value (default `32`, minimum allowed `16`), and do not relax it in production.
- Keep `[auth].max_token_bytes` conservative (default `512`) to bound auth-token comparison/hashing costs under adversarial oversized token inputs.
- Keep auth-failure backoff bounded (`SCCP_MCP_AUTH_FAILURE_BASE_DELAY_MS`, `SCCP_MCP_AUTH_FAILURE_MAX_DELAY_MS`) to slow brute-force token attempts on exposed bridges.
- Keep auth-failure principal tracking bounded (`SCCP_MCP_AUTH_FAILURE_TRACKED_PRINCIPALS`) to avoid unbounded backoff-state memory growth under adversarial token spray.
- Keep RPC timeout envs at bounded values (`SCCP_MCP_RPC_CONNECT_TIMEOUT_MS`, `SCCP_MCP_RPC_IO_TIMEOUT_MS`) to avoid worker starvation.
- Keep retry envs conservative (`SCCP_MCP_RPC_MAX_RETRIES`, `SCCP_MCP_RPC_RETRY_BACKOFF_MS`); retries apply only to retry-safe read RPC methods.
- Keep circuit-breaker envs conservative (`SCCP_MCP_RPC_CIRCUIT_BREAKER_THRESHOLD`, `SCCP_MCP_RPC_CIRCUIT_BREAKER_COOLDOWN_MS`) for fail-fast behavior during sustained upstream failure.
- Keep RPC load-shedding cap conservative (`SCCP_MCP_RPC_MAX_INFLIGHT`) so bursts fail fast before exhausting process resources.
- Keep per-endpoint load-shedding cap conservative (`SCCP_MCP_RPC_MAX_INFLIGHT_PER_ENDPOINT`) to prevent one upstream endpoint from monopolizing process capacity.
- Keep per-principal load-shedding cap conservative (`SCCP_MCP_RPC_MAX_INFLIGHT_PER_PRINCIPAL`) so one requester identity cannot monopolize process RPC capacity.
- Keep per-tool scope load-shedding cap conservative (`SCCP_MCP_RPC_MAX_INFLIGHT_PER_SCOPE`) to prevent one MCP tool from monopolizing process RPC capacity.
- Keep per-method load-shedding cap conservative (`SCCP_MCP_RPC_MAX_INFLIGHT_PER_METHOD`) to prevent one hot RPC method from starving other method classes on the same endpoint.
- Keep weighted principal queueing disabled by default (`SCCP_MCP_RPC_QUEUE_ENABLE=false`) unless sustained multi-tenant contention requires it.
- When queueing is enabled, keep queue pending caps conservative (`SCCP_MCP_RPC_QUEUE_MAX_PENDING`, `SCCP_MCP_RPC_QUEUE_MAX_PENDING_PER_PRINCIPAL`) to bound memory and waiting pressure.
- Keep queue wait timeout bounded (`SCCP_MCP_RPC_QUEUE_WAIT_TIMEOUT_MS`) so queued requests fail explicitly rather than waiting indefinitely.
- Keep DRR controls conservative (`SCCP_MCP_RPC_QUEUE_DRR_QUANTUM`, `SCCP_MCP_RPC_PRINCIPAL_WEIGHT_DEFAULT`, `SCCP_MCP_RPC_PRINCIPAL_WEIGHTS`) and configure weights only from trusted server-side operator config.
- For multi-tenant gateways, set `SCCP_MCP_REQUIRE_REQUESTER_ID=true` so `tools/call` is rejected when a caller identity tag is missing.
- Keep `ton_sccp_get_method` read-only: it should only allow `get*` and `runGetMethod` RPC methods.
- `sccp-mcp` startup now fails closed when risky settings are present without corresponding `[deployment]` acknowledgements.
- `sccp-mcp` startup fails closed if auth token resolution fails.

## Verification Checklist

1. Start service and confirm startup policy checks pass.
2. Confirm no submit tool is listed by `tools/list` unless explicitly intended.
3. Confirm all `rpc_url` values resolve to loopback/private hosts.
4. Verify clients include `params.auth_token` for `tools/list` and `tools/call`.
5. Confirm firewall/service mesh blocks public inbound access to the orchestrator that owns MCP stdio.
6. Confirm external signers/keys are isolated from MCP host process memory as designed.
7. Before `activate_token`, run `sccp_preflight_activation` for the target asset and confirm every required domain is `ready`.
8. Confirm monitoring/alerting captures `SECURITY_AUDIT` (submit-tool allow/deny), `SECURITY_AUTH_BACKOFF`, `SECURITY_RPC_BACKPRESSURE` (`scope=global|endpoint|principal|tool|method`), `SECURITY_RPC_QUEUE_BACKPRESSURE` (`reason=queue_full_global|queue_full_principal|queue_timeout`), and `SECURITY_RPC_QUEUE_ADMIT` events.

## Incident Response Guidance

- If unauthorized submission risk is suspected, immediately remove submit tools from `allow_tools` and restart.
- Rotate any compromised RPC/API credentials.
- Re-validate allowlist and network exposure before re-enabling privileged workflows.
