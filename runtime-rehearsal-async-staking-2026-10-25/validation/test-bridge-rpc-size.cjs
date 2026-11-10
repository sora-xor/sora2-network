'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { normalizeStorageSize } = require('./bridge-rpc-size.cjs');
test('numeric RPC response is present even though it has no isSome property', () => {
  assert.equal((328).isSome, undefined);
  assert.equal(normalizeStorageSize(328), '328');
});
test('absence, empty bytes, and unavailable response remain distinct', () => {
  assert.equal(normalizeStorageSize(null), null);
  assert.equal(normalizeStorageSize(0), '0');
  assert.throws(() => normalizeStorageSize(undefined));
});
test('exact decimal u64 responses retain precision', () => {
  assert.equal(normalizeStorageSize('18446744073709551615'), '18446744073709551615');
  assert.throws(() => normalizeStorageSize('18446744073709551616'));
});
for (const [name, value] of Object.entries({ negative: -1, fractional: 1.5, unsafe: Number.MAX_SAFE_INTEGER + 1,
  nan: NaN, infinity: Infinity, boolean: false, empty: '', noncanonical: '0328',
  optionObject: { isSome: true, unwrap: () => 328 }, scalarCodec: { toString: () => '328' } })) {
  test('reject malformed raw response: ' + name, () => assert.throws(() => normalizeStorageSize(value)));
}
