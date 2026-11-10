'use strict';

const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { parseOptions, guardianTransition } = require('./live-upgrade.cjs');
const { MAINNET_GENESIS, CODE_KEY, POLICY_KEY, finalizedBlock, RECORDED_BLOCK } = require('./release-options.cjs');
const { main: prepareGovernance } = require('./prepare-governance.cjs');

const root = path.resolve(__dirname, '../..');
const sha256 = (buffer) => createHash('sha256').update(buffer).digest('hex');
const knownHash = '0x' + '42'.repeat(32);
const latestHash = '0x' + '43'.repeat(32);
const policyHash = '0x' + '11'.repeat(32);
const asHash = (value) => ({ toHex: () => value });
const asNumber = (value) => ({ toNumber: () => value });
const baseline = Buffer.from('rehearsed deployed code');

function chainRpc() {
  return {
    getFinalizedHead: async () => asHash(latestHash),
    getHeader: async (hash) => {
      const hex = typeof hash === 'string' ? hash : hash.toHex();
      assert([latestHash, knownHash, RECORDED_BLOCK].includes(hex));
      return { number: asNumber(hex === knownHash ? 28296249 : hex === RECORDED_BLOCK ? 28296247 : 28296251) };
    },
    getBlockHash: async (n) => asHash(n === 0 ? MAINNET_GENESIS : n === 28296247 ? RECORDED_BLOCK : n === 28296249 ? knownHash : latestHash),
  };
}

test('the replay defaults to the recorded block and supports explicit archival and fresh modes', async () => {
  assert.equal(parseOptions(['--candidate-spec', '140', 'candidate', 'report']).block, RECORDED_BLOCK);
  const pinned = parseOptions(['--block', knownHash, '--candidate-spec', '141', 'candidate']);
  assert.equal(pinned.spec, 141);
  assert.equal(parseOptions(['--via-140', 'candidate']).via140, true);
  assert.deepEqual(await finalizedBlock(chainRpc(), pinned.block), { hash: knownHash, number: 28296249 });
  const fresh = parseOptions(['candidate', '--latest-finalized']);
  assert.deepEqual(await finalizedBlock(chainRpc(), fresh.block), { hash: latestHash, number: 28296251 });
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
  const rehearsal = JSON.parse(fs.readFileSync(path.join(root, 'runtime-upgrade-4.8.14-spec-140/live-fork-rehearsal.json')));
  rehearsal.checks.forEach((check) => { check.name = check.name.replace(/140/g, String(spec)); });
  rehearsal.genesisHash = MAINNET_GENESIS;
  const labels = {
    'live chain runs the expected runtime': 'fork uses finalized mainnet state and the expected prior runtime',
    'guardian list is exactly the seeded account': 'mainnet recovery policy preserved by the upgrade',
    'no guardian halt before anyone halts': 'last halt and worker signal preserved by the upgrade',
    'guardian halt included as halt #1': 'guardian halt advances the previous number',
    'workers see halt #1 at its block': 'workers see the next halt at its block',
  };
  for (const check of rehearsal.checks) check.name = labels[check.name] ?? check.name;
  rehearsal.deployed.policyCommitment = policyHash;
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
    [(r) => { r.genesisHash = knownHash; }, /genesis differs/],
    [(r) => { delete r.genesisHash; }, /genesis differs/],
    [(r) => { r.intermediate = { specVersion: 140 }; }, /diagnostic only/],
  ]) {
    const changed = structuredClone(rehearsal);
    change(changed);
    fs.writeFileSync(rehearsalPath, JSON.stringify(changed));
    await assert.rejects(prepareGovernance(args, noRpc), expected);
    assert.deepEqual(fs.readdirSync(out), ['set-code-call.hex']);
    assert.equal(fs.readFileSync(path.join(out, 'set-code-call.hex'), 'utf8'), 'previous');
  }
});

function mockApi(metadataBytes, options = {}) {
  const baseHash = RECORDED_BLOCK;
  const scopeFor = (h) => {
    assert([baseHash, knownHash].includes(h));
    const version = { specVersion: asNumber(options.version ?? 138), transactionVersion: asNumber(134) };
    const empty = () => ({ toJSON: () => null, isSome: false });
    return {
      runtimeVersion: version,
      consts: { democracy: { fastTrackVotingPeriod: asNumber(1800) } },
      query: {
        system: { blockHash: (height) => { assert.equal(height, 0); return asHash(options.stateGenesis ?? MAINNET_GENESIS); } },
        council: { members: () => Array(options.seats ?? 8) }, technicalCommittee: { members: () => Array(4) },
        democracy: { blacklist: empty, nextExternal: empty }, preimage: { requestStatusFor: empty },
      },
    };
  };
  return {
    // Historical call encoding must not use the current head's metadata.
    get tx() { throw new Error('used live tx metadata'); },
    at: async (h) => scopeFor(h),
    rpc: {
      chain: options.chain ?? chainRpc(),
      state: {
        getRuntimeVersion: async (h) => scopeFor(h).runtimeVersion,
        getMetadata: async (h) => { assert.equal(h, knownHash); return { toU8a: () => metadataBytes }; },
        getStorage: async (key, h) => {
          assert([knownHash, baseHash].includes(h));
          assert([CODE_KEY, POLICY_KEY].includes(key));
          const value = key === CODE_KEY
            ? ((h === baseHash ? options.baseCode : options.targetCode) ?? baseline)
            : Buffer.from(((h === baseHash ? options.basePolicy : options.targetPolicy) ?? policyHash).slice(2), 'hex');
          return { isSome: true, unwrap: () => ({ toU8a: () => value }) };
        },
      },
    },
    disconnect: async () => {},
  };
}

const metadata = fs.readFileSync(path.join(root, 'runtime-rehearsal-async-staking-2026-10-25/council/evidence/live-metadata.scale'));

for (const spec of [140, 141]) {
  test(`spec ${spec} packages the rehearsed bytes using finalized governance metadata`, async (t) => {
    const { args, out, wasm, rehearsal } = fixture(t, spec);
    await assert.rejects(prepareGovernance(args, () => mockApi(metadata, { targetCode: Buffer.from('changed') })), /differs from the rehearsed baseline/);
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

test('rehearsal number, ancestry, genesis, deployed code and recovery policy must match target mainnet', async (t) => {
  const { args, out, rehearsal, rehearsalPath } = fixture(t, 141);
  const canonical = chainRpc();
  const changedHash = (n) => asHash(n === 28296247 ? latestHash : MAINNET_GENESIS);
  for (const [change, options, expected] of [
    [(r) => { r.base.number++; }, {}, /does not match its finalized mainnet height/],
    [() => {}, { chain: { ...canonical, getBlockHash: changedHash } }, /outside the finalized chain/],
    [() => {}, { chain: { ...canonical, getBlockHash: () => asHash(knownHash) } }, /RPC genesis differs/],
    [() => {}, { stateGenesis: latestHash }, /storage genesis differs/],
    [() => {}, { baseCode: Buffer.from('another runtime') }, /Wrong Wasm at rehearsed/],
    [() => {}, { basePolicy: latestHash }, /Wrong recovery policy at rehearsed/],
    [() => {}, { targetPolicy: latestHash }, /differs from the rehearsed baseline/],
  ]) {
    const modified = structuredClone(rehearsal);
    change(modified);
    fs.writeFileSync(rehearsalPath, JSON.stringify(modified));
    await assert.rejects(prepareGovernance(args, () => mockApi(metadata, options)), expected);
    assert(!fs.existsSync(out));
  }
});

test('existing guardians, an empty guardian set and prior halt are preserved on successor upgrades', () => {
  const original = { guardians: '0xaa', lastHalt: '0xbb', signal: '0xcc' };
  for (const spec of [139, 140]) {
    assert(guardianTransition(spec, original, { ...original }, ['custom'], false));
    const empty = { ...original, guardians: '0x00' };
    assert(guardianTransition(spec, empty, { ...empty }, [], false));
    assert(!guardianTransition(spec, empty, original, ['custom'], false));
    for (const key of ['guardians', 'lastHalt', 'signal']) {
      assert(!guardianTransition(spec, original, { ...original, [key]: null }, [], false));
    }
    assert(!guardianTransition(spec, original, original, [], true));
  }
});

test('spec 140 with established guardians qualifies for a 141 call', async (t) => {
  const { args, rehearsal, rehearsalPath, out } = fixture(t, 141);
  rehearsal.deployed.specVersion = 140;
  rehearsal.checks.find((c) => c.name === 'migration seeded the requested guardian').name = 'existing guardian state preserved';
  fs.writeFileSync(rehearsalPath, JSON.stringify(rehearsal));
  await prepareGovernance(args, () => mockApi(metadata, { version: 140 }));
  assert.equal(JSON.parse(fs.readFileSync(path.join(out, 'governance-calls.json'))).liveRuntime.specVersion, 140);
});

test('publishing refuses stale generations and concurrent writers cannot mix calls', async (t) => {
  const { args, out } = fixture(t, 141);
  fs.mkdirSync(out);
  const results = await Promise.allSettled([8, 10].map((seats) =>
    prepareGovernance(args, () => mockApi(metadata, { seats }))));
  assert.equal(results.filter((r) => r.status === 'fulfilled').length, 1);
  const { council } = JSON.parse(fs.readFileSync(path.join(out, 'governance-calls.json')));
  const files = fs.readdirSync(out);
  assert.deepEqual(files.filter((f) => f.startsWith('council-')),
    [`council-propose-guarded-external-majority-threshold-${council.threshold}-call.hex`]);
  await assert.rejects(prepareGovernance(args, () => { throw new Error('unexpected connection'); }), /new or empty/);
  assert.deepEqual(fs.readdirSync(out), files);
  assert(!fs.readdirSync(path.dirname(out)).some((f) => f.startsWith('.governance-')));
});


test('a write failure cannot expose a partial output generation', async (t) => {
  const { args, out } = fixture(t, 141);
  const write = fs.writeFileSync;
  t.mock.method(fs, 'writeFileSync', (...params) => {
    if (path.basename(params[0]) === 'preimage-note-call.hex') throw new Error('simulated disk failure');
    return write(...params);
  });
  await assert.rejects(prepareGovernance(args, () => mockApi(metadata)), /simulated disk failure/);
  assert(!fs.existsSync(out));
  assert(!fs.readdirSync(path.dirname(out)).some((name) => name.startsWith('.governance-')));
});
