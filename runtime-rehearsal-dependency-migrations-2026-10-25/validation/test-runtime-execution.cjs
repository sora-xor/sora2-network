'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const execution = require('./runtime-execution.cjs');
test('runtime inclusion modes are decoded explicitly and unknown modes rejected', () => {
  assert.equal(execution.inclusionMode('0x01'), 'OnlyInherents');
  assert.equal(execution.inclusionMode('0x00'), 'AllExtrinsics');
  assert.throws(() => execution.inclusionMode('0x02'));
});
test('signed fixtures cannot run in an inherent-only, unknown or wrong-height block', () => {
  for (const report of [{}, { signedFixtureBlock: { inclusionMode: 'OnlyInherents', number: 2 } }, { signedFixtureBlock: { inclusionMode: 'AllExtrinsics', number: 1 } }]) {
    assert.throws(() => execution.assertSignedFixtureBlock(report, { number: 2 }));
  }
  execution.assertSignedFixtureBlock({ signedFixtureBlock: { inclusionMode: 'AllExtrinsics', number: 2 } }, { number: 2 });
});
test('invalid transaction storage changes are retained as evidence but fully rolled back', () => {
  const response = { result: 'invalid', storageDiff: [['0x1234', '0xabcd'], ['0x5678', null]] }, report = {};
  const normalized = execution.normalizeApplyResult(response, true, report, 2, '0x01');
  assert.deepEqual(normalized.storageDiff, []);
  assert.deepEqual(report.runtimeExtrinsicApplications[0].discardedStorageDiff, response.storageDiff);
  assert.equal(response.storageDiff.length, 2);
  assert.equal(normalized.storageTransaction, 'rolled_back_invalid_transaction');
});
test('admitted dispatch errors retain fee/nonce effects instead of rolling back', () => {
  const response = { result: 'Ok(DispatchError)', storageDiff: [['0x1234', '0xabcd']] }, report = {};
  assert.deepEqual(execution.normalizeApplyResult(response, false, report, 2, '0x01').storageDiff, response.storageDiff);
  assert.equal(report.runtimeExtrinsicApplications[0].transaction, 'committed_admitted_extrinsic');
});
test('runtime apply normalization uses the outer validity result', async () => {
  let seen;
  const block = { number: 2, call: async (...args) => { seen = args; return { result: '0x01', storageDiff: [['0x01', '0x02']] }; }, registry: { createType: (name, encoded) => { assert.equal(name, 'ApplyExtrinsicResult'); assert.equal(encoded, '0x01'); return { isErr: true }; } } };
  const result = await execution.applyExtrinsic(block, '0x0304', {});
  assert.deepEqual(seen, ['BlockBuilder_apply_extrinsic', ['0x0304']]);
  assert.deepEqual(result.storageDiff, []);
});
