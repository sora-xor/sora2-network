'use strict';
// Reads public finalized governance state and encodes unsigned proposals only.
// Never signs, submits, cancels, blacklists, or replaces a queued external proposal.
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync, existsSync } = require('node:fs');
const { resolve } = require('node:path');
const { createHash } = require('node:crypto');
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { GenericExtrinsic } = require('@polkadot/types');
const { fullOverrideBundle } = require('@sora-substrate/type-definitions');
const { blake2AsHex } = require('@polkadot/util-crypto');
const { hexToU8a, stringToHex, u8aToHex } = require('@polkadot/util');
const dir = resolve(__dirname, '..');
const info = JSON.parse(readFileSync(resolve(dir, 'runtime-upgrade-4.8.12-info.json')));
const sha256 = value => createHash('sha256').update(value).digest('hex');
const endpoint = 'wss://mof2.sora.org';
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
  assert.equal(version.specVersion.toNumber(), info.preflight.runtimeVersion.specVersion, 'Live runtime changed; regenerate package');
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
  const units = 10n ** 18n;
  const cents = units / 30000n;
  const millicents = cents / 1000n;
  const preimageBaseDeposit = 2n * 2000n * cents + 64n * 100n * millicents;
  const preimageByteDeposit = 100n * millicents;
  const preimageDeposit = preimageBaseDeposit + BigInt(setCode.length) * preimageByteDeposit;
  report.preimage = {
    status: (await at.query.preimage.requestStatusFor(info.proposal.hash)).toJSON(),
    bytes: setCode.length,
    maxBytes: 4 * 1024 * 1024,
    limitSource: 'Pinned SDK e3737178ec726cffe506c907263aaaa417893fd0 pallet-preimage MAX_SIZE; the custom weight cap is not the storage limit',
    baseDepositBaseUnits: preimageBaseDeposit.toString(),
    byteDepositBaseUnits: preimageByteDeposit.toString(),
    unrequestedDepositBaseUnits: preimageDeposit.toString(),
    unrequestedDepositXor: `${preimageDeposit / units}.${(preimageDeposit % units).toString().padStart(18, '0')}`,
    depositSource: 'Captured baseline runtime: deposit(2,64) + encoded preimage bytes * deposit(0,1); source-calculated, not an account fee quote',
    feeInstruction: 'Obtain a fresh signed-account fee/balance quote before noting the preimage; this deposit is additional to transaction fees and depends on request status.',
  };
  assert(setCode.length <= report.preimage.maxBytes, 'Preimage exceeds pinned pallet storage limit');
  assert(council.members.length > 0 && technicalCommittee.members.length > 0, 'Empty governance collective');
  // Always encode the guarded route only. An occupied queue remains untouched;
  // the encoded guard refuses to execute until the queue is clear.
  if (blacklist.isSome) {
    report.status = 'requires-governance-state-review';
    report.reason = 'Candidate proposal hash has blacklist storage. No governance calls were written.';
    save(); throw new Error(report.reason);
  }
  report.governanceQueueReady = nextExternal.isNone;
  report.operatorReadiness = {
    ready: false,
    status: 'requires-operator-attestation',
    requirements: [
      'Install dedicated sr25519 keep keys and fund keeper XOR accounts before activation; verify signed Kensetsu and Apollo submissions on a test network.',
      'Verify zero-XOR authenticated bridge submissions, replay rejection and channel nonce recovery. Bridge peers and protocol relayers have no XOR funding requirement.',
      'Review finalized legacy pending call sizes and explicit weighted dispatch after membership changes; existing pending operations are grandfathered by the new-operation bound.',
      'Update equivocation reporters to submit signed funded reports; automatic unsigned report submission is disabled.',
      'For Iroha migration onboarding, fund the signed claimant with enough XOR for normal fees on success and failure; no sponsor or success refund is available.',
    ],
  };
  report.activationReady = report.governanceQueueReady && report.operatorReadiness.ready;
  report.activation = { ready: report.activationReady,
    existingQueuedProposal: nextExternal.toJSON(),
    reason: nextExternal.isSome
      ? 'An existing external proposal is queued and operator readiness is unconfirmed; the guarded route rejects replacing the queue.'
      : 'Operator readiness for funded keepers/reporters and bridge compatibility checks is unconfirmed.',
    prerequisites: [
      'Resolve the existing external proposal through normal governance so NextExternal is clear; never overwrite it.',
      'Refresh finalized baseline, metadata, governance membership and unsigned calls before use. If runtime code or metadata changed, repeat compatibility and exact-Wasm rehearsal against the new deployed baseline.',
      ...report.operatorReadiness.requirements,
    ] };
  for (const name of ['democracy-external-propose-majority-call.hex',
    `council-propose-external-majority-threshold-${report.minimumCouncilThreshold}-call.hex`]) {
    assert(!existsSync(resolve(dir, name)), 'Unsafe direct supersession artifact must be removed: ' + name);
  }
  const target = { Lookup: { hash: info.proposal.hash, len: setCode.length } };
  const guarded = api.tx.utility.batchAll([api.tx.democracy.externalPropose(target), api.tx.democracy.externalProposeMajority(target)]).method;
  const fast = api.tx.democracy.fastTrack(info.proposal.hash, report.fastTrackVotingPeriod, report.enactmentDelay).method;
  const calls = {
    'utility-guarded-external-majority-call': guarded,
    [`council-propose-guarded-external-majority-threshold-${report.minimumCouncilThreshold}-call`]: api.tx.council.propose(report.minimumCouncilThreshold, guarded, guarded.encodedLength).method,
    'democracy-fast-track-call': fast,
    [`technical-committee-fast-track-threshold-${report.technicalCommitteeThreshold}-call`]: api.tx.technicalCommittee.propose(report.technicalCommitteeThreshold, fast, fast.encodedLength).method,
  };
  for (const [name, call] of Object.entries(calls)) {
    const decoded = at.registry.createType('Call', call.toHex());
    assert.equal(decoded.toHex(), call.toHex()); assert.equal(decoded.section, call.section); assert.equal(decoded.method, call.method);
    writeFileSync(resolve(dir, name + '.hex'), call.toHex() + '\n');
    report.calls[name] = { file: name + '.hex', bytes: call.encodedLength, hash: blake2AsHex(call.toU8a()), section: call.section, method: call.method, decoded: call.toHuman() };
  }
  report.preferredCouncilCall = `council-propose-guarded-external-majority-threshold-${report.minimumCouncilThreshold}-call.hex`;
  report.preferredCouncilMotionHash = blake2AsHex(guarded.toU8a());
  report.technicalMotionHash = blake2AsHex(fast.toU8a());
  report.reviewOnlySupersessionCalls = [];
  report.directSupersessionArtifactsExcluded = true;
  const weightJson = weight => ({ refTime: weight.refTime.toString(), proofSize: weight.proofSize.toString() });
  const queryWeight = async method => {
    const tx = new GenericExtrinsic(at.registry, method, { version: 4 });
    // The runtime takes SCALE Extrinsic directly, not a Bytes wrapper. Use the
    // same raw API encoding as the exact-Wasm execution rehearsal.
    const input = tx.toHex() + u8aToHex(at.registry.createType('u32', tx.encodedLength).toU8a()).slice(2);
    const raw = await api.rpc.state.call('TransactionPaymentApi_query_info', input, hash);
    const result = at.registry.createType('RuntimeDispatchInfo', raw.toHex());
    return weightJson(result.weight);
  };
  const [councilWeight, technicalWeight] = await Promise.all([queryWeight(guarded), queryWeight(fast)]);
  report.closeBounds = {
    council: { lengthBound: guarded.encodedLength + 4, weightBound: councilWeight, maxProposalWeight: weightJson(at.consts.council.maxProposalWeight) },
    technicalCommittee: { lengthBound: fast.encodedLength + 4, weightBound: technicalWeight, maxProposalWeight: weightJson(at.consts.technicalCommittee.maxProposalWeight) },
    weightSource: 'Read-only TransactionPaymentApi.query_info of the exact inner call wrapped as an unsigned extrinsic at the finalized preflight block. This weight covers dispatch; its fee is not a signed-account fee quote. Requery for actual stored motion and live runtime before close.',
    source: 'Pinned collective validate_and_get_proposal documents storage::read length overhead of four bytes; motion indices must come from Proposed events.',
  };
  report.instructions = [
    'These are unsigned review artifacts. No governance action has been submitted.',
    'Repeat finalized governance-state inspection immediately before use. Existing unrelated motions are recorded and remain untouched.',
    'Council majority proposal needs at least half the current council. Technical fast-track needs more than half the current committee.',
    'Use the preferred guarded council call: atomic batchAll([externalPropose(target), externalProposeMajority(target)]). It rejects an occupied queue at execution and rolls back on failure.',
    'Direct supersession alternatives are excluded. An occupied external queue requires resolution and baseline refresh before this upgrade is activated.',
    'Proposers must cast explicit aye votes. Generate vote and close calls only after the actual motion index, votes and live weight bounds are known.',
    'Before executing technical fast-track, confirm the council inner execution succeeded and NextExternal contains this exact setCode proposal hash.',
    'Use Democracy.Started to obtain the actual referendum index. Encoding these calls neither approves nor enacts the runtime.',
  ];
  report.status = 'prepared'; save();
  writeFileSync(resolve(dir, 'governance-calls.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify({ status: report.status, block: report.blockNumber, councilThreshold: report.minimumCouncilThreshold, technicalThreshold: report.technicalCommitteeThreshold, activationReady: report.activationReady, unsignedCalls: Object.keys(report.calls), submittedTransactions: 0 }));
}
const timeout = setTimeout(() => { report.status = 'timed_out'; save(); process.exit(2); }, 300000);
main().catch(error => { if (report.status === 'running') report.status = 'failed'; report.error = { message: error.message, stack: error.stack }; save(); console.error(error); process.exitCode = 1; }).finally(async () => { clearTimeout(timeout); if (api) await api.disconnect().catch(() => {}); });
