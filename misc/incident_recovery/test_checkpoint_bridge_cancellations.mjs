import assert from 'node:assert/strict';
import {test} from 'node:test';
import {createHash} from 'node:crypto';
import {bindPayload, storedCallMatches, uniqueBy} from './checkpoint_bridge_cancellations.mjs';

// The production runner supplies BLAKE2-256 from the checkpoint registry.
// A deterministic hash is sufficient to exercise preimage binding here.
const hash = value => `0x${createHash('sha256').update(value).digest('hex')}`;
const call = Buffer.from([30, 11, 0, 7]);
const pending = {account: 'public-owner', hash: hash(call)};
const payload = {...pending, callHex: `0x${call.toString('hex')}`, callBytes: call.length,
  source: {kind: 'retained-historical-request-reconstruction'}};
const proof = {...pending, callBytes: call.length, sourceKind: payload.source.kind,
  deployedDecode: 'rejected-absent-DEXAPI-call-11', rawRequestsVerified: true};

test('recovered calls stay opaque and every pending identity and byte is bound', () => {
  assert.deepEqual(bindPayload(pending, payload, proof, hash), {call, incompatible: true});
  for (const changed of [
    {...payload, hash: hash(Buffer.from([0]))}, {...payload, account: 'another-owner'},
    {...payload, callBytes: call.length + 1}, {...payload, callHex: '0x1e0b0008'},
  ]) assert.throws(() => bindPayload(pending, changed, proof, hash));
  assert.throws(() => bindPayload(pending, payload, {...proof, rawRequestsVerified: false}, hash));
  assert.throws(() => bindPayload(pending, payload, {...proof, deployedDecode: 'unknown'}, hash));
  assert.throws(() => bindPayload(pending, undefined, proof, hash));
});

test('archived calls require a roundtrip, header, extrinsics root and adjacent transition', () => {
  const archived = {...payload, source: {kind: 'archived-pending-creation-extrinsic'}};
  const evidence = {...proof, sourceKind: archived.source.kind, deployedDecode: 'rejected',
    originalExtrinsicRoundtrip: true, headerHashVerified: true,
    extrinsicsRootVerified: true, witnessedAdjacentStorageTransition: true};
  assert.equal(bindPayload(pending, archived, evidence, hash).incompatible, true);
  for (const key of ['originalExtrinsicRoundtrip', 'headerHashVerified', 'extrinsicsRootVerified', 'witnessedAdjacentStorageTransition']) {
    assert.throws(() => bindPayload(pending, archived, {...evidence, [key]: false}, hash));
  }
  assert.equal(bindPayload(pending, archived, {...evidence, deployedDecode: 'decoded'}, hash).incompatible, false);
});

test('hash-only operations have no invented deposit and stored envelopes are byte exact', () => {
  assert.equal(storedCallMatches(undefined, call), null);
  const stored = Buffer.concat([Buffer.from([call.length << 2]), call, Buffer.alloc(32, 5), Buffer.alloc(16, 9)]);
  assert.deepEqual(storedCallMatches(`0x${stored.toString('hex')}`, call), stored);
  assert.throws(() => storedCallMatches(`0x${stored.toString('hex')}00`, call));
  assert.throws(() => storedCallMatches(`0x${stored.subarray(0, -1).toString('hex')}`, call));
  assert.throws(() => storedCallMatches(`0x${stored.toString('hex')}`, Buffer.from([30, 11, 0, 8])));
  const noncanonical = Buffer.concat([Buffer.from([17, 0]), call, Buffer.alloc(48)]);
  assert.throws(() => storedCallMatches(`0x${noncanonical.toString('hex')}`, call));
});

test('duplicate payload identities fail instead of replacing evidence silently', () => {
  assert.equal(uniqueBy([pending], row => row.hash, 'payload').size, 1);
  assert.throws(() => uniqueBy([pending, pending], row => row.hash, 'payload'), /duplicate payload/);
});
