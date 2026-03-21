#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

tmp_dir="$(mktemp -d)"
mock_rpc_pid=""
cleanup() {
  if [[ -n "${mock_rpc_pid}" ]]; then
    kill "${mock_rpc_pid}" >/dev/null 2>&1 || true
    wait "${mock_rpc_pid}" >/dev/null 2>&1 || true
  fi
  rm -rf "${tmp_dir}"
}
trap cleanup EXIT

expect_stderr() {
  local expected="$1"
  shift

  local out_file="${tmp_dir}/stderr-check.$RANDOM"
  if "$@" > /dev/null 2> "${out_file}"; then
    echo "[cli-helpers] command unexpectedly succeeded: $*" >&2
    exit 1
  fi

  if ! grep -Fq -- "${expected}" "${out_file}"; then
    echo "[cli-helpers] expected stderr to contain '${expected}' for: $*" >&2
    cat "${out_file}" >&2
    exit 1
  fi
}

expect_stderr "missing value for --runs" bash ./scripts/fuzz_foundry.sh --runs
expect_stderr "missing value for --timeout-secs" bash ./scripts/fuzz_echidna.sh --timeout-secs
expect_stderr "missing value for --profile" bash ./scripts/test_formal_assisted.sh --profile

if grep -Eq '^[[:space:]]*contract:' echidna.yaml; then
  echo "[cli-helpers] echidna.yaml must not set contract; scripts/fuzz_echidna.sh selects it via CLI" >&2
  exit 1
fi

if command -v gh >/dev/null 2>&1; then
  expect_stderr "missing value for --approvals" bash ./scripts/apply_branch_protection.sh --approvals
  expect_stderr "missing value for --approvals" bash ./scripts/check_branch_protection.sh --approvals
fi

receipt_path="${tmp_dir}/receipt.json"
node - "${receipt_path}" <<'NODE'
const fs = require('node:fs');
const { Interface, encodeBytes32String, keccak256, concat, toUtf8Bytes } = require('ethers');

function encodeLE(value, width) {
  let v = BigInt(value);
  const out = Buffer.alloc(width);
  for (let i = 0; i < width; i += 1) {
    out[i] = Number(v & 0xffn);
    v >>= 8n;
  }
  return out;
}

function encodeBurnPayload({ sourceDomain, destDomain, nonce, soraAssetId, amount, recipient }) {
  return `0x${Buffer.concat([
    Buffer.from([1]),
    encodeLE(sourceDomain, 4),
    encodeLE(destDomain, 4),
    encodeLE(nonce, 8),
    Buffer.from(soraAssetId.slice(2), 'hex'),
    encodeLE(amount, 16),
    Buffer.from(recipient.slice(2), 'hex'),
  ]).toString('hex')}`;
}

const receiptPath = process.argv[2];
const iface = new Interface([
  'event SccpBurned(bytes32 indexed messageId, bytes32 indexed soraAssetId, address indexed sender, uint128 amount, uint32 destDomain, bytes32 recipient, uint64 nonce, bytes payload)',
]);
const router = '0x1234567890123456789012345678901234567890';
const sender = '0x9999999999999999999999999999999999999999';
const soraAssetId = `0x${'11'.repeat(32)}`;
const recipient = encodeBytes32String('sora-recipient');
const payloadHex = encodeBurnPayload({
  sourceDomain: 2,
  destDomain: 0,
  nonce: 7,
  soraAssetId,
  amount: 25,
  recipient,
});
const messageId = keccak256(concat([toUtf8Bytes('sccp:burn:v1'), payloadHex]));
const encoded = iface.encodeEventLog(
  iface.getEvent('SccpBurned'),
  [messageId, soraAssetId, sender, 25n, 0, recipient, 7n, payloadHex],
);

fs.writeFileSync(
  receiptPath,
  JSON.stringify({
    transactionHash: `0x${'aa'.repeat(32)}`,
    blockHash: `0x${'bb'.repeat(32)}`,
    blockNumber: '0x2a',
    status: '0x1',
    logs: [
      {
        address: router,
        topics: encoded.topics,
        data: encoded.data,
        logIndex: '0x3',
        transactionHash: `0x${'aa'.repeat(32)}`,
        blockHash: `0x${'bb'.repeat(32)}`,
      },
    ],
  }, null, 2),
  'utf8',
);
NODE

burn_inputs_json="${tmp_dir}/burn-inputs.json"
node ./scripts/extract_burn_proof_inputs.mjs \
  --receipt-file "${receipt_path}" \
  --router 0x1234567890123456789012345678901234567890 \
  > "${burn_inputs_json}"

node - "${burn_inputs_json}" <<'NODE'
const fs = require('node:fs');

const out = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
if (out.schema !== 'sccp-bsc-burn-proof-inputs/v1') {
  throw new Error(`unexpected schema: ${out.schema}`);
}
if (out.decoded_payload.source_domain !== 2 || out.decoded_payload.dest_domain !== 0) {
  throw new Error(`unexpected decoded payload: ${JSON.stringify(out.decoded_payload)}`);
}
if (out.proof_public_inputs.event_topic0 !== out.event_topic0) {
  throw new Error('proof_public_inputs.event_topic0 mismatch');
}
NODE

expect_stderr "no matching SccpBurned log found" \
  node ./scripts/extract_burn_proof_inputs.mjs \
  --receipt-file "${receipt_path}" \
  --router 0x0000000000000000000000000000000000000001

mock_fixture_path="${tmp_dir}/mock-rpc-fixture.json"
node - "${burn_inputs_json}" "${mock_fixture_path}" <<'NODE'
const fs = require('node:fs');
const { encodeRlp, keccak256, concat, getBytes } = require('ethers');

function q(hex) {
  const stripped = hex.slice(2).replace(/^0+/, '');
  if (stripped.length === 0) {
    return '0x';
  }
  return `0x${stripped.length % 2 === 0 ? stripped : `0${stripped}`}`;
}

const burnInputs = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const fixturePath = process.argv[3];
const messageId = burnInputs.message_id;
const payloadHex = burnInputs.payload_hex;

const slotBase = keccak256(concat([messageId, `0x${'0'.repeat(63)}4`]));

const extraData = `0x${
  'aa'.repeat(32)
}02${
  'b0'.repeat(20)
}${
  '00'.repeat(48)
}${
  'c0'.repeat(20)
}${
  '11'.repeat(48)
}10${
  '99'.repeat(65)
}`;

const block = {
  parentHash: `0x${'11'.repeat(32)}`,
  sha3Uncles: `0x${'22'.repeat(32)}`,
  miner: `0x${'33'.repeat(20)}`,
  stateRoot: `0x${'44'.repeat(32)}`,
  transactionsRoot: `0x${'55'.repeat(32)}`,
  receiptsRoot: `0x${'66'.repeat(32)}`,
  logsBloom: `0x${'00'.repeat(256)}`,
  difficulty: '0x2',
  number: '0x10',
  gasLimit: '0x1c9c380',
  gasUsed: '0x5208',
  timestamp: '0x6611f2c0',
  extraData,
  mixHash: `0x${'77'.repeat(32)}`,
  nonce: '0x0000000000000000',
  baseFeePerGas: '0x3b9aca00',
};

const header = encodeRlp([
  block.parentHash,
  block.sha3Uncles,
  block.miner,
  block.stateRoot,
  block.transactionsRoot,
  block.receiptsRoot,
  block.logsBloom,
  q(block.difficulty),
  q(block.number),
  q(block.gasLimit),
  q(block.gasUsed),
  q(block.timestamp),
  block.extraData,
  block.mixHash,
  block.nonce,
  q(block.baseFeePerGas),
]);
block.hash = keccak256(header);

const fixture = {
  chainId: '0x38',
  router: burnInputs.router.toLowerCase(),
  payloadHex,
  messageId,
  burnsSlotBase: slotBase,
  block,
  accountProof: ['0xf8518080808080'],
  storageProof: ['0xe301a0' + '12'.repeat(32)],
};

fs.writeFileSync(fixturePath, JSON.stringify(fixture, null, 2), 'utf8');
NODE

mock_rpc_port_file="${tmp_dir}/mock-rpc.port"
mock_rpc_log="${tmp_dir}/mock-rpc.log"
python3 -u - "${mock_rpc_port_file}" "${mock_fixture_path}" > "${mock_rpc_log}" 2>&1 <<'PY' &
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

port_file = sys.argv[1]
fixture = json.load(open(sys.argv[2], "r", encoding="utf-8"))

class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("content-length", "0"))
        request = json.loads(self.rfile.read(length))
        method = request.get("method")
        params = request.get("params", [])

        if method == "eth_chainId":
            result = fixture["chainId"]
        elif method == "eth_getBlockByNumber":
            if params[0] != fixture["block"]["number"]:
                raise RuntimeError(f"unexpected block selector {params[0]}")
            result = fixture["block"]
        elif method == "eth_getBlockByHash":
            if params[0].lower() != fixture["block"]["hash"]:
                raise RuntimeError(f"unexpected block hash {params[0]}")
            result = fixture["block"]
        elif method == "eth_getProof":
            if params[0].lower() != fixture["router"]:
                raise RuntimeError(f"unexpected router {params[0]}")
            if params[1] != [fixture["burnsSlotBase"]]:
                raise RuntimeError(f"unexpected storage slots {params[1]}")
            if params[2] != fixture["block"]["number"]:
                raise RuntimeError(f"unexpected proof block selector {params[2]}")
            result = {
                "address": fixture["router"],
                "accountProof": fixture["accountProof"],
                "storageProof": [{
                    "key": fixture["burnsSlotBase"],
                    "value": "0x1",
                    "proof": fixture["storageProof"],
                }],
            }
        else:
            raise RuntimeError(f"unexpected method {method}")

        response = json.dumps({"jsonrpc": "2.0", "id": request.get("id"), "result": result}).encode("utf-8")
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(response)))
        self.end_headers()
        self.wfile.write(response)

    def log_message(self, *_args):
        pass

server = HTTPServer(("127.0.0.1", 0), Handler)
with open(port_file, "w", encoding="utf-8") as handle:
    handle.write(str(server.server_port))
server.serve_forever()
PY
mock_rpc_pid=$!

for _ in {1..50}; do
  if [[ -s "${mock_rpc_port_file}" ]]; then
    break
  fi
  sleep 0.1
done

if [[ ! -s "${mock_rpc_port_file}" ]]; then
  echo "[cli-helpers] mock RPC failed to start" >&2
  cat "${mock_rpc_log}" >&2 || true
  exit 1
fi

mock_rpc_url="http://127.0.0.1:$(cat "${mock_rpc_port_file}")"
proof_builder_json="${tmp_dir}/burn-proof.json"
payload_hex="$(node - "${burn_inputs_json}" <<'NODE'
const fs = require('node:fs');
const out = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
process.stdout.write(out.payload_hex);
NODE
)"
node ./scripts/build_burn_proof_to_sora.mjs \
  --rpc-url "${mock_rpc_url}" \
  --router 0x1234567890123456789012345678901234567890 \
  --payload "${payload_hex}" \
  --block 16 \
  > "${proof_builder_json}"

node - "${proof_builder_json}" "${mock_fixture_path}" <<'NODE'
const fs = require('node:fs');
const { keccak256 } = require('ethers');

function readCompact(bytes, offsetRef) {
  const first = bytes[offsetRef.offset];
  const mode = first & 0x03;
  if (mode === 0) {
    offsetRef.offset += 1;
    return first >> 2;
  }
  if (mode === 1) {
    const value = ((bytes[offsetRef.offset + 1] << 8) | first) >> 2;
    offsetRef.offset += 2;
    return value;
  }
  if (mode === 2) {
    const value = (
      bytes[offsetRef.offset] |
      (bytes[offsetRef.offset + 1] << 8) |
      (bytes[offsetRef.offset + 2] << 16) |
      (bytes[offsetRef.offset + 3] << 24)
    ) >>> 2;
    offsetRef.offset += 4;
    return value;
  }
  throw new Error('unsupported compact mode');
}

function readVecOfBytes(bytes, offsetRef) {
  const outerLen = readCompact(bytes, offsetRef);
  const out = [];
  for (let i = 0; i < outerLen; i += 1) {
    const innerLen = readCompact(bytes, offsetRef);
    out.push(bytes.subarray(offsetRef.offset, offsetRef.offset + innerLen));
    offsetRef.offset += innerLen;
  }
  return out;
}

const out = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const fixture = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
if (out.schema !== 'sccp-bsc-burn-proof/v1') {
  throw new Error(`unexpected schema: ${out.schema}`);
}
if (out.burns_mapping_slot !== 4) {
  throw new Error(`unexpected burns_mapping_slot: ${out.burns_mapping_slot}`);
}
if (out.burns_slot_base !== fixture.burnsSlotBase) {
  throw new Error(`unexpected burns_slot_base: ${out.burns_slot_base}`);
}
if (out.storage_trie_key !== keccak256(out.burns_slot_base).toLowerCase()) {
  throw new Error(`unexpected storage_trie_key: ${out.storage_trie_key}`);
}
if (out.block.hash !== fixture.block.hash) {
  throw new Error(`unexpected block hash: ${out.block.hash}`);
}
if (out.suggested_sora_call?.call_name !== 'mint_from_proof') {
  throw new Error(`unexpected suggested_sora_call: ${JSON.stringify(out.suggested_sora_call)}`);
}

const bytes = Buffer.from(out.proof_scale_hex.slice(2), 'hex');
const offsetRef = { offset: 32 };
const anchor = `0x${bytes.subarray(0, 32).toString('hex')}`;
const accountProof = readVecOfBytes(bytes, offsetRef);
const storageProof = readVecOfBytes(bytes, offsetRef);
if (offsetRef.offset !== bytes.length) {
  throw new Error('trailing bytes in SCALE proof');
}
if (anchor !== out.block.hash) {
  throw new Error(`anchor hash mismatch: ${anchor}`);
}
if (accountProof.length !== 1 || storageProof.length !== 1) {
  throw new Error(`unexpected proof node counts: ${accountProof.length}/${storageProof.length}`);
}
NODE

header_rlp_json="${tmp_dir}/header-rlp.json"
node ./scripts/build_bsc_header_rlp.mjs \
  --rpc-url "${mock_rpc_url}" \
  --block-number 16 \
  --bsc-epoch-length 16 \
  > "${header_rlp_json}"

node - "${header_rlp_json}" "${mock_fixture_path}" <<'NODE'
const fs = require('node:fs');
const out = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const fixture = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
if (out.schema !== 'sccp-bsc-bsc-header-rlp/v1') {
  throw new Error(`unexpected schema: ${out.schema}`);
}
if (out.chain_id !== 56) {
  throw new Error(`unexpected chain_id: ${out.chain_id}`);
}
if (out.block_number !== '16') {
  throw new Error(`unexpected block_number: ${out.block_number}`);
}
if (out.block_hash !== fixture.block.hash) {
  throw new Error(`unexpected block_hash: ${out.block_hash}`);
}
if (!Array.isArray(out.bsc_epoch_validators) || out.bsc_epoch_validators.length !== 2) {
  throw new Error(`unexpected validators: ${JSON.stringify(out.bsc_epoch_validators)}`);
}
if (out.bsc_epoch_turn_length !== 16) {
  throw new Error(`unexpected turn length: ${out.bsc_epoch_turn_length}`);
}
NODE

expect_stderr "payload-derived messageId" \
  node ./scripts/build_burn_proof_to_sora.mjs \
  --rpc-url "${mock_rpc_url}" \
  --router 0x1234567890123456789012345678901234567890 \
  --payload "${payload_hex}" \
  --message-id 0x1111111111111111111111111111111111111111111111111111111111111111 \
  --block 16

expect_stderr "--block-number must be a decimal or 0x-prefixed block number" \
  node ./scripts/build_bsc_header_rlp.mjs \
  --rpc-url "${mock_rpc_url}" \
  --block-number finalized

echo "[cli-helpers] OK"
