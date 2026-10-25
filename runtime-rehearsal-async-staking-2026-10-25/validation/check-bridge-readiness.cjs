'use strict';
// Read-only, bounded inspection of legacy bridge operations at one finalized block.
const assert = require('node:assert/strict');
const { existsSync, readFileSync, writeFileSync } = require('node:fs');
const { resolve } = require('node:path');
const { createHash } = require('node:crypto');
const { execFileSync } = require('node:child_process');
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { Metadata, TypeRegistry } = require('@polkadot/types');
const { fullOverrideBundle } = require('@sora-substrate/type-definitions');
const sha256 = data => createHash('sha256').update(data).digest('hex');
const options = {};
for (let i = 2; i < process.argv.length; i++) {
  const name = process.argv[i];
  assert(['--current', '--payload-evidence', '--operator-evidence'].includes(name), 'Unknown option: ' + name);
  if (name === '--current') options.current = true;
  else { assert(process.argv[i + 1] && !process.argv[i + 1].startsWith('--'), 'Missing option value'); options[name.slice(2)] = resolve(process.argv[++i]); }
}
const outputPath = resolve(__dirname, options.current ? 'bridge-activation-current.json' : 'bridge-readiness.json');
const rawStorage = new Map();
const rawSizes = new Map();
const makeRegistry = bytes => { const registry = new TypeRegistry(); registry.setMetadata(new Metadata(registry, bytes)); return registry; };
const methods = {};
const report = { status: 'running', checkedAt: new Date().toISOString(), endpoint: 'wss://mof2.sora.org',
  submittedTransactions: 0, privateKeysRead: 0, limits: { pendingOperationsPerAccount: 10000, pageSize: 128, concurrentSizeQueries: 8, newOperationLimit: 128, storedCallBytes: 16384 }, accounts: [] };
const allowed = new Set(['chain_getBlock', 'chain_getBlockHash', 'chain_getHeader', 'chain_getFinalizedHead',
  'state_getStorage', 'state_getStorageSize', 'state_getKeysPaged', 'state_queryStorageAt', 'state_getMetadata', 'state_getRuntimeVersion',
  'state_call', 'system_properties', 'system_chain', 'system_name', 'system_version', 'rpc_methods',
  'state_subscribeRuntimeVersion', 'state_unsubscribeRuntimeVersion']);
let api;
async function main() {
  let snapshot = JSON.parse(readFileSync(resolve(__dirname, 'snapshot.json')));
  const candidatePath = resolve(__dirname, '../candidate.compact.compressed.wasm');
  const build = JSON.parse(readFileSync(resolve(__dirname, 'candidate-build-evidence.json')));
  assert.equal(sha256(readFileSync(candidatePath)), build.candidateSha256, 'Candidate must match final build');
  report.candidateBinding = { sha256: build.candidateSha256, specVersion: build.specVersion, transactionVersion: build.transactionVersion };
  const baselinePath = resolve(__dirname, 'live-runtime.compact.compressed.wasm');
  assert.equal(sha256(readFileSync(baselinePath)), snapshot.code.sha256, 'Baseline code must match captured snapshot');
  const extracted = JSON.parse(execFileSync('subwasm', ['metadata', '--format', 'json', baselinePath], {
    env: { ...process.env, NO_COLOR: 'true' }, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 }));
  const metadataBytes = JSON.stringify(extracted, null, 2) + '\n';
  writeFileSync(resolve(__dirname, 'bridge-baseline-metadata.json'), metadataBytes);
  const metadata = extracted.V14;
  report.metadataBinding = { source: 'Runtime metadata executed from exact captured deployed code',
    deployedCodeSha256: snapshot.code.sha256, metadataSha256: sha256(metadataBytes) };
  const provider = new WsProvider(report.endpoint);
  const send = provider.send.bind(provider);
  provider.send = async (method, ...params) => {
    assert(allowed.has(method), 'Forbidden RPC: ' + method);
    methods[method] = (methods[method] || 0) + 1;
    const result = await send(method, ...params);
    if (method === 'state_queryStorageAt') {
      for (const changeSet of result) for (const [key, value] of changeSet.changes) rawStorage.set(key, value);
    }
    if (method === 'state_getStorage') rawStorage.set(params[0][0], result);
    if (method === 'state_getStorageSize') rawSizes.set(params[0][0], result);
    return result;
  };
  api = await ApiPromise.create({ provider, noInitWarn: true,
    typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } });
  assert.equal(api.genesisHash.toHex(), snapshot.genesis);
  if (options.current) {
    const blockHash = (await api.rpc.chain.getFinalizedHead()).toHex();
    const header = await api.rpc.chain.getHeader(blockHash);
    const code = await api.rpc.state.getStorage('0x3a636f6465', blockHash);
    assert.equal(sha256(Buffer.from(code.toHex().slice(2), 'hex')), snapshot.code.sha256, 'Current runtime changed; refresh baseline and full rehearsals');
    snapshot = { status: 'passed', capturedAt: new Date().toISOString(),
      scope: 'Bridge-only finalized snapshot; no fresh staking, governance, or full-chain-state capture is claimed.',
      originalSnapshotSha256: sha256(readFileSync(resolve(__dirname, 'snapshot.json'))),
      endpoint: snapshot.endpoint, genesis: snapshot.genesis, code: snapshot.code, metadata: snapshot.metadata,
      runtimeVersion: snapshot.runtimeVersion, blockHash, blockNumber: header.number.toNumber(), header: header.toJSON(),
      finality: { selectedCanonicalHash: blockHash, source: 'chain_getFinalizedHead' } };
    const snapshotBytes = JSON.stringify(snapshot, null, 2) + '\n';
    writeFileSync(resolve(__dirname, 'bridge-activation-snapshot.json'), snapshotBytes);
    report.snapshotSha256 = sha256(snapshotBytes);
  }
  const block = snapshot.blockHash;
  assert.equal((await api.rpc.chain.getBlockHash(snapshot.blockNumber)).toHex(), block, 'Snapshot block is no longer canonical');
  assert(snapshot.finality && snapshot.finality.selectedCanonicalHash === block, 'Snapshot finality proof missing');
  const at = await api.at(block);
  report.blockHash = block;
  report.blockNumber = (await api.rpc.chain.getHeader(block)).number.toNumber();
  report.runtimeVersion = (await api.rpc.state.getRuntimeVersion(block)).toJSON();
  assert.deepEqual(report.runtimeVersion, snapshot.runtimeVersion, 'Rehearsal baseline changed');
  const networks = await at.query.ethBridge.bridgeAccount.entriesPaged({ args: [], pageSize: 17 });
  assert(networks.length < 17, 'Unexpectedly many bridge networks; inspect separately');
  report.networkInventory = { prefix: at.query.ethBridge.bridgeAccount.keyPrefix(), pageSize: 17, complete: true, entries: networks.map(([key]) => ({ key: key.toHex(), value: rawStorage.get(key.toHex()) })) };
  for (const [key, accountOption] of networks) {
    const id = accountOption.unwrap().toString();
    const pending = []; let startKey; let pendingScanComplete = false; let pages = 0;
    const inventoryProof = { prefix: at.query.bridgeMultisig.multisigs.keyPrefix(id), pages: [], terminalKeys: [] };
    while (pending.length < report.limits.pendingOperationsPerAccount) {
      const pageSize = Math.min(report.limits.pageSize, report.limits.pendingOperationsPerAccount - pending.length);
      const page = await at.query.bridgeMultisig.multisigs.entriesPaged({ args: [id], pageSize, ...(startKey ? { startKey } : {}) });
      pages++;
      if (startKey && page.length) assert(page[0][0].toHex() > startKey, 'Pagination must advance');
      inventoryProof.pages.push({ startKey: startKey || null, pageSize, keys: page.map(([key]) => key.toHex()) });
      pending.push(...page);
      if (page.length < pageSize) { pendingScanComplete = true; break; }
      startKey = page.at(-1)[0].toHex();
    }
    if (!pendingScanComplete && startKey) {
      const extra = await at.query.bridgeMultisig.multisigs.entriesPaged({ args: [id], pageSize: 1, startKey });
      pendingScanComplete = extra.length === 0;
      inventoryProof.terminalStartKey = startKey;
      inventoryProof.terminalKeys = extra.map(([key]) => key.toHex());
    }
    const account = await at.query.bridgeMultisig.accounts(id);
    const peers = account.unwrap().signatories;
    assert(peers.length > 0, 'Configured bridge account must have at least one peer');
    const row = { network: key.args[0].toJSON(), account: id, pendingOperationsObserved: pending.length,
      pendingScanComplete, pages, truncated: !pendingScanComplete, legacyBacklogAtOrAboveSharedLimit: pending.length >= 128,
      peerCount: peers.length, perProposerQuotaAfterUpgrade: Math.max(1, Math.floor(128 / peers.length)),
      quorum: peers.length - Math.floor((peers.length - 1) / 3),
      accountStorageKey: at.query.bridgeMultisig.accounts.key(id),
      rawAccountValue: rawStorage.get(at.query.bridgeMultisig.accounts.key(id)),
      networkStorageKey: key.toHex(), rawNetworkValue: rawStorage.get(key.toHex()),
      inventoryProof, peers: [], operations: [] };
    row.currentBridgePeers = (await at.query.ethBridge.peers(key.args[0])).toJSON();
    row.currentBridgePeersStorageKey = at.query.ethBridge.peers.key(key.args[0]);
    row.rawCurrentBridgePeersValue = rawStorage.get(row.currentBridgePeersStorageKey);
    for (const peer of peers) {
      const balance = await at.query.system.account(peer);
      row.peers.push({ account: peer.toString(), nonce: balance.nonce.toString(),
        providers: balance.providers.toNumber(), consumers: balance.consumers.toNumber(), sufficients: balance.sufficients.toNumber(),
        freeXorBaseUnits: balance.data.free.toString(), reservedXorBaseUnits: balance.data.reserved.toString(),
        xorFundingRequiredForAuthenticatedProtocol: false });
    }
    // Calls stores (Vec<u8>, AccountId32, u128). Validate that exact SCALE
    // layout, then derive payload length from storage size without downloading
    // arbitrary call bytes. One size request per pending hash, at pinned block.
    const types = new Map(metadata.types.types.map(entry => [entry.id, entry.type]));
    const callType = metadata.pallets.find(pallet => pallet.name === 'BridgeMultisig').storage.entries.find(entry => entry.name === 'Calls').ty.Map.value;
    const tuple = types.get(callType).def.tuple;
    assert.equal(tuple.length, 3);
    assert.equal(types.get(types.get(tuple[0]).def.sequence.type).def.primitive, 'u8');
    const accountField = types.get(tuple[1]).def.composite.fields;
    assert.equal(accountField.length, 1);
    const accountArray = types.get(accountField[0].type).def.array;
    assert.equal(accountArray.len, 32); assert.equal(types.get(accountArray.type).def.primitive, 'u8');
    assert.equal(types.get(tuple[2]).def.primitive, 'u128');
    const callLength = encoded => {
      const total = BigInt(encoded);
      const candidates = [[1n,0n,64n],[2n,64n,16384n],[4n,16384n,1073741824n]]
        .map(([prefix,min,max]) => ({ value: total - 48n - prefix, min, max }))
        .filter(({value,min,max}) => value >= min && value < max);
      assert.equal(candidates.length, 1, 'Unsupported or inconsistent stored Calls encoding size');
      return Number(candidates[0].value);
    };
    for (let offset = 0; offset < pending.length; offset += report.limits.concurrentSizeQueries) {
      const rows = await Promise.all(pending.slice(offset, offset + report.limits.concurrentSizeQueries).map(async ([operationKey, operation]) => {
        const hash = operationKey.args[1].toHex();
        const key = at.query.bridgeMultisig.calls.key(hash);
        await api.rpc.state.getStorageSize(key, block);
        const encodedBytes = require('./bridge-rpc-size.cjs').normalizeStorageSize(rawSizes.get(key));
        const size = encodedBytes === null ? null : callLength(encodedBytes);
        const decoded = operation.toJSON();
        const row = { hash, storageKey: operationKey.toHex(), rawValue: rawStorage.get(operationKey.toHex()),
          when: decoded.when, depositor: decoded.depositor, approvals: decoded.approvals,
          storedCallKey: key, storageSizeRpcValue: rawSizes.get(key), storedEncodedBytes: encodedBytes, storedCallBytes: size,
          exceedsNewCallLimit: size !== null && size > 16384 };
        if (size !== null && size <= 16384) {
          const storedValue = await api.rpc.state.getStorage(key, block);
          row.storedRawValue = storedValue.toHex();
          const storedCall = await at.query.bridgeMultisig.calls(hash);
          row.storedCallHex = storedCall.unwrap()[0].toHex();
        }
        return row;
      }));
      row.operations.push(...rows);
    }
    report.accounts.push(row);
  }
  // Calls is global: an orphan value can be referenced by a future hash-only
  // approval, so inspect the entire map rather than only current pending hashes.
  const globalCalls = { prefix: at.query.bridgeMultisig.calls.keyPrefix(), pages: [], terminalKeys: [], complete: false, entries: [] };
  let callCursor;
  while (globalCalls.entries.length < 10000) {
    const pageSize = Math.min(128, 10000 - globalCalls.entries.length);
    const keys = await at.query.bridgeMultisig.calls.keysPaged({ args: [], pageSize, ...(callCursor ? { startKey: callCursor } : {}) });
    globalCalls.pages.push({ startKey: callCursor || null, pageSize, keys: keys.map(key => key.toHex()) });
    for (let offset = 0; offset < keys.length; offset += 8) {
      const entries = await Promise.all(keys.slice(offset, offset + 8).map(async key => {
        await api.rpc.state.getStorageSize(key.toHex(), block);
        const encodedBytes = require('./bridge-rpc-size.cjs').normalizeStorageSize(rawSizes.get(key.toHex()));
        assert(encodedBytes !== null, 'Enumerated stored call is missing at pinned state');
        return { key: key.toHex(), hash: key.args[0].toHex(), storageSizeRpcValue: rawSizes.get(key.toHex()), encodedBytes,
          callBytes: require('./bridge-readiness-policy.cjs').storedLength(encodedBytes) };
      }));
      globalCalls.entries.push(...entries);
    }
    if (keys.length < pageSize) { globalCalls.complete = true; break; }
    callCursor = keys.at(-1).toHex();
  }
  if (!globalCalls.complete && callCursor) {
    const terminal = await at.query.bridgeMultisig.calls.keysPaged({ args: [], pageSize: 1, startKey: callCursor });
    globalCalls.terminalStartKey = callCursor;
    globalCalls.terminalKeys = terminal.map(key => key.toHex());
    globalCalls.complete = terminal.length === 0;
  }
  report.globalStoredCallInventory = globalCalls;
  assert(globalCalls.complete, 'Global stored-call inventory exceeded explicit 10,000-key cap');
  report.legacyBacklogGrandfathered = true;
  report.newOperationLimitUsesAdditiveTracking = true;
  report.fairPerProposerQuotaConfigured = true;
  report.currentPeerQuorumCancellationConfigured = true;
  report.cancellationCall = 'ethBridge.cancelPendingMultisig(networkId, callHash, timepoint)';
  report.capacityRecoveryScope = 'Current peer quorum releases only the pending multisig operation/capacity; incoming status, canonical hash and unrelated execution tombstones are not retried or reset.';
  report.pendingInventoryComplete = report.accounts.every(row => row.pendingScanComplete);
  report.totalPendingOperationsObserved = report.accounts.reduce((sum,row) => sum + row.pendingOperationsObserved, 0);
  report.callSizeInspection = { method: 'state_getStorageSize', payloadFetched: report.accounts.some(row => row.operations.some(op => op.storedRawValue !== undefined)), verifiedScaleLayout: '(Vec<u8>, AccountId32, u128)', metadataSha256: report.metadataBinding.metadataSha256 };
  const operations = report.accounts.flatMap(row => row.operations);
  report.storedCallsInspected = operations.filter(op => op.storedCallBytes !== null).length;
  report.hashOnlyOperationsWithUnknownCallLength = operations.length - report.storedCallsInspected;
  report.sampledStoredCallsWithinLimit = report.storedCallsInspected === 0 ? null : operations.every(op => !op.exceedsNewCallLimit);
  report.storedCallBoundObservable = report.storedCallsInspected > 0;
  assert(report.pendingInventoryComplete, 'Full pending inventory gate failed: pagination cap reached');
  report.fullPendingInventoryGatePassed = true;
  const baselineMetadata = readFileSync(resolve(__dirname, 'deployed-metadata.scale'));
  const candidateMetadata = readFileSync(resolve(__dirname, 'candidate-metadata.scale'));
  report.policyMetadataBinding = { baselineSha256: sha256(baselineMetadata), candidateSha256: sha256(candidateMetadata) };
  const payloadPath = options['payload-evidence'] || resolve(__dirname, 'bridge-payload-evidence.json');
  const payloadInput = existsSync(payloadPath) ? JSON.parse(readFileSync(payloadPath)) : null;
  const pendingIdentities = new Set(report.accounts.flatMap(account => account.operations.map(op => account.account + ':' + op.hash)));
  const payloadEvidence = (payloadInput?.payloads || []).filter(row => pendingIdentities.has(row.account + ':' + row.hash)).map(row => {
    if (row.candidateSha256 !== undefined) assert.equal(row.candidateSha256, report.candidateBinding.sha256, 'Recovered payload has stale candidate binding');
    if (row.blockHash !== undefined) assert.equal(row.blockHash, block, 'Recovered payload has stale pending-state binding');
    return { ...row, candidateSha256: report.candidateBinding.sha256, blockHash: block };
  });
  report.payloadEvidence = payloadEvidence;
  if (payloadInput) report.payloadEvidenceBinding = { path: payloadPath, sha256: sha256(readFileSync(payloadPath)) };
  const operatorPath = options['operator-evidence'];
  const operatorEvidence = operatorPath ? JSON.parse(readFileSync(operatorPath)) : null;
  if (operatorPath) report.operatorEvidenceBinding = { path: operatorPath, sha256: sha256(readFileSync(operatorPath)) };
  report.readiness = require('./bridge-readiness-policy.cjs').evaluateBridgeReadiness({ report,
    candidate: report.candidateBinding, snapshot, payloadEvidence, operatorEvidence,
    baselineRegistry: makeRegistry(baselineMetadata), candidateRegistry: makeRegistry(candidateMetadata) });
  report.operatorReadinessConfirmed = report.readiness.operatorReadinessConfirmed;
  report.activationAuthorized = false;
  report.limitations = [
    'Bridge peers have no XOR funding requirement for authenticated exempt protocol calls. Balances and account references are public observations only.',
    'The legacy backlog is grandfathered and does not consume the additive new-operation limit. Pagination is capped at 10,000 operations per account; pendingInventoryComplete explicitly records whether that bound truncated any account.',
    'The per-proposer quota is a source-derived preview max(1,floor(128/current_signatories)); this public-state read does not execute the candidate. Exact-Wasm synthetic fixtures cover quota admission and quorum recovery separately.',
    'Only currently configured EthBridge accounts are inspected; arbitrary standalone BridgeMultisig accounts are outside bridge operator readiness.',
    'Hash-only operations require a matching recovered preimage to establish payload length and candidate decoding. Execution of unresolved entries is not certified; current peer quorum may cancel them without decoding their payloads. Unknown future supplied bytes remain subject to the runtime bound.',
    'Refresh before activation; pending operations and balances can change.',
  ];
  report.status = 'passed';
}
const timer = setTimeout(() => { report.status = 'timed_out'; report.remoteRpcMethods = methods; writeFileSync(outputPath, JSON.stringify(report, null, 2) + '\n'); console.error('Bridge readiness timeout'); process.exit(2); }, 900000);
main().catch(error => { report.status = 'failed'; report.error = error.message; process.exitCode = 1; }).finally(async () => {
  clearTimeout(timer);
  if (api) await api.disconnect().catch(error => { report.status = 'failed'; report.cleanupError = error.message; process.exitCode = 1; });
  report.remoteRpcMethods = methods;
  report.scriptSha256 = sha256(readFileSync(__filename));
  report.snapshotSha256 ||= sha256(readFileSync(resolve(__dirname, 'snapshot.json')));
  writeFileSync(outputPath, JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify({ status: report.status, block: report.blockNumber, networks: report.accounts.length,
    legacyBacklogGrandfathered: report.legacyBacklogGrandfathered, sampledStoredCallsWithinLimit: report.sampledStoredCallsWithinLimit, error: report.error }));
});
