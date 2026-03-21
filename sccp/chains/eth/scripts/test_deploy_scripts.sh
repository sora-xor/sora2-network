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
node --check scripts/compile_deploy_contracts.mjs
node --check scripts/extract_burn_proof_inputs.mjs
bash -n scripts/run_hardhat.sh
bash -n scripts/select_node22_path.sh
python3 -B - <<'PY'
import ast
from pathlib import Path

ast.parse(Path("scripts/deploy_mainnet.py").read_text(encoding="utf-8"), filename="scripts/deploy_mainnet.py")
PY
python3 -B scripts/deploy_mainnet.py --help >/dev/null
python3 -B - <<'PY'
import importlib.util
import tempfile
from pathlib import Path
from types import SimpleNamespace

module_path = Path("scripts/deploy_mainnet.py")
spec = importlib.util.spec_from_file_location("deploy_mainnet", module_path)
mod = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(mod)

tmpdir = Path(tempfile.mkdtemp(prefix="sccp-deploy-test-"))
rpc_file = tmpdir / "rpc.txt"
pk_file = tmpdir / "pk.txt"
rpc_file.write_text("http://127.0.0.1:8545\n", encoding="utf-8")
pk_file.write_text("0x" + "11" * 32 + "\n", encoding="utf-8")

captured = []

def fake_run(cmd, cwd):
    captured.append((cmd, cwd))

mod.run = fake_run
mod.parse_args = lambda: SimpleNamespace(
    rpc_url=None,
    rpc_url_file=str(rpc_file),
    private_key_file=str(pk_file),
    latest_beefy_block="1",
    current_vset_id="1",
    current_vset_len="1",
    current_vset_root="0x" + "22" * 32,
    next_vset_id="2",
    next_vset_len="1",
    next_vset_root="0x" + "33" * 32,
    out=None,
    state_file=None,
    resume=False,
    skip_compile=False,
    execute=False,
    ack_mainnet=None,
)

rc = mod.main()
assert rc == 0, rc
assert len(captured) == 2, captured
assert captured[0][0] == ["npm", "run", "compile:deploy"], captured
assert captured[1][0][:2] == ["node", "scripts/deploy_mainnet.mjs"], captured
PY
python3 -B - <<'PY'
from pathlib import Path
import subprocess

readme = Path("README.md")
original = readme.read_text(encoding="utf-8")

try:
    readme.write_text(
        original + "\n```bash\nnpm definitely-not-a-real-command\n```\n",
        encoding="utf-8",
    )
    proc = subprocess.run(
        ["bash", "./scripts/check_readme_commands.sh"],
        text=True,
        capture_output=True,
        check=False,
    )
    if proc.returncode == 0:
        raise SystemExit("check_readme_commands.sh unexpectedly ignored invalid bare npm command")
    if "unsupported bare npm command 'definitely-not-a-real-command'" not in proc.stderr:
        raise SystemExit(f"unexpected stderr for invalid bare npm command:\n{proc.stderr}")
finally:
    readme.write_text(original, encoding="utf-8")
PY
bash -n scripts/sccp_e2e_adapter.sh
python3 -B - <<'PY'
import json
import subprocess

cases = [
    ("burn", 'script-test "quoted"'),
    ("mint_verify", "script-test"),
    ("negative_verify", "script-test"),
]

for action, scenario_id in cases:
    proc = subprocess.run(
        [
            "bash",
            "./scripts/sccp_e2e_adapter.sh",
            action,
            "--json",
            json.dumps({"scenario_id": scenario_id}),
        ],
        text=True,
        capture_output=True,
        check=False,
    )
    if proc.returncode != 0:
        raise SystemExit(f"{action} adapter invocation failed:\nSTDOUT:\n{proc.stdout}\nSTDERR:\n{proc.stderr}")
    if "0 passing" in proc.stdout:
        raise SystemExit(f"{action} adapter matched zero tests:\n{proc.stdout}")

    lines = [line for line in proc.stdout.splitlines() if line.strip()]
    result = json.loads(lines[-1])
    assert result["ok"] is True, result
    assert result["action"] == action, result
    assert result["scenario_id"] == scenario_id, result
PY
node_with_ethers - <<'NODE'
const { execFileSync } = require('node:child_process');
const { mkdtempSync, writeFileSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');
const { Interface, encodeBytes32String, keccak256, concat, toUtf8Bytes } = require('ethers');

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

function encodeBurnPayload({
  sourceDomain,
  destDomain,
  nonce,
  soraAssetId,
  amount,
  recipient,
}) {
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
  sourceDomain: 1,
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

const receiptPath = join(mkdtempSync(join(tmpdir(), 'sccp-burn-proof-inputs-')), 'receipt.json');
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
      'scripts/extract_burn_proof_inputs.mjs',
      '--receipt-file',
      receiptPath,
      '--router',
      router,
    ],
    { encoding: 'utf8' },
  ),
);

if (out.schema !== 'sccp-eth-burn-proof-inputs/v1') {
  throw new Error(`unexpected schema: ${out.schema}`);
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
if (out.proof_public_inputs.event_topic0 !== iface.getEvent('SccpBurned').topicHash) {
  throw new Error(`unexpected topic0: ${out.proof_public_inputs.event_topic0}`);
}
if (out.decoded_payload.source_domain !== 1 || out.decoded_payload.dest_domain !== 0) {
  throw new Error(`unexpected decoded payload domains: ${JSON.stringify(out.decoded_payload)}`);
}
NODE
node_with_ethers - <<'NODE'
const { mkdtempSync, writeFileSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');
const { spawnSync } = require('node:child_process');

const dir = mkdtempSync(join(tmpdir(), 'sccp-node-deploy-'));
const pkFile = join(dir, 'pk.txt');
writeFileSync(pkFile, `0x${'11'.repeat(32)}\n`, 'utf8');

const proc = spawnSync(
  'node',
  [
    'scripts/deploy_mainnet.mjs',
    '--chain-label',
    'eth',
    '--rpc-url',
    '--private-key-file',
    pkFile,
    '--local-domain',
    '1',
    '--expected-chain-id',
    '1',
    '--latest-beefy-block',
    '1',
    '--current-vset-id',
    '1',
    '--current-vset-len',
    '1',
    '--current-vset-root',
    `0x${'22'.repeat(32)}`,
    '--next-vset-id',
    '2',
    '--next-vset-len',
    '1',
    '--next-vset-root',
    `0x${'33'.repeat(32)}`,
  ],
  { encoding: 'utf8' },
);

if (proc.status === 0) {
  throw new Error('deploy_mainnet.mjs unexpectedly accepted --rpc-url without a value');
}

if (!proc.stderr.includes('Missing value for --rpc-url')) {
  throw new Error(`unexpected stderr for missing rpc-url value:\n${proc.stderr}`);
}
NODE
node - <<'NODE'
const http = require('node:http');
const { mkdtempSync, writeFileSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');
const { spawn } = require('node:child_process');

function runNode(args) {
  return new Promise((resolve, reject) => {
    const child = spawn('node', args, { stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (chunk) => {
      stdout += chunk;
    });
    child.stderr.on('data', (chunk) => {
      stderr += chunk;
    });
    child.on('error', reject);
    child.on('close', (status) => resolve({ status, stdout, stderr }));
  });
}

(async () => {
  const dir = mkdtempSync(join(tmpdir(), 'sccp-node-deploy-dryrun-'));
  const pkFile = join(dir, 'pk.txt');
  writeFileSync(pkFile, `0x${'11'.repeat(32)}\n`, 'utf8');

  const server = http.createServer((req, res) => {
    let body = '';
    req.on('data', (chunk) => {
      body += chunk;
    });
    req.on('end', () => {
      const parsed = body ? JSON.parse(body) : { id: 1, method: 'eth_chainId' };
      const method = parsed.method;
      const result = method === 'net_version' ? '1' : '0x1';
      res.writeHead(200, { 'content-type': 'application/json', connection: 'close' });
      res.end(JSON.stringify({ jsonrpc: '2.0', id: parsed.id ?? 1, result }));
    });
  });

  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  try {
    const { port } = server.address();
    const commonArgs = [
      'scripts/deploy_mainnet.mjs',
      '--chain-label',
      'eth',
      '--rpc-url',
      `http://127.0.0.1:${port}`,
      '--private-key-file',
      pkFile,
      '--local-domain',
      '1',
      '--expected-chain-id',
      '1',
      '--latest-beefy-block',
      '1',
      '--current-vset-id',
      '1',
      '--current-vset-len',
      '1',
      '--current-vset-root',
      `0x${'22'.repeat(32)}`,
      '--next-vset-id',
      '2',
      '--next-vset-len',
      '1',
      '--next-vset-root',
      `0x${'33'.repeat(32)}`,
    ];

    const ok = await runNode(commonArgs);
    if (ok.status !== 0) {
      throw new Error(`dry-run invocation failed:\nSTDOUT:\n${ok.stdout}\nSTDERR:\n${ok.stderr}`);
    }
    const payload = JSON.parse(ok.stdout);
    if (payload.mode !== 'dry-run' || typeof payload.paramsHash !== 'string' || payload.paramsHash.length === 0) {
      throw new Error(`unexpected dry-run payload:\n${ok.stdout}`);
    }

    const bad = await runNode([...commonArgs, '--execut', 'yes']);
    if (bad.status === 0) {
      throw new Error('deploy_mainnet.mjs unexpectedly accepted an unknown flag');
    }
    if (!bad.stderr.includes('Unknown argument: --execut')) {
      throw new Error(`unexpected stderr for unknown flag:\n${bad.stderr}`);
    }
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
})().catch((err) => {
  console.error(err);
  process.exit(1);
});
NODE
python3 -B - <<'PY'
import subprocess

cases = [
    (["bash", "./scripts/test_formal_assisted.sh", "--profile"], "missing value for --profile"),
    (["bash", "./scripts/fuzz_foundry.sh", "--runs"], "missing value for --runs"),
    (["bash", "./scripts/fuzz_echidna.sh", "--timeout-secs"], "missing value for --timeout-secs"),
    (["bash", "./scripts/check_branch_protection.sh", "--approvals"], "missing value for --approvals"),
    (["bash", "./scripts/apply_branch_protection.sh", "--repo"], "missing value for --repo"),
]

for cmd, expected in cases:
    proc = subprocess.run(cmd, text=True, capture_output=True, check=False)
    if proc.returncode == 0:
        raise SystemExit(f"unexpected success for {' '.join(cmd)}")
    stderr = proc.stderr.strip()
    if expected not in stderr:
        raise SystemExit(f"unexpected stderr for {' '.join(cmd)}:\n{stderr}")
PY
python3 -B - <<'PY'
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

jq_path = shutil.which("jq")
if not jq_path:
    raise SystemExit("jq is required for branch-protection stub test")

tmpdir = Path(tempfile.mkdtemp(prefix="sccp-branch-protection-"))
bindir = tmpdir / "bin"
bindir.mkdir()
log_path = tmpdir / "gh.log"

(bindir / "gh").write_text(
    f"""#!/usr/bin/env bash
set -euo pipefail
printf '%s\\n' "$*" >> "{log_path}"
if [[ "$1" == "repo" && "$2" == "view" ]]; then
  if [[ "$*" == *"defaultBranchRef"* ]]; then
    printf 'main\\n'
  else
    printf 'owner/repo\\n'
  fi
  exit 0
fi
if [[ "$1" == "api" ]]; then
  case "$*" in
    *"release/2026-03/protection"*)
      echo "unencoded branch path" >&2
      exit 7
      ;;
    *"release%2F2026-03/protection"*)
      cat <<'JSON'
{{"required_status_checks":{{"contexts":["SCCP CI Lint / lint","SCCP Formal Assisted / formal_assisted"],"strict":true}},"required_pull_request_reviews":{{"required_approving_review_count":1,"require_code_owner_reviews":false}},"required_linear_history":{{"enabled":true}},"allow_force_pushes":{{"enabled":false}},"allow_deletions":{{"enabled":false}},"required_conversation_resolution":{{"enabled":true}},"enforce_admins":{{"enabled":true}}}}
JSON
      exit 0
      ;;
  esac
fi
echo "unexpected gh args: $*" >&2
exit 9
""",
    encoding="utf-8",
)
(bindir / "gh").chmod(0o755)
(bindir / "jq").write_text(f"""#!/usr/bin/env bash
exec "{jq_path}" "$@"
""", encoding="utf-8")
(bindir / "jq").chmod(0o755)

env = os.environ.copy()
env["PATH"] = f"{bindir}:{env['PATH']}"

check_proc = subprocess.run(
    [
        "bash",
        "./scripts/check_branch_protection.sh",
        "--repo",
        "owner/repo",
        "--branch",
        "release/2026-03",
    ],
    text=True,
    capture_output=True,
    check=False,
    env=env,
)
if check_proc.returncode != 0:
    raise SystemExit(
        "check_branch_protection.sh failed for slash-containing branch:\n"
        f"STDOUT:\n{check_proc.stdout}\nSTDERR:\n{check_proc.stderr}"
    )

apply_proc = subprocess.run(
    [
        "bash",
        "./scripts/apply_branch_protection.sh",
        "--repo",
        "owner/repo",
        "--branch",
        "release/2026-03",
    ],
    text=True,
    capture_output=True,
    check=False,
    env=env,
)
if apply_proc.returncode != 0:
    raise SystemExit(
        "apply_branch_protection.sh failed for slash-containing branch:\n"
        f"STDOUT:\n{apply_proc.stdout}\nSTDERR:\n{apply_proc.stderr}"
    )

log = log_path.read_text(encoding="utf-8")
if "release%2F2026-03/protection" not in log:
    raise SystemExit(f"encoded branch ref missing from gh calls:\n{log}")
if "release/2026-03/protection" in log:
    raise SystemExit(f"unencoded branch ref still present in gh calls:\n{log}")
PY
python3 -B - <<'PY'
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

tmpdir = Path(tempfile.mkdtemp(prefix="sccp-echidna-wrapper-"))
bindir = tmpdir / "bin"
bindir.mkdir()
outdir_name = f".tmp-sccp-echidna-out-{next(tempfile._get_candidate_names())}"

(bindir / "forge").write_text(
    """#!/usr/bin/env bash
set -euo pipefail
out_dir="out"
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --out)
      out_dir="${2:-}"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done
mkdir -p "${out_dir}/build-info"
printf '%s\n' '{"output":{}}' > "${out_dir}/build-info/fake.json"
""",
    encoding="utf-8",
)
(bindir / "forge").chmod(0o755)

(bindir / "echidna").write_text(
    """#!/usr/bin/env bash
set -euo pipefail
saw_timeout=0
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --timeout)
      if [[ "${2:-}" != "7" ]]; then
        echo "unexpected timeout: ${2:-}" >&2
        exit 1
      fi
      saw_timeout=1
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done
if [[ "${saw_timeout}" != "1" ]]; then
  echo "missing --timeout" >&2
  exit 1
fi
""",
    encoding="utf-8",
)
(bindir / "echidna").chmod(0o755)

env = os.environ.copy()
env["PATH"] = f"{bindir}:{env['PATH']}"

proc = subprocess.run(
    [
        "bash",
        "./scripts/fuzz_echidna.sh",
        "--timeout-secs",
        "7",
        "--foundry-out-dir",
        outdir_name,
    ],
    text=True,
    capture_output=True,
    check=False,
    env=env,
)

if proc.returncode != 0:
    raise SystemExit(
        "stubbed fuzz_echidna.sh invocation failed:\n"
        f"STDOUT:\n{proc.stdout}\nSTDERR:\n{proc.stderr}"
    )

if "[sccp-fuzz-echidna] timeout=7s" not in proc.stdout:
    raise SystemExit(f"unexpected stdout:\n{proc.stdout}")

sentinel = tmpdir / "sentinel.txt"
sentinel.write_text("present\n", encoding="utf-8")

danger = subprocess.run(
    [
        "bash",
        "./scripts/fuzz_echidna.sh",
        "--foundry-out-dir",
        ".",
    ],
    text=True,
    capture_output=True,
    check=False,
    env=env,
)

if danger.returncode == 0:
    raise SystemExit("fuzz_echidna.sh unexpectedly accepted --foundry-out-dir .")

if "foundry-out-dir must not contain empty, '.' , or '..' path segments" not in danger.stderr:
    raise SystemExit(f"unexpected stderr for dangerous foundry-out-dir:\n{danger.stderr}")

if sentinel.read_text(encoding="utf-8") != "present\n":
    raise SystemExit("dangerous foundry-out-dir validation ran too late")

shutil.rmtree(outdir_name, ignore_errors=True)
PY
SCCP_FUZZ_RUNS=3 npm run test:fuzz:fastcheck >/dev/null
