'use strict';
// Reads public finalized governance state and encodes unsigned proposals only.
// Never signs, submits, cancels, blacklists, or replaces a queued external proposal.
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync } = require('node:fs');
const { resolve } = require('node:path');
const { createHash } = require('node:crypto');
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { fullOverrideBundle } = require('@sora-substrate/type-definitions');
const { blake2AsHex } = require('@polkadot/util-crypto');
const { hexToU8a } = require('@polkadot/util');
const dir = resolve(__dirname, '..');
const info = JSON.parse(readFileSync(resolve(dir, 'runtime-upgrade-4.8.11-info.json')));
const sha256 = value => createHash('sha256').update(value).digest('hex');
const endpoint = 'wss://ws.mof.sora.org';
const report = { status: 'running', checkedAt: new Date().toISOString(), endpoint, submittedTransactions: 0, privateKeysRead: 0, signed: false, remoteRpcMethods: {}, calls: {} };
let api;
const save = () => writeFileSync(resolve(__dirname, 'governance-preflight.json'), JSON.stringify(report, null, 2) + '\n');
async function collectiveState(at, collective) {
  const [members, proposals, proposalCount] = await Promise.all([at.query[collective].members(), at.query[collective].proposals(), at.query[collective].proposalCount()]);
  const motions = await Promise.all(proposals.map(async hash => {
    const [voting, proposal] = await Promise.all([at.query[collective].voting(hash), at.query[collective].proposalOf(hash)]);
    return { hash: hash.toHex(), voting: voting.toJSON(), proposal: proposal.isSome ? {
      section: proposal.unwrap().section, method: proposal.unwrap().method, encodedBytes: proposal.unwrap().encodedLength,
    } : null };
  }));
  return { members: members.map(value => value.toString()), proposalCount: proposalCount.toNumber(), motions };
}
async function main() {
  const setCode = readFileSync(resolve(dir, 'set-code-call.scale'));
  assert.equal(blake2AsHex(setCode), info.proposal.hash);
  assert.equal(setCode.length, info.proposal.bytes);
  const provider = new WsProvider(endpoint);
  const send = provider.send.bind(provider);
  const readonly = new Set(['chain_getBlock', 'chain_getBlockHash', 'chain_getHeader', 'chain_getFinalizedHead', 'state_getStorage', 'state_getKeysPaged', 'state_queryStorageAt', 'state_getMetadata', 'state_getRuntimeVersion', 'state_call', 'system_properties', 'system_chain', 'system_name', 'system_version', 'rpc_methods', 'state_subscribeRuntimeVersion', 'state_unsubscribeRuntimeVersion']);
  provider.send = (method, ...params) => { assert(readonly.has(method), 'Forbidden public RPC: ' + method); report.remoteRpcMethods[method] = (report.remoteRpcMethods[method] || 0) + 1; return send(method, ...params); };
  api = await ApiPromise.create({ provider, noInitWarn: true, typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } });
  assert.equal(api.genesisHash.toHex(), info.preflight.genesis);
  const hash = await api.rpc.chain.getFinalizedHead();
  const at = await api.at(hash);
  const [header, version, code, nextExternal, blacklist, publicProps, council, technicalCommittee] = await Promise.all([
    api.rpc.chain.getHeader(hash), api.rpc.state.getRuntimeVersion(hash), api.rpc.state.getStorage('0x3a636f6465', hash),
    at.query.democracy.nextExternal(), at.query.democracy.blacklist(info.proposal.hash), at.query.democracy.publicProps(),
    collectiveState(at, 'council'), collectiveState(at, 'technicalCommittee'),
  ]);
  assert.equal(version.specVersion.toNumber(), 132, 'Live runtime changed; regenerate package');
  assert.deepEqual(api.runtimeVersion.toJSON(), version.toJSON(), 'Best and finalized runtime versions differ');
  const snapshot = JSON.parse(readFileSync(resolve(__dirname, 'snapshot.json')));
  assert.equal(sha256((await api.rpc.state.getMetadata(hash)).toU8a()), snapshot.metadata.sha256, 'Finalized V14 metadata differs from captured baseline');
  assert.equal(sha256(hexToU8a(code.unwrap().toHex())), info.preflight.deployedSha256);
  report.genesis = api.genesisHash.toHex(); report.blockHash = hash.toHex(); report.blockNumber = header.number.toNumber();
  report.runtimeVersion = version.toJSON(); report.setCodeProposalHash = info.proposal.hash; report.setCodeProposalBytes = setCode.length;
  report.nextExternal = nextExternal.toJSON(); report.candidateBlacklist = blacklist.toJSON(); report.publicProposals = publicProps.toJSON();
  report.council = council; report.technicalCommittee = technicalCommittee;
  report.fastTrackVotingPeriod = at.consts.democracy.fastTrackVotingPeriod.toNumber();
  report.enactmentDelay = 0;
  report.minimumCouncilThreshold = Math.ceil(council.members.length / 2);
  report.technicalCommitteeThreshold = Math.floor(technicalCommittee.members.length / 2) + 1;
  assert(council.members.length > 0 && technicalCommittee.members.length > 0, 'Empty governance collective');
  // A new majority proposal may overwrite NextExternal in this pallet. Refuse to
  // package an executable replacement while another proposal is queued.
  if (nextExternal.isSome || blacklist.isSome) {
    report.status = 'requires-governance-state-review';
    report.reason = nextExternal.isSome ? 'NextExternal is already occupied. No external-majority or fast-track calls were written.' : 'Candidate proposal hash has blacklist storage. No governance calls were written.';
    save(); throw new Error(report.reason);
  }
  const external = api.tx.democracy.externalProposeMajority({ Lookup: { hash: info.proposal.hash, len: setCode.length } }).method;
  const fast = api.tx.democracy.fastTrack(info.proposal.hash, report.fastTrackVotingPeriod, report.enactmentDelay).method;
  const calls = {
    'democracy-external-propose-majority-call': external,
    [`council-propose-external-majority-threshold-${report.minimumCouncilThreshold}-call`]: api.tx.council.propose(report.minimumCouncilThreshold, external, external.encodedLength).method,
    'democracy-fast-track-call': fast,
    [`technical-committee-fast-track-threshold-${report.technicalCommitteeThreshold}-call`]: api.tx.technicalCommittee.propose(report.technicalCommitteeThreshold, fast, fast.encodedLength).method,
  };
  for (const [name, call] of Object.entries(calls)) {
    const decoded = at.registry.createType('Call', call.toHex());
    assert.equal(decoded.toHex(), call.toHex()); assert.equal(decoded.section, call.section); assert.equal(decoded.method, call.method);
    writeFileSync(resolve(dir, name + '.hex'), call.toHex() + '\n');
    report.calls[name] = { file: name + '.hex', bytes: call.encodedLength, hash: blake2AsHex(call.toU8a()), section: call.section, method: call.method, decoded: call.toHuman() };
  }
  report.instructions = [
    'These are unsigned review artifacts. No governance action has been submitted.',
    'Repeat finalized governance-state inspection immediately before use. Existing unrelated motions are recorded and remain untouched.',
    'Council majority proposal needs at least half the current council. Technical fast-track needs more than half the current committee.',
    'Proposers must cast explicit aye votes. Generate vote and close calls only after the actual motion index, votes and live weight bounds are known.',
    'Before executing technical fast-track, confirm the council inner execution succeeded and NextExternal contains this exact setCode proposal hash.',
    'Use Democracy.Started to obtain the actual referendum index. Encoding these calls neither approves nor enacts the runtime.',
  ];
  report.status = 'passed'; save();
  writeFileSync(resolve(dir, 'governance-calls.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify({ status: report.status, block: report.blockNumber, councilThreshold: report.minimumCouncilThreshold, technicalThreshold: report.technicalCommitteeThreshold, unsignedCalls: Object.keys(report.calls), submittedTransactions: 0 }));
}
const timeout = setTimeout(() => { report.status = 'timed_out'; save(); process.exit(2); }, 300000);
main().catch(error => { if (report.status === 'running') report.status = 'failed'; report.error = { message: error.message, stack: error.stack }; save(); console.error(error); process.exitCode = 1; }).finally(async () => { clearTimeout(timeout); if (api) await api.disconnect().catch(() => {}); });
