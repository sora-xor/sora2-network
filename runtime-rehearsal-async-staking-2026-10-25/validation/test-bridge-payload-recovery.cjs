'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { test } = require('node:test');
const { verifyRecovery } = require('./verify-bridge-payload-recovery.cjs');
const files = {
  evidencePath: 'bridge-payload-evidence.json', requestsPath: 'bridge-request-recovery-source.json',
  archivePath: 'bridge-archived-payload-source.json', inventoryPath: 'bridge-request-recovery-capture.json',
  baselineMetadataPath: 'baseline-metadata.scale', candidateMetadataPath: 'candidate-metadata.scale',
};
const options = Object.fromEntries(Object.entries(files).map(([key, value]) => [key, path.join(__dirname, value)]));
const inputs = Object.fromEntries(['evidencePath', 'requestsPath', 'archivePath', 'inventoryPath']
  .map(key => [key, JSON.parse(fs.readFileSync(options[key]))]));
const flip = hex => hex.slice(0, -1) + (hex.endsWith('0') ? '1' : '0');
const archived = evidence => evidence.payloads.find(payload => payload.source.kind === 'archived-pending-creation-extrinsic');

async function rejects(mutator, keys = ['evidencePath']) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-payload-negative-'));
  try {
    const changed = Object.fromEntries(keys.map(key => [key, structuredClone(inputs[key])]));
    mutator(changed);
    const args = { ...options };
    for (const key of keys) {
      args[key] = path.join(directory, key + '.json');
      fs.writeFileSync(args[key], JSON.stringify(changed[key]));
    }
    await assert.rejects(() => verifyRecovery(args));
  } finally { fs.rmSync(directory, { recursive: true, force: true }); }
}

test('all captured pending payloads have verified preimages and unchanged decode behavior', async () => {
  const proof = await verifyRecovery(options);
  assert.equal(proof.payloadCount, 2061);
  assert.equal(proof.maxCallBytes, 278);
  assert.deepEqual(proof.classifications, { rejectedByBoth: 2049, decodedIdentically: 12 });
  assert.equal(proof.rows.filter(row => row.extrinsicsRootVerified).length, 13);
});
test('rejects altered call bytes', () => rejects(({ evidencePath }) => { evidencePath.payloads[0].callHex = flip(evidencePath.payloads[0].callHex); }));
test('rejects altered pending hash', () => rejects(({ evidencePath }) => { evidencePath.payloads[0].hash = flip(evidencePath.payloads[0].hash); }));
test('rejects altered call length', () => rejects(({ evidencePath }) => { evidencePath.payloads[0].callBytes++; }));
test('rejects altered historic pallet index', () => rejects(({ evidencePath }) => { evidencePath.payloads[0].source.palletIndex = 31; }));
test('rejects altered retained request bytes', () => rejects(({ requestsPath }) => {
  const key = inputs.evidencePath.payloads[0].source.requestKey;
  const row = requestsPath.find(row => row.key === key); row.rawValue = flip(row.rawValue);
}, ['requestsPath']));
test('rejects missing raw source binding', () => rejects(({ evidencePath }) => { delete evidencePath.payloads[0].source.loadRequestKey; }));
test('rejects changed raw source reference', () => rejects(({ evidencePath }) => { evidencePath.payloads[0].source.reference = 'unrelated.json'; }));
test('rejects omitted pending operation', () => rejects(({ evidencePath }) => { evidencePath.payloads.pop(); }));
test('rejects duplicate pending identity', () => rejects(({ evidencePath }) => { evidencePath.payloads.push(evidencePath.payloads[0]); }));
test('rejects changed pending account identity', () => rejects(({ evidencePath }) => { evidencePath.payloads[0].account = inputs.inventoryPath.peers[0]; }));
test('rejects an altered archived header', () => rejects(({ archivePath }) => {
  const source = archived(inputs.evidencePath).source;
  archivePath.blocks.find(block => block.blockHash === source.blockHash).header.number = '0x1';
}, ['archivePath']));
test('rejects changed original extrinsic source bytes', () => rejects(({ evidencePath }) => {
  const payload = archived(evidencePath); payload.source.extrinsicHex = flip(payload.source.extrinsicHex);
}));
test('rejects consistent extrinsic and source tampering through the header extrinsics root', () => rejects(({ evidencePath, archivePath }) => {
  const payload = archived(evidencePath), source = payload.source;
  const before = source.extrinsicHex;
  source.extrinsicHex = before.slice(0, 100) + (before[100] === '0' ? '1' : '0') + before.slice(101);
  archivePath.blocks.find(block => block.blockHash === source.blockHash).extrinsics[source.extrinsicIndex] = source.extrinsicHex;
}, ['evidencePath', 'archivePath']));
test('rejects missing archived block evidence', () => rejects(({ archivePath }) => {
  const source = archived(inputs.evidencePath).source;
  archivePath.blocks = archivePath.blocks.filter(block => block.blockHash !== source.blockHash);
}, ['archivePath']));
test('rejects a non-adjacent storage transition', () => rejects(({ archivePath }) => {
  const hash = archived(inputs.evidencePath).hash;
  archivePath.transitions.find(transition => transition.hash === hash).before.height--;
}, ['archivePath']));
