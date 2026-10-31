'use strict';
// Offline re-evaluation: a passing evidence check does not imply operator readiness.
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync } = require('node:fs');
const { resolve } = require('node:path');
const { createHash } = require('node:crypto');
const { Metadata, TypeRegistry } = require('@polkadot/types');
const { evaluateBridgeReadiness } = require('./bridge-readiness-policy.cjs');
const sha = value => createHash('sha256').update(value).digest('hex');
const read = name => JSON.parse(readFileSync(resolve(__dirname, name)));
const hashFile = name => sha(readFileSync(resolve(__dirname, name)));
function registry(name) {
  const r = new TypeRegistry();
  r.setMetadata(new Metadata(r, readFileSync(resolve(__dirname, name))));
  return r;
}
function verify(name, snapshotName) {
  const report = read(name), snapshot = read(snapshotName);
  const build = read('candidate-build-evidence.json');
  const compatibility = read('wasm-compatibility.json');
  assert.equal(report.status, 'passed');
  assert.equal(report.scriptSha256, hashFile('check-bridge-readiness.cjs'));
  assert.equal(report.snapshotSha256, hashFile(snapshotName));
  assert.equal(report.endpoint, 'wss://mof2.sora.org');
  assert.equal(compatibility.status, 'passed');
  assert.equal(build.candidateSha256, hashFile('../candidate.compact.compressed.wasm'));
  assert.equal(compatibility.inputs.candidate.sha256, build.candidateSha256);
  assert.equal(compatibility.inputs.candidate.metadataScaleSha256, hashFile('candidate-metadata.scale'));
  assert.equal(report.policyMetadataBinding.candidateSha256, hashFile('candidate-metadata.scale'));
  assert.equal(report.policyMetadataBinding.baselineSha256, hashFile('deployed-metadata.scale'));
  assert.equal(compatibility.inputs.baseline.metadataScaleSha256, hashFile('deployed-metadata.scale'));
  assert.equal(snapshot.metadata.sha256, hashFile('deployed-metadata.scale'));
  assert.equal(compatibility.inputs.baseline.sha256, snapshot.code.sha256);
  assert.equal(snapshot.code.sha256, hashFile('live-runtime.compact.compressed.wasm'));
  const candidate = { sha256: build.candidateSha256, specVersion: build.specVersion, transactionVersion: build.transactionVersion };
  const baselineRegistry = registry('deployed-metadata.scale'), candidateRegistry = registry('candidate-metadata.scale');
  let operatorEvidence = null;
  if (report.operatorEvidenceBinding) {
    const file = report.operatorEvidenceBinding.path;
    assert.equal(hashFile(file), report.operatorEvidenceBinding.sha256);
    operatorEvidence = read(file);
  }
  if (report.payloadEvidenceBinding) {
    assert.equal(hashFile(report.payloadEvidenceBinding.path), report.payloadEvidenceBinding.sha256);
    const identities = new Set(report.accounts.flatMap(account => account.operations.map(op => account.account + ':' + op.hash)));
    const payloads = read(report.payloadEvidenceBinding.path).payloads.filter(row => identities.has(row.account + ':' + row.hash)).map(row => {
      if (row.candidateSha256 !== undefined) assert.equal(row.candidateSha256, candidate.sha256);
      if (row.blockHash !== undefined) assert.equal(row.blockHash, snapshot.blockHash);
      return { ...row, candidateSha256: candidate.sha256, blockHash: snapshot.blockHash };
    });
    assert.deepEqual(report.payloadEvidence, payloads, 'Recovered payloads must match bound source file');
  } else assert.deepEqual(report.payloadEvidence, [], 'Payloads without source evidence');
  const result = evaluateBridgeReadiness({ report, candidate, snapshot,
    payloadEvidence: report.payloadEvidence, operatorEvidence, baselineRegistry, candidateRegistry });
  assert.deepEqual(report.readiness, result, 'Readiness must reproduce from raw evidence');
  assert.equal(report.operatorReadinessConfirmed, result.operatorReadinessConfirmed);
  assert.equal(report.activationAuthorized, false);
  return { status: 'passed', report: name, reportSha256: hashFile(name), snapshot: snapshotName,
    snapshotSha256: hashFile(snapshotName), ...result };
}
function recoveryManifest(reportName) {
  const report = read(reportName), r = registry('candidate-metadata.scale');
  const pallet = r.metadata.pallets.find(p => p.name.toString() === 'EthBridge');
  const call = r.lookup.getSiType(pallet.calls.unwrap().type).def.asVariant.variants.find(v => v.name.toString() === 'cancel_pending_multisig');
  assert(call, 'Candidate must offer peer-quorum cancellation');
  const selected = new Set(report.readiness.operations.filter(op =>
    (op.baselineDecodable === false && op.candidateDecodable === false) || op.withinBound === false)
    .map(op => op.account + ':' + op.hash));
  return { status: 'unsigned-review-only', candidate: report.candidateBinding, blockHash: report.blockHash,
    reportSha256: hashFile(reportName), submittedTransactions: 0, activationAuthorized: false,
    instructions: 'Review each operation before any submission. These calls only cancel the pending multisig; they do not execute payloads, reset incoming status, clear replay tombstones, or retry transfers. Current eligible peer quorum must authorize each cancellation; refresh state before use.',
    preservedOperations: report.readiness.operations.filter(op => !selected.has(op.account + ':' + op.hash))
      .map(op => ({ account: op.account, hash: op.hash, baselineDecodable: op.baselineDecodable,
        candidateDecodable: op.candidateDecodable, selection: 'retained-no-cancellation-prepared' })),
    operations: report.accounts.flatMap(row => row.operations.filter(op => selected.has(row.account + ':' + op.hash)).map(op => {
      const encoded = r.createType('Call', { callIndex: [pallet.index.toNumber(), call.index.toNumber()],
        args: { network_id: row.network, call_hash: op.hash, timepoint: op.when } });
      const decoded = r.createType('Call', encoded.toU8a());
      assert.equal(decoded.toHex(), encoded.toHex());
      return { network: row.network, account: row.account, hash: op.hash, when: op.when,
        quorum: row.quorum, eligiblePeers: report.readiness.accounts.find(account => account.network === row.network).eligiblePeers,
        callHex: encoded.toHex(), callBytes: encoded.encodedLength, selection: 'not-authorized-for-submission' };
    })) };
}
if (require.main === module) {
  const current = process.argv.includes('--current');
  assert(process.argv.slice(2).every(x => x === '--current'), 'Unknown option');
  const cases = [verify('bridge-readiness.json', 'snapshot.json')];
  if (current) cases.push(verify('bridge-activation-current.json', 'bridge-activation-snapshot.json'));
  const report = { status: 'passed', createdAt: new Date().toISOString(), verifierSha256: hashFile('verify-bridge-readiness.cjs'),
    policySha256: hashFile('bridge-readiness-policy.cjs'), sizeNormalizerSha256: hashFile('bridge-rpc-size.cjs'), activationAuthorized: false, cases };
  const recovery = recoveryManifest(current ? 'bridge-activation-current.json' : 'bridge-readiness.json');
  writeFileSync(resolve(__dirname, 'bridge-cancellation-review.json'), JSON.stringify(recovery, null, 2) + '\n');
  report.cancellationReviewSha256 = hashFile('bridge-cancellation-review.json');
  writeFileSync(resolve(__dirname, 'bridge-readiness-verification.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify(report));
}
module.exports = { verify, recoveryManifest };
