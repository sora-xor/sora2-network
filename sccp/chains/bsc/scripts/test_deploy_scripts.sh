#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "${repo_root}"

expect_failure_contains() {
  local expected="$1"
  shift

  local out_file="${tmp_dir}/failure.$RANDOM"
  if "$@" > "${out_file}" 2>&1; then
    echo "[deploy-scripts] command unexpectedly succeeded: $*" >&2
    exit 1
  fi

  if ! grep -Fq "${expected}" "${out_file}"; then
    echo "[deploy-scripts] expected stderr/stdout to contain '${expected}' for: $*" >&2
    cat "${out_file}" >&2
    exit 1
  fi
}

node --check scripts/deploy_mainnet.mjs
node --check scripts/build_bsc_header_rlp.mjs
node --check scripts/build_burn_proof_to_sora.mjs
node --check scripts/extract_burn_proof_inputs.mjs
node --check scripts/sccp_bsc_proof_lib.mjs
python3 -B - <<'PY'
import ast
from pathlib import Path

ast.parse(Path("scripts/deploy_mainnet.py").read_text(encoding="utf-8"), filename="scripts/deploy_mainnet.py")
PY
python3 -B scripts/deploy_mainnet.py --help >/dev/null

bash ./scripts/compile.sh >/dev/null
test -f artifacts/contracts/SccpRouter.sol/SccpRouter.json
test -f artifacts/contracts/verifiers/SoraBeefyLightClientVerifier.sol/SoraBeefyLightClientVerifier.json

tmp_key="$(mktemp)"
tmp_dir="$(mktemp -d)"
mock_rpc_port_file="${tmp_dir}/mock-rpc.port"
mock_rpc_log="${tmp_dir}/mock-rpc.log"
cleanup() {
  rm -f "${tmp_key}"
  if [[ -n "${mock_rpc_pid:-}" ]]; then
    kill "${mock_rpc_pid}" >/dev/null 2>&1 || true
    wait "${mock_rpc_pid}" >/dev/null 2>&1 || true
  fi
  rm -rf "${tmp_dir}"
}
trap cleanup EXIT
printf '0x59c6995e998f97a5a0044976f7d515d205e7f4d7fdddf9f47f4a8ff0f6f0c7f4' > "${tmp_key}"

python3 -u - "${mock_rpc_port_file}" > "${mock_rpc_log}" 2>&1 <<'PY' &
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

port_file = sys.argv[1]

class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("content-length", "0"))
        request = json.loads(self.rfile.read(length))
        method = request.get("method")
        if method == "eth_chainId":
            result = "0x38"
        else:
            result = None
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
  echo "[deploy-scripts] mock RPC failed to start" >&2
  cat "${mock_rpc_log}" >&2 || true
  exit 1
fi

mock_rpc_url="http://127.0.0.1:$(cat "${mock_rpc_port_file}")"

expect_failure_contains 'Missing value for --rpc-url' \
  node ./scripts/deploy_mainnet.mjs \
  --chain-label bsc \
  --rpc-url \
  --private-key-file "${tmp_key}" \
  --local-domain 2 \
  --expected-chain-id 56 \
  --latest-beefy-block 1 \
  --current-vset-id 1 \
  --current-vset-len 1 \
  --current-vset-root 0x1111111111111111111111111111111111111111111111111111111111111111 \
  --next-vset-id 2 \
  --next-vset-len 1 \
  --next-vset-root 0x2222222222222222222222222222222222222222222222222222222222222222

expect_failure_contains 'Flag --execute does not take a value' \
  node ./scripts/deploy_mainnet.mjs \
  --chain-label bsc \
  --rpc-url http://127.0.0.1:1 \
  --private-key-file "${tmp_key}" \
  --local-domain 2 \
  --expected-chain-id 56 \
  --latest-beefy-block 1 \
  --current-vset-id 1 \
  --current-vset-len 1 \
  --current-vset-root 0x1111111111111111111111111111111111111111111111111111111111111111 \
  --next-vset-id 2 \
  --next-vset-len 1 \
  --next-vset-root 0x2222222222222222222222222222222222222222222222222222222222222222 \
  --execute false

expect_failure_contains 'Flag --resume does not take a value' \
  node ./scripts/deploy_mainnet.mjs \
  --chain-label bsc \
  --rpc-url http://127.0.0.1:1 \
  --private-key-file "${tmp_key}" \
  --local-domain 2 \
  --expected-chain-id 56 \
  --latest-beefy-block 1 \
  --current-vset-id 1 \
  --current-vset-len 1 \
  --current-vset-root 0x1111111111111111111111111111111111111111111111111111111111111111 \
  --next-vset-id 2 \
  --next-vset-len 1 \
  --next-vset-root 0x2222222222222222222222222222222222222222222222222222222222222222 \
  --resume false

dry_run_out="${tmp_dir}/dry-run.json"
node ./scripts/deploy_mainnet.mjs \
  --chain-label bsc \
  --rpc-url "${mock_rpc_url}" \
  --private-key-file "${tmp_key}" \
  --local-domain 2 \
  --expected-chain-id 56 \
  --latest-beefy-block 1 \
  --current-vset-id 1 \
  --current-vset-len 1 \
  --current-vset-root 0x1111111111111111111111111111111111111111111111111111111111111111 \
  --next-vset-id 2 \
  --next-vset-len 1 \
  --next-vset-root 0x2222222222222222222222222222222222222222222222222222222222222222 \
  > "${dry_run_out}"

node - "${dry_run_out}" <<'NODE'
const fs = require('node:fs');

const out = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
if (out.mode !== 'dry-run') {
  throw new Error(`expected dry-run mode, got ${out.mode}`);
}
if (out.currentVset?.id !== '1') {
  throw new Error(`expected currentVset.id='1', got ${out.currentVset?.id}`);
}
if (out.nextVset?.id !== '2') {
  throw new Error(`expected nextVset.id='2', got ${out.nextVset?.id}`);
}
if (typeof out.paramsHash !== 'string' || !/^[0-9a-f]{64}$/i.test(out.paramsHash)) {
  throw new Error('expected paramsHash to be a 64-char hex string');
}
NODE

expect_failure_contains 'current-vset-len must be a positive integer' \
  node ./scripts/deploy_mainnet.mjs \
  --chain-label bsc \
  --rpc-url "${mock_rpc_url}" \
  --private-key-file "${tmp_key}" \
  --local-domain 2 \
  --expected-chain-id 56 \
  --latest-beefy-block 1 \
  --current-vset-id 1 \
  --current-vset-len abc \
  --current-vset-root 0x1111111111111111111111111111111111111111111111111111111111111111 \
  --next-vset-id 2 \
  --next-vset-len 1 \
  --next-vset-root 0x2222222222222222222222222222222222222222222222222222222222222222

expect_failure_contains 'latest-beefy-block must be a non-negative integer' \
  node ./scripts/deploy_mainnet.mjs \
  --chain-label bsc \
  --rpc-url "${mock_rpc_url}" \
  --private-key-file "${tmp_key}" \
  --local-domain 2 \
  --expected-chain-id 56 \
  --latest-beefy-block -1 \
  --current-vset-id 1 \
  --current-vset-len 1 \
  --current-vset-root 0x1111111111111111111111111111111111111111111111111111111111111111 \
  --next-vset-id 2 \
  --next-vset-len 1 \
  --next-vset-root 0x2222222222222222222222222222222222222222222222222222222222222222
