'use strict';

// Offline regression coverage using the captured mainnet metadata. No network or signing.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const { TypeRegistry, Metadata, expandMetadata } = require('@polkadot/types');
const { blake2AsHex } = require('@polkadot/util-crypto');
const { inspectMotion } = require('./prepare-collective-actions.cjs');

function fixture(collective, { absent = false, index = 123, threshold } = {}) {
  const registry = new TypeRegistry();
  const metadata = new Metadata(registry, fs.readFileSync(path.join(__dirname, 'live-metadata.scale')));
  registry.setMetadata(metadata);
  const expanded = expandMetadata(registry, metadata);
  const live = JSON.parse(fs.readFileSync(path.join(__dirname, 'live-chain.json'), 'utf8'));
  const checked = JSON.parse(fs.readFileSync(path.join(__dirname, 'collective-actions-check.json'), 'utf8'));
  const recorded = checked.motions.find((item) => item.collective === collective);
  const filename = collective === 'council' ? 'democracy-external-propose-majority-call.hex' : 'democracy-fast-track-call.hex';
  const expected = registry.createType('Call', fs.readFileSync(path.join(__dirname, '..', filename), 'utf8').trim());
  const members = collective === 'council' ? live.councilMembers : live.technicalCommitteeMembers;
  const required = threshold ?? (collective === 'council' ? 4 : 3);
  const votes = {
    index: registry.createType('u32', index), threshold: registry.createType('u32', required),
    ayes: members.slice(0, 2), nays: [], end: registry.createType('u32', 500)
  };
  const some = (value) => ({ isSome: true, isNone: false, unwrap: () => value });
  const none = { isSome: false, isNone: true };
  const proposalOf = async () => absent ? none : some(expected);
  proposalOf.key = (hash) => { assert.equal(hash, blake2AsHex(expected.toU8a(), 256)); return '0x1234'; };
  const at = {
    registry,
    query: { [collective]: {
      members: async () => members,
      voting: async () => absent ? none : some(votes), proposalOf
    } },
    call: { transactionPaymentApi: { queryInfo: { meta: { type: 'RuntimeDispatchInfo' } } } }
  };
  const api = {
    extrinsicVersion: 4,
    tx: { [collective]: Object.fromEntries(['vote', 'close'].map((method) => [method,
      (...args) => ({ method: expanded.tx[collective][method](...args) })])) },
    rpc: { state: {
      getStorage: async (key, block) => { assert.equal(key, '0x1234'); assert.equal(block, '0xabcdef'); return some(expected); },
      call: async (name, input, block) => {
        assert.equal(name, 'TransactionPaymentApi_query_info');
        assert.equal(block, '0xabcdef');
        const bytes = Buffer.from(input.slice(2), 'hex');
        const tx = registry.createType('Extrinsic', bytes.subarray(0, -4));
        assert.equal(tx.method.toHex(), expected.toHex());
        assert.equal(tx.isSigned, false);
        assert.equal(bytes.readUInt32LE(bytes.length - 4), tx.encodedLength);
        return registry.createType('RuntimeDispatchInfo', { weight: recorded.proposalWeightBound, class: 'Normal', partialFee: 0 });
      }
    } }
  };
  return { api, at, expected, members, recorded, collective };
}

for (const collective of ['council', 'technicalCommittee']) {
  test(`${collective}: discovers actual stored index and creates correctly bounded unsigned calls`, async () => {
    const f = fixture(collective, { index: 371 });
    const result = await inspectMotion(f.api, f.at, '0xabcdef', 450, collective, f.expected, f.members);
    assert.equal(result.proposalIndex, 371);
    assert.equal(result.enoughExplicitAyesToClose, false);
    assert.equal(result.closeLengthBound, f.expected.encodedLength + 4);
    const aye = f.at.registry.createType('Call', result.unsignedCalls.aye.hex);
    const close = f.at.registry.createType('Call', result.unsignedCalls.close.hex);
    assert.equal(aye.args[1].toNumber(), 371);
    assert.equal(aye.args[2].isTrue, true);
    assert.equal(close.args[1].toNumber(), 371);
    assert.equal(close.args[2].refTime.toString(), f.recorded.proposalWeightBound.refTime);
    assert.equal(close.args[2].proofSize.toString(), f.recorded.proposalWeightBound.proofSize);
    assert.equal(close.args[3].toNumber(), f.expected.encodedLength + 4);
  });
  test(`${collective}: does not fabricate index-specific calls before submission`, async () => {
    const f = fixture(collective, { absent: true });
    const result = await inspectMotion(f.api, f.at, '0xabcdef', 450, collective, f.expected, f.members);
    assert.equal(result.unsignedCalls, null);
  });
  test(`${collective}: rejects an inadequate threshold`, async () => {
    const f = fixture(collective, { threshold: 2 });
    await assert.rejects(inspectMotion(f.api, f.at, '0xabcdef', 450, collective, f.expected, f.members), /cannot satisfy/);
  });
}
