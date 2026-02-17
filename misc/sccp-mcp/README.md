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
cargo run
```

Or point to a custom config path:

```bash
SCCP_MCP_CONFIG=/path/to/config.toml cargo run
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

- `ton_sccp_get_method`
- `ton_sccp_build_message`
- `ton_sccp_submit_signed_message`

## Notes

- `sora_sccp_build_call` returns unsigned call envelopes for external signing workflow.
- SORA call building requires correct `sccp_pallet_index` in profile (or `pallet_index` argument override).
- `block_number_bytes` controls SCALE encoding width for SORA `BlockNumber` arguments (`4` or `8`).
- `sol_sccp_build_transaction` and `ton_sccp_build_message` return signer-oriented templates (not signed payloads).
- The server can restrict tool access via `[policy]` in `config.toml`.
- The dev integration script submits via MCP and verifies finalization by scanning finalized blocks and decoding `system.events` for the extrinsic index.
