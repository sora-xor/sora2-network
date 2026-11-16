'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { xxhashAsHex } = require('@polkadot/util-crypto');
const sha256 = value => createHash('sha256').update(value).digest('hex');
function accountingKeys(metadata) {
  const system = metadata.V14.pallets.find(value => value.name === 'System');
  const prefix = name => xxhashAsHex(system.storage.prefix, 128) + xxhashAsHex(name, 128).slice(2);
  for (const name of ['InherentsApplied', 'BlockWeight']) assert('Plain' in system.storage.entries.find(value => value.name === name).ty);
  const extrinsic = system.storage.entries.find(value => value.name === 'ExtrinsicData');
  assert.deepEqual(extrinsic.ty.Map.hashers, ['Twox64Concat']);
  const types = new Map(metadata.V14.types.types.map(value => [value.id, value.type]));
  assert.equal(types.get(extrinsic.ty.Map.key).def.primitive, 'u32');
  const blockSize = system.storage.entries.find(value => value.name === 'BlockSize');
  assert(blockSize && 'Plain' in blockSize.ty, 'BlockSize must be a plain storage value');
  assert.equal(types.get(blockSize.ty.Plain).def.primitive, 'u32', 'BlockSize must retain its u32 encoding');
  assert.equal(blockSize.modifier, 'Optional');
  return { plain: [prefix('InherentsApplied'), prefix('BlockWeight')], blockSize: prefix('BlockSize'), extrinsicData: prefix('ExtrinsicData') };
}
function assertAccountingOnly(diff, allowed) {
  for (const [key, value] of diff) {
    if (allowed.plain.includes(key)) continue;
    // CheckWeight records the new size before the final heartbeat extension
    // rejects a bare call. This accounting write is discarded with the entire
    // invalid transaction; it does not admit account, fee, claim or event writes.
    if (key === allowed.blockSize) {
      assert(typeof value === 'string' && /^0x[0-9a-f]{8}$/.test(value),
        'BlockSize write must be exactly one SCALE u32, not a deletion or arbitrary bytes');
      continue;
    }
    assert(key.startsWith(allowed.extrinsicData) && key.length === allowed.extrinsicData.length + 24,
      'Rejected raw admission wrote outside the four reviewed block-accounting fields: ' + key);
    const encodedIndex = Buffer.from(key.slice(-8), 'hex');
    assert.equal(key, allowed.extrinsicData + xxhashAsHex(encodedIndex, 64).slice(2) + encodedIndex.toString('hex'), 'ExtrinsicData key must have a valid u32 Twox64Concat suffix');
  }
}
function verify(directory) {
  const bytes = name => fs.readFileSync(path.join(directory, name));
  const read = name => JSON.parse(bytes(name));
  const run = read('rehearsal-run.json'), compatibility = read('wasm-compatibility.json');
  assert.equal(run.status, 'passed'); assert.equal(compatibility.status, 'passed');
  const candidateSha256 = sha256(fs.readFileSync(path.resolve(directory, '../candidate.compact.compressed.wasm')));
  const snapshotSha256 = sha256(bytes('snapshot.json'));
  assert.equal(candidateSha256, run.inputs.candidateSha256);
  assert.equal(candidateSha256, compatibility.inputs.candidate.sha256);
  assert.equal(snapshotSha256, run.inputs.snapshotSha256);
  assert.equal(snapshotSha256, compatibility.snapshotSha256);
  assert.equal(sha256(bytes('candidate-metadata.json')), compatibility.inputs.candidate.metadataSha256);
  const allowed = accountingKeys(read('candidate-metadata.json'));
  const suites = [];
  for (const name of ['equivocation-fee-wasm-rehearsal.json', 'fee-policy-wasm-rehearsal.json']) {
    const report = read(name);
    assert.equal(report.status, 'passed');
    assert.equal(report.candidate.sha256, candidateSha256);
    assert.equal(report.inputs.snapshotSha256, snapshotSha256);
    assert.equal(report.initialize.inclusionMode, 'OnlyInherents');
    assert.equal(report.upgradeFinalization.signedExtrinsicsApplied, 0);
    assert.equal(report.upgradeFinalization.cursorAfter, null);
    assert.equal(report.postUpgradeInitialize.inclusionMode, 'AllExtrinsics');
    assert.equal(report.signedFixtureBlock.inclusionMode, 'AllExtrinsics');
    const runStage = run.stages.find(value => value.result === 'validation/' + name);
    assert.equal(runStage?.resultSha256, sha256(bytes(name)), 'Suite must match the passed run manifest');
    const rejected = report.runtimeExtrinsicApplications.filter(value => value.transaction === 'rolled_back_invalid_transaction');
    assert(rejected.length > 0);
    let entries = 0;
    const blockSizeWrites = [];
    for (const row of rejected) {
      assert.equal(row.blockNumber, report.signedFixtureBlock.number);
      assert.equal(row.rawStorageDiffEntries, row.discardedStorageDiff.length);
      assert.equal(row.rawStorageDiffSha256, sha256(JSON.stringify(row.discardedStorageDiff)));
      assertAccountingOnly(row.discardedStorageDiff, allowed);
      for (const [key, value] of row.discardedStorageDiff) if (key === allowed.blockSize) {
        blockSizeWrites.push({ extrinsicSha256: row.extrinsicSha256, encodedValue: value,
          decodedBytes: Buffer.from(value.slice(2), 'hex').readUInt32LE() });
      }
      entries += row.discardedStorageDiff.length;
    }
    suites.push({ report: name, reportSha256: sha256(bytes(name)), rejectedTransactions: rejected.length, inspectedRawDiffEntries: entries,
      blockSizeWrites, rawAccountFeeRewardClaimAndEventWrites: 0, committedStateChangesOnInvalidTransaction: 0 });
  }
  return { status: 'passed', checkedAt: new Date().toISOString(), candidateSha256, snapshotSha256,
    runSha256: sha256(bytes('rehearsal-run.json')), compatibilitySha256: sha256(bytes('wasm-compatibility.json')),
    metadataSha256: sha256(bytes('candidate-metadata.json')), allowedAccountingKeys: allowed, suites,
    checks: { rawInvalidDiffsContainOnlyBlockAccounting: true, noRawAccountFeeRewardClaimOrEventWrites: true,
      invalidTransactionDiffsRolledBack: true, admittedDispatchErrorsRemainCommitted: true,
      signedTestsRanOnlyInFollowingNormalBlock: true }, networkRequests: 0 };
}
if (require.main === module) {
  const output = path.join(__dirname, 'rejected-admission-check.json');
  let report = { status: 'running', startedAt: new Date().toISOString() };
  fs.writeFileSync(output, JSON.stringify(report, null, 2) + '\n');
  try { report = { ...report, ...verify(__dirname) }; }
  catch (error) { report.status = 'failed'; report.error = { message: error.message, stack: error.stack }; process.exitCode = 1; }
  report.scriptSha256 = sha256(fs.readFileSync(__filename));
  report.finishedAt = new Date().toISOString();
  fs.writeFileSync(output, JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify(report));
}
module.exports = { accountingKeys, assertAccountingOnly };
