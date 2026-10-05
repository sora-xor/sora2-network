'use strict';
// Read-only, bounded inspection of legacy bridge operations at one finalized block.
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync } = require('node:fs');
const { resolve } = require('node:path');
const { createHash } = require('node:crypto');
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { fullOverrideBundle } = require('@sora-substrate/type-definitions');
const sha256 = data => createHash('sha256').update(data).digest('hex');
const methods = {};
const report = { status: 'running', checkedAt: new Date().toISOString(), endpoint: 'wss://mof2.sora.org',
  submittedTransactions: 0, privateKeysRead: 0, limits: { pendingOperationsPerAccount: 128, storedCallBytes: 16384 }, accounts: [] };
const allowed = new Set(['chain_getBlock', 'chain_getBlockHash', 'chain_getHeader', 'chain_getFinalizedHead',
  'state_getStorage', 'state_getKeysPaged', 'state_queryStorageAt', 'state_getMetadata', 'state_getRuntimeVersion',
  'state_call', 'system_properties', 'system_chain', 'system_name', 'system_version', 'rpc_methods',
  'state_subscribeRuntimeVersion', 'state_unsubscribeRuntimeVersion']);
let api;
async function main() {
  const provider = new WsProvider(report.endpoint);
  const send = provider.send.bind(provider);
  provider.send = (method, ...params) => {
    assert(allowed.has(method), 'Forbidden RPC: ' + method);
    methods[method] = (methods[method] || 0) + 1;
    return send(method, ...params);
  };
  api = await ApiPromise.create({ provider, noInitWarn: true,
    typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } });
  const snapshot = JSON.parse(readFileSync(resolve(__dirname, 'snapshot.json')));
  assert.equal(api.genesisHash.toHex(), snapshot.genesis);
  const block = await api.rpc.chain.getFinalizedHead();
  const at = await api.at(block);
  report.blockHash = block.toHex();
  report.blockNumber = (await api.rpc.chain.getHeader(block)).number.toNumber();
  report.runtimeVersion = (await api.rpc.state.getRuntimeVersion(block)).toJSON();
  assert.deepEqual(report.runtimeVersion, snapshot.runtimeVersion, 'Rehearsal baseline changed');
  const networks = await at.query.ethBridge.bridgeAccount.entriesPaged({ args: [], pageSize: 17 });
  assert(networks.length < 17, 'Unexpectedly many bridge networks; inspect separately');
  for (const [key, accountOption] of networks) {
    const id = accountOption.unwrap().toString();
    const pending = await at.query.bridgeMultisig.multisigs.entriesPaged({ args: [id], pageSize: 129 });
    const account = await at.query.bridgeMultisig.accounts(id);
    const peers = account.unwrap().signatories;
    const row = { network: key.args[0].toJSON(), account: id, pendingOperationsObserved: pending.length,
      pendingScanComplete: pending.length < 129, atOrAboveNewOperationCap: pending.length >= 128,
      peerCount: peers.length, peers: [], operations: [] };
    for (const peer of peers) {
      const balance = await at.query.system.account(peer);
      row.peers.push({ account: peer.toString(), nonce: balance.nonce.toString(),
        providers: balance.providers.toNumber(), consumers: balance.consumers.toNumber(), sufficients: balance.sufficients.toNumber(),
        freeXorBaseUnits: balance.data.free.toString(), reservedXorBaseUnits: balance.data.reserved.toString(),
        xorFundingRequiredForAuthenticatedProtocol: false });
    }
    for (const [operationKey] of pending) {
      const hash = operationKey.args[1].toHex();
      const stored = await at.query.bridgeMultisig.calls(hash);
      const size = stored.isSome ? stored.unwrap()[0].length : null;
      row.operations.push({ hash, storedCallBytes: size, exceedsNewCallLimit: size !== null && size > 16384 });
    }
    report.accounts.push(row);
  }
  report.legacyBacklogGrandfathered = true;
  report.newOperationLimitUsesAdditiveTracking = true;
  report.sampledStoredCallsWithinLimit = report.accounts.every(row => row.operations.every(op => !op.exceedsNewCallLimit));
  report.operatorReadinessConfirmed = false;
  report.limitations = [
    'Bridge peers have no XOR funding requirement for authenticated exempt protocol calls. Balances and account references are public observations only.',
    'The legacy backlog is grandfathered and does not consume the additive new-operation limit. The bounded 129-operation sample does not prove the size of every stored call when the scan is incomplete.',
    'Only currently configured EthBridge accounts are inspected; arbitrary standalone BridgeMultisig accounts are outside bridge operator readiness.',
    'An unstored hash-only operation has no observable call length. Its submitting client must respect the new bound.',
    'Refresh before activation; pending operations and balances can change.',
  ];
  report.status = 'passed';
}
const timer = setTimeout(() => { console.error('Bridge readiness timeout'); process.exit(2); }, 300000);
main().catch(error => { report.status = 'failed'; report.error = error.message; process.exitCode = 1; }).finally(async () => {
  clearTimeout(timer);
  report.remoteRpcMethods = methods;
  report.scriptSha256 = sha256(readFileSync(__filename));
  report.snapshotSha256 = sha256(readFileSync(resolve(__dirname, 'snapshot.json')));
  writeFileSync(resolve(__dirname, 'bridge-readiness.json'), JSON.stringify(report, null, 2) + '\n');
  if (api) await api.disconnect().catch(() => {});
  console.log(JSON.stringify({ status: report.status, block: report.blockNumber, networks: report.accounts.length,
    legacyBacklogGrandfathered: report.legacyBacklogGrandfathered, sampledStoredCallsWithinLimit: report.sampledStoredCallsWithinLimit, error: report.error }));
});
