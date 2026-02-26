# sccp-mcp

Stateless MCP (`stdio`) server for SCCP operations across:

- `sora2-network` (`pallets/sccp`)
- `sccp-eth`
- `sccp-bsc`
- `sccp-tron`
- `sccp-sol`
- `sccp-ton`

## Design

- External signer only: server never stores private keys.
- Stateless: no persistent DB/state files.
- Transport: MCP over stdio (JSON-RPC framed with `Content-Length`).
- Secure defaults: read-only tool allowlist; submit-capable tools require explicit opt-in.

## Build

```bash
cd misc/sccp-mcp
cargo build
```

## Run

```bash
cd misc/sccp-mcp
cp config.example.toml config.toml
# edit RPC endpoints/router addresses and set correct SORA `sccp_pallet_index`
# set a strong auth token (used by tools/list + tools/call auth_token checks)
# default policy enforces auth token length bounds (`[auth].min_required_token_bytes`..`[auth].max_token_bytes`)
export SCCP_MCP_AUTH_TOKEN="$(openssl rand -hex 32)"
cargo run
```

Or point to a custom config path:

```bash
SCCP_MCP_AUTH_TOKEN="$(openssl rand -hex 32)" SCCP_MCP_CONFIG=/path/to/config.toml cargo run
```

Smoke test:

```bash
./scripts/smoke_stdio.sh
```

Dev-node integration (real signed SORA extrinsic + finalized event decoding):

```bash
./scripts/run_sora_dev_integration.sh
```

Optional env overrides:

- `SORA_WS_URL` (default: `ws://127.0.0.1:9944`)
- `MCP_NETWORK` (default: `sora_testnet`)
- `SIGNER_URI` (default: `//Alice`)
- `SCCP_CALL_NAME` (default: `set_outbound_domain_paused`)
- `SCCP_CALL_ARGS` (JSON; default: `{"domain_id":1,"paused":false}`)
- `WATCH_TIMEOUT_MS` (default: `120000`)
- `CONNECT_TIMEOUT_MS` (default: `15000`)
- `SCCP_MCP_AUTH_TOKEN` (required unless `[auth].required_token` is set directly)
- `SCCP_MCP_RPC_CONNECT_TIMEOUT_MS` (default: `5000`)
- `SCCP_MCP_RPC_IO_TIMEOUT_MS` (default: `30000`)
- `SCCP_MCP_RPC_MAX_RETRIES` (default: `1`, applies only to retry-safe read RPC methods)
- `SCCP_MCP_RPC_RETRY_BACKOFF_MS` (default: `250`)
- `SCCP_MCP_RPC_CIRCUIT_BREAKER_THRESHOLD` (default: `5`, set `0` to disable)
- `SCCP_MCP_RPC_CIRCUIT_BREAKER_COOLDOWN_MS` (default: `30000`)
- `SCCP_MCP_RPC_MAX_INFLIGHT` (default: `32`, max: `1024`; fail-fast rejects requests beyond this cap)
- `SCCP_MCP_RPC_MAX_INFLIGHT_PER_ENDPOINT` (default: `16`, max: `1024`; clamped to global max-in-flight)
- `SCCP_MCP_RPC_MAX_INFLIGHT_PER_PRINCIPAL` (default: `12`, max: `1024`; requester/auth principal cap, clamped to global max-in-flight)
- `SCCP_MCP_RPC_MAX_INFLIGHT_PER_SCOPE` (default: `12`, max: `1024`; per-tool scope cap, clamped to global limit)
- `SCCP_MCP_RPC_MAX_INFLIGHT_PER_METHOD` (default: `8`, max: `1024`; clamped to per-endpoint and global limits)
- `SCCP_MCP_RPC_QUEUE_ENABLE` (default: `false`; enables weighted principal-aware pre-admission queue before in-flight slot acquisition)
- `SCCP_MCP_RPC_QUEUE_MAX_PENDING` (default: `256`, max: `4096`; global queue pending cap)
- `SCCP_MCP_RPC_QUEUE_MAX_PENDING_PER_PRINCIPAL` (default: `32`, max: `4096`; clamped to global pending cap)
- `SCCP_MCP_RPC_QUEUE_WAIT_TIMEOUT_MS` (default: `200`; queued request timeout before explicit rejection)
- `SCCP_MCP_RPC_QUEUE_DRR_QUANTUM` (default: `1`; positive DRR quantum multiplier)
- `SCCP_MCP_RPC_PRINCIPAL_WEIGHT_DEFAULT` (default: `1`; positive default principal weight)
- `SCCP_MCP_RPC_PRINCIPAL_WEIGHTS` (optional: `principal=weight,principal=weight` using fairness principal IDs such as `requester:<fingerprint>` or `auth:<fingerprint>`)
- `SCCP_MCP_REQUIRE_REQUESTER_ID` (default: `false`; when `true`, `tools/call` requires non-empty `params.requester_id`)
- `SCCP_MCP_AUTH_FAILURE_BASE_DELAY_MS` (default: `50`)
- `SCCP_MCP_AUTH_FAILURE_MAX_DELAY_MS` (default: `1000`)
- `SCCP_MCP_AUTH_FAILURE_TRACKED_PRINCIPALS` (default: `2048`; max number of principals tracked for auth-failure backoff state)

## MCP tools

Global:

- `sccp_list_networks`
- `sccp_health`
- `sccp_get_message_id`
- `sccp_validate_payload`
- `sccp_encode_attester_quorum_proof`
- `sccp_list_supported_calls`

SORA SCCP:

- `sccp_get_token_state`
- `sccp_get_remote_token`
- `sccp_get_domain_endpoint`
- `sccp_preflight_activation` (checks per-domain remote token/endpoint readiness before activation)
- `sccp_get_light_client_state`
- `sccp_get_message_status`
- `sora_sccp_build_call` (returns SCALE `call_data_hex`)
- `sora_sccp_estimate_fee`
- `sora_sccp_submit_signed_extrinsic`

EVM SCCP (`sccp-eth`, `sccp-bsc`, `sccp-tron`):

- `evm_sccp_read_contract`
- `evm_sccp_build_tx`
- `evm_sccp_submit_signed_tx`

Solana SCCP (`sccp-sol`):

- `sol_sccp_get_account`
- `sol_sccp_build_transaction`
- `sol_sccp_submit_signed_transaction`

TON SCCP (`sccp-ton`):

- `ton_sccp_get_method` (read-only TON RPC methods: `get*` and `runGetMethod`)
- `ton_sccp_build_message`
- `ton_sccp_submit_signed_message`

## Notes

- `sora_sccp_build_call` returns unsigned call envelopes for external signing workflow.
- SORA call building requires correct `sccp_pallet_index` in profile (or `pallet_index` argument override).
- `block_number_bytes` controls SCALE encoding width for SORA `BlockNumber` arguments (`4` or `8`).
- `sol_sccp_build_transaction` and `ton_sccp_build_message` return signer-oriented templates (not signed payloads).
- The server can restrict tool access via `[policy]` in `config.toml`.
- Clients must include `"auth_token"` in `params` for `tools/list` and `tools/call`.
- Clients may include `"requester_id"` in `tools/call` params so per-principal fairness can distinguish callers behind a shared gateway token.
- Set `SCCP_MCP_REQUIRE_REQUESTER_ID=true` on multi-tenant gateways so every `tools/call` must carry a non-empty `requester_id`.
- Keep `SCCP_MCP_RPC_QUEUE_ENABLE=false` by default unless you need contention management; queueing is additive and does not replace existing in-flight fail-fast limits.
- Token source is `[auth].required_token` or environment key from `[auth].required_token_env` (default: `SCCP_MCP_AUTH_TOKEN`).
- Startup fails closed if resolved auth token length is outside `[auth].min_required_token_bytes`..`[auth].max_token_bytes` (defaults: `32`..`512`, minimum allowed config value for min: `16`).
- Invalid/missing `auth_token` attempts are rate-limited with bounded per-principal backoff and emit `SECURITY_AUTH_BACKOFF` stderr records.
- Submit-capable tool decisions emit `SECURITY_AUDIT` stderr records for allow/deny outcomes.
- RPC load-shedding rejections emit `SECURITY_RPC_BACKPRESSURE` stderr records with `scope=global`, `scope=endpoint`, `scope=principal`, `scope=tool`, or `scope=method`.
- RPC queue rejections emit `SECURITY_RPC_QUEUE_BACKPRESSURE` stderr records with `reason=queue_full_global|queue_full_principal|queue_timeout`.
- RPC queue admissions emit `SECURITY_RPC_QUEUE_ADMIT` stderr records with principal, wait time, and effective weight.
- The dev integration script submits via MCP and verifies finalization by scanning finalized blocks and decoding `system.events` for the extrinsic index.

Example (`tools/call` with auth token):

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "auth_token": "replace-with-32-byte-plus-shared-secret",
    "requester_id": "gateway-client-01",
    "name": "sccp_list_networks",
    "arguments": {}
  }
}
```

## Deployment Guardrails

- Keep this MCP server private (`stdio` between trusted local process and server).
- Use loopback/private RPC URLs for all configured networks.
- Do not expose submit-capable tools publicly without an authenticated gateway and strict policy.
- Treat `[policy].allow_tools` as deny-by-default. Add submit tools only for controlled operator workflows.
- Startup fails closed if:
  - submit-capable tools are enabled while `[deployment].allow_mutating_tools = false`, or
  - non-private RPC URLs are configured while `[deployment].allow_non_private_rpc = false`.
- Startup fails closed if no MCP auth token is configured/resolvable.
- Any deployment that sets either override to `true` must document the external authn/authz boundary and isolation controls.
