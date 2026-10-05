'use strict';
// Public read-only preflight and unsigned-call encoding. No signing/submission.
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync } = require('node:fs');
const { resolve } = require('node:path');
const { createHash } = require('node:crypto');
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { fullOverrideBundle } = require('@sora-substrate/type-definitions');
const { blake2AsHex, cryptoWaitReady } = require('@polkadot/util-crypto');
const { hexToU8a, stringToHex } = require('@polkadot/util');
const dir = resolve(__dirname, '..');
const sha256 = data => createHash('sha256').update(data).digest('hex');
const snapshot = JSON.parse(readFileSync(resolve(__dirname, 'snapshot.json')));
const endpoint = 'wss://ws.mof.sora.org';
const methods = {};
const readonly = new Set(['chain_getBlock', 'chain_getBlockHash', 'chain_getHeader', 'chain_getFinalizedHead', 'state_getStorage', 'state_getKeysPaged', 'state_queryStorageAt', 'state_getMetadata', 'state_getRuntimeVersion', 'state_call', 'system_properties', 'system_chain', 'system_name', 'system_version', 'rpc_methods', 'state_subscribeRuntimeVersion', 'state_unsubscribeRuntimeVersion']);
let api;
async function main() {
  await cryptoWaitReady();
  const wasmName = 'framenode-runtime-4.8.11.compact.compressed.wasm';
  const wasm = readFileSync(resolve(dir, wasmName));
  const rehearsal = JSON.parse(readFileSync(resolve(__dirname, 'staking-reward-wasm-rehearsal.json')));
  const compatibility = JSON.parse(readFileSync(resolve(__dirname, 'wasm-compatibility.json')));
  assert.equal(rehearsal.status, 'passed'); assert.equal(compatibility.status, 'passed');
  assert.equal(rehearsal.candidate.sha256, sha256(wasm), 'Rehearsal must cover exact packaged Wasm');
  assert.equal(compatibility.inputs.candidate.sha256, sha256(wasm));
  assert.equal(rehearsal.candidate.runtimeVersion.specVersion, 133);
  const provider = new WsProvider(endpoint);
  const send = provider.send.bind(provider);
  provider.send = (method, ...params) => { assert(readonly.has(method), 'Forbidden public RPC: ' + method); methods[method] = (methods[method] || 0) + 1; return send(method, ...params); };
  api = await ApiPromise.create({ provider, noInitWarn: true, typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } });
  assert.equal(api.genesisHash.toHex(), snapshot.genesis);
  const hash = await api.rpc.chain.getFinalizedHead();
  const at = await api.at(hash);
  const header = await api.rpc.chain.getHeader(hash);
  const version = (await api.rpc.state.getRuntimeVersion(hash)).toJSON();
  assert.equal(version.specVersion, 132, 'Live baseline changed; repeat validation before encoding');
  assert.deepEqual(api.runtimeVersion.toJSON(), version, 'Best and finalized runtime versions differ');
  assert.equal(sha256((await api.rpc.state.getMetadata(hash)).toU8a()), snapshot.metadata.sha256, 'Finalized V14 metadata differs from captured baseline');
  const deployed = hexToU8a((await api.rpc.state.getStorage('0x3a636f6465', hash)).unwrap().toHex());
  assert.equal(sha256(deployed), snapshot.code.sha256, 'Live deployed Wasm differs from tested baseline');
  assert.equal((await api.rpc.state.getStorage(stringToHex('runtime:migrations:val_staking_rewards_published'), hash)).toJSON(), null, 'Publication migration already ran; review live state');
  const active = (await at.query.staking.activeEra()).unwrap().toJSON();
  const current = (await at.query.staking.currentEra()).unwrap().toNumber();
  const depth = at.consts.staking.historyDepth.toNumber();
  assert.equal(depth, 84);
  const latestBudgets = [];
  for (let era = active.index - 1; era >= active.index - 5; era--) {
    const [standard, val] = await Promise.all([at.query.staking.erasValidatorReward(era), at.query.xorFee.valStakingEraReward(era)]);
    latestBudgets.push({ era, standard: standard.isSome ? standard.unwrap().toString() : null, val: val.toString() });
  }
  assert(latestBudgets.some(row => row.standard === '0' && BigInt(row.val) > 0n), 'Reward defect precondition changed; review live state');
  const call = api.tx.system.setCode('0x' + wasm.toString('hex')).method;
  const bytes = call.toU8a();
  const proposalHash = blake2AsHex(bytes);
  writeFileSync(resolve(dir, 'set-code-call.scale'), bytes);
  writeFileSync(resolve(dir, 'set-code-call.hex'), call.toHex() + '\n');
  writeFileSync(resolve(dir, 'preimage-note-call.hex'), api.tx.preimage.notePreimage(call.toHex()).method.toHex() + '\n');
  const report = { preparedAt: new Date().toISOString(), release: '4.8.11',
    candidate: { file: wasmName, bytes: wasm.length, sha256: sha256(wasm), specVersion: 133, transactionVersion: 131 },
    proposal: { method: 'system.setCode', hash: proposalHash, bytes: bytes.length, signed: false },
    preflight: { endpoint, genesis: snapshot.genesis, hash: hash.toHex(), block: header.number.toNumber(), runtimeVersion: version,
      deployedSha256: sha256(deployed), publicationMarkerAbsent: true, active, current, historyDepth: depth, latestBudgets, remoteRpcMethods: methods },
    validation: { exactCandidateWasmRehearsalPassed: true, metadataAndHostInterfaceCompatibilityPassed: true,
      rehearsalPinnedBlockHash: rehearsal.pinnedBlockHash, publishedCompletedEras: rehearsal.migrationBudgets.nonzeroPublishedEras },
    activation: 'The next block after code enactment executes the bounded completed-era publication migration. Future era closure publishes the recorded VAL budget through standard staking storage. Ordinary staking payout calls retain claim protection and pay VAL only.',
    submittedTransactions: 0, privateKeysRead: 0,
    limitations: ['Preparation encodes unsigned calls only; no proposal or code enactment is submitted.', 'Repeat the read-only live preflight before enactment if the baseline or chain state changes.', 'Expired eras remain outside the existing claim window; migration preserves the history depth and existing claim markers.'] };
  writeFileSync(resolve(dir, 'runtime-upgrade-4.8.11-info.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify({ candidateSha256: report.candidate.sha256, proposalHash, proposalBytes: bytes.length, preflightBlock: report.preflight.block, submittedTransactions: 0 }));
}
const timeout = setTimeout(() => { console.error('Read-only package preflight timed out'); process.exit(2); }, 300000);
main().catch(error => { console.error(error); process.exitCode = 1; }).finally(async () => { clearTimeout(timeout); if (api) await api.disconnect().catch(() => {}); });
