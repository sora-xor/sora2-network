'use strict';
// Public read-only finalized state capture. No keys, signing, or submission.
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync } = require('node:fs');
const { resolve } = require('node:path');
const { createHash } = require('node:crypto');
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { fullOverrideBundle } = require('@sora-substrate/type-definitions');
const { hexToU8a } = require('@polkadot/util');
const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
const endpoint = option('--endpoint', 'wss://mof2.sora.org');
const genesis = '0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5';
const reportedAccount = 'cnRVHUYq6zah5dwdxD7B3CehfyfREb21GXBwyqqwbneyJoTRq';
const sha256 = value => createHash('sha256').update(value).digest('hex');
const report = { status: 'running', capturedAt: new Date().toISOString(), endpoint,
  submittedTransactions: 0, privateKeysRead: 0, remoteRpcMethods: {} };
const readonly = new Set(['chain_getBlock', 'chain_getBlockHash', 'chain_getHeader', 'chain_getFinalizedHead',
  'state_getStorage', 'state_getKeysPaged', 'state_queryStorageAt', 'state_getMetadata', 'state_getRuntimeVersion',
  'state_call', 'system_properties', 'system_chain', 'system_name', 'system_version', 'rpc_methods',
  'state_subscribeRuntimeVersion', 'state_unsubscribeRuntimeVersion']);
let api;
const save = () => writeFileSync(resolve(__dirname, 'snapshot.json'), JSON.stringify(report, null, 2) + '\n');
async function main() {
  const provider = new WsProvider(endpoint);
  const send = provider.send.bind(provider);
  provider.send = (method, ...params) => { assert(readonly.has(method), 'Forbidden public RPC: ' + method);
    report.remoteRpcMethods[method] = (report.remoteRpcMethods[method] || 0) + 1; return send(method, ...params); };
  api = await ApiPromise.create({ provider, noInitWarn: true,
    typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } });
  assert.equal(api.genesisHash.toHex(), genesis);
  const hash = option('--block', (await api.rpc.chain.getFinalizedHead()).toHex());
  const at = await api.at(hash);
  const header = await api.rpc.chain.getHeader(hash);
  const version = (await api.rpc.state.getRuntimeVersion(hash)).toJSON();
  assert(version.specVersion < 134, 'Candidate spec 134 must advance deployed runtime');
  report.genesis = genesis; report.blockHash = hash; report.blockNumber = header.number.toNumber();
  report.header = header.toJSON(); report.runtimeVersion = version;
  const metadata = (await api.rpc.state.getMetadata(hash)).toU8a();
  writeFileSync(resolve(__dirname, 'deployed-metadata.scale'), metadata);
  report.metadata = { file: 'deployed-metadata.scale', bytes: metadata.length, sha256: sha256(metadata) };
  const code = hexToU8a((await api.rpc.state.getStorage('0x3a636f6465', hash)).unwrap().toHex());
  writeFileSync(resolve(__dirname, 'live-runtime.compact.compressed.wasm'), code);
  report.code = { file: 'live-runtime.compact.compressed.wasm', bytes: code.length, sha256: sha256(code) };
  const [session, validators, members, slot] = await Promise.all([at.query.session.currentIndex(),
    at.query.session.validators(), at.query.council.members(), at.query.babe.currentSlot()]);
  report.session = session.toNumber(); report.validatorCount = validators.length;
  report.currentSlot = slot.toString();
  report.feePayerCandidates = [...new Set([reportedAccount, ...members.map(String), ...validators.map(String)])];
  report.accounts = [];
  for (const address of report.feePayerCandidates) {
    const account = await at.query.system.account(address);
    report.accounts.push({ address, nonce: account.nonce.toString(), free: account.data.free.toString() });
  }
  // A bounded scan establishes signer attribution only for matching extrinsics.
  const count = Number(option('--scan-blocks', '20'));
  assert(Number.isSafeInteger(count) && count >= 0 && count <= 20);
  report.spamObservation = { reportedAccount, scannedFinalizedBlocks: count, reports: [],
    interpretation: 'Unsigned extrinsics have no signer. A block author is not necessarily the transaction submitter.' };
  let scanHash = hash;
  for (let index = 0; index < count; index++) {
    const block = await api.rpc.chain.getBlock(scanHash);
    const scanAt = await api.at(scanHash);
    const events = await scanAt.query.system.events();
    for (let txIndex = 0; txIndex < block.block.extrinsics.length; txIndex++) {
      const tx = block.block.extrinsics[txIndex];
      if (!['babe', 'grandpa'].includes(tx.method.section) || !['reportEquivocation', 'reportEquivocationUnsigned'].includes(tx.method.method)) continue;
      const row = { block: block.block.header.number.toNumber(), blockHash: scanHash, index: txIndex,
        section: tx.method.section, method: tx.method.method, signed: tx.isSigned,
        signer: tx.isSigned ? tx.signer.toString() : null, callHex: tx.method.toHex(),
        events: events.filter(record => record.phase.isApplyExtrinsic && record.phase.asApplyExtrinsic.toNumber() === txIndex)
          .map(({ event }) => ({ section: event.section, method: event.method, data: event.data.toJSON() })) };
      row.reportedAccountIsSigner = row.signer === reportedAccount;
      report.spamObservation.reports.push(row);
    }
    scanHash = block.block.header.parentHash.toHex();
  }
  report.inputs = { scriptSha256: sha256(readFileSync(__filename)),
    packageLockSha256: sha256(readFileSync(resolve(__dirname, 'package-lock.json'))) };
  report.status = 'passed';
}
const timeout = setTimeout(() => { report.status = 'timed_out'; save(); process.exit(2); }, 300000);
main().catch(error => { report.status = 'failed'; report.error = { message: error.message, stack: error.stack };
  console.error(error); process.exitCode = 1; }).finally(async () => { clearTimeout(timeout);
  report.finishedAt = new Date().toISOString(); save(); if (api) await api.disconnect().catch(() => {});
  console.log(JSON.stringify({ status: report.status, block: report.blockNumber, specVersion: report.runtimeVersion?.specVersion,
    reports: report.spamObservation?.reports.length, error: report.error?.message })); });
