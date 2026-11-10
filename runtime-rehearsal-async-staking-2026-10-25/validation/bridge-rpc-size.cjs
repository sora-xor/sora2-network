'use strict';
const assert = require('node:assert/strict');
// Inspect the raw JSON-RPC response. The installed PolkadotJS method decorates
// state_getStorageSize as u64, so codec.isSome silently misclassifies numbers.
function normalizeStorageSize(value) {
  if (value === null) return null;
  assert((typeof value === 'number' && Number.isSafeInteger(value) && value >= 0)
    || (typeof value === 'string' && /^(0|[1-9][0-9]*)$/.test(value)), 'Invalid or missing raw storage-size result');
  const size = BigInt(value);
  assert(size <= 18446744073709551615n, 'Storage size exceeds u64');
  return size.toString();
}
module.exports = { normalizeStorageSize };
