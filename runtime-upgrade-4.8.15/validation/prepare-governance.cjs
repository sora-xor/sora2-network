'use strict';
// Encodes the unsigned governance calls for a runtime upgrade from public
// finalized state. Read-only RPC; never signs or submits anything.
// Usage: node prepare-governance.cjs [--candidate-spec 140|141] [--rehearsal <json>] [--block <hash>] <wasm> <outdir>
const { createRequire } = require('node:module');
const { parseArgs } = require('node:util');
const { MAINNET_GENESIS, sha256, candidateSpec, mainnetBaseline } = require('./release-options.cjs');
const req = createRequire(process.env.SORA_VALIDATION_PACKAGE || require('node:path').resolve(__dirname, '../../runtime-upgrade-4.8.12/validation/package.json'));
const { ApiPromise, WsProvider } = req('@polkadot/api');
const { TypeRegistry, Metadata } = req('@polkadot/types');
const { decorateExtrinsics } = req('@polkadot/types/metadata');
const { fullOverrideBundle } = req('@sora-substrate/type-definitions');
const { blake2AsHex, cryptoWaitReady } = req('@polkadot/util-crypto');
const { u8aToHex } = req('@polkadot/util');
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');

const ENDPOINT = 'wss://mof2.sora.org';
const READONLY = new Set(['chain_getBlock', 'chain_getBlockHash', 'chain_getHeader', 'chain_getFinalizedHead', 'state_getStorage',
  'state_getKeysPaged', 'state_queryStorageAt', 'state_getMetadata', 'state_getRuntimeVersion', 'state_call', 'system_properties',
  'system_chain', 'system_name', 'system_version', 'rpc_methods', 'state_subscribeRuntimeVersion', 'state_unsubscribeRuntimeVersion']);
// The published spec-140 report predates the genesis field. Only its exact
// released bytes are eligible for compatibility; its base is checked on RPC.
const ARCHIVED_140_REPORT_SHA256 = '3c6f31e21b8fd5391ef9d8fc47a3ee84d2146c1c91d4e5bc367552e94ed17235';

function validateCandidate(wasm, rehearsal, spec, archived) {
  assert(rehearsal.ok === true && !rehearsal.error, 'A passing live fork rehearsal is required');
  assert(Array.isArray(rehearsal.checks) && rehearsal.checks.every((check) => check.ok === true), 'Rehearsal checks did not pass');
  assert(!rehearsal.intermediate, 'A simulated intermediate upgrade is diagnostic only');
  const names = new Set(rehearsal.checks.map((check) => check.name));
  if (!archived) {
    assert.equal(rehearsal.genesisHash, MAINNET_GENESIS, 'Rehearsal genesis differs from mainnet');
    assert.match(rehearsal.deployed?.policyCommitment ?? '', /^0x[0-9a-f]{64}$/, 'Missing rehearsed recovery policy');
    const required = [
      'fork uses finalized mainnet state and the expected prior runtime', `candidate is spec ${spec}, transaction version 134`,
      `upgrade block applied runtime ${spec}`, 'no inherent failed in the upgrade block',
      rehearsal.deployed.specVersion >= 139 ? 'existing guardian state preserved' : 'migration seeded the requested guardian',
      'upgrade block weight fits in a block', 'native XOR issuance unchanged by the upgrade',
      'mainnet recovery policy preserved by the upgrade', "Root's recovery latch left as it was",
      'last halt and worker signal preserved by the upgrade', `the next block builds on runtime ${spec}`,
      'a non-guardian halt is rejected before inclusion', 'guardian halt advances the previous number',
      'guardian paid no fee', 'guardian balance unchanged', 'workers see the next halt at its block',
      'LastHalt records number, guardian and block', "Root's recovery latch still untouched", 'a second halt inside the cooldown is refused',
    ];
    assert(required.every((name) => names.has(name)), 'Rehearsal is missing completed upgrade or halt checks');
  }
  assert([138, 139, 140].includes(rehearsal.deployed?.specVersion) && rehearsal.deployed.specVersion < spec, 'Unsupported rehearsal baseline');
  assert(Number.isSafeInteger(rehearsal.base?.number) && rehearsal.base.number > 0, 'Missing rehearsed block number');
  assert.equal(rehearsal.candidate?.specVersion, spec, `Expected a spec ${spec} rehearsal`);
  assert.equal(rehearsal.candidate.transactionVersion, 134, 'Unexpected candidate transaction version');
  assert.equal(rehearsal.candidate.sha256, sha256(wasm), 'Wasm differs from the rehearsed candidate');
  assert.equal(rehearsal.candidate.bytes, wasm.length, 'Rehearsed Wasm length differs');
  assert(/^0x[0-9a-f]{64}$/.test(rehearsal.base?.hash), 'Missing rehearsed block hash');
  assert.match(rehearsal.deployed?.codeSha256 ?? '', /^[0-9a-f]{64}$/, 'Missing rehearsed baseline Wasm digest');
}

function callsForMetadata(metadataBytes) {
  const registry = new TypeRegistry();
  const metadata = new Metadata(registry, metadataBytes);
  registry.setMetadata(metadata, undefined, undefined, true);
  return { registry, tx: decorateExtrinsics(registry, metadata.asLatest, metadata.version) };
}

async function connectReadOnly() {
  const provider = new WsProvider(ENDPOINT);
  const send = provider.send.bind(provider);
  provider.send = (method, ...params) => { assert(READONLY.has(method), `forbidden RPC ${method}`); return send(method, ...params); };
  return ApiPromise.create({ provider, noInitWarn: true, typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } });
}

async function main(args, connect = connectReadOnly) {
  const { values, positionals } = parseArgs({
    args, allowPositionals: true,
    options: { rehearsal: { type: 'string' }, block: { type: 'string' }, 'candidate-spec': { type: 'string' } },
  });
  assert(positionals.length === 2, 'Expected <wasm> <outdir>');
  const [wasmPath, outDir] = positionals;
  const spec = candidateSpec(values['candidate-spec']);
  const rehearsalPath = values.rehearsal ?? path.join(__dirname, '../live-fork-rehearsal.json');
  const wasm = fs.readFileSync(wasmPath);
  const reportBytes = fs.readFileSync(rehearsalPath);
  const rehearsal = JSON.parse(reportBytes);
  const archived = spec === 140 && sha256(reportBytes) === ARCHIVED_140_REPORT_SHA256;
  validateCandidate(wasm, rehearsal, spec, archived);
  if (fs.existsSync(outDir)) assert.equal(fs.readdirSync(outDir).length, 0, 'Governance output directory must be new or empty');
  await cryptoWaitReady();
  const api = await connect();
  try {
    await writeCalls(api, { requestedBlock: values.block ?? null, wasmPath, outDir, wasm, rehearsal });
  } finally {
    await api.disconnect();
  }
}

async function writeCalls(api, { requestedBlock, wasmPath, outDir, wasm, rehearsal }) {
  const observed = await mainnetBaseline(api, rehearsal.base.hash);
  assert.deepEqual(observed.base, rehearsal.base, 'Rehearsal block does not match its finalized mainnet height');
  assert.equal(observed.deployed.specVersion, rehearsal.deployed.specVersion, 'Wrong runtime at rehearsed mainnet base');
  assert.equal(observed.deployed.codeSha256, rehearsal.deployed.codeSha256, 'Wrong Wasm at rehearsed mainnet base');
  if (rehearsal.deployed.policyCommitment !== undefined) {
    assert.equal(observed.deployed.policyCommitment, rehearsal.deployed.policyCommitment, 'Wrong recovery policy at rehearsed mainnet base');
  }
  const targetBase = requestedBlock === rehearsal.base.hash ? observed : await mainnetBaseline(api, requestedBlock);
  const { base: finalized, at, version } = targetBase;
  assert(finalized.number >= observed.base.number, 'Governance block predates rehearsed mainnet block');
  assert.deepEqual(targetBase.deployed, observed.deployed, 'Finalized runtime or policy differs from the rehearsed baseline');
  const metadata = await api.rpc.state.getMetadata(finalized.hash);
  const metadataBytes = metadata.toU8a();
  const { registry, tx } = callsForMetadata(metadataBytes);
  const setCode = tx.system.setCode(u8aToHex(wasm));
  const setCodeBytes = setCode.toU8a();
  const proposalHash = blake2AsHex(setCodeBytes);
  const target = { Lookup: { hash: proposalHash, len: setCodeBytes.length } };
  const [councilMembers, tcMembers, nextExternal, blacklist, requestStatus] = await Promise.all([
    at.query.council.members(), at.query.technicalCommittee.members(), at.query.democracy.nextExternal(),
    at.query.democracy.blacklist(proposalHash), at.query.preimage.requestStatusFor(proposalHash),
  ]);
  const councilThreshold = Math.ceil(councilMembers.length / 2);
  const tcThreshold = Math.floor(tcMembers.length / 2) + 1;
  const votingPeriod = at.consts.democracy.fastTrackVotingPeriod.toNumber();
  const guarded = tx.utility.batchAll([tx.democracy.externalPropose(target), tx.democracy.externalProposeMajority(target)]);
  const fast = tx.democracy.fastTrack(proposalHash, votingPeriod, 0);
  const calls = {
    'preimage-note-call': tx.preimage.notePreimage(u8aToHex(setCodeBytes)),
    'utility-guarded-external-majority-call': guarded,
    [`council-propose-guarded-external-majority-threshold-${councilThreshold}-call`]: tx.council.propose(councilThreshold, guarded, guarded.encodedLength),
    'democracy-fast-track-call': fast,
    [`technical-committee-fast-track-threshold-${tcThreshold}-call`]: tx.technicalCommittee.propose(tcThreshold, fast, fast.encodedLength),
  };
  const report = {
    preparedAt: new Date().toISOString(), endpoint: ENDPOINT, genesisHash: MAINNET_GENESIS, signed: false, submittedTransactions: 0,
    finalized: { ...finalized, metadataSha256: sha256(metadataBytes) },
    rehearsal: { base: rehearsal.base, candidateSha256: rehearsal.candidate.sha256, deployedCodeSha256: rehearsal.deployed.codeSha256 },
    liveRuntime: { specVersion: version.specVersion.toNumber(), transactionVersion: version.transactionVersion.toNumber() },
    wasm: { file: path.basename(wasmPath), bytes: wasm.length, sha256: sha256(wasm), blake2_256: blake2AsHex(wasm) },
    setCode: { file: 'set-code-call.scale', bytes: setCodeBytes.length, proposalHash },
    council: { members: councilMembers.length, threshold: councilThreshold },
    technicalCommittee: { members: tcMembers.length, threshold: tcThreshold },
    democracy: { fastTrackVotingPeriod: votingPeriod, enactmentDelay: 0, nextExternal: nextExternal.toJSON(), proposalBlacklisted: blacklist.isSome },
    preimageRequestStatus: requestStatus.toJSON(),
    calls: {},
  };
  for (const [name, call] of Object.entries(calls)) {
    const decoded = registry.createType('Call', call.toHex());
    assert.equal(decoded.toHex(), call.toHex());
    report.calls[name] = { file: `${name}.hex`, bytes: call.encodedLength, hash: blake2AsHex(call.toU8a()), section: call.section, method: call.method };
  }
  const dest = path.resolve(outDir);
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  const staged = fs.mkdtempSync(path.join(path.dirname(dest), '.governance-'));
  try {
    fs.writeFileSync(path.join(staged, 'set-code-call.scale'), setCodeBytes);
    fs.writeFileSync(path.join(staged, 'set-code-call.hex'), setCode.toHex() + '\n');
    for (const [name, call] of Object.entries(calls)) {
      fs.writeFileSync(path.join(staged, `${name}.hex`), call.toHex() + '\n');
    }
    fs.writeFileSync(path.join(staged, 'governance-calls.json'), JSON.stringify(report, null, 2) + '\n');
    // POSIX rename can publish over an empty directory, never over a previous
    // completed generation. Concurrent writers cannot interleave their files.
    fs.renameSync(staged, dest);
  } finally {
    fs.rmSync(staged, { recursive: true, force: true });
  }
  console.log(JSON.stringify({ finalized: report.finalized, proposalHash, setCodeBytes: setCodeBytes.length, council: report.council,
    technicalCommittee: report.technicalCommittee, nextExternal: report.democracy.nextExternal, calls: Object.keys(report.calls) }, null, 1));
}

if (require.main === module) {
  main(process.argv.slice(2)).catch((error) => { console.error(error); process.exit(1); });
}

module.exports = { main };
