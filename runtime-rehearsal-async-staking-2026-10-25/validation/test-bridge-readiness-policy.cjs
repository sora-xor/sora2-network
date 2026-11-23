'use strict';
const { test, before } = require('node:test');
const assert = require('node:assert/strict');
const { TypeRegistry } = require('@polkadot/types');
const { blake2AsHex, cryptoWaitReady, xxhashAsHex } = require('@polkadot/util-crypto');
const { evaluateBridgeReadiness, accountKey, pendingPrefix, pendingKey, networkKey, storedLength } = require('./bridge-readiness-policy.cjs');
before(async () => { assert(await cryptoWaitReady()); });

const id = n => '0x' + n.toString(16).padStart(2, '0').repeat(32);
const sha = n => n.toString(16).padStart(2, '0').repeat(32);
function registryFixture() {
  const registry = new TypeRegistry();
  registry.register({
    FixtureAccount: { signatories: 'Vec<AccountId>', threshold: 'u8' },
    FixtureHeight: { _enum: { Thischain: 'u32', Sidechain: 'u64' } },
    FixtureWhen: { height: 'FixtureHeight', index: 'u32' },
    FixturePending: { when: 'FixtureWhen', deposit: 'u128', depositor: 'AccountId', approvals: 'Vec<AccountId>' },
    FixtureCall: { _enum: { ping: 'u32', blob: 'Bytes' } },
  });
  const metadataPallet = (name, types) => ({ name, storage: { unwrap: () => ({ items: Object.entries(types).map(([name, value]) => ({ name, type: { isMap: true, asMap: { value } } })) }) } });
  const baselineRegistry = {
    metadata: { pallets: [metadataPallet('BridgeMultisig', { Accounts: 'FixtureAccount', Multisigs: 'FixturePending', Calls: '(Bytes,AccountId,u128)' }),
      metadataPallet('EthBridge', { BridgeAccount: 'AccountId', Peers: 'Vec<AccountId>' })] },
    createLookupType: type => type,
    createType: (type, bytes) => registry.createType(type === 'Call' ? 'FixtureCall' : type, bytes),
  };
  const candidateRegistry = { createType: (type, bytes) => { assert.equal(type, 'Call'); return registry.createType('FixtureCall', bytes); } };
  return { registry, baselineRegistry, candidateRegistry };
}

function fixture({ stored = false, recovered = false, empty = false } = {}) {
  const { registry, baselineRegistry, candidateRegistry } = registryFixture();
  const account = id(99), members = [id(1), id(2), id(3)];
  const callHex = registry.createType('FixtureCall', { ping: 7 }).toHex();
  const hash = blake2AsHex(Buffer.from(callHex.slice(2), 'hex'), 256);
  const candidate = { sha256: sha(5), specVersion: 137, transactionVersion: 133 };
  const snapshot = { blockHash: id(6), blockNumber: 28179122 };
  const rawPending = registry.createType('FixturePending', { when: { height: { Thischain: 12 }, index: 1 }, deposit: 0, depositor: members[0], approvals: [members[0]] });
  const operation = { hash, storageKey: pendingKey(account, hash), storedCallKey: xxhashAsHex('BridgeMultisig', 128) + xxhashAsHex('Calls', 128).slice(2) + hash.slice(2), rawValue: rawPending.toHex(), when: rawPending.when.toJSON(),
    depositor: members[0], approvals: [members[0]], storageSizeRpcValue: null, storedEncodedBytes: null, storedCallBytes: null };
  if (stored) {
    operation.storedRawValue = registry.createType('(Bytes,AccountId,u128)', [callHex, members[0], 0]).toHex();
    operation.storedEncodedBytes = String((operation.storedRawValue.length - 2) / 2);
    operation.storageSizeRpcValue = Number(operation.storedEncodedBytes);
    operation.storedCallBytes = (callHex.length - 2) / 2;
    operation.storedCallHex = callHex;
  }
  const row = { network: 0, account, peerCount: 3, quorum: 3, perProposerQuotaAfterUpgrade: 42,
    peers: members.map(account => ({ account })), currentBridgePeers: members,
    currentBridgePeersStorageKey: networkKey(0, 'Peers'), rawCurrentBridgePeersValue: registry.createType('Vec<AccountId>', members).toHex(),
    accountStorageKey: accountKey(account), rawAccountValue: registry.createType('FixtureAccount', { signatories: members, threshold: 67 }).toHex(),
    networkStorageKey: networkKey(0, 'BridgeAccount'), rawNetworkValue: account,
    pendingOperationsObserved: empty ? 0 : 1, pendingScanComplete: true, truncated: false,
    operations: empty ? [] : [operation],
    inventoryProof: { prefix: pendingPrefix(account), pages: [{ startKey: null, pageSize: 128, keys: empty ? [] : [operation.storageKey] }], terminalKeys: [] } };
  const report = { candidateBinding: { ...candidate }, blockHash: snapshot.blockHash, blockNumber: snapshot.blockNumber,
    submittedTransactions: 0, privateKeysRead: 0, limits: { storedCallBytes: 16384, pageSize: 128 },
    pendingInventoryComplete: true, totalPendingOperationsObserved: row.pendingOperationsObserved, accounts: [row],
    networkInventory: { prefix: row.networkStorageKey.slice(0, 66), pageSize: 17, complete: true,
      entries: [{ key: row.networkStorageKey, value: row.rawNetworkValue }] } };
  const payloadEvidence = recovered ? [{ account, hash, callHex, callBytes: (callHex.length - 2) / 2,
    candidateSha256: candidate.sha256, blockHash: snapshot.blockHash, source: { kind: 'historical-extrinsic', reference: 'block:12/extrinsic:1' } }] : [];
  return { report, candidate, snapshot, payloadEvidence, baselineRegistry, candidateRegistry, registry };
}
const evaluate = input => evaluateBridgeReadiness(input);
const confirmed = input => ({ status: 'confirmed', candidateSha256: input.candidate.sha256, blockHash: input.snapshot.blockHash,
  source: { kind: 'trusted-user-provided-operator-record', reference: 'operator-readiness/record-1' },
  accounts: input.report.accounts.map(row => ({ account: row.account, quorum: row.quorum,
    peers: row.peers.map(peer => peer.account), compatibleClientConfirmed: true, configurationConfirmed: true, recoveryProcedureConfirmed: true })) });

test('hash-only inventory proves absence of oversized stored bytes, not unknown payload or operator readiness', () => {
  const result = evaluate(fixture());
  assert.equal(result.inventoryVerified, true);
  assert.equal(result.storedCallBoundsVerified, true);
  assert.equal(result.storedCallBoundObservable, false);
  assert.equal(result.payloadBoundsResolved, false);
  assert.equal(result.payloadCompatibilityVerified, false);
  assert.equal(result.operatorReadinessConfirmed, false);
  assert.equal(result.activationAuthorized, false);
});

test('stored and recovered bytes independently establish bounds and candidate decode', () => {
  for (const options of [{ stored: true }, { recovered: true }, { stored: true, recovered: true }]) {
    const result = evaluate(fixture(options));
    assert.equal(result.payloadBoundsVerified, true);
    assert.equal(result.payloadCompatibilityVerified, true);
    assert.equal(result.operatorReadinessConfirmed, false);
  }
});

test('operator evidence remains separate from payload evidence and activation authority', () => {
  const input = fixture(); input.operatorEvidence = confirmed(input);
  const result = evaluate(input);
  assert.equal(result.operatorReadinessConfirmed, true);
  assert.equal(result.payloadBoundsResolved, false);
  assert.equal(result.activationAuthorized, false);
});

test('empty pending inventory needs no invented payload evidence', () => {
  const result = evaluate(fixture({ empty: true }));
  assert.equal(result.payloadBoundsVerified, true);
  assert.equal(result.payloadCompatibilityVerified, true);
  assert.equal(result.operatorReadinessConfirmed, false);
});

const invalidCases = [
  ['stale candidate', x => { x.report.candidateBinding.sha256 = sha(9); }, /candidate binding/],
  ['stale snapshot', x => { x.report.blockHash = id(9); }, /snapshot differs/],
  ['truncated inventory', x => { x.report.accounts[0].truncated = true; }, /truncated/],
  ['omitted network', x => { x.report.networkInventory.entries = []; }, /network omitted/],
  ['wrong raw network binding', x => { x.report.accounts[0].rawNetworkValue = id(8); }, /Raw network value/],
  ['duplicate pending key', x => { x.report.accounts[0].inventoryProof.pages[0].keys.push(x.report.accounts[0].operations[0].storageKey); }, /duplicate or out of order/],
  ['unobserved full-page completion', x => { x.report.accounts[0].inventoryProof.pages[0].pageSize = 1; }, /terminal probe/],
  ['nonempty terminal probe', x => { x.report.accounts[0].inventoryProof.terminalKeys = ['0x00']; }, /terminal probe/],
  ['pending raw trailing bytes', x => { x.report.accounts[0].operations[0].rawValue += '00'; }, /noncanonical or trailing/],
  ['pending wrong raw timepoint', x => { x.report.accounts[0].operations[0].when.index++; }, /timepoint differs/],
  ['wrong pending hash key', x => { x.report.accounts[0].operations[0].hash = id(10); }, /storage key differs/],
  ['missing peer', x => { x.report.accounts[0].peers.pop(); }, /peers differ/],
  ['wrong quorum', x => { x.report.accounts[0].quorum = 2; }, /Quorum differs/],
  ['wrong current peer raw projection', x => { x.report.accounts[0].currentBridgePeers.pop(); }, /Current bridge peers differ/],
  ['invented absent stored length', x => { x.report.accounts[0].operations[0].storedCallBytes = 1; }, /Absent stored call/],
];
for (const [name, mutate, error] of invalidCases) test(name + ' fails closed', () => {
  const input = fixture(); mutate(input); assert.throws(() => evaluate(input), error);
});

const payloadCases = [
  ['payload tampered', x => { x.payloadEvidence[0].callHex = '0x0008000000'; }, /payload hash differs/],
  ['payload length tampered', x => { x.payloadEvidence[0].callBytes++; }, /byte length differs/],
  ['payload candidate stale', x => { x.payloadEvidence[0].candidateSha256 = sha(9); }, /stale candidate/],
  ['payload snapshot stale', x => { x.payloadEvidence[0].blockHash = id(9); }, /stale snapshot/],
  ['payload provenance absent', x => { delete x.payloadEvidence[0].source; }, /provenance missing/],
  ['duplicate payload evidence', x => { x.payloadEvidence.push({ ...x.payloadEvidence[0] }); }, /Duplicate recovered/],
  ['unrelated payload account', x => { x.payloadEvidence[0].account = id(77); }, /does not belong/],
];
for (const [name, mutate, error] of payloadCases) test(name + ' fails closed', () => {
  const input = fixture({ recovered: true }); mutate(input); assert.throws(() => evaluate(input), error);
});

test('candidate full consumption rejects otherwise hash-matching trailing bytes', () => {
  const input = fixture({ recovered: true });
  const payload = input.payloadEvidence[0], row = input.report.accounts[0];
  payload.callHex += '00'; payload.callBytes++;
  payload.hash = blake2AsHex(Buffer.from(payload.callHex.slice(2), 'hex'), 256);
  row.operations[0].hash = payload.hash; row.operations[0].storageKey = pendingKey(row.account, payload.hash);
  row.operations[0].storedCallKey = row.operations[0].storedCallKey.slice(0, 66) + payload.hash.slice(2);
  row.inventoryProof.pages[0].keys = [row.operations[0].storageKey];
  const result = evaluate(input);
  assert.equal(result.payloadBoundsVerified, true);
  assert.equal(result.payloadCompatibilityVerified, false);
  assert.equal(result.operations[0].baselineDecodable, false);
  assert.equal(result.operations[0].candidateDecodable, false);
  assert.equal(result.blockers[0].reason, 'pre-existing-payload-incompatible-with-baseline-and-candidate');
  assert.equal(result.preExistingIncompatiblePayloads, 1);
  assert.equal(result.newCandidatePayloadRegressions, 0);
});

test('candidate-only decode failure is attributed to the candidate', () => {
  const input = fixture({ recovered: true });
  input.candidateRegistry = { createType: () => { throw new Error('Call removed in candidate'); } };
  const result = evaluate(input);
  assert.equal(result.operations[0].baselineDecodable, true);
  assert.equal(result.operations[0].candidateDecodable, false);
  assert.equal(result.blockers[0].reason, 'candidate-payload-decoding-regression');
  assert.equal(result.newCandidatePayloadRegressions, 1);
  assert.equal(result.preExistingIncompatiblePayloads, 0);
  assert.equal(result.payloadCompatibilityVerified, false);
});

test('a previously invalid body becoming decodable is not reported as preserved compatibility', () => {
  const input = fixture({ recovered: true });
  const createType = input.baselineRegistry.createType;
  input.baselineRegistry.createType = (type, bytes) => {
    if (type === 'Call') throw new Error('No baseline call at this pallet index');
    return createType(type, bytes);
  };
  const result = evaluate(input);
  assert.equal(result.operations[0].baselineDecodable, false);
  assert.equal(result.operations[0].candidateDecodable, true);
  assert.equal(result.blockers[0].reason, 'previously-invalid-payload-now-decodable');
  assert.equal(result.newCandidatePayloadRegressions, 0);
  assert.equal(result.newlyDecodableLegacyPayloads, 1);
  assert.equal(result.payloadCompatibilityVerified, false);
});

test('pre-existing invalid payload remains a recovery item without a candidate decoder', () => {
  const input = fixture({ recovered: true });
  const createType = input.baselineRegistry.createType;
  input.baselineRegistry.createType = (type, bytes) => {
    if (type === 'Call') throw new Error('Invalid baseline call');
    return createType(type, bytes);
  };
  delete input.candidateRegistry;
  const result = evaluate(input);
  assert.equal(result.blockers[0].reason, 'pre-existing-payload-incompatible-with-baseline');
  assert.equal(result.preExistingIncompatiblePayloads, 1);
  assert.equal(result.newCandidatePayloadRegressions, 0);
  assert.equal(result.payloadCompatibilityVerified, false);
});

test('observed oversized legacy call is a blocker even without fetching its payload', () => {
  const input = fixture();
  Object.assign(input.report.accounts[0].operations[0], { storageSizeRpcValue: 16437, storedEncodedBytes: '16437', storedCallBytes: 16385 });
  const result = evaluate(input);
  assert.equal(result.payloadBoundsResolved, true);
  assert.equal(result.payloadBoundsVerified, false);
  assert.equal(result.blockers[0].reason, 'payload-exceeds-bound');
});

test('stored byte arithmetic and raw bytes cannot be silently substituted', () => {
  const input = fixture({ stored: true }); input.report.accounts[0].operations[0].storedEncodedBytes = '55';
  input.report.accounts[0].operations[0].storageSizeRpcValue = 55;
  assert.throws(() => evaluate(input), /stored-call length differs/);
  assert.equal(storedLength('112'), 63);
  assert.equal(storedLength('114'), 64);
  assert.equal(storedLength('16432'), 16382);
  assert.equal(storedLength('16436'), 16384);
  assert.throws(() => storedLength('48'), /Invalid SCALE/);
  assert.throws(() => storedLength('16434'), /Invalid SCALE/);
});

test('missing candidate decoder leaves compatibility unavailable', () => {
  const input = fixture({ recovered: true }); delete input.candidateRegistry;
  const result = evaluate(input);
  assert.equal(result.payloadBoundsVerified, true);
  assert.equal(result.payloadCompatibilityVerified, false);
});

const operatorCases = [
  ['stale operator candidate', x => { x.operatorEvidence.candidateSha256 = sha(7); }, /stale candidate/],
  ['stale operator snapshot', x => { x.operatorEvidence.blockHash = id(7); }, /stale snapshot/],
  ['operator omits network', x => { x.operatorEvidence.accounts = []; }, /omits a bridge account/],
  ['operator omits quorum peer', x => { x.operatorEvidence.accounts[0].peers.pop(); }, /current quorum/],
  ['operator duplicates peer', x => { x.operatorEvidence.accounts[0].peers[1] = x.operatorEvidence.accounts[0].peers[0]; }, /duplicates/],
  ['operator foreign peer', x => { x.operatorEvidence.accounts[0].peers[0] = id(7); }, /foreign or ineligible/],
  ['operator wrong quorum', x => { x.operatorEvidence.accounts[0].quorum = 2; }, /quorum differs/],
  ['operator client unconfirmed', x => { x.operatorEvidence.accounts[0].compatibleClientConfirmed = false; }, /compatibleClientConfirmed/],
  ['operator provenance omitted', x => { delete x.operatorEvidence.source; }, /provenance missing/],
];
for (const [name, mutate, error] of operatorCases) test(name + ' fails closed', () => {
  const input = fixture(); input.operatorEvidence = confirmed(input); mutate(input); assert.throws(() => evaluate(input), error);
});

test('current peer/multisig mismatch yields a quorum blocker before any attestation', () => {
  const input = fixture(), row = input.report.accounts[0];
  row.currentBridgePeers = row.currentBridgePeers.slice(0, 2);
  row.rawCurrentBridgePeersValue = input.registry.createType('Vec<AccountId>', row.currentBridgePeers).toHex();
  const result = evaluate(input);
  assert.equal(result.blockers[0].reason, 'insufficient-current-peer-quorum');
  input.operatorEvidence = confirmed(input);
  assert.throws(() => evaluate(input), /foreign or ineligible/);
});

function addGlobal(input, extra = []) {
  const prefix = xxhashAsHex('BridgeMultisig', 128) + xxhashAsHex('Calls', 128).slice(2);
  const entries = input.report.accounts.flatMap(row => row.operations.filter(op => op.storedEncodedBytes !== null)
    .map(op => ({ key: op.storedCallKey, hash: op.hash, storageSizeRpcValue: op.storageSizeRpcValue, encodedBytes: op.storedEncodedBytes, callBytes: op.storedCallBytes }))).concat(extra)
    .sort((a, b) => a.key.localeCompare(b.key));
  input.report.globalStoredCallInventory = { prefix, complete: true, entries,
    pages: [{ startKey: null, pageSize: 128, keys: entries.map(entry => entry.key) }], terminalKeys: [] };
  return input;
}

test('absent global proof remains unavailable, not a global bound pass', () => {
  const result = evaluate(fixture());
  assert.equal(result.globalStoredCallInventoryVerified, false);
  assert.equal(result.globalStoredCallCount, null);
  assert.equal(result.globalStoredCallBoundsVerified, null);
  assert(result.unavailableEvidence.includes('complete-global-stored-call-inventory'));
});

test('empty global prefix proves no stored calls, independently of hash-only backlog', () => {
  const result = evaluate(addGlobal(fixture()));
  assert.equal(result.globalStoredCallInventoryVerified, true);
  assert.equal(result.globalStoredCallCount, 0);
  assert.equal(result.globalStoredCallBoundsVerified, true);
  assert.equal(result.payloadBoundsResolved, false);
});

test('global prefix and each pending stored call must agree', () => {
  const result = evaluate(addGlobal(fixture({ stored: true })));
  assert.equal(result.globalStoredCallBoundsVerified, true);
  assert.equal(result.globalStoredCallCount, 1);
  assert.equal(result.payloadCompatibilityVerified, true);
});

test('oversized orphan/nonconfigured-account call cannot be hidden by the pending bridge scan', () => {
  const prefix = xxhashAsHex('BridgeMultisig', 128) + xxhashAsHex('Calls', 128).slice(2);
  const result = evaluate(addGlobal(fixture({ empty: true }), [{ key: prefix + id(77).slice(2), hash: id(77), storageSizeRpcValue: 16437, encodedBytes: '16437', callBytes: 16385 }]));
  assert.equal(result.storedCallsInspected, 0);
  assert.equal(result.globalStoredCallCount, 1);
  assert.equal(result.globalStoredCallBoundsVerified, false);
  assert.equal(result.blockers[0].reason, 'global-stored-payload-exceeds-bound');
});

const globalCases = [
  ['incomplete global prefix', x => { x.report.globalStoredCallInventory.complete = false; }, /inventory incomplete/],
  ['duplicate global key', x => { x.report.globalStoredCallInventory.pages[0].keys.push(x.report.globalStoredCallInventory.pages[0].keys[0]); }, /duplicate or unordered/],
  ['unproved full global page', x => { x.report.globalStoredCallInventory.pages[0].pageSize = 1; }, /bound terminal probe/],
  ['nonempty global terminal', x => { x.report.globalStoredCallInventory.terminalKeys.push('0x00'); }, /terminal probe/],
  ['omitted global entry', x => { x.report.globalStoredCallInventory.entries = []; }, /entries differ/],
  ['wrong global hash', x => { x.report.globalStoredCallInventory.entries[0].hash = id(4); }, /key\/hash differs/],
  ['wrong global derived size', x => { x.report.globalStoredCallInventory.entries[0].callBytes++; }, /payload length differs/],
  ['pending stored entry omitted globally', x => { x.report.globalStoredCallInventory.entries = []; x.report.globalStoredCallInventory.pages[0].keys = []; }, /omitted from global/],
  ['pending absence contradicts global entry', x => { const op = x.report.accounts[0].operations[0]; op.storageSizeRpcValue = null; op.storedEncodedBytes = null; op.storedCallBytes = null; }, /absent but global inventory/],
  ['pending and global sizes disagree', x => { x.report.globalStoredCallInventory.entries[0].storageSizeRpcValue = 55; x.report.globalStoredCallInventory.entries[0].encodedBytes = '55'; x.report.globalStoredCallInventory.entries[0].callBytes = 6; }, /Pending\/global encoded/],
];
for (const [name, mutate, error] of globalCases) test(name + ' fails closed', () => {
  const input = addGlobal(fixture({ stored: true })); mutate(input); assert.throws(() => evaluate(input), error);
});

test('full global page accepts a recorded empty probe at its last key', () => {
  const input = addGlobal(fixture({ stored: true }));
  input.report.globalStoredCallInventory.pages[0].pageSize = 1;
  input.report.globalStoredCallInventory.terminalStartKey = input.report.globalStoredCallInventory.pages[0].keys[0];
  assert.equal(evaluate(input).globalStoredCallBoundsVerified, true);
});

test('numeric raw RPC size 328 cannot masquerade as absent pending storage', () => {
  const input = fixture(); input.report.accounts[0].operations[0].storageSizeRpcValue = 328;
  assert.throws(() => evaluate(input), /Pending encoded size differs from raw RPC/);
});

test('missing raw RPC size cannot be silently normalized to absent', () => {
  const input = fixture(); delete input.report.accounts[0].operations[0].storageSizeRpcValue;
  assert.throws(() => evaluate(input), /Invalid or missing raw storage-size result/);
});

test('global numeric 328 is retained as a 278-byte payload observation', () => {
  const prefix = xxhashAsHex('BridgeMultisig', 128) + xxhashAsHex('Calls', 128).slice(2);
  const input = addGlobal(fixture({ empty: true }), [{ key: prefix + id(77).slice(2), hash: id(77), storageSizeRpcValue: 328, encodedBytes: '328', callBytes: 278 }]);
  assert.equal(evaluate(input).globalStoredCallCount, 1);
  input.report.globalStoredCallInventory.entries[0].encodedBytes = null;
  assert.throws(() => evaluate(input), /Global Calls encoded size differs from raw RPC/);
});

test('enumerated global key cannot have null or missing raw RPC size', () => {
  const input = addGlobal(fixture({ stored: true }));
  input.report.globalStoredCallInventory.entries[0].storageSizeRpcValue = null;
  assert.throws(() => evaluate(input), /enumerated entry has absent raw size/);
  delete input.report.globalStoredCallInventory.entries[0].storageSizeRpcValue;
  assert.throws(() => evaluate(input), /Invalid or missing raw storage-size result/);
});
