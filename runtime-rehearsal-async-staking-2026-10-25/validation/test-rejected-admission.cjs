'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { xxhashAsHex } = require('@polkadot/util-crypto');
const { accountingKeys, assertAccountingOnly } = require('./verify-rejected-admission.cjs');
const allowed = { plain: ['0x' + '11'.repeat(32), '0x' + '22'.repeat(32)], blockSize: '0x' + '55'.repeat(32), extrinsicData: '0x' + '33'.repeat(32) };
const index = Buffer.from([1,0,0,0]);
const key = allowed.extrinsicData + xxhashAsHex(index,64).slice(2) + index.toString('hex');
test('only exact block-accounting keys and valid extrinsic-data suffixes pass', () => {
  assertAccountingOnly([[allowed.plain[0], '0x01'], [allowed.plain[1], null], [key, '0x01']], allowed);
});
test('payer/event/fee/reward or other storage writes are rejected', () => {
  assert.throws(() => assertAccountingOnly([['0x' + '44'.repeat(32), '0x01']], allowed), /outside/);
});
test('forged, extended and truncated extrinsic-data keys are rejected', () => {
  for (const bad of [key + '00', key.slice(0,-2), allowed.extrinsicData + '00'.repeat(8) + index.toString('hex')]) {
    assert.throws(() => assertAccountingOnly([[bad, '0x01']], allowed));
  }
});
test('only an exact BlockSize key with a SCALE u32 value passes', () => {
  assertAccountingOnly([[allowed.blockSize, '0xd9000000']], allowed);
  assertAccountingOnly([[allowed.blockSize, '0x00000000']], allowed);
  for (const value of [null, 217, '0x', '0xd90000', '0xd900000000', '0xzz000000']) {
    assert.throws(() => assertAccountingOnly([[allowed.blockSize, value]], allowed), /SCALE u32/);
  }
  for (const bad of [allowed.blockSize + '00', allowed.blockSize.slice(0, -2)]) {
    assert.throws(() => assertAccountingOnly([[bad, '0xd9000000']], allowed), /outside/);
  }
});
function metadata() {
  return { V14: { pallets: [{ name: 'System', storage: { prefix: 'System', entries: [
    { name: 'InherentsApplied', ty: { Plain: 1 } },
    { name: 'BlockWeight', ty: { Plain: 2 } },
    { name: 'BlockSize', modifier: 'Optional', ty: { Plain: 4 } },
    { name: 'ExtrinsicData', ty: { Map: { hashers: ['Twox64Concat'], key: 4 } } },
  ] } }], types: { types: [{ id: 4, type: { def: { primitive: 'u32' } } }] } } };
}
test('BlockSize allowance is derived from exact metadata storage name and u32 type', () => {
  const keys = accountingKeys(metadata());
  assert.equal(keys.blockSize, '0x26aa394eea5630e07c48ae0c9558cef7dd2ecd4efaa88c313bb2e876dc3521ea');
  assertAccountingOnly([[keys.blockSize, '0xd9000000']], keys);
  const account = xxhashAsHex('System', 128) + xxhashAsHex('Account', 128).slice(2);
  const events = xxhashAsHex('System', 128) + xxhashAsHex('Events', 128).slice(2);
  for (const key of [account, account + '00'.repeat(48), events]) {
    assert.throws(() => assertAccountingOnly([[key, '0xd9000000']], keys), /outside/);
  }
});
test('BlockSize metadata changes cannot broaden the allowance', () => {
  const edits = [
    value => value.V14.pallets[0].storage.entries.splice(2, 1),
    value => { value.V14.pallets[0].storage.entries[2].name = 'Account'; },
    value => { value.V14.pallets[0].storage.entries[2].ty = { Map: { key: 4 } }; },
    value => { value.V14.pallets[0].storage.entries[2].modifier = 'Default'; },
    value => { value.V14.types.types[0].type.def.primitive = 'u64'; },
  ];
  for (const edit of edits) { const value = metadata(); edit(value); assert.throws(() => accountingKeys(value)); }
});
