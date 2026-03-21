import test from 'node:test';
import assert from 'node:assert/strict';
import { chmodSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const proofCellScript = join(process.cwd(), 'scripts', 'encode_sora_proof_cell.mjs');
const attesterProofScript = join(process.cwd(), 'scripts', 'encode_attester_quorum_proof.mjs');
const e2eAdapterScript = join(process.cwd(), 'scripts', 'sccp_e2e_adapter.sh');
const applyBranchProtectionScript = join(process.cwd(), 'scripts', 'apply_branch_protection.sh');
const checkBranchProtectionScript = join(process.cwd(), 'scripts', 'check_branch_protection.sh');
const deriveMasterAddressScript = join(process.cwd(), 'scripts', 'derive_master_address.mjs');
const deployMainnetScript = join(process.cwd(), 'scripts', 'deploy_mainnet.mjs');
const deployMainnetPythonScript = join(process.cwd(), 'scripts', 'deploy_mainnet.py');
const ZERO_ADDR = '0:0000000000000000000000000000000000000000000000000000000000000000';
const SAMPLE_SORA_ASSET_ID = `0x${'11'.repeat(32)}`;

function makeStubToolDir() {
  const dir = mkdtempSync(join(tmpdir(), 'sccp-ton-stub-tools-'));
  for (const tool of ['gh', 'jq']) {
    const path = join(dir, tool);
    writeFileSync(path, '#!/bin/sh\nexit 0\n', 'utf8');
    chmodSync(path, 0o755);
  }
  return dir;
}

test('proof-cell encoder rejects missing option values cleanly', () => {
  const res = spawnSync(process.execPath, [proofCellScript, '--input', '--format', 'hex'], {
    encoding: 'utf8',
  });

  assert.notEqual(res.status, 0);
  assert.match(res.stderr, /missing value for --input/);
});

test('attester proof encoder rejects missing option values with usage output', () => {
  const res = spawnSync(
    process.execPath,
    [
      attesterProofScript,
      '--message-id',
      `0x${'11'.repeat(32)}`,
      '--sig',
      '--privkey',
      `0x${'22'.repeat(32)}`,
    ],
    {
      encoding: 'utf8',
    },
  );

  assert.equal(res.status, 2);
  assert.match(res.stderr, /^Usage:/m);
});

test('attester proof encoder rejects malformed hex inputs without leaking stack traces', () => {
  const badMessageId = spawnSync(
    process.execPath,
    [
      attesterProofScript,
      '--message-id',
      '0x1',
      '--sig',
      `0x${'11'.repeat(65)}`,
    ],
    {
      encoding: 'utf8',
    },
  );

  assert.notEqual(badMessageId.status, 0);
  assert.match(badMessageId.stderr, /error: messageId must have an even number of hex digits/);
  assert.doesNotMatch(badMessageId.stderr, /node_modules\/ethers|TypeError:/);

  const badPrivkey = spawnSync(
    process.execPath,
    [
      attesterProofScript,
      '--message-id',
      SAMPLE_SORA_ASSET_ID,
      '--privkey',
      '123',
    ],
    {
      encoding: 'utf8',
    },
  );

  assert.notEqual(badPrivkey.status, 0);
  assert.match(badPrivkey.stderr, /error: private key must have an even number of hex digits/);
  assert.doesNotMatch(badPrivkey.stderr, /node_modules\/ethers|TypeError:/);
});

test('e2e adapter keeps quoted scenario ids as valid JSON output', () => {
  const res = spawnSync(
    'bash',
    [e2eAdapterScript, 'burn', '--json', JSON.stringify({ scenario_id: 'bad"id' })],
    {
      encoding: 'utf8',
    },
  );

  assert.equal(res.status, 0);
  assert.deepEqual(JSON.parse(res.stdout), {
    ok: true,
    domain: 'ton',
    scenario_id: 'bad"id',
    action: 'burn',
    assertions: ['adapter-command-succeeded'],
  });
});

test('e2e adapter preserves explicit falsy scenario ids', () => {
  const res = spawnSync('bash', [e2eAdapterScript, 'burn', '--json', JSON.stringify({ scenario_id: 0 })], {
    encoding: 'utf8',
  });

  assert.equal(res.status, 0);
  assert.deepEqual(JSON.parse(res.stdout), {
    ok: true,
    domain: 'ton',
    scenario_id: '0',
    action: 'burn',
    assertions: ['adapter-command-succeeded'],
  });
});

test('apply branch protection rejects missing option values cleanly', () => {
  const stubToolDir = makeStubToolDir();
  try {
    const res = spawnSync('bash', [applyBranchProtectionScript, '--repo'], {
      encoding: 'utf8',
      env: { ...process.env, PATH: `${stubToolDir}:${process.env.PATH}` },
    });

    assert.notEqual(res.status, 0);
    assert.match(res.stderr, /Missing value for --repo/);
  } finally {
    rmSync(stubToolDir, { recursive: true, force: true });
  }
});

test('check branch protection rejects missing option values cleanly', () => {
  const stubToolDir = makeStubToolDir();
  try {
    const res = spawnSync('bash', [checkBranchProtectionScript, '--branch'], {
      encoding: 'utf8',
      env: { ...process.env, PATH: `${stubToolDir}:${process.env.PATH}` },
    });

    assert.notEqual(res.status, 0);
    assert.match(res.stderr, /Missing value for --branch/);
  } finally {
    rmSync(stubToolDir, { recursive: true, force: true });
  }
});

test('derive master address rejects missing values for optional flags', () => {
  const res = spawnSync(
    process.execPath,
    [
      deriveMasterAddressScript,
      '--governor',
      ZERO_ADDR,
      '--sora-asset-id',
      SAMPLE_SORA_ASSET_ID,
      '--metadata-uri',
      '--verifier',
      ZERO_ADDR,
    ],
    {
      encoding: 'utf8',
    },
  );

  assert.notEqual(res.status, 0);
  assert.match(res.stderr, /Missing value for --metadata-uri/);
});

test('derive master address rejects unexpected positional arguments', () => {
  const res = spawnSync(
    process.execPath,
    [
      deriveMasterAddressScript,
      '--governor',
      ZERO_ADDR,
      '--sora-asset-id',
      SAMPLE_SORA_ASSET_ID,
      'stray',
    ],
    {
      encoding: 'utf8',
    },
  );

  assert.notEqual(res.status, 0);
  assert.match(res.stderr, /Unexpected positional argument: stray/);
});

test('deploy script rejects unknown and positional arguments cleanly', () => {
  const dir = mkdtempSync(join(tmpdir(), 'sccp-ton-deploy-args-'));
  const mnemonicPath = join(dir, 'mnemonic.txt');
  writeFileSync(mnemonicPath, 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about\n', 'utf8');

  try {
    const positionalRes = spawnSync(
      process.execPath,
      [
        deployMainnetScript,
        '--mnemonic-file',
        mnemonicPath,
        '--governor',
        ZERO_ADDR,
        '--sora-asset-id',
        SAMPLE_SORA_ASSET_ID,
        'stray',
      ],
      {
        encoding: 'utf8',
      },
    );

    assert.notEqual(positionalRes.status, 0);
    assert.match(positionalRes.stderr, /Unexpected positional argument: stray/);

    const unknownFlagRes = spawnSync(
      process.execPath,
      [
        deployMainnetScript,
        '--mnemonic-file',
        mnemonicPath,
        '--governor',
        ZERO_ADDR,
        '--sora-asset-id',
        SAMPLE_SORA_ASSET_ID,
        '--bogus-flag',
        'value',
      ],
      {
        encoding: 'utf8',
      },
    );

    assert.notEqual(unknownFlagRes.status, 0);
    assert.match(unknownFlagRes.stderr, /Unknown argument: --bogus-flag/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('python deploy wrapper resolves relative output paths before handing off to node', () => {
  const stubToolDir = mkdtempSync(join(tmpdir(), 'sccp-ton-stub-node-'));
  const callerDir = mkdtempSync(join(tmpdir(), 'sccp-ton-python-wrapper-'));
  const resolvedCallerDir = realpathSync(callerDir);
  const capturePath = join(callerDir, 'capture.json');
  const mnemonicPath = join(callerDir, 'mnemonic.txt');
  writeFileSync(mnemonicPath, 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about\n', 'utf8');
  writeFileSync(
    join(stubToolDir, 'node'),
    `#!/bin/sh
python3 - "$CAPTURE_PATH" "$PWD" "$@" <<'PY'
import json
import sys
from pathlib import Path
capture_path = Path(sys.argv[1])
cwd = sys.argv[2]
argv = sys.argv[3:]
capture_path.write_text(json.dumps({"cwd": cwd, "argv": argv}), encoding="utf-8")
PY
exit 0
`,
    'utf8',
  );
  chmodSync(join(stubToolDir, 'node'), 0o755);

  try {
    const res = spawnSync(
      'python3',
      [
        deployMainnetPythonScript,
        '--skip-build',
        '--mnemonic-file',
        mnemonicPath,
        '--governor',
        ZERO_ADDR,
        '--sora-asset-id',
        SAMPLE_SORA_ASSET_ID,
        '--out',
        'out.json',
        '--state-file',
        'state.json',
      ],
      {
        cwd: callerDir,
        encoding: 'utf8',
        env: { ...process.env, PATH: `${stubToolDir}:${process.env.PATH}`, CAPTURE_PATH: capturePath },
      },
    );

    assert.equal(res.status, 0);
    const capture = JSON.parse(readFileSync(capturePath, 'utf8'));
    const outIndex = capture.argv.indexOf('--out');
    const stateIndex = capture.argv.indexOf('--state-file');
    assert.notEqual(outIndex, -1);
    assert.notEqual(stateIndex, -1);
    assert.equal(capture.argv[outIndex + 1], join(resolvedCallerDir, 'out.json'));
    assert.equal(capture.argv[stateIndex + 1], join(resolvedCallerDir, 'state.json'));
  } finally {
    rmSync(stubToolDir, { recursive: true, force: true });
    rmSync(callerDir, { recursive: true, force: true });
  }
});
