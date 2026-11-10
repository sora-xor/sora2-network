'use strict';
// Pure evidence validation: no filesystem, RPC, signing, submission, or deployment.
// The caller must construct both registries from the independently hash-bound
// deployed/candidate metadata. Registry presence is not deployment evidence.
const assert = require('node:assert/strict');
const { blake2AsHex, decodeAddress, xxhashAsHex } = require('@polkadot/util-crypto');
const { normalizeStorageSize } = require('./bridge-rpc-size.cjs');

const hex = (value, label, bytes) => {
  assert(typeof value === 'string' && /^0x(?:[0-9a-f]{2})*$/i.test(value), `${label}: invalid hex`);
  if (bytes !== undefined) assert.equal(value.length, 2 + 2 * bytes, `${label}: wrong length`);
  return value.toLowerCase();
};
const buffer = value => Buffer.from(hex(value, 'bytes').slice(2), 'hex');
const accountHex = value => '0x' + Buffer.from(decodeAddress(value)).toString('hex');
const sameAccount = (a, b) => accountHex(a) === accountHex(b);
const canonical = value => JSON.stringify(value, (_key, item) => item && typeof item === 'object' && !Array.isArray(item)
  ? Object.fromEntries(Object.entries(item).sort(([a], [b]) => a.localeCompare(b))) : item);
const unique = (values, label) => assert.equal(new Set(values).size, values.length, `${label}: duplicates`);
const prefix = (item, pallet = 'BridgeMultisig') => xxhashAsHex(pallet, 128) + xxhashAsHex(item, 128).slice(2);
const networkKey = (network, item) => {
  assert(Number.isInteger(network) && network >= 0 && network <= 0xffffffff, 'Invalid bridge network id');
  const bytes = Buffer.alloc(4); bytes.writeUInt32LE(network);
  return prefix(item, 'EthBridge') + xxhashAsHex(bytes, 64).slice(2) + bytes.toString('hex');
};
const accountKey = account => {
  const bytes = buffer(accountHex(account));
  return prefix('Accounts') + xxhashAsHex(bytes, 64).slice(2) + bytes.toString('hex');
};
const pendingPrefix = account => {
  const bytes = buffer(accountHex(account));
  return prefix('Multisigs') + xxhashAsHex(bytes, 64).slice(2) + bytes.toString('hex');
};
const pendingKey = (account, hash) => pendingPrefix(account) + blake2AsHex(buffer(hash), 128).slice(2) + hex(hash, 'call hash', 32).slice(2);
const quorumFor = count => count - Math.floor((count - 1) / 3);

function typeFor(registry, item, palletName = 'BridgeMultisig') {
  assert(registry, 'Baseline registry is required for raw inventory validation');
  const pallet = registry.metadata.pallets.find(p => p.name.toString() === palletName);
  assert(pallet, `${palletName} metadata missing`);
  const entry = pallet.storage.unwrap().items.find(i => i.name.toString() === item);
  assert(entry && entry.type.isMap !== false, `${item} storage metadata missing`);
  return registry.createLookupType(entry.type.asMap.value);
}

function decodeAll(registry, type, encoded, label) {
  const bytes = buffer(encoded);
  const value = registry.createType(type, bytes);
  assert.equal(Buffer.from(value.toU8a()).toString('hex'), bytes.toString('hex'), `${label}: noncanonical or trailing bytes`);
  return value;
}

function sourceReference(source, label) {
  assert(source && typeof source.kind === 'string' && source.kind.length &&
    typeof source.reference === 'string' && source.reference.length, `${label}: evidence provenance missing`);
  if (source.sha256 !== undefined) assert(/^[0-9a-f]{64}$/i.test(source.sha256), `${label}: invalid provenance digest`);
}

function binding(evidence, candidate, snapshot, label) {
  assert.equal(evidence.candidateSha256, candidate.sha256, `${label}: stale candidate`);
  assert.equal(evidence.blockHash, snapshot.blockHash, `${label}: stale snapshot`);
}

function storedLength(encoded) {
  assert((typeof encoded === 'string' && /^\d+$/.test(encoded)) || Number.isSafeInteger(encoded), 'Invalid stored byte size');
  const total = BigInt(encoded);
  const matches = [[1n, 0n, 64n], [2n, 64n, 16384n], [4n, 16384n, 1073741824n]]
    .map(([p, min, max]) => ({ n: total - 48n - p, min, max })).filter(({ n, min, max }) => n >= min && n < max);
  assert.equal(matches.length, 1, 'Invalid SCALE Calls tuple byte size');
  return Number(matches[0].n);
}

function globalCallsInventory(proof, pageLimit, callLimit) {
  if (!proof) return { available: false, count: null, boundsVerified: null, byHash: new Map(), blockers: [] };
  assert.equal(proof.complete, true, 'Global Calls inventory incomplete');
  assert.equal(hex(proof.prefix, 'Global Calls prefix'), prefix('Calls'), 'Global Calls prefix differs');
  assert(Array.isArray(proof.pages) && proof.pages.length > 0, 'Global Calls pagination missing');
  assert.deepEqual(proof.terminalKeys, [], 'Global Calls terminal probe was not empty');
  const keys = [];
  let previous;
  for (const [index, page] of proof.pages.entries()) {
    assert.equal(page.startKey ?? null, previous ?? null, 'Global Calls pagination cursor differs');
    assert(Number.isSafeInteger(page.pageSize) && page.pageSize > 0 && page.pageSize <= pageLimit,
      'Global Calls invalid page size');
    assert(Array.isArray(page.keys) && page.keys.length <= page.pageSize, 'Global Calls page exceeds bound');
    if (index < proof.pages.length - 1) assert.equal(page.keys.length, page.pageSize, 'Global Calls pagination continues after short page');
    for (const value of page.keys) {
      const key = hex(value, 'Global Calls key', 64);
      assert(key.startsWith(proof.prefix), 'Global Calls key outside prefix');
      assert(previous === undefined || key > previous, 'Global Calls duplicate or unordered key');
      keys.push(key); previous = key;
    }
  }
  if (proof.pages.at(-1).keys.length === proof.pages.at(-1).pageSize)
    assert.equal(proof.terminalStartKey, previous, 'Global Calls requires a bound terminal probe');
  assert(Array.isArray(proof.entries), 'Global Calls entries missing');
  assert.deepEqual(proof.entries.map(entry => hex(entry.key, 'Global Calls entry key')), keys, 'Global Calls entries differ from pagination');
  const byHash = new Map(), blockers = [];
  for (const entry of proof.entries) {
    const hash = hex(entry.hash, 'Global Calls hash', 32);
    assert.equal(entry.key, proof.prefix + hash.slice(2), 'Global Calls key/hash differs');
    assert(!byHash.has(hash), 'Global Calls hash duplicates');
    const rawSize = normalizeStorageSize(entry.storageSizeRpcValue);
    assert.notEqual(rawSize, null, 'Global Calls enumerated entry has absent raw size');
    assert.equal(rawSize, String(entry.encodedBytes), 'Global Calls encoded size differs from raw RPC result');
    const bytes = storedLength(entry.encodedBytes);
    assert.equal(entry.callBytes, bytes, 'Global Calls derived payload length differs');
    byHash.set(hash, { encodedBytes: String(entry.encodedBytes), callBytes: bytes });
    if (bytes > callLimit) blockers.push({ hash, reason: 'global-stored-payload-exceeds-bound', bytes });
  }
  return { available: true, count: byHash.size, boundsVerified: blockers.length === 0, byHash, blockers };
}

function evaluateBridgeReadiness({ report, candidate, snapshot, payloadEvidence = [], operatorEvidence = null,
  baselineRegistry, candidateRegistry = null }) {
  assert(report && candidate && snapshot, 'Readiness inputs missing');
  assert(/^[0-9a-f]{64}$/i.test(candidate.sha256), 'Candidate hash missing');
  hex(snapshot.blockHash, 'snapshot block', 32);
  assert.deepEqual(report.candidateBinding, candidate, 'Inventory candidate binding differs');
  assert.equal(report.blockHash, snapshot.blockHash, 'Inventory snapshot differs');
  assert.equal(report.blockNumber, snapshot.blockNumber, 'Inventory block number differs');
  if (snapshot.sha256 !== undefined) assert.equal(report.snapshotSha256, snapshot.sha256, 'Inventory snapshot file differs');
  assert.equal(report.submittedTransactions, 0, 'Inventory submitted transactions');
  assert.equal(report.privateKeysRead, 0, 'Inventory read private keys');
  assert.equal(report.pendingInventoryComplete, true, 'Incomplete pending inventory');
  assert(Array.isArray(report.accounts) && report.accounts.length > 0, 'Bridge accounts missing');
  const limit = report.limits.storedCallBytes;
  assert.equal(limit, 16384, 'Unexpected candidate call-byte bound');
  const globalCalls = globalCallsInventory(report.globalStoredCallInventory, report.limits.pageSize, limit);
  const accountType = typeFor(baselineRegistry, 'Accounts');
  const pendingType = typeFor(baselineRegistry, 'Multisigs');
  const callType = typeFor(baselineRegistry, 'Calls');
  const networkType = typeFor(baselineRegistry, 'BridgeAccount', 'EthBridge');
  const peersType = typeFor(baselineRegistry, 'Peers', 'EthBridge');
  unique(report.accounts.map(row => accountHex(row.account)), 'Bridge accounts');
  unique(report.accounts.map(row => canonical(row.network)), 'Bridge networks');
  const networks = report.networkInventory;
  assert(networks && networks.complete === true && Number.isSafeInteger(networks.pageSize) && networks.pageSize > 0,
    'Network inventory is incomplete');
  assert.equal(hex(networks.prefix, 'network prefix'), prefix('BridgeAccount', 'EthBridge'), 'Network prefix differs');
  assert(Array.isArray(networks.entries) && networks.entries.length < networks.pageSize, 'Network inventory page truncated');
  unique(networks.entries.map(entry => hex(entry.key, 'network key')), 'Network inventory keys');
  assert.equal(networks.entries.length, report.accounts.length, 'Bridge network omitted from inventory');

  const recovered = new Map();
  for (const entry of payloadEvidence) {
    binding(entry, candidate, snapshot, 'Recovered payload');
    sourceReference(entry.source, 'Recovered payload');
    const key = accountHex(entry.account) + ':' + hex(entry.hash, 'recovered call hash', 32);
    assert(!recovered.has(key), 'Duplicate recovered payload');
    const bytes = buffer(entry.callHex);
    assert.equal(bytes.length, entry.callBytes, 'Recovered payload byte length differs');
    assert.equal(blake2AsHex(bytes, 256), hex(entry.hash, 'recovered call hash', 32), 'Recovered payload hash differs');
    recovered.set(key, entry);
  }

  const operations = [], blockers = [...globalCalls.blockers], accounts = [];
  for (const row of report.accounts) {
    const networkEntry = networks.entries.find(entry => entry.key === row.networkStorageKey);
    assert(networkEntry, 'Account missing from network inventory');
    assert.equal(hex(networkEntry.key, 'network key'), networkKey(row.network, 'BridgeAccount'), 'Network key differs');
    assert.equal(networkEntry.value, row.rawNetworkValue, 'Raw network value differs');
    const rawNetwork = decodeAll(baselineRegistry, networkType, row.rawNetworkValue, 'Raw bridge account');
    assert(sameAccount(rawNetwork.toString(), row.account), 'Network account differs from raw storage');
    assert.equal(row.pendingScanComplete, true, 'Account inventory incomplete');
    assert.equal(row.truncated, false, 'Account inventory truncated');
    assert.equal(hex(row.accountStorageKey, 'account storage key'), accountKey(row.account), 'Account storage key differs');
    const rawAccount = decodeAll(baselineRegistry, accountType, row.rawAccountValue, 'Raw multisig account');
    const members = Array.from(rawAccount.signatories, peer => accountHex(peer.toString()));
    assert(members.length > 0 && members.length <= 100, 'Invalid multisig member count');
    unique(members, 'Raw multisig members');
    assert.deepEqual(members, [...members].sort(), 'Raw multisig members are not sorted');
    const peers = row.peers.map(peer => accountHex(typeof peer === 'string' ? peer : peer.account));
    unique(peers, 'Reported peers');
    assert.deepEqual([...peers].sort(), members, 'Reported peers differ from raw account');
    assert.equal(row.peerCount, members.length, 'Peer count differs');
    const quorum = quorumFor(members.length);
    assert.equal(row.quorum, quorum, 'Quorum differs from runtime formula');
    assert.equal(row.perProposerQuotaAfterUpgrade, Math.max(1, Math.floor(128 / members.length)), 'Proposer quota differs');
    assert.equal(hex(row.currentBridgePeersStorageKey, 'current peers key'), networkKey(row.network, 'Peers'), 'Current bridge peers key differs');
    const currentPeers = Array.from(decodeAll(baselineRegistry, peersType, row.rawCurrentBridgePeersValue, 'Raw bridge peers'), peer => accountHex(peer.toString()));
    unique(currentPeers, 'Current bridge peers');
    assert.deepEqual(currentPeers, row.currentBridgePeers.map(accountHex), 'Current bridge peers differ from raw storage');
    const eligiblePeers = members.filter(peer => currentPeers.includes(peer));
    const proof = row.inventoryProof;
    assert(proof && Array.isArray(proof.pages) && proof.pages.length > 0, 'Pagination evidence missing');
    assert.equal(hex(proof.prefix, 'pending prefix'), pendingPrefix(row.account), 'Pending prefix differs');
    assert.deepEqual(proof.terminalKeys, [], 'Pagination terminal probe was not empty');
    const keys = [];
    let previous;
    for (const [pageIndex, page] of proof.pages.entries()) {
      assert.equal(page.startKey ?? null, previous ?? null, 'Pagination cursor differs');
      assert(Array.isArray(page.keys), 'Pagination keys missing');
      assert(Number.isSafeInteger(page.pageSize) && page.pageSize > 0 && page.pageSize <= report.limits.pageSize,
        'Invalid pagination page size');
      assert(page.keys.length <= page.pageSize, 'Pagination page exceeds requested bound');
      if (pageIndex < proof.pages.length - 1) assert.equal(page.keys.length, page.pageSize, 'Pagination continues after short page');
      for (const keyValue of page.keys) {
        const key = hex(keyValue, 'pending key');
        assert(key.startsWith(proof.prefix), 'Pending key outside account prefix');
        assert(previous === undefined || key > previous, 'Pagination keys duplicate or out of order');
        keys.push(key); previous = key;
      }
    }
    const finalPage = proof.pages.at(-1);
    if (finalPage.keys.length === finalPage.pageSize)
      assert.equal(proof.terminalStartKey, previous, 'Pagination requires a bound terminal probe');
    assert.equal(row.operations.length, row.pendingOperationsObserved, 'Pending operation count differs');
    assert.deepEqual(row.operations.map(op => hex(op.storageKey, 'operation storage key')), keys, 'Raw inventory keys differ from pagination');
    unique(row.operations.map(op => hex(op.hash, 'operation hash', 32)), 'Account operations');
    for (const op of row.operations) {
      const hash = hex(op.hash, 'operation hash', 32);
      assert.equal(hex(op.storageKey, 'operation key'), pendingKey(row.account, hash), 'Operation storage key differs');
      assert.equal(hex(op.storedCallKey, 'stored call key'), prefix('Calls') + hash.slice(2), 'Stored call key differs');
      const raw = decodeAll(baselineRegistry, pendingType, op.rawValue, 'Raw pending operation');
      if (op.when !== undefined) assert.equal(canonical(op.when), canonical(raw.when.toJSON()), 'Operation timepoint differs');
      if (op.depositor !== undefined) assert(sameAccount(op.depositor, raw.depositor.toString()), 'Operation depositor differs');
      if (op.approvals !== undefined) assert.deepEqual(op.approvals.map(accountHex), Array.from(raw.approvals, a => accountHex(a.toString())), 'Operation approvals differ');
      const key = accountHex(row.account) + ':' + hash;
      const entry = recovered.get(key);
      recovered.delete(key);
      let body = entry ? buffer(entry.callHex) : null;
      let payloadSource = entry ? entry.source.kind : null;
      let storedBytes = null;
      assert(Object.hasOwn(op, 'storedEncodedBytes') && Object.hasOwn(op, 'storedCallBytes'), 'Stored-call observation missing');
      assert.equal(normalizeStorageSize(op.storageSizeRpcValue), op.storedEncodedBytes,
        'Pending encoded size differs from raw RPC result');
      if (globalCalls.available) {
        const global = globalCalls.byHash.get(hash);
        if (op.storedEncodedBytes === null) assert(!global, 'Pending call absent but global inventory has its stored payload');
        else {
          assert(global, 'Pending stored call omitted from global inventory');
          assert.equal(String(op.storedEncodedBytes), global.encodedBytes, 'Pending/global encoded byte size differs');
          assert.equal(op.storedCallBytes, global.callBytes, 'Pending/global payload byte size differs');
        }
      }
      if (op.storedEncodedBytes !== null) {
        storedBytes = storedLength(op.storedEncodedBytes);
        assert.equal(storedBytes, op.storedCallBytes, 'Derived stored-call length differs');
        if (op.storedRawValue !== undefined && op.storedRawValue !== null) {
          const stored = decodeAll(baselineRegistry, callType, op.storedRawValue, 'Raw stored call');
          assert.equal(buffer(op.storedRawValue).length, Number(op.storedEncodedBytes), 'Raw stored byte size differs');
          const storedBody = Buffer.from(stored[0].toU8a(true));
          assert.equal(storedBody.length, storedBytes, 'Raw stored payload size differs');
          assert.equal(blake2AsHex(storedBody, 256), hash, 'Stored payload hash differs');
          if (op.storedCallHex !== undefined) assert.equal(hex(op.storedCallHex, 'Stored payload'), '0x' + storedBody.toString('hex'), 'Stored payload projection differs');
          if (body) assert(body.equals(storedBody), 'Recovered and stored payloads differ');
          body = storedBody; payloadSource = 'stored-call';
        }
      } else {
        assert.equal(op.storedCallBytes, null, 'Absent stored call has a length');
        assert(op.storedRawValue === undefined || op.storedRawValue === null, 'Absent stored call has raw bytes');
        assert(op.storedCallHex === undefined || op.storedCallHex === null, 'Absent stored call has a payload');
      }
      if (body && storedBytes !== null) assert.equal(body.length, storedBytes, 'Payload and observed storage size differ');
      const bytes = body ? body.length : storedBytes;
      const withinBound = bytes === null ? null : bytes <= limit;
      let baselineDecodable = null;
      let candidateDecodable = null;
      if (body) {
        try { decodeAll(baselineRegistry, 'Call', '0x' + body.toString('hex'), 'Baseline call'); baselineDecodable = true; }
        catch { baselineDecodable = false; }
      }
      if (body && candidateRegistry) {
        try { decodeAll(candidateRegistry, 'Call', '0x' + body.toString('hex'), 'Candidate call'); candidateDecodable = true; }
        catch { candidateDecodable = false; }
      }
      if (withinBound === false) blockers.push({ account: row.account, hash, reason: 'payload-exceeds-bound', bytes });
      if (baselineDecodable === false) blockers.push({ account: row.account, hash, reason:
        candidateDecodable === false ? 'pre-existing-payload-incompatible-with-baseline-and-candidate' :
          candidateDecodable === true ? 'previously-invalid-payload-now-decodable' : 'pre-existing-payload-incompatible-with-baseline' });
      else if (baselineDecodable === true && candidateDecodable === false)
        blockers.push({ account: row.account, hash, reason: 'candidate-payload-decoding-regression' });
      operations.push({ account: row.account, hash, stored: storedBytes !== null, bytes, payloadSource, withinBound, baselineDecodable, candidateDecodable });
    }
    if (eligiblePeers.length < quorum) blockers.push({ account: row.account, reason: 'insufficient-current-peer-quorum', quorum, eligiblePeers: eligiblePeers.length });
    accounts.push({ account: row.account, network: row.network, peers: members, eligiblePeers, quorum, operations: row.operations.length });
  }
  assert.equal(recovered.size, 0, 'Recovered evidence does not belong to pending inventory');
  assert.equal(report.totalPendingOperationsObserved, operations.length, 'Total pending count differs');

  let operatorReadinessConfirmed = false;
  if (operatorEvidence && operatorEvidence.status === 'confirmed') {
    binding(operatorEvidence, candidate, snapshot, 'Operator evidence');
    sourceReference(operatorEvidence.source, 'Operator evidence');
    assert(Array.isArray(operatorEvidence.accounts), 'Operator accounts missing');
    unique(operatorEvidence.accounts.map(row => accountHex(row.account)), 'Operator evidence accounts');
    assert.equal(operatorEvidence.accounts.length, accounts.length, 'Operator evidence omits a bridge account');
    for (const account of accounts) {
      const row = operatorEvidence.accounts.find(item => sameAccount(item.account, account.account));
      assert(row, 'Operator evidence account missing');
      assert.equal(row.quorum, account.quorum, 'Operator evidence quorum differs');
      const peers = row.peers.map(accountHex);
      unique(peers, 'Operator evidence peers');
      assert(peers.every(peer => account.eligiblePeers.includes(peer)), 'Operator evidence includes a foreign or ineligible peer');
      assert(peers.length >= account.quorum, 'Operator evidence does not cover a current quorum');
      for (const flag of ['compatibleClientConfirmed', 'configurationConfirmed', 'recoveryProcedureConfirmed'])
        assert.equal(row[flag], true, `Operator evidence missing ${flag}`);
    }
    operatorReadinessConfirmed = true;
  } else if (operatorEvidence) {
    assert(['unavailable', 'unconfirmed'].includes(operatorEvidence.status), 'Unrecognized operator evidence status');
  }
  const payloadBoundsResolved = operations.every(op => op.withinBound !== null);
  const payloadBoundsVerified = payloadBoundsResolved && operations.every(op => op.withinBound);
  const payloadCompatibilityVerified = payloadBoundsVerified && operations.every(op => op.baselineDecodable === true && op.candidateDecodable === true);
  return {
    inventoryVerified: true, pendingOperations: operations.length,
    storedCallsInspected: operations.filter(op => op.stored).length,
    storedCallBoundsVerified: operations.filter(op => op.stored).every(op => op.withinBound === true),
    storedCallBoundObservable: operations.some(op => op.stored),
    globalStoredCallInventoryVerified: globalCalls.available,
    globalStoredCallCount: globalCalls.count,
    globalStoredCallBoundsVerified: globalCalls.boundsVerified,
    recoveredPayloads: operations.filter(op => op.payloadSource && op.payloadSource !== 'stored-call').length,
    unresolvedPayloads: operations.filter(op => op.withinBound === null).length,
    preExistingIncompatiblePayloads: operations.filter(op => op.baselineDecodable === false).length,
    newCandidatePayloadRegressions: operations.filter(op => op.baselineDecodable === true && op.candidateDecodable === false).length,
    newlyDecodableLegacyPayloads: operations.filter(op => op.baselineDecodable === false && op.candidateDecodable === true).length,
    payloadBoundsResolved, payloadBoundsVerified, payloadCompatibilityVerified,
    operatorReadinessConfirmed, operatorEvidenceStatus: operatorReadinessConfirmed ? 'confirmed' : 'unavailable',
    unavailableEvidence: [
      ...(!globalCalls.available ? ['complete-global-stored-call-inventory'] : []),
      ...(operations.some(op => op.withinBound === null) ? ['one-or-more-pending-payload-lengths'] : []),
      ...(operations.some(op => op.candidateDecodable === null) ? ['one-or-more-candidate-payload-decodes'] : []),
      ...(!operatorReadinessConfirmed ? ['trusted-current-operator-readiness-evidence'] : []),
    ],
    activationAuthorized: false, blockers, accounts, operations,
  };
}

module.exports = { evaluateBridgeReadiness, accountKey, pendingPrefix, pendingKey, networkKey, quorumFor, storedLength };
