'use strict';
// Encodes the unsigned governance calls for a runtime upgrade from public
// finalized state. Read-only RPC; never signs or submits anything.
// Usage: node prepare-governance.cjs <wasm> <outdir>
const { createRequire } = require('node:module');
const req = createRequire(process.env.SORA_VALIDATION_PACKAGE || require('node:path').resolve(__dirname, '../../runtime-upgrade-4.8.12/validation/package.json'));
const { ApiPromise, WsProvider } = req('@polkadot/api');
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

(async () => {
  await cryptoWaitReady();
  const [wasmPath, outDir] = process.argv.slice(2);
  const wasm = fs.readFileSync(wasmPath);
  const provider = new WsProvider(ENDPOINT);
  const send = provider.send.bind(provider);
  provider.send = (method, ...params) => { assert(READONLY.has(method), `forbidden RPC ${method}`); return send(method, ...params); };
  const api = await ApiPromise.create({ provider, noInitWarn: true, typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } });
  const hash = await api.rpc.chain.getFinalizedHead();
  const at = await api.at(hash);
  const header = await api.rpc.chain.getHeader(hash);
  const version = await api.rpc.state.getRuntimeVersion(hash);

  const setCode = api.tx.system.setCode(u8aToHex(wasm)).method;
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
  const guarded = api.tx.utility.batchAll([api.tx.democracy.externalPropose(target), api.tx.democracy.externalProposeMajority(target)]).method;
  const fast = api.tx.democracy.fastTrack(proposalHash, votingPeriod, 0).method;
  const calls = {
    'preimage-note-call': api.tx.preimage.notePreimage(u8aToHex(setCodeBytes)).method,
    'utility-guarded-external-majority-call': guarded,
    [`council-propose-guarded-external-majority-threshold-${councilThreshold}-call`]: api.tx.council.propose(councilThreshold, guarded, guarded.encodedLength).method,
    'democracy-fast-track-call': fast,
    [`technical-committee-fast-track-threshold-${tcThreshold}-call`]: api.tx.technicalCommittee.propose(tcThreshold, fast, fast.encodedLength).method,
  };
  fs.mkdirSync(outDir, { recursive: true });
  fs.writeFileSync(path.join(outDir, 'set-code-call.scale'), setCodeBytes);
  fs.writeFileSync(path.join(outDir, 'set-code-call.hex'), setCode.toHex() + '\n');
  const report = {
    preparedAt: new Date().toISOString(), endpoint: ENDPOINT, signed: false, submittedTransactions: 0,
    finalized: { number: header.number.toNumber(), hash: hash.toHex() },
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
    const decoded = at.registry.createType('Call', call.toHex());
    assert.equal(decoded.toHex(), call.toHex());
    if (name !== 'preimage-note-call') fs.writeFileSync(path.join(outDir, `${name}.hex`), call.toHex() + '\n');
    else fs.writeFileSync(path.join(outDir, `${name}.hex`), call.toHex() + '\n');
    report.calls[name] = { file: `${name}.hex`, bytes: call.encodedLength, hash: blake2AsHex(call.toU8a()), section: call.section, method: call.method };
  }
  fs.writeFileSync(path.join(outDir, 'governance-calls.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify({ finalized: report.finalized, proposalHash, setCodeBytes: setCodeBytes.length, council: report.council,
    technicalCommittee: report.technicalCommittee, nextExternal: report.democracy.nextExternal, calls: Object.keys(report.calls) }, null, 1));
  await api.disconnect();
})().catch((error) => { console.error(error); process.exit(1); });
