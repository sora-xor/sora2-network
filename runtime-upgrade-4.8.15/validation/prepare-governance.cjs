'use strict';
// Encodes the unsigned governance calls for a runtime upgrade from public
// finalized state. Read-only RPC; never signs or submits anything.
// Usage: node prepare-governance.cjs [--candidate-spec 140|141] [--rehearsal <json>] [--block <hash>] <wasm> <outdir>
const { createRequire } = require('node:module');
const { parseArgs } = require('node:util');
const { candidateSpec, finalizedBlock } = require('./release-options.cjs');
const req = createRequire(process.env.SORA_VALIDATION_PACKAGE || require('node:path').resolve(__dirname, '../../runtime-upgrade-4.8.12/validation/package.json'));
const { ApiPromise, WsProvider } = req('@polkadot/api');
const { TypeRegistry, Metadata } = req('@polkadot/types');
const { decorateExtrinsics } = req('@polkadot/types/metadata');
const { fullOverrideBundle } = req('@sora-substrate/type-definitions');
const { blake2AsHex, cryptoWaitReady } = req('@polkadot/util-crypto');
const { u8aToHex } = req('@polkadot/util');
const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');
const assert = require('node:assert/strict');

const ENDPOINT = 'wss://mof2.sora.org';
const READONLY = new Set(['chain_getBlock', 'chain_getBlockHash', 'chain_getHeader', 'chain_getFinalizedHead', 'state_getStorage',
  'state_getKeysPaged', 'state_queryStorageAt', 'state_getMetadata', 'state_getRuntimeVersion', 'state_call', 'system_properties',
  'system_chain', 'system_name', 'system_version', 'rpc_methods', 'state_subscribeRuntimeVersion', 'state_unsubscribeRuntimeVersion']);
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');

function validateCandidate(wasm, rehearsal, spec) {
  assert(rehearsal.ok === true && !rehearsal.error, 'A passing live fork rehearsal is required');
  assert(Array.isArray(rehearsal.checks) && rehearsal.checks.every((check) => check.ok === true), 'Rehearsal checks did not pass');
  const names = new Set(rehearsal.checks.map((check) => check.name));
  assert(names.size >= 19 && names.has(`upgrade block applied runtime ${spec}`) &&
    names.has(`the next block builds on runtime ${spec}`) && names.has('workers see halt #1 at its block'),
  'Rehearsal is missing completed upgrade or halt checks');
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
  const rehearsal = JSON.parse(fs.readFileSync(rehearsalPath, 'utf8'));
  validateCandidate(wasm, rehearsal, spec);
  await cryptoWaitReady();
  const api = await connect();
  try {
    await writeCalls(api, { requestedBlock: values.block ?? null, wasmPath, outDir, wasm, rehearsal });
  } finally {
    await api.disconnect();
  }
}

async function writeCalls(api, { requestedBlock, wasmPath, outDir, wasm, rehearsal }) {
  const finalized = await finalizedBlock(api.rpc.chain, requestedBlock);
  const { hash } = finalized;
  const [at, version, metadata, code] = await Promise.all([
    api.at(hash), api.rpc.state.getRuntimeVersion(hash),
    api.rpc.state.getMetadata(hash), api.rpc.state.getStorage('0x3a636f6465', hash),
  ]);
  assert.equal(version.specVersion.toNumber(), rehearsal.deployed.specVersion, 'Finalized runtime differs from the rehearsed baseline');
  assert.equal(at.runtimeVersion.specVersion.toNumber(), version.specVersion.toNumber(), 'Finalized block crosses a runtime upgrade; retry on the next finalized block');
  assert.equal(sha256(code.unwrap().toU8a(true)), rehearsal.deployed.codeSha256, 'Finalized Wasm differs from the rehearsed baseline');
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
    preparedAt: new Date().toISOString(), endpoint: ENDPOINT, signed: false, submittedTransactions: 0,
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
  fs.mkdirSync(outDir, { recursive: true });
  fs.writeFileSync(path.join(outDir, 'set-code-call.scale'), setCodeBytes);
  fs.writeFileSync(path.join(outDir, 'set-code-call.hex'), setCode.toHex() + '\n');
  for (const [name, call] of Object.entries(calls)) {
    fs.writeFileSync(path.join(outDir, `${name}.hex`), call.toHex() + '\n');
  }
  fs.writeFileSync(path.join(outDir, 'governance-calls.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify({ finalized: report.finalized, proposalHash, setCodeBytes: setCodeBytes.length, council: report.council,
    technicalCommittee: report.technicalCommittee, nextExternal: report.democracy.nextExternal, calls: Object.keys(report.calls) }, null, 1));
}

if (require.main === module) {
  main(process.argv.slice(2)).catch((error) => { console.error(error); process.exit(1); });
}

module.exports = { main };
