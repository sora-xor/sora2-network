#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "${repo_root}"

ethers_spec="$(node -p "JSON.parse(require('fs').readFileSync('package.json','utf8')).devDependencies.ethers")"
ethers_root=""

resolve_ethers_root() {
  if [[ -n "${ethers_root}" ]]; then
    printf '%s\n' "${ethers_root}"
    return 0
  fi
  ethers_root="$(
    npx -y -p "ethers@${ethers_spec}" -c \
      'node -p "require(\"path\").resolve(process.env.PATH.split(require(\"path\").delimiter)[0], \"..\")"'
  )"
  printf '%s\n' "${ethers_root}"
}

node_with_ethers() {
  local root
  root="$(resolve_ethers_root)"
  NODE_PATH="${root}${NODE_PATH:+:${NODE_PATH}}" node "$@"
}

node --check scripts/deploy_mainnet.mjs
node --check scripts/extract_burn_export.mjs
node --check scripts/extract_burn_proof_inputs.mjs
node --input-type=module - <<'JS'
import assert from 'node:assert/strict';
import { ensureStatePolicy, hashParams, parseArgs, stringifyJson } from './scripts/deploy_mainnet.mjs';
import { encodePayload as encodeGovernancePayload, parseArgs as parseGovernanceArgs } from './scripts/encode_governance_payload.mjs';

const params = {
  currentVset: { id: 1n, len: 4, root: '0x11' },
  nextVset: { id: 2n, len: 4, root: '0x22' },
};

assert.doesNotThrow(() => hashParams(params));
assert.equal(
  stringifyJson(params),
  '{"currentVset":{"id":"1","len":4,"root":"0x11"},"nextVset":{"id":"2","len":4,"root":"0x22"}}',
);
assert.throws(
  () => ensureStatePolicy({ execute: false, resume: true, stateFile: '/definitely/missing/state.json' }),
  /--resume requested but state file does not exist/,
);
assert.throws(() => parseArgs(['--execute', 'false']), /Boolean flag --execute does not take a value: false/);
assert.throws(() => parseArgs(['--resume', 'false']), /Boolean flag --resume does not take a value: false/);
assert.throws(() => parseArgs(['--bogus', '1']), /Unknown argument: --bogus/);
assert.throws(() => parseArgs(['oops']), /Unexpected positional argument: oops/);
assert.throws(() => parseGovernanceArgs(['--bogus', '1']), /Unknown argument: --bogus/);
assert.throws(() => parseGovernanceArgs(['--action', 'pause', '--nonce', '1', '--nonce', '2']), /Duplicate argument: --nonce/);
assert.throws(
  () =>
    encodeGovernancePayload(
      parseGovernanceArgs([
        '--action',
        'pause',
        '--target-domain',
        '5',
        '--nonce',
        '1',
        '--sora-asset-id',
        '0x1111111111111111111111111111111111111111111111111111111111111111',
        '--decimals',
        '18',
      ]),
    ),
  /--decimals is not valid with --action pause/,
);
JS

tmp_compile_repo="$(mktemp -d)"
tmp_asset_repo="$(mktemp -d)"
cleanup() {
  rm -rf "${tmp_compile_repo}"
  rm -rf "${tmp_asset_repo}"
}
trap cleanup EXIT

cp -R "${repo_root}/contracts" "${tmp_compile_repo}/contracts"
cp -R "${repo_root}/scripts" "${tmp_compile_repo}/scripts"
cp "${repo_root}/hardhat.config.js" "${tmp_compile_repo}/hardhat.config.js"
cp "${repo_root}/package.json" "${tmp_compile_repo}/package.json"
ln -s "${repo_root}/node_modules" "${tmp_compile_repo}/node_modules"

(
  cd "${tmp_compile_repo}"
  rm -rf artifacts cache
  bash ./scripts/compile.sh >/dev/null
  if [[ ! -f artifacts/contracts/SccpRouter.sol/SccpRouter.json ]]; then
    echo "[test-compile] expected router hardhat artifact from scripts/compile.sh" >&2
    exit 1
  fi
  if [[ ! -f artifacts/contracts/verifiers/SoraBeefyLightClientVerifier.sol/SoraBeefyLightClientVerifier.json ]]; then
    echo "[test-compile] expected verifier hardhat artifact from scripts/compile.sh" >&2
    exit 1
  fi
)

bash ./scripts/test_readme_commands.sh
for case_args in \
  "scripts/test_formal_assisted.sh --profile" \
  "scripts/fuzz_foundry.sh --runs" \
  "scripts/fuzz_echidna.sh --timeout-secs" \
  "scripts/fuzz_echidna.sh --foundry-out-dir" \
  "scripts/check_branch_protection.sh --repo" \
  "scripts/apply_branch_protection.sh --approvals"
do
  set +e
  output="$(bash -lc "cd '${repo_root}' && bash ./${case_args}" 2>&1)"
  rc=$?
  set -e
  if [[ ${rc} -eq 0 ]]; then
    echo "[test-script-args] expected failure for '${case_args}'" >&2
    exit 1
  fi
  normalized_output="$(printf '%s' "${output}" | tr '[:upper:]' '[:lower:]')"
  if [[ "${normalized_output}" != *"missing value for"* ]]; then
    echo "[test-script-args] expected missing-value diagnostic for '${case_args}', got:" >&2
    echo "${output}" >&2
    exit 1
  fi
done

for case_args in \
  "scripts/ci_assets/check_branch_protection.sh --repo" \
  "scripts/ci_assets/apply_branch_protection.sh --approvals"
do
  set +e
  output="$(bash -lc "cd '${repo_root}' && bash ./${case_args}" 2>&1)"
  rc=$?
  set -e
  if [[ ${rc} -eq 0 ]]; then
    echo "[test-ci-assets] expected failure for '${case_args}'" >&2
    exit 1
  fi
  normalized_output="$(printf '%s' "${output}" | tr '[:upper:]' '[:lower:]')"
  if [[ "${normalized_output}" != *"missing value for"* ]]; then
    echo "[test-ci-assets] expected missing-value diagnostic for '${case_args}', got:" >&2
    echo "${output}" >&2
    exit 1
  fi
done

mkdir -p "${tmp_asset_repo}/scripts"
cp "${repo_root}/scripts/ci_assets/check_readme_commands.sh" "${tmp_asset_repo}/scripts/check_readme_commands.sh"
cat > "${tmp_asset_repo}/README.md" <<'EOF'
# tmp

```bash
npm test
npm run present
node ./scripts/present.mjs
```
EOF
cat > "${tmp_asset_repo}/package.json" <<'EOF'
{
  "scripts": {
    "present": "echo ok"
  }
}
EOF
cat > "${tmp_asset_repo}/scripts/present.mjs" <<'EOF'
console.log('ok');
EOF

set +e
output="$(cd "${tmp_asset_repo}" && bash ./scripts/check_readme_commands.sh 2>&1)"
rc=$?
set -e
if [[ ${rc} -eq 0 ]]; then
  echo "[test-ci-assets] expected asset README checker to fail on missing npm test" >&2
  echo "${output}" >&2
  exit 1
fi
if [[ "${output}" != *"npm script 'test' not found"* ]]; then
  echo "[test-ci-assets] expected missing npm test diagnostic from asset README checker, got:" >&2
  echo "${output}" >&2
  exit 1
fi

cat > "${tmp_asset_repo}/package.json" <<'EOF'
{
  "scripts": {
    "test": "echo ok",
    "present": "echo ok"
  }
}
EOF

(cd "${tmp_asset_repo}" && bash ./scripts/check_readme_commands.sh >/dev/null)

for case_args in \
  "--execute false" \
  "--resume false" \
  "--bogus 1" \
  "oops"
do
  set +e
  output="$(node ./scripts/deploy_mainnet.mjs ${case_args} 2>&1)"
  rc=$?
  set -e
  if [[ ${rc} -eq 0 ]]; then
    echo "[test-deploy-mainnet-cli] expected failure for '${case_args}'" >&2
    exit 1
  fi
  case "${case_args}" in
    "--execute false")
      expected="Boolean flag --execute does not take a value: false"
      ;;
    "--resume false")
      expected="Boolean flag --resume does not take a value: false"
      ;;
    "--bogus 1")
      expected="Unknown argument: --bogus"
      ;;
    "oops")
      expected="Unexpected positional argument: oops"
      ;;
  esac
  if [[ "${output}" != *"${expected}"* ]]; then
    echo "[test-deploy-mainnet-cli] expected diagnostic '${expected}' for '${case_args}', got:" >&2
    echo "${output}" >&2
    exit 1
  fi
done

output="$(
  node ./scripts/encode_governance_payload.mjs \
    --action add \
    --target-domain 5 \
    --nonce 1 \
    --sora-asset-id 0x1111111111111111111111111111111111111111111111111111111111111111 \
    --decimals 18 \
    --name "SCCP Wrapped" \
    --symbol "wSORA"
)"
if [[ "${output}" != *'"action": "add"'* ]]; then
  echo "[test-encode-governance] expected valid add payload output, got:" >&2
  echo "${output}" >&2
  exit 1
fi

node_with_ethers - <<'NODE'
const { execFileSync } = require('node:child_process');
const { mkdtempSync, writeFileSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');
const { Interface, concat, encodeBytes32String, keccak256, toUtf8Bytes } = require('ethers');

const iface = new Interface([
  'event SccpBurned(bytes32 indexed messageId, bytes32 indexed soraAssetId, address indexed sender, uint128 amount, uint32 destDomain, bytes32 recipient, uint64 nonce, bytes payload)',
]);

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

const router = '0x1234567890123456789012345678901234567890';
const sender = '0x9999999999999999999999999999999999999999';
const soraAssetId = `0x${'11'.repeat(32)}`;
const recipient = encodeBytes32String('sora-recipient');
const payloadHex = encodeBurnPayload({
  sourceDomain: 5,
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

const receiptPath = join(mkdtempSync(join(tmpdir(), 'sccp-tron-burn-export-')), 'receipt.json');
writeFileSync(
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

const out = JSON.parse(
  execFileSync(
      'node',
      [
      'scripts/extract_burn_export.mjs',
      '--receipt-file',
      receiptPath,
      '--router',
      router,
    ],
    { encoding: 'utf8' },
  ),
);

if (out.schema !== 'sccp-tron-burn-export/v1') {
  throw new Error(`unexpected schema: ${out.schema}`);
}
if (out.artifact_kind !== 'canonical_burn_export') {
  throw new Error(`unexpected artifact_kind: ${out.artifact_kind}`);
}
if (JSON.stringify(out.schema_aliases) !== JSON.stringify(['sccp-tron-burn-proof-inputs/v1'])) {
  throw new Error(`unexpected schema_aliases: ${JSON.stringify(out.schema_aliases)}`);
}
if (JSON.stringify(out.deprecated_fields) !== JSON.stringify(['proof_public_inputs'])) {
  throw new Error(`unexpected deprecated_fields: ${JSON.stringify(out.deprecated_fields)}`);
}
if (out.router.toLowerCase() !== router.toLowerCase()) {
  throw new Error(`unexpected router: ${out.router}`);
}
if (out.message_id !== messageId.toLowerCase()) {
  throw new Error(`unexpected message_id: ${out.message_id}`);
}
if (out.payload_hex !== payloadHex.toLowerCase()) {
  throw new Error(`unexpected payload_hex: ${out.payload_hex}`);
}
if (out.export_surface.source_domain !== 5 || out.export_surface.dest_domain !== 0) {
  throw new Error(`unexpected export_surface domains: ${JSON.stringify(out.export_surface)}`);
}
if (JSON.stringify(out.proof_public_inputs) !== JSON.stringify(out.export_surface)) {
  throw new Error('proof_public_inputs alias does not match export_surface');
}
NODE

for case_args in \
  "--action pause --target-domain 5 --nonce 1 --sora-asset-id 0x1111111111111111111111111111111111111111111111111111111111111111 --decimals 18 --name SCCPWrapped --symbol wSORA" \
  "--action pause --target-domain 5 --nonce 1 --nonce 2 --sora-asset-id 0x1111111111111111111111111111111111111111111111111111111111111111" \
  "--bogus 1"
do
  set +e
  output="$(node ./scripts/encode_governance_payload.mjs ${case_args} 2>&1)"
  rc=$?
  set -e
  if [[ ${rc} -eq 0 ]]; then
    echo "[test-encode-governance] expected failure for '${case_args}'" >&2
    exit 1
  fi
  case "${case_args}" in
    --action\ pause*)
      if [[ "${case_args}" == *"--decimals 18"* ]]; then
        expected="--decimals is not valid with --action pause"
      else
        expected="Duplicate argument: --nonce"
      fi
      ;;
    "--bogus 1")
      expected="Unknown argument: --bogus"
      ;;
  esac
  if [[ "${output}" != *"${expected}"* ]]; then
    echo "[test-encode-governance] expected diagnostic '${expected}' for '${case_args}', got:" >&2
    echo "${output}" >&2
    exit 1
  fi
done

set +e
output="$(bash -lc "cd '${repo_root}' && bash ./scripts/fuzz_echidna.sh --foundry-out-dir ." 2>&1)"
rc=$?
set -e
if [[ ${rc} -eq 0 ]]; then
  echo "[test-script-args] expected destructive foundry-out-dir to fail" >&2
  exit 1
fi
if [[ "${output}" != *"refusing destructive foundry out dir"* ]]; then
  echo "[test-script-args] expected destructive foundry-out-dir diagnostic, got:" >&2
  echo "${output}" >&2
  exit 1
fi

python3 -B - <<'PY'
import ast
from pathlib import Path

ast.parse(Path("scripts/deploy_mainnet.py").read_text(encoding="utf-8"), filename="scripts/deploy_mainnet.py")
PY
python3 -B scripts/deploy_mainnet.py --help >/dev/null
