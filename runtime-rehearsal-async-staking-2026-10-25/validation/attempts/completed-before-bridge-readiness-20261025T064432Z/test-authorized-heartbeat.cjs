'use strict';
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { TypeRegistry } = require('@polkadot/types/create');
const { encodeGeneralTransaction } = require('./authorized-heartbeat-fixture.cjs');

test('authorized heartbeat uses General version5 and version0 extension bytes without an outer signature', () => {
  const registry = new TypeRegistry();
  registry.setSignedExtensions(['AuthorizeCall', 'CheckSpecVersion', 'CheckTxVersion', 'CheckGenesis',
    'CheckMortality', 'CheckNonce', 'CheckWeight', 'ChargeTransactionPayment'], { AuthorizeCall: { extrinsic: {}, payload: {} } });
  const call = { toU8a: () => new Uint8Array([1, 2]) };
  assert.equal(encodeGeneralTransaction(registry, call), '0x1c45000000000102');
  assert.equal(encodeGeneralTransaction(registry, call, 1), '0x1c45000000040102');
});

test('unreviewed extension bytes cannot silently enter the General fixture', () => {
  const registry = new TypeRegistry();
  registry.setSignedExtensions(['CheckMortality', 'CheckNonce', 'ChargeTransactionPayment', 'Unexpected'], {
    Unexpected: { extrinsic: { unexpected: 'u32' }, payload: {} },
  });
  assert.throws(() => encodeGeneralTransaction(registry, { toU8a: () => new Uint8Array([1, 2]) }), /era\/nonce\/tip/);
});
