'use strict';

const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { parseOptions } = require('./live-upgrade.cjs');
const { finalizedBlock, RECORDED_BLOCK } = require('./release-options.cjs');
const { main: prepareGovernance } = require('./prepare-governance.cjs');

const root = path.resolve(__dirname, '../..');
const sha256 = (buffer) => createHash('sha256').update(buffer).digest('hex');
const knownHash = '0x' + '42'.repeat(32);
const latestHash = '0x' + '43'.repeat(32);
const asHash = (value) => ({ toHex: () => value });
const asNumber = (value) => ({ toNumber: () => value });
const baseline = Buffer.from('rehearsed deployed code');

function chainRpc() {
  return {
    getFinalizedHead: async () => asHash(latestHash),
    getHeader: async (hash) => {
      const hex = typeof hash === 'string' ? hash : hash.toHex();
      assert([latestHash, knownHash].includes(hex));
      return { number: asNumber(hex === knownHash ? 27 : 31) };
    },
    getBlockHash: async (n) => asHash(n === 27 ? knownHash : latestHash),
  };
}

test('the replay defaults to the recorded block and supports explicit archival and fresh modes', async () => {
  assert.equal(parseOptions(['--candidate-spec', '140', 'candidate', 'report']).block, RECORDED_BLOCK);
  const pinned = parseOptions(['--block', knownHash, '--candidate-spec', '141', 'candidate']);
  assert.equal(pinned.spec, 141);
  assert.deepEqual(await finalizedBlock(chainRpc(), pinned.block), { hash: knownHash, number: 27 });
  const fresh = parseOptions(['candidate', '--latest-finalized']);
  assert.deepEqual(await finalizedBlock(chainRpc(), fresh.block), { hash: latestHash, number: 31 });
  assert.throws(() => parseOptions(['--latest-finalized', '--block', knownHash, 'candidate']), /either/);
  assert.throws(() => parseOptions(['candidate', '--block', '27']), /32-byte/);
  assert.throws(() => parseOptions(['candidate', '--candidate-spec', '138']), /140 or 141/);
  const fork = { ...chainRpc(), getBlockHash: async () => asHash(latestHash) };
  await assert.rejects(finalizedBlock(fork, knownHash), /outside the finalized chain/);
});

function fixture(t, spec) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'sora-release-validation-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const wasmPath = path.join(dir, 'candidate.wasm');
  const wasm = Buffer.from([0, 97, 115, 109, spec]);
  fs.writeFileSync(wasmPath, wasm);
  const rehearsalPath = path.join(dir, 'rehearsal.json');
  const out = path.join(dir, 'out');
  const rehearsal = JSON.parse(fs.readFileSync(path.join(root, 'runtime-upgrade-4.8.14/live-fork-rehearsal.json')));
  rehearsal.checks.forEach((check) => { check.name = check.name.replace(/140/g, String(spec)); });
  rehearsal.candidate = { specVersion: spec, transactionVersion: 134, bytes: wasm.length, sha256: sha256(wasm) };
  rehearsal.deployed.codeSha256 = sha256(baseline);
  fs.writeFileSync(rehearsalPath, JSON.stringify(rehearsal));
  const args = ['--candidate-spec', String(spec), '--rehearsal', rehearsalPath, '--block', knownHash, wasmPath, out];
  return { wasm, wasmPath, rehearsal, rehearsalPath, args, out };
}

test('invalid candidate or rehearsal leaves the output untouched and opens no RPC connection', async (t) => {
  const { args, out, rehearsal, rehearsalPath } = fixture(t, 141);
  fs.mkdirSync(out);
  fs.writeFileSync(path.join(out, 'set-code-call.hex'), 'previous');
  const noRpc = () => { throw new Error('unexpected RPC connection'); };
  for (const [change, expected] of [
    [(r) => { r.ok = false; }, /passing/],
    [(r) => { r.checks[0].ok = false; }, /checks did not pass/],
    [(r) => { r.checks.pop(); }, /missing completed/],
    [(r) => { r.candidate.specVersion = 140; }, /spec 141 rehearsal/],
    [(r) => { r.candidate.sha256 = '0'.repeat(64); }, /differs from the rehearsed/],
    [(r) => { r.candidate.bytes++; }, /length differs/],
  ]) {
    const changed = structuredClone(rehearsal);
    change(changed);
    fs.writeFileSync(rehearsalPath, JSON.stringify(changed));
    await assert.rejects(prepareGovernance(args, noRpc), expected);
    assert.deepEqual(fs.readdirSync(out), ['set-code-call.hex']);
    assert.equal(fs.readFileSync(path.join(out, 'set-code-call.hex'), 'utf8'), 'previous');
  }
});

function mockApi(metadataBytes, deployedCode = baseline) {
  const version = { specVersion: asNumber(138), transactionVersion: asNumber(134) };
  const empty = () => ({ toJSON: () => null, isSome: false });
  const scope = {
    runtimeVersion: version,
    consts: { democracy: { fastTrackVotingPeriod: asNumber(1800) } },
    query: {
      council: { members: () => Array(8) }, technicalCommittee: { members: () => Array(4) },
      democracy: { blacklist: empty, nextExternal: empty }, preimage: { requestStatusFor: empty },
    },
  };
  return {
    // Fail if the encoder accidentally uses the current head's call metadata.
    get tx() { throw new Error('used live tx metadata'); },
    at: async (h) => { assert.equal(h, knownHash); return scope; },
    rpc: {
      chain: chainRpc(),
      state: {
        getRuntimeVersion: async (h) => { assert.equal(h, knownHash); return version; },
        getMetadata: async (h) => { assert.equal(h, knownHash); return { toU8a: () => metadataBytes }; },
        getStorage: async (key, h) => {
          assert.equal(key, '0x3a636f6465');
          assert.equal(h, knownHash);
          return { unwrap: () => ({ toU8a: () => deployedCode }) };
        },
      },
    },
    disconnect: async () => {},
  };
}

for (const spec of [140, 141]) {
  test(`spec ${spec} packages the rehearsed bytes using finalized governance metadata`, async (t) => {
    const { args, out, wasm, rehearsal } = fixture(t, spec);
    const metadata = fs.readFileSync(path.join(root, 'runtime-rehearsal-async-staking-2026-10-25/council/evidence/live-metadata.scale'));
    await assert.rejects(prepareGovernance(args, () => mockApi(metadata, Buffer.from('changed'))), /differs from the rehearsed baseline/);
    assert(!fs.existsSync(out));
    await prepareGovernance(args, () => mockApi(metadata));
    const report = JSON.parse(fs.readFileSync(path.join(out, 'governance-calls.json')));
    assert.equal(report.finalized.hash, knownHash);
    assert.equal(report.finalized.metadataSha256, sha256(metadata));
    assert.equal(report.rehearsal.candidateSha256, rehearsal.candidate.sha256);
    const setCode = fs.readFileSync(path.join(out, 'set-code-call.scale'));
    assert.deepEqual(setCode, Buffer.concat([Buffer.from([0, 2, wasm.length << 2]), wasm]));
    const hex = (name) => fs.readFileSync(path.join(out, name + '.hex'), 'utf8').trim();
    assert.equal(hex('preimage-note-call'), '0x3800' + (setCode.length << 2).toString(16).padStart(2, '0') + setCode.toString('hex'));
    assert(hex('technical-committee-fast-track-threshold-3-call').includes(report.setCode.proposalHash.slice(2)));
  });
}
