'use strict';
// Offline SCALE decoding with the exact captured deployed metadata. No provider.
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync, readdirSync } = require('node:fs');
const { resolve } = require('node:path');
const { createHash } = require('node:crypto');
const { TypeRegistry, Metadata } = require('@polkadot/types');
const { blake2AsHex } = require('@polkadot/util-crypto');
const { u8aToHex } = require('@polkadot/util');
const dir = resolve(__dirname, '..');
const info = JSON.parse(readFileSync(resolve(dir, 'runtime-upgrade-4.8.12-info.json')));
const governance = JSON.parse(readFileSync(resolve(dir, 'governance-calls.json')));
const snapshot = JSON.parse(readFileSync(resolve(__dirname, 'snapshot.json')));
const sha256 = value => createHash('sha256').update(value).digest('hex');
const metadataBytes = readFileSync(resolve(__dirname, 'deployed-metadata.scale'));
assert.equal(sha256(metadataBytes), snapshot.metadata.sha256);
const registry = new TypeRegistry();
registry.setMetadata(new Metadata(registry, metadataBytes));
const wasm = readFileSync(resolve(dir, info.candidate.file));
const setCode = readFileSync(resolve(dir, 'set-code-call.scale'));
assert.equal(sha256(wasm), info.candidate.sha256);
assert.equal(blake2AsHex(setCode), info.proposal.hash);
assert.equal(governance.setCodeProposalHash, info.proposal.hash);
assert.equal(governance.status, 'prepared');
assert.equal(governance.candidateBlacklist, null);
assert.equal(governance.governanceQueueReady, governance.nextExternal === null);
assert.equal(governance.activationReady, governance.governanceQueueReady && governance.operatorReadiness.ready);
assert.equal(governance.operatorReadiness.status, 'requires-operator-attestation');
assert.equal(governance.operatorReadiness.ready, false);
assert.equal(governance.activation.ready, governance.activationReady);
assert.deepEqual(governance.activation.existingQueuedProposal, governance.nextExternal);
assert.equal(governance.directSupersessionArtifactsExcluded, true);
assert.equal(governance.minimumCouncilThreshold, Math.ceil(governance.council.members.length / 2));
assert.equal(governance.technicalCommitteeThreshold, Math.floor(governance.technicalCommittee.members.length / 2) + 1);
const checkedCallFiles = new Set();
const decode = (file, section, method) => {
  checkedCallFiles.add(file);
  const hex = readFileSync(resolve(dir, file), 'utf8').trim();
  const call = registry.createType('Call', hex);
  assert.equal(call.toHex(), hex); assert.equal(call.section, section); assert.equal(call.method, method);
  return call;
};
const codeCall = decode('set-code-call.hex', 'system', 'setCode');
assert.equal(codeCall.toHex(), u8aToHex(setCode));
assert.equal(codeCall.args[0].toHex(), u8aToHex(wasm));
const note = decode('preimage-note-call.hex', 'preimage', 'notePreimage');
assert.equal(note.args[0].toHex(), u8aToHex(setCode));
const fast = decode('democracy-fast-track-call.hex', 'democracy', 'fastTrack');
assert.equal(fast.args[0].toHex(), info.proposal.hash);
assert.equal(fast.args[1].toNumber(), governance.fastTrackVotingPeriod);
assert.equal(fast.args[2].toNumber(), governance.enactmentDelay);
const guarded = decode('utility-guarded-external-majority-call.hex', 'utility', 'batchAll');
assert.equal(guarded.args[0].length, 2);
const [queueGuard, guardedMajority] = guarded.args[0];
assert.equal(queueGuard.section, 'democracy');
assert.equal(queueGuard.method, 'externalPropose');
assert.equal(guardedMajority.section, 'democracy');
assert.equal(guardedMajority.method, 'externalProposeMajority');
assert.equal(queueGuard.args[0].toHex(), guardedMajority.args[0].toHex());
assert.equal(queueGuard.args[0].type, 'Lookup');
assert.equal(queueGuard.args[0].value.get('hash_').toHex(), info.proposal.hash);
assert.equal(queueGuard.args[0].value.len.toNumber(), setCode.length);
const guardedCouncilFile = `council-propose-guarded-external-majority-threshold-${governance.minimumCouncilThreshold}-call.hex`;
const guardedCouncil = decode(guardedCouncilFile, 'council', 'propose');
assert.equal(guardedCouncil.args[0].toNumber(), governance.minimumCouncilThreshold);
assert.equal(guardedCouncil.args[1].toHex(), guarded.toHex());
assert.equal(guardedCouncil.args[2].toNumber(), guarded.encodedLength);
assert.equal(governance.preferredCouncilCall, guardedCouncilFile);
assert.equal(governance.preferredCouncilMotionHash, blake2AsHex(guarded.toU8a()));
assert.equal(governance.closeBounds.council.lengthBound, guarded.encodedLength + 4);
assert.equal(governance.closeBounds.technicalCommittee.lengthBound, fast.encodedLength + 4);
assert.equal(governance.technicalMotionHash, blake2AsHex(fast.toU8a()));
assert.deepEqual(governance.reviewOnlySupersessionCalls, []);
assert(!readdirSync(dir).some(name => name === 'democracy-external-propose-majority-call.hex' || /^council-propose-external-majority-threshold-/.test(name)), 'Direct supersession artifacts must be absent');
const technical = decode(`technical-committee-fast-track-threshold-${governance.technicalCommitteeThreshold}-call.hex`, 'technicalCommittee', 'propose');
assert.equal(technical.args[0].toNumber(), governance.technicalCommitteeThreshold);
assert.equal(technical.args[1].toHex(), fast.toHex());
assert.equal(technical.args[2].toNumber(), fast.encodedLength);
for (const entry of Object.values(governance.calls)) {
  const raw = readFileSync(resolve(dir, entry.file), 'utf8').trim();
  const call = registry.createType('Call', raw);
  assert.equal(blake2AsHex(call.toU8a()), entry.hash); assert.equal(call.encodedLength, entry.bytes);
}
assert.equal(checkedCallFiles.size, 6);
assert.deepEqual(new Set(readdirSync(dir).filter(name => name.endsWith('.hex'))), checkedCallFiles, 'Unexpected or missing call file');
const settings = JSON.parse(readFileSync(resolve(dir, 'council-settings.json')));
assert.equal(settings.network.genesisHash, governance.genesis);
assert.equal(settings.network.finalizedBlockHash, governance.blockHash);
assert.equal(settings.network.finalizedBlock, governance.blockNumber);
assert.deepEqual(settings.runtime, { baselineSpecVersion: snapshot.runtimeVersion.specVersion, ...info.candidate });
assert.equal(settings.preimage.hash, info.proposal.hash);
assert.equal(settings.preimage.bytes, setCode.length);
assert.equal(settings.preimage.bytesArgumentFile, 'set-code-call.hex');
assert.equal(settings.preimage.completeCallFile, 'preimage-note-call.hex');
assert.equal(settings.preimage.unrequestedDepositBaseUnits,
  (BigInt(settings.preimage.baseDepositBaseUnits) + BigInt(setCode.length) * BigInt(settings.preimage.byteDepositBaseUnits)).toString());
assert.equal(settings.preimage.unrequestedDepositBaseUnits, governance.preimage.unrequestedDepositBaseUnits);
assert.equal(settings.council.callFile, guardedCouncilFile);
assert.equal(settings.council.threshold, guardedCouncil.args[0].toNumber());
assert.equal(settings.council.members, governance.council.members.length);
assert.equal(settings.council.innerCallFile, 'utility-guarded-external-majority-call.hex');
assert.equal(settings.council.proposalLengthBound, guarded.encodedLength);
assert.equal(settings.council.motionHashForVoteAndClose, blake2AsHex(guarded.toU8a()));
assert.equal(settings.technicalCommittee.callFile, `technical-committee-fast-track-threshold-${governance.technicalCommitteeThreshold}-call.hex`);
assert.equal(settings.technicalCommittee.threshold, technical.args[0].toNumber());
assert.equal(settings.technicalCommittee.members, governance.technicalCommittee.members.length);
assert.equal(settings.technicalCommittee.proposalLengthBound, fast.encodedLength);
assert.equal(settings.technicalCommittee.proposalHash, fast.args[0].toHex());
assert.equal(settings.technicalCommittee.votingPeriodBlocks, fast.args[1].toNumber());
assert.equal(settings.technicalCommittee.enactmentDelayBlocks, fast.args[2].toNumber());
assert.equal(settings.technicalCommittee.motionHashForVoteAndClose, blake2AsHex(fast.toU8a()));
for (const collective of ['council', 'technicalCommittee']) {
  assert.equal(settings[collective].motionIndex, null);
  assert.deepEqual(settings[collective].close, governance.closeBounds[collective]);
  for (const component of ['refTime', 'proofSize']) {
    assert(BigInt(settings[collective].close.weightBound[component]) > 0n);
    assert(BigInt(settings[collective].close.weightBound[component]) <= BigInt(settings[collective].close.maxProposalWeight[component]));
  }
}
assert.equal(settings.activationReady, governance.activationReady);
assert.deepEqual(settings.activation, governance.activation);
assert.equal(settings.referendumIndex, null);
assert.equal(settings.signed, false);
assert.equal(settings.submittedTransactions, 0);
for (const [file, digest] of Object.entries(settings.inputSha256)) {
  assert(['runtime-upgrade-4.8.12-info.json', 'governance-calls.json'].includes(file));
  assert.equal(sha256(readFileSync(resolve(dir, file))), digest);
}
const checkedInputFiles = [...checkedCallFiles, 'set-code-call.scale', info.candidate.file,
  'runtime-upgrade-4.8.12-info.json', 'governance-calls.json', 'validation/snapshot.json',
  'council-settings.json', 'validation/prepare-council-settings.py',
  'validation/prepare-governance-calls.cjs',
  'validation/deployed-metadata.scale', 'validation/verify-governance-calls.cjs'];
const inputSha256 = Object.fromEntries(checkedInputFiles.map(file =>
  [file, sha256(readFileSync(resolve(dir, file)))]));
const report = { status: 'passed', checkedAt: new Date().toISOString(), networkRequests: 0, signedExtrinsics: 0,
  inputSha256,
  candidateSha256: info.candidate.sha256, setCodeProposalHash: info.proposal.hash,
  capturedMetadataSha256: snapshot.metadata.sha256, verifiedCalls: 6,
  councilInnerCallAndThresholdChecked: true, technicalInnerCallThresholdPeriodAndDelayChecked: true,
  guardedCouncilExactTwoCallOrderThresholdLengthAndHashChecked: true,
  councilSettingsMatchDecodedCallsAndFinalizedPreflight: true,
  exactPreimageAndWasmChecked: true, candidateBlacklistClearAtPreflight: true, activationReady: governance.activationReady,
  existingExternalQueuePreserved: true, noDirectSupersessionArtifacts: true };
writeFileSync(resolve(__dirname, 'governance-call-check.json'), JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify(report));
