'use strict';
// Offline client proof for the exact metadata and complete pinned staking values.
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { gunzipSync } = require('node:zlib');
const { createHash } = require('node:crypto');
const { Metadata, TypeRegistry } = require('@polkadot/types');
const { xxhashAsHex } = require('@polkadot/util-crypto');

const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const STORAGE_ITEMS = ['BondedEras', 'ClaimedRewards', 'ErasRewardPoints', 'Ledger', 'ErasStakersPaged'];

function metadataRegistry(bytes) {
  const registry = new TypeRegistry();
  const metadata = new Metadata(registry, new Uint8Array(bytes));
  registry.setMetadata(metadata);
  const staking = metadata.asLatest.pallets.find(pallet => pallet.name.toString() === 'Staking');
  assert(staking && staking.storage.isSome, 'Staking metadata is required');
  const storage = staking.storage.unwrap();
  assert.equal(storage.prefix.toString(), 'Staking');
  const entries = new Map([...storage.items].map(entry => [entry.name.toString(), entry]));
  return { registry, entries };
}

function verifyStakingQueryCompatibility({ baselineMetadataPath, candidateMetadataPath, inventoryPath, statePath, blockHash }) {
  const baselineBytes = readFileSync(baselineMetadataPath);
  const candidateBytes = readFileSync(candidateMetadataPath);
  const inventoryBytes = readFileSync(inventoryPath);
  const stateBytes = readFileSync(statePath);
  const inventory = JSON.parse(inventoryBytes);
  const state = JSON.parse(gunzipSync(stateBytes));
  assert.equal(inventory.status, 'passed');
  assert.equal(inventory.complete, true);
  assert.equal(state.complete, true);
  assert.equal(state.status, 'captured-complete-pinned-staking');
  assert.equal(inventory.blockHash, blockHash);
  assert.equal(state.blockHash, blockHash);
  assert.equal(sha256(stateBytes), inventory.stateSha256);
  assert.equal(sha256(baselineBytes), inventory.metadataSha256);
  assert.equal(state.records.length, inventory.totalKeys);
  assert.equal(state.records.length, state.keys);
  assert.equal(state.prefix, xxhashAsHex('Staking', 128));
  assert(state.records.every(([key]) => key.startsWith(state.prefix)));
  assert.equal(new Map(state.records).size, state.records.length);
  const baseline = metadataRegistry(baselineBytes), candidate = metadataRegistry(candidateBytes);
  const results = [];
  for (const name of STORAGE_ITEMS) {
    const prefix = xxhashAsHex('Staking', 128) + xxhashAsHex(name, 128).slice(2);
    const values = state.records.filter(([key]) => key.startsWith(prefix)).sort(([a], [b]) => a.localeCompare(b));
    assert.equal(values.length, inventory.inventories[name].count, 'Complete prefix coverage: ' + name);
    assert(values.length > 0, 'Pinned query proof cannot silently skip ' + name);
    const decode = ({ registry, entries }, value) => {
      const entry = entries.get(name);
      assert(entry, 'Missing old storage query: ' + name);
      const type = entry.type.isPlain ? entry.type.asPlain : entry.type.asMap.value;
      const bytes = Uint8Array.from(Buffer.from(value.slice(2), 'hex'));
      const decoded = registry.createTypeUnsafe('Lookup' + type.toString(), [bytes], { isPedantic: true });
      assert.equal(decoded.toHex(), value, 'Query decode must consume and roundtrip all SCALE bytes: ' + name);
      assert.equal(decoded.encodedLength, bytes.length);
      return decoded.toJSON();
    };
    const decoded = values.map(([key, value]) => {
      const before = decode(baseline, value), after = decode(candidate, value);
      assert.deepEqual(after, before, 'Existing PolkadotJS query JSON changed: ' + name + ':' + key);
      return [key, before];
    });
    results.push({ storage: name, prefix, entries: values.length,
      rawPairsSha256: sha256(Buffer.from(JSON.stringify(values))),
      decodedPairsSha256: sha256(Buffer.from(JSON.stringify(decoded))),
      completePrefixVerified: true, allScaleBytesRoundtrip: true, decodedJsonIdentical: true });
  }
  return { status: 'passed', localOnly: true, remoteRequests: 0, blockHash,
    inputs: { baselineMetadataSha256: sha256(baselineBytes), candidateMetadataSha256: sha256(candidateBytes),
      inventorySha256: sha256(inventoryBytes), stateSha256: sha256(stateBytes), verifierSha256: sha256(readFileSync(__filename)) },
    storageItems: results, totalValues: results.reduce((total, item) => total + item.entries, 0),
    scope: 'Every pinned value for all five named prefixes, decoded independently with deployed and candidate PolkadotJS registries; exact JSON equality and full SCALE roundtrip.' };
}

module.exports = { verifyStakingQueryCompatibility, STORAGE_ITEMS };
