'use strict';
const assert = require('node:assert/strict');
const { createHash } = require('node:crypto');
const sha256 = value => createHash('sha256').update(value).digest('hex');
function inclusionMode(encoded) {
  assert(['0x00', '0x01'].includes(encoded), 'Unknown Core_initialize_block inclusion mode');
  return encoded === '0x00' ? 'AllExtrinsics' : 'OnlyInherents';
}
function assertSignedFixtureBlock(report, block) {
  assert.equal(report.signedFixtureBlock?.inclusionMode, 'AllExtrinsics', 'Signed fixtures require a normally initialized block');
  assert.equal(block.number, report.signedFixtureBlock.number, 'Signed fixture must use the reviewed normal block');
}
function normalizeApplyResult(response, invalid, report, blockNumber, encoded) {
  const raw = response.storageDiff;
  const row = { blockNumber, extrinsicSha256: sha256(encoded), result: response.result,
    rawStorageDiffEntries: raw.length, rawStorageDiffSha256: sha256(JSON.stringify(raw)),
    transaction: invalid ? 'rolled_back_invalid_transaction' : 'committed_admitted_extrinsic' };
  if (invalid) row.discardedStorageDiff = raw;
  (report.runtimeExtrinsicApplications ||= []).push(row);
  // Substrate's block builder rolls back its runtime-API storage transaction on
  // InvalidTransaction. A dispatch error inside Ok is admitted and commits.
  return { ...response, storageDiff: invalid ? [] : raw, storageTransaction: row.transaction };
}
async function applyExtrinsic(block, encoded, report) {
  const response = await block.call('BlockBuilder_apply_extrinsic', [encoded]);
  const registry = await block.registry;
  const outcome = registry.createType('ApplyExtrinsicResult', response.result);
  return normalizeApplyResult(response, outcome.isErr, report, block.number, encoded);
}
module.exports = { inclusionMode, assertSignedFixtureBlock, normalizeApplyResult, applyExtrinsic };
