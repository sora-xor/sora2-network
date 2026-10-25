'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { xxhashAsHex } = require('@polkadot/util-crypto');
const { assertAccountingOnly } = require('./verify-rejected-admission.cjs');
const allowed = { plain: ['0x' + '11'.repeat(32), '0x' + '22'.repeat(32)], extrinsicData: '0x' + '33'.repeat(32) };
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
