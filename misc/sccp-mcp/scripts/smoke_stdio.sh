#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"
MCP_AUTH_TOKEN="${SCCP_MCP_AUTH_TOKEN:-0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef}"

send_frame() {
  local body="$1"
  printf 'Content-Length: %s\r\n\r\n%s' "${#body}" "$body"
}

req_initialize='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}'
req_tools_list="{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{\"auth_token\":\"${MCP_AUTH_TOKEN}\"}}"
req_list_networks="{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"auth_token\":\"${MCP_AUTH_TOKEN}\",\"name\":\"sccp_list_networks\",\"arguments\":{}}}"
req_message_id="{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{\"auth_token\":\"${MCP_AUTH_TOKEN}\",\"name\":\"sccp_get_message_id\",\"arguments\":{\"payload\":{\"version\":1,\"source_domain\":0,\"dest_domain\":1,\"nonce\":42,\"sora_asset_id\":\"0x1111111111111111111111111111111111111111111111111111111111111111\",\"amount\":\"1000000000000000000\",\"recipient\":\"0x0000000000000000000000002222222222222222222222222222222222222222\"}}}}"

output="$({
  send_frame "${req_initialize}"
  send_frame "${req_tools_list}"
  send_frame "${req_list_networks}"
  send_frame "${req_message_id}"
} | SCCP_MCP_AUTH_TOKEN="${MCP_AUTH_TOKEN}" cargo run --quiet 2>/dev/null)"

assert_contains() {
  local needle="$1"
  if [[ "${output}" != *"${needle}"* ]]; then
    echo "[smoke] expected output to contain: ${needle}" >&2
    echo "[smoke] full output:" >&2
    echo "${output}" >&2
    exit 1
  fi
}

assert_contains '"protocolVersion":"2024-11-05"'
assert_contains '"name":"sccp_list_networks"'
assert_contains '"name":"sora_sccp_build_call"'
assert_contains '"name":"sccp_get_message_status"'
assert_contains '"message_id":"0x96f68e7cb4c8d01b237295459b956d4982e521232173d3dd1dc7e25cec46d208"'

echo "[smoke] MCP stdio smoke test passed"
