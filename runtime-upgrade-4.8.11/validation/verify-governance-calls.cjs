'use strict';
// Offline SCALE decoding with the exact captured deployed metadata. No provider.
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync } = require('node:fs');
const { resolve } = require('node:path');
const { createHash } = require('node:crypto');
const { TypeRegistry, Metadata } = require('@polkadot/types');
const { blake2AsHex } = require('@polkadot/util-crypto');
const { u8aToHex } = require('@polkadot/util');
const dir = resolve(__dirname, '..');
const info = JSON.parse(readFileSync(resolve(dir, 'runtime-upgrade-4.8.11-info.json')));
const governance = JSON.parse(readFileSync(resolve(dir, 'governance-calls.json')));
const snapshot = JSON.parse(readFileSync(resolve(__dirname, 'snapshot.json')));
const sha256 = value => createHash('sha256').update(value).digest('hex');
const metadataBytes = readFileSync(resolve(__dirname, 'deployed-spec-132-metadata.scale'));
assert.equal(sha256(metadataBytes), snapshot.metadata.sha256);
const registry = new TypeRegistry();
registry.setMetadata(new Metadata(registry, metadataBytes));
const wasm = readFileSync(resolve(dir, info.candidate.file));
const setCode = readFileSync(resolve(dir, 'set-code-call.scale'));
assert.equal(sha256(wasm), info.candidate.sha256);
assert.equal(blake2AsHex(setCode), info.proposal.hash);
assert.equal(governance.setCodeProposalHash, info.proposal.hash);
assert.equal(governance.status, 'passed');
assert.equal(governance.nextExternal, null); assert.equal(governance.candidateBlacklist, null);
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
const external = decode('democracy-external-propose-majority-call.hex', 'democracy', 'externalProposeMajority');
assert.equal(external.args[0].type, 'Lookup');
assert.equal(external.args[0].value.get('hash_').toHex(), info.proposal.hash);
assert.equal(external.args[0].value.len.toNumber(), setCode.length);
const fast = decode('democracy-fast-track-call.hex', 'democracy', 'fastTrack');
assert.equal(fast.args[0].toHex(), info.proposal.hash);
assert.equal(fast.args[1].toNumber(), governance.fastTrackVotingPeriod);
assert.equal(fast.args[2].toNumber(), governance.enactmentDelay);
const council = decode(`council-propose-external-majority-threshold-${governance.minimumCouncilThreshold}-call.hex`, 'council', 'propose');
assert.equal(council.args[0].toNumber(), governance.minimumCouncilThreshold);
assert.equal(council.args[1].toHex(), external.toHex());
assert.equal(council.args[2].toNumber(), external.encodedLength);
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
const checkedInputFiles = [...checkedCallFiles, 'set-code-call.scale', info.candidate.file,
  'runtime-upgrade-4.8.11-info.json', 'governance-calls.json', 'validation/snapshot.json',
  'validation/deployed-spec-132-metadata.scale', 'validation/verify-governance-calls.cjs'];
const inputSha256 = Object.fromEntries(checkedInputFiles.map(file =>
  [file, sha256(readFileSync(resolve(dir, file)))]));
const report = { status: 'passed', checkedAt: new Date().toISOString(), networkRequests: 0, signedExtrinsics: 0,
  inputSha256,
  candidateSha256: info.candidate.sha256, setCodeProposalHash: info.proposal.hash,
  capturedMetadataSha256: snapshot.metadata.sha256, verifiedCalls: 6,
  councilInnerCallAndThresholdChecked: true, technicalInnerCallThresholdPeriodAndDelayChecked: true,
  exactPreimageAndWasmChecked: true, governanceStateQueueAndBlacklistClearAtPreflight: true };
writeFileSync(resolve(__dirname, 'governance-call-check.json'), JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify(report));
