'use strict';
const assert = require('node:assert/strict');
const { GenericExtrinsic } = require('@polkadot/types');
const { compactAddLength, u8aConcat, u8aToHex } = require('@polkadot/util');
const { sr25519PairFromSeed, sr25519Sign } = require('@polkadot/util-crypto');

function encodeGeneralTransaction(registry, call, tip = 0) {
  const types = registry.getSignedExtensionTypes();
  assert.deepEqual(Object.keys(types), ['era', 'nonce', 'tip'], 'Reviewed extension encoding must remain era/nonce/tip');
  const values = { era: 0, nonce: 0, tip };
  const extra = Object.entries(types).map(([name, type]) => registry.createType(type, values[name]).toU8a());
  // Version5 General preamble, transaction-extension pipeline version0. The
  // metadata gate separately proves the zero-encoded AuthorizeCall addition.
  return u8aToHex(compactAddLength(u8aConcat(new Uint8Array([0x45, 0]), ...extra, call.toU8a())));
}

async function authorizedHeartbeatFixture(block, context) {
  const { chain, Block, report, rawQuery, storageKey, signedCheck, applyExtrinsic, progress, save, syntheticFixtures } = context;
  assert(syntheticFixtures, 'Authorized heartbeat rehearsal requires disclosed synthetic validator keys');
  const meta = await block.meta, registry = meta.registry;
  const session = (await rawQuery(block, 'session', 'currentIndex')).toNumber();
  const validators = await rawQuery(block, 'session', 'validators');
  assert(validators.length > 0);
  const validator = validators[0];
  const pair = sr25519PairFromSeed(new Uint8Array(32).fill(201));
  const overrides = [];
  async function override(pallet, name, params, value) {
    const key = storageKey(meta, pallet, name, params);
    overrides.push({ pallet, name, key: key.toHex(), before: await block.get(key.toHex()) ?? null,
      value: value === null ? null : registry.createType(key.outputType, value).toHex() });
  }
  await override('imOnline', 'keys', [], [u8aToHex(pair.publicKey)]);
  await override('imOnline', 'receivedHeartbeats', [session, 0], null);
  await override('imOnline', 'authoredBlocks', [session, validator], null);
  const fixture = new Block(chain, block.number, block.hash, block, { header: await block.header, extrinsics: [], storage: block.storage });
  fixture.pushStorageLayer().setAll(overrides.map(({ key, value }) => [key, value]));
  report.fixtureOverrides.push({ kind: 'sdk2609AuthorizedHeartbeat', fixtureOnly: true,
    purpose: 'One local synthetic authority key and cleared liveness markers; real inner sr25519 signature, no production key or outer signer', overrides });
  const heartbeatType = meta.tx.imOnline.heartbeat.meta.args[0].type.toString();
  function heartbeatCall(changes = {}, invalidSignature = false) {
    const heartbeat = registry.createType(heartbeatType, { blockNumber: block.number, sessionIndex: session,
      authorityIndex: 0, validatorsLen: 1, ...changes });
    const signature = invalidSignature ? new Uint8Array(64) : sr25519Sign(heartbeat.toU8a(), pair);
    assert.notEqual(u8aToHex(signature), '0xdeadbeef' + 'cd'.repeat(60), 'Inner signature must not use the mocked-signature marker');
    return meta.tx.imOnline.heartbeat(heartbeat, u8aToHex(signature));
  }
  report.authorizedHeartbeatChecks = [];
  async function check(parent, label, encoded, accepted, reason = null) {
    progress('Exact Wasm: authorized heartbeat ' + label);
    const row = { label, encoded, expectedAccepted: accepted, sources: {} };
    report.authorizedHeartbeatChecks.push(row); save();
    for (const source of ['External', 'Local', 'InBlock']) {
      const response = await parent.call('TaggedTransactionQueue_validate_transaction',
        [registry.createType('TransactionSource', source).toHex(), encoded, parent.hash]);
      const result = registry.createType('TransactionValidity', response.result);
      row.sources[source] = result.toJSON();
      assert.equal(result.isOk, accepted, label + ' ' + source);
      if (reason) assert.equal(result.asErr.asInvalid.type, reason);
    }
    const beforeEvents = await rawQuery(parent, 'system', 'events');
    const beforeIssuance = (await rawQuery(parent, 'balances', 'totalIssuance')).toHex();
    const response = await applyExtrinsic(parent, encoded);
    const result = registry.createType('ApplyExtrinsicResult', response.result);
    row.apply = result.toJSON();
    assert.equal(result.isOk, accepted, label + ' application');
    if (accepted) assert(result.asOk.isOk, 'Authorized heartbeat must dispatch successfully');
    else if (reason) assert.equal(result.asErr.asInvalid.type, reason);
    const branch = new Block(chain, parent.number, parent.hash, parent, { header: await parent.header,
      extrinsics: [], storage: parent.storage, storageDiff: Object.fromEntries(response.storageDiff) });
    const accountPrefix = meta.query.system.account.keyPrefix().toHex();
    assert(!response.storageDiff.some(([key]) => key.startsWith(accountPrefix)), 'Heartbeat/general rejection must not write any account');
    assert.equal((await rawQuery(branch, 'balances', 'totalIssuance')).toHex(), beforeIssuance);
    row.events = (await rawQuery(branch, 'system', 'events')).slice(beforeEvents.length)
      .map(({ event }) => ({ section: event.section, method: event.method, data: event.data.toJSON() }));
    assert(!row.events.some(event => event.section === 'transactionPayment'), 'Unsigned authorization must not produce a fictitious fee event');
    if (accepted) {
      assert(row.events.some(event => event.section === 'imOnline' && event.method === 'HeartbeatReceived'));
      assert.equal((await rawQuery(branch, 'imOnline', 'receivedHeartbeats', [session, 0])).unwrap().valueOf(), true);
    } else {
      assert.deepEqual(response.storageDiff, [], 'Invalid authorization must be rolled back');
      assert.equal((await rawQuery(branch, 'system', 'events')).toHex(), beforeEvents.toHex());
    }
    row.accountAndIssuancePreserved = true; row.passed = true; save();
    return branch;
  }
  const failures = [];
  async function checked(...args) {
    try {
      return await check(...args);
    } catch (error) {
      const label = args[1];
      failures.push({ label, error: error.message });
      const row = report.authorizedHeartbeatChecks.find(row => row.label === label);
      if (row) { row.passed = false; row.failure = error.message; }
      report.authorizedHeartbeatFailures = failures;
      save();
      return undefined;
    }
  }
  const call = heartbeatCall();
  await checked(fixture, 'legacy bare heartbeat rejects', new GenericExtrinsic(registry, call).toHex(), false);
  await checked(fixture, 'invalid authority signature rejects', encodeGeneralTransaction(registry, heartbeatCall({}, true)), false, 'BadProof');
  await checked(fixture, 'wrong session rejects', encodeGeneralTransaction(registry, heartbeatCall({ sessionIndex: session + 1 })), false, 'Stale');
  await checked(fixture, 'nonzero unsigned tip rejects', encodeGeneralTransaction(registry, call, 1), false, 'Payment');
  await checked(fixture, 'ordinary paid call cannot use General authorization', encodeGeneralTransaction(registry, meta.tx.system.remark('0x01')), false);
  const accepted = await checked(fixture, 'real signed authority heartbeat succeeds without payer', encodeGeneralTransaction(registry, call), true);
  if (accepted) await checked(accepted, 'heartbeat replay rejects', encodeGeneralTransaction(registry, call), false, 'Stale');
  await signedCheck(fixture, 'imOnline.heartbeat signed outer transaction remains paid BadOrigin', call, false, false, 'BadOrigin');
  report.authorizedHeartbeatCryptography = { scheme: 'sr25519', deterministicSyntheticAuthority: true,
    validInnerSignatureAccepted: report.authorizedHeartbeatChecks.some(row => row.label === 'real signed authority heartbeat succeeds without payer' && row.passed),
    invalidInnerSignatureRejected: report.authorizedHeartbeatChecks.some(row => row.label === 'invalid authority signature rejects' && row.passed),
    innerSignaturesUseMockMarker: false, productionKeysUsed: 0 };
  save();
  assert.deepEqual(failures, [], 'Every independent authorization case must pass');
  assert.equal(report.authorizedHeartbeatChecks.length, 7, 'Replay coverage requires successful authorization');
  return true;
}

module.exports = { authorizedHeartbeatFixture, encodeGeneralTransaction };
