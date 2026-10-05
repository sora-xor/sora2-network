'use strict';
// Exact-Wasm evidence gate, public read-only preflight, unsigned call encoding.
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync } = require('node:fs');
const { resolve } = require('node:path');
const { createHash } = require('node:crypto');
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { fullOverrideBundle } = require('@sora-substrate/type-definitions');
const { blake2AsHex, cryptoWaitReady } = require('@polkadot/util-crypto');
const { hexToU8a } = require('@polkadot/util');
const dir = resolve(__dirname, '..');
const sha256 = value => createHash('sha256').update(value).digest('hex');
const snapshot = JSON.parse(readFileSync(resolve(__dirname, 'snapshot.json')));
const endpoint = 'wss://mof2.sora.org';
const methods = {};
const readonly = new Set(['chain_getBlock', 'chain_getBlockHash', 'chain_getHeader', 'chain_getFinalizedHead',
  'state_getStorage', 'state_getKeysPaged', 'state_queryStorageAt', 'state_getMetadata', 'state_getRuntimeVersion',
  'state_call', 'system_properties', 'system_chain', 'system_name', 'system_version', 'rpc_methods',
  'state_subscribeRuntimeVersion', 'state_unsubscribeRuntimeVersion']);
let api;
async function main() {
  await cryptoWaitReady();
  const wasmName = 'framenode-runtime-4.8.12.compact.compressed.wasm';
  const wasm = readFileSync(resolve(dir, wasmName));
  const rehearsal = JSON.parse(readFileSync(resolve(__dirname, 'equivocation-fee-wasm-rehearsal.json')));
  const compatibility = JSON.parse(readFileSync(resolve(__dirname, 'wasm-compatibility.json')));
  const policy = JSON.parse(readFileSync(resolve(__dirname, 'fee-policy-wasm-rehearsal.json')));
  assert.equal(rehearsal.status, 'passed'); assert.equal(compatibility.status, 'passed');
  assert.equal(policy.status, 'passed');
  assert.equal(policy.candidate.sha256, sha256(wasm), 'Fee policy must cover exact packaged Wasm');
  assert.equal(policy.pinnedBlockHash, snapshot.blockHash);
  assert(Object.values(policy.checks).every(Boolean));
  assert.equal(rehearsal.candidate.sha256, sha256(wasm), 'Rehearsal must cover exact packaged Wasm');
  assert.equal(compatibility.inputs.candidate.sha256, sha256(wasm));
  assert.equal(rehearsal.candidate.runtimeVersion.specVersion, 134);
  assert(Object.values(rehearsal.checks).every(Boolean));
  const provider = new WsProvider(endpoint);
  const send = provider.send.bind(provider);
  provider.send = (method, ...params) => { assert(readonly.has(method), 'Forbidden public RPC: ' + method);
    methods[method] = (methods[method] || 0) + 1; return send(method, ...params); };
  api = await ApiPromise.create({ provider, noInitWarn: true,
    typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } });
  assert.equal(api.genesisHash.toHex(), snapshot.genesis);
  const hash = await api.rpc.chain.getFinalizedHead();
  const header = await api.rpc.chain.getHeader(hash);
  const version = (await api.rpc.state.getRuntimeVersion(hash)).toJSON();
  assert.deepEqual(version, snapshot.runtimeVersion, 'Live baseline changed; repeat validation before encoding');
  assert.deepEqual(api.runtimeVersion.toJSON(), version, 'Best and finalized runtime versions differ');
  assert.equal(sha256((await api.rpc.state.getMetadata(hash)).toU8a()), snapshot.metadata.sha256);
  const deployed = hexToU8a((await api.rpc.state.getStorage('0x3a636f6465', hash)).unwrap().toHex());
  assert.equal(sha256(deployed), snapshot.code.sha256, 'Live deployed Wasm differs from tested baseline');
  const call = api.tx.system.setCode('0x' + wasm.toString('hex')).method;
  const bytes = call.toU8a();
  const proposalHash = blake2AsHex(bytes);
  writeFileSync(resolve(dir, 'set-code-call.scale'), bytes);
  writeFileSync(resolve(dir, 'set-code-call.hex'), call.toHex() + '\n');
  writeFileSync(resolve(dir, 'preimage-note-call.hex'), api.tx.preimage.notePreimage(call.toHex()).method.toHex() + '\n');
  const report = { preparedAt: new Date().toISOString(), release: '4.8.12',
    candidate: { file: wasmName, bytes: wasm.length, sha256: sha256(wasm), specVersion: 134, transactionVersion: 131 },
    proposal: { method: 'system.setCode', hash: proposalHash, bytes: bytes.length, signed: false },
    preflight: { endpoint, genesis: snapshot.genesis, hash: hash.toHex(), block: header.number.toNumber(),
      runtimeVersion: version, deployedSha256: sha256(deployed), remoteRpcMethods: methods },
    validation: { exactCandidateWasmFeeRehearsalPassed: true, exactCandidateWasmFeePolicyPassed: true, metadataAndHostInterfaceCompatibilityPassed: true,
      rehearsalPinnedBlockHash: rehearsal.pinnedBlockHash },
    activation: 'After governance enacts code, the next block initializes spec 134. Consensus reports and maintenance calls require funded signed submission. Successful order cancellation remains free. Bridge peers retain zero-XOR authenticated protocol operations, including accepted failures; invalid and replayed requests are rejected before execution. Unrelated peer calls remain paid. Iroha migration requires the signed claimant to fund its own normal transaction fee; success and failure both pay, and no sponsorship is available. Complete keeper/reporter rollout and bridge compatibility checks before activation.',
    submittedTransactions: 0, privateKeysRead: 0,
    limitations: ['Preparation encodes unsigned review artifacts only; it does not submit a proposal or enact code.',
      'Repeat the read-only live preflight immediately before use when baseline or governance state changes.',
      'Protocol inherents and bounded validator/election/bridge-signature submissions retain their protocol-specific admission controls; this is not a blanket fee on every unsigned protocol message.',
      'Operator readiness is not inferred from the Wasm build. Keepers, inbound relayers, legacy bridge peers, reporters and XOR-funded migration onboarding must satisfy the rollout requirements in the governance preflight.',
      'The candidate contains the prior 4.8.11 staking reward repair; upgrading from deployed spec 132 also executes its bounded one-time reward publication migration.'] };
  writeFileSync(resolve(dir, 'runtime-upgrade-4.8.12-info.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify({ candidateSha256: report.candidate.sha256, proposalHash,
    proposalBytes: bytes.length, preflightBlock: report.preflight.block, submittedTransactions: 0 }));
}
const timeout = setTimeout(() => { console.error('Read-only package preflight timed out'); process.exit(2); }, 300000);
main().catch(error => { console.error(error); process.exitCode = 1; }).finally(async () => {
  clearTimeout(timeout); if (api) await api.disconnect().catch(() => {}); });
