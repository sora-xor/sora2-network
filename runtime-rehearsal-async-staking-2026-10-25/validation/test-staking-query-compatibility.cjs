'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync, mkdtempSync, rmSync } = require('node:fs');
const { join } = require('node:path');
const { tmpdir } = require('node:os');
const { gzipSync, gunzipSync } = require('node:zlib');
const { createHash } = require('node:crypto');
const { verifyStakingQueryCompatibility } = require('./verify-staking-query-compatibility.cjs');
const baselineMetadataPath = join(__dirname, 'deployed-metadata.scale');
const inventoryPath = join(__dirname, 'staking-public-inventory.json');
const statePath = join(__dirname, 'staking-public-state.json.gz');
const blockHash = JSON.parse(readFileSync(inventoryPath)).blockHash;
const options = { baselineMetadataPath, candidateMetadataPath: baselineMetadataPath, inventoryPath, statePath, blockHash };

test('pinned baseline self-check covers every value in all five prefixes', () => {
  const result = verifyStakingQueryCompatibility(options);
  assert.deepEqual(result.storageItems.map(row => [row.storage, row.entries]), [
    ['BondedEras', 1], ['ClaimedRewards', 746], ['ErasRewardPoints', 85], ['Ledger', 2626], ['ErasStakersPaged', 1275],
  ]);
  assert.equal(result.totalValues, 4733);
});

test('candidate query field rename fails despite byte-identical storage encoding', () => {
  const temporary = mkdtempSync(join(tmpdir(), 'staking-query-field-'));
  try {
    const metadata = readFileSync(baselineMetadataPath);
    const name = Buffer.from('legacy_claimed_rewards');
    const position = metadata.indexOf(name);
    assert(position > 0);
    Buffer.from('legacy_claimed_rewardx').copy(metadata, position);
    const candidateMetadataPath = join(temporary, 'changed.scale');
    writeFileSync(candidateMetadataPath, metadata);
    assert.throws(() => verifyStakingQueryCompatibility({ ...options, candidateMetadataPath }), /query JSON changed/);
  } finally { rmSync(temporary, { recursive: true, force: true }); }
});

for (const corruption of ['missing-prefix', 'trailing-bytes']) test(corruption + ' fails even with matching fixture inventory hash', () => {
  const temporary = mkdtempSync(join(tmpdir(), 'staking-query-state-'));
  try {
    const state = JSON.parse(gunzipSync(readFileSync(statePath)));
    const inventory = JSON.parse(readFileSync(inventoryPath));
    const bondedKey = inventory.inventories.BondedEras.samples[0].key;
    if (corruption === 'missing-prefix') {
      state.records = state.records.filter(([key]) => key !== bondedKey);
      state.keys -= 1;
      inventory.totalKeys -= 1;
    } else {
      state.records.find(([key]) => key === bondedKey)[1] += '00';
    }
    const bytes = gzipSync(Buffer.from(JSON.stringify(state)));
    inventory.stateSha256 = createHash('sha256').update(bytes).digest('hex');
    const changedState = join(temporary, 'state.json.gz'), changedInventory = join(temporary, 'inventory.json');
    writeFileSync(changedState, bytes);
    writeFileSync(changedInventory, JSON.stringify(inventory));
    assert.throws(() => verifyStakingQueryCompatibility({ ...options, statePath: changedState, inventoryPath: changedInventory }));
  } finally { rmSync(temporary, { recursive: true, force: true }); }
});
