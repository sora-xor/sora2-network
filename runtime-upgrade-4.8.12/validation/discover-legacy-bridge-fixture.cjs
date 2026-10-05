'use strict';
// Bounded finalized public history discovery; no transaction submission.
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync } = require('node:fs');
const { createHash } = require('node:crypto');
const { resolve } = require('node:path');
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { fullOverrideBundle } = require('@sora-substrate/type-definitions');
const output = resolve(process.argv[2] || resolve(__dirname, 'legacy-bridge-public-fixture.json'));
const endpoint = 'wss://mof2.sora.org';
const report = { status: 'running', endpoint, startedAt: new Date().toISOString(),
  maximumFinalizedBlocks: 256, scannedBlocks: 0, submittedTransactions: 0,
  privateKeysRead: 0, remoteRpcMethods: {}, candidates: [] };
const save = () => writeFileSync(output, JSON.stringify(report, null, 2) + '\n');
let api;
async function main() {
  const provider = new WsProvider(endpoint);
  const send = provider.send.bind(provider);
  const allowed = new Set(['rpc_methods', 'state_getMetadata', 'state_getRuntimeVersion',
    'state_subscribeRuntimeVersion', 'state_unsubscribeRuntimeVersion', 'chain_getBlockHash',
    'chain_getHeader', 'chain_getFinalizedHead', 'chain_getBlock', 'state_getStorage',
    'state_queryStorageAt', 'state_call', 'system_chain', 'system_properties', 'system_name', 'system_version']);
  provider.send = (method, ...args) => { assert(allowed.has(method), 'Forbidden public RPC: ' + method);
    report.remoteRpcMethods[method] = (report.remoteRpcMethods[method] || 0) + 1; return send(method, ...args); };
  api = await ApiPromise.create({ provider,
    typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } });
  const finalized = await api.rpc.chain.getFinalizedHead();
  const header = await api.rpc.chain.getHeader(finalized);
  const latest = header.number.toNumber();
  report.finalizedHead = finalized.toHex(); report.finalizedNumber = latest;
  report.earliestPossibleBlock = Math.max(0, latest - 255); save();
  for (let offset = 0; offset < 256 && report.candidates.length < 16; offset += 8) {
    const numbers = Array.from({ length: Math.min(8, 256 - offset) }, (_, i) => latest - offset - i).filter(n => n >= 0);
    const blocks = await Promise.all(numbers.map(async number => {
      const hash = await api.rpc.chain.getBlockHash(number);
      return { number, hash: hash.toHex(), block: (await api.rpc.chain.getBlock(hash)).block };
    }));
    for (const { number, hash, block } of blocks) {
      report.scannedBlocks++;
      block.extrinsics.forEach((tx, index) => {
        if (!tx.isSigned || !['bridgeMultisig', 'ethBridge'].includes(tx.method.section)) return;
        if (tx.method.section === 'ethBridge' && !['approveRequest', 'registerIncomingRequest',
            'finalizeIncomingRequest', 'importIncomingRequest', 'abortRequest'].includes(tx.method.method)) return;
        if (report.candidates.length >= 16) return;
        report.candidates.push({ blockNumber: number, blockHash: hash, parentHash: block.header.parentHash.toHex(),
          extrinsicIndex: index, section: tx.method.section, method: tx.method.method,
          signer: tx.signer.toString(), nonce: tx.nonce.toString(), callHex: tx.method.toHex(),
          extrinsicHex: tx.toHex(), args: tx.method.args.map(arg => arg.toJSON()) });
      });
    }
    save(); console.log(JSON.stringify({ scannedBlocks: report.scannedBlocks, candidates: report.candidates.length }));
  }
  for (const candidate of report.candidates) {
    const at = await api.at(candidate.parentHash);
    const account = await at.query.system.account(candidate.signer);
    candidate.publicParentAccount = account.toJSON();
    candidate.publicParentFreeXor = account.data.free.toString();
    candidate.parentRuntimeVersion = (await api.rpc.state.getRuntimeVersion(candidate.parentHash)).toJSON();
  }
  report.scriptSha256 = createHash('sha256').update(readFileSync(__filename)).digest('hex');
  report.status = 'passed'; report.foundProtocolCandidates = report.candidates.length > 0;
}
main().catch(error => { report.status = 'failed'; report.error = error.message; process.exitCode = 1; })
  .finally(async () => { report.finishedAt = new Date().toISOString(); save(); if (api) await api.disconnect(); });
