'use strict';
// Read-only, bounded inspection of legacy bridge operations at one finalized block.
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync } = require('node:fs');
const { resolve } = require('node:path');
const { createHash } = require('node:crypto');
const { execFileSync } = require('node:child_process');
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { fullOverrideBundle } = require('@sora-substrate/type-definitions');
const sha256 = data => createHash('sha256').update(data).digest('hex');
const methods = {};
const report = { status: 'running', checkedAt: new Date().toISOString(), endpoint: 'wss://mof2.sora.org',
  submittedTransactions: 0, privateKeysRead: 0, limits: { pendingOperationsPerAccount: 10000, pageSize: 128, concurrentSizeQueries: 8, newOperationLimit: 128, storedCallBytes: 16384 }, accounts: [] };
const allowed = new Set(['chain_getBlock', 'chain_getBlockHash', 'chain_getHeader', 'chain_getFinalizedHead',
  'state_getStorage', 'state_getStorageSize', 'state_getKeysPaged', 'state_queryStorageAt', 'state_getMetadata', 'state_getRuntimeVersion',
  'state_call', 'system_properties', 'system_chain', 'system_name', 'system_version', 'rpc_methods',
  'state_subscribeRuntimeVersion', 'state_unsubscribeRuntimeVersion']);
let api;
async function main() {
  const snapshot = JSON.parse(readFileSync(resolve(__dirname, 'snapshot.json')));
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
  provider.send = (method, ...params) => {
    assert(allowed.has(method), 'Forbidden RPC: ' + method);
    methods[method] = (methods[method] || 0) + 1;
    return send(method, ...params);
  };
  api = await ApiPromise.create({ provider, noInitWarn: true,
    typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } });
  assert.equal(api.genesisHash.toHex(), snapshot.genesis);
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
  for (const [key, accountOption] of networks) {
    const id = accountOption.unwrap().toString();
    const pending = []; let startKey; let pendingScanComplete = false; let pages = 0;
    while (pending.length < report.limits.pendingOperationsPerAccount) {
      const pageSize = Math.min(report.limits.pageSize, report.limits.pendingOperationsPerAccount - pending.length);
      const page = await at.query.bridgeMultisig.multisigs.entriesPaged({ args: [id], pageSize, ...(startKey ? { startKey } : {}) });
      pages++;
      if (startKey && page.length) assert(page[0][0].toHex() > startKey, 'Pagination must advance');
      pending.push(...page);
      if (page.length < pageSize) { pendingScanComplete = true; break; }
      startKey = page.at(-1)[0].toHex();
    }
    if (!pendingScanComplete && startKey) {
      const extra = await at.query.bridgeMultisig.multisigs.entriesPaged({ args: [id], pageSize: 1, startKey });
      pendingScanComplete = extra.length === 0;
    }
    const account = await at.query.bridgeMultisig.accounts(id);
    const peers = account.unwrap().signatories;
    assert(peers.length > 0, 'Configured bridge account must have at least one peer');
    const row = { network: key.args[0].toJSON(), account: id, pendingOperationsObserved: pending.length,
      pendingScanComplete, pages, truncated: !pendingScanComplete, legacyBacklogAtOrAboveSharedLimit: pending.length >= 128,
      peerCount: peers.length, perProposerQuotaAfterUpgrade: Math.max(1, Math.floor(128 / peers.length)),
      peers: [], operations: [] };
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
      const rows = await Promise.all(pending.slice(offset, offset + report.limits.concurrentSizeQueries).map(async ([operationKey]) => {
        const hash = operationKey.args[1].toHex();
        const key = at.query.bridgeMultisig.calls.key(hash);
        const stored = await api.rpc.state.getStorageSize(key, block);
        const encodedBytes = stored.isSome ? stored.unwrap().toString() : null;
        const size = encodedBytes === null ? null : callLength(encodedBytes);
        return { hash, storedEncodedBytes: encodedBytes, storedCallBytes: size, exceedsNewCallLimit: size !== null && size > 16384 };
      }));
      row.operations.push(...rows);
    }
    report.accounts.push(row);
  }
  report.legacyBacklogGrandfathered = true;
  report.newOperationLimitUsesAdditiveTracking = true;
  report.fairPerProposerQuotaConfigured = true;
  report.currentPeerQuorumCancellationConfigured = true;
  report.cancellationCall = 'ethBridge.cancelPendingMultisig(networkId, callHash, timepoint)';
  report.capacityRecoveryScope = 'Current peer quorum releases only the pending multisig operation/capacity; incoming status, canonical hash and unrelated execution tombstones are not retried or reset.';
  report.pendingInventoryComplete = report.accounts.every(row => row.pendingScanComplete);
  report.totalPendingOperationsObserved = report.accounts.reduce((sum,row) => sum + row.pendingOperationsObserved, 0);
  report.callSizeInspection = { method: 'state_getStorageSize', payloadFetched: false, verifiedScaleLayout: '(Vec<u8>, AccountId32, u128)', metadataSha256: report.metadataBinding.metadataSha256 };
  const operations = report.accounts.flatMap(row => row.operations);
  report.storedCallsInspected = operations.filter(op => op.storedCallBytes !== null).length;
  report.hashOnlyOperationsWithUnknownCallLength = operations.length - report.storedCallsInspected;
  report.sampledStoredCallsWithinLimit = report.storedCallsInspected === 0 ? null : operations.every(op => !op.exceedsNewCallLimit);
  report.storedCallBoundObservable = report.storedCallsInspected > 0;
  assert(report.pendingInventoryComplete, 'Full pending inventory gate failed: pagination cap reached');
  report.fullPendingInventoryGatePassed = true;
  report.operatorReadinessConfirmed = false;
  report.limitations = [
    'Bridge peers have no XOR funding requirement for authenticated exempt protocol calls. Balances and account references are public observations only.',
    'The legacy backlog is grandfathered and does not consume the additive new-operation limit. Pagination is capped at 10,000 operations per account; pendingInventoryComplete explicitly records whether that bound truncated any account.',
    'The per-proposer quota is a source-derived preview max(1,floor(128/current_signatories)); this public-state read does not execute the candidate. Exact-Wasm synthetic fixtures cover quota admission and quorum recovery separately.',
    'Only currently configured EthBridge accounts are inspected; arbitrary standalone BridgeMultisig accounts are outside bridge operator readiness.',
    'An unstored hash-only operation has no observable call length. Its submitting client must respect the new bound.',
    'Refresh before activation; pending operations and balances can change.',
  ];
  report.status = 'passed';
}
const timer = setTimeout(() => { report.status = 'timed_out'; report.remoteRpcMethods = methods; writeFileSync(resolve(__dirname, 'bridge-readiness.json'), JSON.stringify(report, null, 2) + '\n'); console.error('Bridge readiness timeout'); process.exit(2); }, 900000);
main().catch(error => { report.status = 'failed'; report.error = error.message; process.exitCode = 1; }).finally(async () => {
  clearTimeout(timer);
  if (api) await api.disconnect().catch(error => { report.status = 'failed'; report.cleanupError = error.message; process.exitCode = 1; });
  report.remoteRpcMethods = methods;
  report.scriptSha256 = sha256(readFileSync(__filename));
  report.snapshotSha256 = sha256(readFileSync(resolve(__dirname, 'snapshot.json')));
  writeFileSync(resolve(__dirname, 'bridge-readiness.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify({ status: report.status, block: report.blockNumber, networks: report.accounts.length,
    legacyBacklogGrandfathered: report.legacyBacklogGrandfathered, sampledStoredCallsWithinLimit: report.sampledStoredCallsWithinLimit, error: report.error }));
});
