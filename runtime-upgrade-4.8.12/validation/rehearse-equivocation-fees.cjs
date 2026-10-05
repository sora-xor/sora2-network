 'use strict';
// Exact candidate Wasm, pinned public state, local dispatch only; no real keys.
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync, existsSync } = require('node:fs');
const { resolve } = require('node:path');
const { createHash } = require('node:crypto');
const { gunzipSync, gzipSync } = require('node:zlib');
const { StorageKey, GenericExtrinsic } = require('@polkadot/types');
const { hexToU8a, u8aToHex, compactAddLength } = require('@polkadot/util');
const { blake2AsHex, cryptoWaitReady } = require('@polkadot/util-crypto');
const { setup, BuildBlockMode, Block, destroyWorker } = require('@acala-network/chopsticks-core');
const { fullOverrideBundle } = require('@sora-substrate/type-definitions');
const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
const opts = {
  endpoint: option('--endpoint', 'wss://mof2.sora.org'),
  wasm: resolve(option('--wasm', resolve(__dirname, '../framenode-runtime-4.8.12.compact.compressed.wasm'))),
  snapshot: resolve(option('--snapshot', resolve(__dirname, 'snapshot.json'))),
  cache: resolve(option('--cache', resolve(__dirname, '.public-read-cache.json.gz'))),
  output: resolve(option('--output', resolve(__dirname, 'equivocation-fee-wasm-rehearsal.json'))),
};
const snapshot = JSON.parse(readFileSync(opts.snapshot));
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const fakeSignature = new Uint8Array(64).fill(0xcd); fakeSignature.set([0xde, 0xad, 0xbe, 0xef]);
const report = { status: 'running', startedAt: new Date().toISOString(), localOnly: true,
  endpoint: opts.endpoint, pinnedBlockHash: snapshot.blockHash, pinnedBlockNumber: snapshot.blockNumber,
  executor: '@acala-network/chopsticks-core@1.5.1', mockSignatureHost: true, allowUnresolvedImports: true,
  offchainWorker: false, submittedTransactions: 0, privateKeysRead: 0, realSignatures: 0, remoteRpcMethods: {},
  unsignedChecks: [], signedChecks: [], localOnlySyntheticEvidence: true,
  constraints: [
    'Only enumerated read-only public RPC supplies block-hash-pinned state.',
    'Only :code is replaced locally. Upgrade hooks and report fees execute the exact candidate Wasm.',
    'Synthetic headers, reports, and extrinsics use the mocked signature host. Real cryptographic verification is covered by native tests; consensus block validity and governance enactment are outside this rehearsal.',
    'Every transaction branch starts from the same initialized local state; slashing/offence changes remain inside its local branch.',
  ] };
let chain, cache;
const save = () => writeFileSync(opts.output, JSON.stringify(report, null, 2) + '\n');
const progress = value => { report.stage = value; save(); console.log(new Date().toISOString(), value); };
function storageKey(meta, pallet, name, params = []) {
  return new StorageKey(meta.registry, [meta.query[pallet][name], params]);
}
async function rawQuery(block, pallet, name, params = []) {
  const meta = await block.meta;
  const key = storageKey(meta, pallet, name, params);
  const raw = await block.get(key.toHex());
  if (meta.query[pallet][name].meta.modifier.isOptional) return meta.registry.createType('Option<' + key.outputType + '>', hexToU8a(raw ? '0x01' + raw.slice(2) : '0x00'));
  return meta.registry.createType(key.outputType, raw ? hexToU8a(raw) : undefined);
}
function installReadCache() {
  cache = existsSync(opts.cache) ? JSON.parse(gunzipSync(readFileSync(opts.cache)).toString()) : { format: 1, values: {}, keyRanges: {} };
  assert.equal(cache.format, 1);
  report.readCache = { path: opts.cache, content: 'Immutable public storage scoped by block hash', existingValues: Object.keys(cache.values).length, storageHits: 0, keyPageHits: 0, fetchedValues: 0, fetchedKeyPages: 0 };
  const scoped = (hash, key) => hash + ':' + key;
  const getStorage = chain.api.getStorage.bind(chain.api);
  const getKeys = chain.api.getKeysPaged.bind(chain.api);
  const batch = async (keys, hash) => {
    if (!keys.length) return;
    try {
      const result = await chain.api.send('state_queryStorageAt', [keys, hash], true);
      const changes = new Map(result[0]?.changes || []);
      for (const key of keys) { assert(changes.has(key)); cache.values[scoped(hash, key)] = changes.get(key); report.readCache.fetchedValues++; }
    } catch (error) {
      if (keys.length < 2 || !(error.code === -32008 || /Response is too big|Exceeded max limit/.test(error.message))) throw error;
      const mid = Math.floor(keys.length / 2); await batch(keys.slice(0, mid), hash); await batch(keys.slice(mid), hash);
    }
  };
  chain.api.getStorage = async (key, hash) => {
    if (!hash) return getStorage(key, hash);
    const lookup = scoped(hash, key);
    if (Object.hasOwn(cache.values, lookup)) { report.readCache.storageHits++; return cache.values[lookup]; }
    const value = await getStorage(key, hash); cache.values[lookup] = value ?? null; report.readCache.fetchedValues++; return value;
  };
  chain.api.getKeysPaged = async (prefix, count, start, hash) => {
    if (!hash || prefix.startsWith('0x3a6368696c645f73746f726167653a')) return getKeys(prefix, count, start, hash);
    const ranges = cache.keyRanges[scoped(hash, prefix)] ||= [];
    const normalized = start || prefix;
    for (const range of ranges) {
      if (normalized < range.start || (!range.complete && normalized > range.end)) continue;
      const remaining = range.keys.filter(key => key > normalized);
      if (remaining.length >= count || range.complete) { report.readCache.keyPageHits++; return remaining.slice(0, count); }
    }
    const fetchCount = prefix.length < 66 ? count : Math.max(1000, count);
    const keys = await getKeys(prefix, fetchCount, normalized, hash);
    const missing = keys.filter(key => !Object.hasOwn(cache.values, scoped(hash, key)));
    for (let i = 0; i < missing.length; i += 256) await batch(missing.slice(i, i + 256), hash);
    ranges.push({ start: normalized, end: keys.at(-1) || normalized, complete: keys.length < fetchCount, keys }); report.readCache.fetchedKeyPages++;
    return keys.slice(0, count);
  };
}
function secondaryIndex(epoch, slot) {
  const encoded = Buffer.alloc(8); encoded.writeBigUInt64LE(BigInt(slot));
  return Number(BigInt(blake2AsHex(Buffer.concat([Buffer.from(epoch.randomness.slice(2), 'hex'), encoded]), 256)) % BigInt(epoch.authorities.length));
}
async function initializeUpgrade(parent) {
  const registry = await parent.registry;
  registry.register({ RehearsalEpoch: { epochIndex: 'u64', startSlot: 'u64', duration: 'u64', authorities: 'Vec<(AuthorityId,u64)>', randomness: 'H256', config: 'BabeEpochConfiguration' } });
  const epochRaw = await parent.call('BabeApi_current_epoch', []);
  const epoch = registry.createType('RehearsalEpoch', epochRaw.result).toJSON();
  const slot = (await rawQuery(parent, 'babe', 'currentSlot')).toNumber() + 1;
  assert(slot < epoch.startSlot + epoch.duration, 'Select a non-boundary snapshot to isolate migration');
  const predigest = registry.createType('RawBabePreDigest', { SecondaryPlain: { authorityIndex: secondaryIndex(epoch, slot), slotNumber: slot } });
  const digest = registry.createType('DigestItem', { PreRuntime: ['BABE', compactAddLength(predigest.toU8a())] });
  const header = registry.createType('Header', { parentHash: parent.hash, number: parent.number + 1, stateRoot: '0x' + '00'.repeat(32), extrinsicsRoot: '0x' + '00'.repeat(32), digest: { logs: [digest] } });
  const block = new Block(chain, parent.number + 1, header.hash.toHex(), parent, { header, extrinsics: [], storage: parent.storage });
  progress('Executing exact candidate Core_initialize_block and on-runtime-upgrade hooks');
  const response = await block.call('Core_initialize_block', [header.toHex()]);
  block.pushStorageLayer().setAll(response.storageDiff);
  report.initialize = { header: header.toJSON(), slot, result: response.result, storageDiffEntries: response.storageDiff.length, runtimeLogs: response.runtimeLogs };
  assert.equal((await rawQuery(block, 'system', 'lastRuntimeUpgrade')).unwrap().specVersion.toNumber(), 134);
  const meta = await block.meta;
  const protectedPrefixes = ['claimedRewards', 'ledger'].map(name => meta.query.staking[name].keyPrefix().toHex());
  const protectedWrites = response.storageDiff.filter(([key]) => protectedPrefixes.some(prefix => key.startsWith(prefix)));
  assert.deepEqual(protectedWrites, [], 'Upgrade hooks must preserve all staking claim and ledger entries');
  assert.equal((await rawQuery(block, 'balances', 'totalIssuance')).toString(),
    (await rawQuery(parent, 'balances', 'totalIssuance')).toString(), 'Upgrade hooks must preserve native XOR issuance');
  report.initialize.stakingClaimAndLedgerWrites = protectedWrites.length;
  report.initialize.nativeXorIssuancePreserved = true;
  const timestamp = new GenericExtrinsic(registry, meta.tx.timestamp.set(slot * 6000)).toHex();
  const applied = await block.call('BlockBuilder_apply_extrinsic', [timestamp]);
  block.pushStorageLayer().setAll(applied.storageDiff);
  const outcome = registry.createType('ApplyExtrinsicResult', applied.result);
  assert(outcome.isOk && outcome.asOk.isOk);
  report.timestamp = { value: slot * 6000, outcome: outcome.toJSON() };
  return block;
}
async function reportFixtures(block) {
  const registry = await block.registry;
  const meta = await block.meta;
  const epochResponse = await block.call('BabeApi_current_epoch', []);
  const epoch = registry.createType('RehearsalEpoch', epochResponse.result);
  const slot = (await rawQuery(block, 'babe', 'currentSlot')).toString();
  const babeAuthority = epoch.authorities[0][0].toHex();
  const babeOwnership = await block.call('BabeApi_generate_key_ownership_proof', [u8aToHex(registry.createType('u64', slot).toU8a()), babeAuthority]);
  const opaqueBabe = registry.createType('Option<Bytes>', hexToU8a(babeOwnership.result));
  assert(opaqueBabe.isSome, 'Current BABE key ownership must exist');
  const membershipType = meta.tx.babe.reportEquivocation.meta.args[1].type.toString();
  const babeMembership = registry.createType(membershipType, opaqueBabe.unwrap().toU8a(true));
  const predigest = registry.createType('RawBabePreDigest', { SecondaryPlain: { authorityIndex: 0, slotNumber: slot } });
  const makeHeader = marker => registry.createType('Header', { parentHash: block.hash, number: block.number,
    stateRoot: '0x' + marker.repeat(32), extrinsicsRoot: '0x' + '00'.repeat(32), digest: { logs: [
      registry.createType('DigestItem', { PreRuntime: ['BABE', compactAddLength(predigest.toU8a())] }),
      registry.createType('DigestItem', { Seal: ['BABE', compactAddLength(fakeSignature)] }),
    ] } });
  const babe = { offender: babeAuthority, slot, firstHeader: makeHeader('01'), secondHeader: makeHeader('02') };
  const authorities = await rawQuery(block, 'grandpa', 'authorities');
  const grandpaAuthority = authorities[0][0].toHex();
  const setId = (await rawQuery(block, 'grandpa', 'currentSetId')).toString();
  const grandpaOwnership = await block.call('GrandpaApi_generate_key_ownership_proof', [u8aToHex(registry.createType('u64', setId).toU8a()), grandpaAuthority]);
  const opaqueGrandpa = registry.createType('Option<Bytes>', hexToU8a(grandpaOwnership.result));
  assert(opaqueGrandpa.isSome, 'Current GRANDPA key ownership must exist');
  const grandpaMembership = registry.createType(membershipType, opaqueGrandpa.unwrap().toU8a(true));
  const grandpa = { setId, equivocation: { Prevote: { roundNumber: snapshot.blockNumber,
    identity: grandpaAuthority,
    first: [{ targetHash: '0x' + '01'.repeat(32), targetNumber: block.number }, u8aToHex(fakeSignature)],
    second: [{ targetHash: '0x' + '02'.repeat(32), targetNumber: block.number }, u8aToHex(fakeSignature)],
  } } };
  report.fixtures = { babe: { offender: babeAuthority, slot, membership: babeMembership.toJSON() },
    grandpa: { offender: grandpaAuthority, setId, membership: grandpaMembership.toJSON() }, syntheticSignatures: true };
  const fixtures = [
    ['babe.reportEquivocation', meta.tx.babe.reportEquivocation(babe, babeMembership)],
    ['babe.reportEquivocationUnsigned', meta.tx.babe.reportEquivocationUnsigned(babe, babeMembership)],
    ['grandpa.reportEquivocation', meta.tx.grandpa.reportEquivocation(grandpa, grandpaMembership)],
    ['grandpa.reportEquivocationUnsigned', meta.tx.grandpa.reportEquivocationUnsigned(grandpa, grandpaMembership)],
  ];
  report.automaticUnsignedSubmission = {};
  for (const [runtimeApi, call, membership] of [
    ['BabeApi_submit_report_equivocation_unsigned_extrinsic', fixtures[0][1], babeMembership],
    ['GrandpaApi_submit_report_equivocation_unsigned_extrinsic', fixtures[2][1], grandpaMembership],
  ]) {
    const response = await block.call(runtimeApi, [call.args[0].toHex(),
      u8aToHex(registry.createType('Bytes', membership.toHex()).toU8a())]);
    const result = registry.createType('Option<Null>', hexToU8a(response.result));
    assert(result.isNone, runtimeApi + ' must return None without submitting an unsigned report');
    report.automaticUnsignedSubmission[runtimeApi] = { result: response.result,
      storageDiffEntries: response.storageDiff.length, returnsNone: true };
  }
  return fixtures;
}
function signedExtrinsic(registry, call, block, account) {
  const tx = new GenericExtrinsic(registry, call);
  tx.signFake(report.signer.address, { blockHash: block.hash, genesisHash: snapshot.genesis,
    runtimeVersion: report.candidate.runtimeVersion, nonce: account.nonce });
  tx.signature.set(fakeSignature);
  return tx;
}
async function chooseSigner(block, fixture) {
  const registry = await block.registry;
  report.signer = { address: snapshot.feePayerCandidates[0] };
  const account = await rawQuery(block, 'system', 'account', [report.signer.address]);
  const tx = signedExtrinsic(registry, fixture, block, account);
  const feeResponse = await block.call('TransactionPaymentApi_query_info', [tx.toHex(), u8aToHex(registry.createType('u32', tx.encodedLength).toU8a())]);
  const info = registry.createType('RuntimeDispatchInfo', feeResponse.result);
  assert(info.partialFee.toBigInt() > 0n);
  const minimum = info.partialFee.toBigInt() * 2n + 10n ** 18n;
  report.signerSelection = { estimatedReportFee: info.partialFee.toString(), minimumFree: minimum.toString() }; save();
  for (const address of snapshot.feePayerCandidates) {
    const candidate = await rawQuery(block, 'system', 'account', [address]);
    if (candidate.data.free.toBigInt() > minimum) {
      report.signer = { address, free: candidate.data.free.toString(), minimumFree: minimum.toString(),
        selectedFromPublicAccounts: true, storageOverrides: [] };
      return;
    }
  }
  throw new Error('Public fee payer candidates lack funding for the report fee');
}
async function unsignedCheck(block, label, call) {
  const registry = await block.registry;
  const tx = new GenericExtrinsic(registry, call);
  const row = { method: label, callHex: call.toHex(), signed: tx.isSigned, sources: {} };
  for (const source of ['External', 'Local', 'InBlock']) {
    const response = await block.call('TaggedTransactionQueue_validate_transaction', [
      registry.createType('TransactionSource', source).toHex(), tx.toHex(), block.hash,
    ]);
    const result = registry.createType('TransactionValidity', response.result);
    row.sources[source] = result.toJSON();
    assert(result.isErr && result.asErr.isInvalid && result.asErr.asInvalid.isCall,
      label + ' unsigned ' + source + ' must be InvalidTransaction::Call');
  }
  const response = await block.call('BlockBuilder_apply_extrinsic', [tx.toHex()]);
  const applied = registry.createType('ApplyExtrinsicResult', response.result);
  row.apply = applied.toJSON(); row.storageDiffEntries = response.storageDiff.length;
  assert(applied.isErr && applied.asErr.isInvalid && applied.asErr.asInvalid.isCall,
    label + ' direct unsigned block application must reject Call');
  row.passed = true; report.unsignedChecks.push(row); save();
}
async function signedCheck(block, label, call, expectSuccess = true) {
  const registry = await block.registry;
  const before = await rawQuery(block, 'system', 'account', [report.signer.address]);
  const tx = signedExtrinsic(registry, call, block, before);
  const quoteResponse = await block.call('TransactionPaymentApi_query_info', [tx.toHex(), u8aToHex(registry.createType('u32', tx.encodedLength).toU8a())]);
  const quote = registry.createType('RuntimeDispatchInfo', quoteResponse.result);
  assert(quote.partialFee.toBigInt() > 0n, label + ' signed fee query must be positive');
  const response = await block.call('BlockBuilder_apply_extrinsic', [tx.toHex()]);
  const applied = registry.createType('ApplyExtrinsicResult', response.result);
  assert(applied.isOk, label + ' funded signed extrinsic must pass transaction validity');
  const shadow = new Block(chain, block.number, block.hash, block, { header: await block.header,
    extrinsics: [], storage: block.storage, storageDiff: Object.fromEntries(response.storageDiff) });
  const after = await rawQuery(shadow, 'system', 'account', [report.signer.address]);
  const previousEvents = await rawQuery(block, 'system', 'events');
  const events = (await rawQuery(shadow, 'system', 'events')).slice(previousEvents.length)
    .map(({ event }) => ({ section: event.section, method: event.method, data: event.data.toJSON() }));
  const charged = before.data.free.toBigInt() - after.data.free.toBigInt();
  const row = { method: label, expectSuccess, callHex: call.toHex(), signed: tx.isSigned,
    outcome: applied.toJSON(), estimatedFee: quote.partialFee.toString(), xorCharged: charged.toString(),
    signerBefore: before.toJSON(), signerAfter: after.toJSON(), events, storageDiffEntries: response.storageDiff.length };
  report.signedChecks.push(row); save();
  assert.equal(applied.asOk.isOk, expectSuccess, label + ' dispatch outcome');
  assert(charged > 0n, label + ' must retain a positive XOR transaction fee');
  assert.equal(after.nonce.toBigInt(), before.nonce.toBigInt() + 1n);
  row.passed = true; save(); return shadow;
}
async function main() {
  assert.equal(snapshot.status, 'passed'); assert(existsSync(opts.wasm));
  const manifest = JSON.parse(readFileSync(resolve(__dirname, 'package.json')));
  report.dependencies = { manifest, installed: {}, lockSha256: sha256(readFileSync(resolve(__dirname, 'package-lock.json'))) };
  for (const [name, version] of Object.entries(manifest.dependencies)) {
    const installed = JSON.parse(readFileSync(resolve(__dirname, 'node_modules', name, 'package.json'))).version;
    assert.equal(installed, version); report.dependencies.installed[name] = installed;
  }
  await cryptoWaitReady(); progress('Loading pinned deployed public state');
  chain = await setup({ endpoint: opts.endpoint, block: snapshot.blockHash,
    registeredTypes: { typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } },
    buildBlockMode: BuildBlockMode.Manual, mockSignatureHost: true, allowUnresolvedImports: true,
    offchainWorker: false, processQueuedMessages: false, saveBlocks: false, runtimeLogLevel: 3, rpcTimeout: 60000 });
  const readonly = new Set(['chain_getBlock', 'chain_getBlockHash', 'chain_getHeader', 'chain_getFinalizedHead',
    'state_getStorage', 'state_getKeysPaged', 'state_queryStorageAt', 'state_getMetadata', 'state_getRuntimeVersion',
    'state_call', 'system_properties', 'system_chain', 'system_name', 'system_version', 'rpc_methods']);
  const send = chain.api.send.bind(chain.api);
  chain.api.send = (method, ...params) => { assert(readonly.has(method), 'Forbidden public RPC: ' + method);
    report.remoteRpcMethods[method] = (report.remoteRpcMethods[method] || 0) + 1; return send(method, ...params); };
  installReadCache();
  const deployed = chain.head;
  assert.equal(deployed.number, snapshot.blockNumber);
  assert.equal((await rawQuery(deployed, 'system', 'blockHash', [0])).toHex(), snapshot.genesis);
  report.deployedVersion = await deployed.runtimeVersion;
  assert.equal(report.deployedVersion.specVersion, snapshot.runtimeVersion.specVersion);
  report.deployedWasmSha256 = sha256(hexToU8a(await deployed.get('0x3a636f6465')));
  assert.equal(report.deployedWasmSha256, snapshot.code.sha256);
  const wasm = readFileSync(opts.wasm); report.candidate = { path: opts.wasm, bytes: wasm.length, sha256: sha256(wasm) };
  deployed.setWasm(u8aToHex(wasm)); report.candidate.runtimeVersion = await deployed.runtimeVersion;
  assert.equal(report.candidate.runtimeVersion.specVersion, 134);
  assert.equal(report.candidate.runtimeVersion.transactionVersion, 131);
  const initialized = await initializeUpgrade(deployed);
  progress('Generating synthetic report fixtures using real current-session ownership proofs');
  const fixtures = await reportFixtures(initialized);
  await chooseSigner(initialized, fixtures[0][1]);
  for (const [label, call] of fixtures) {
    progress('Charging signed ' + label); const applied = await signedCheck(initialized, label, call);
    progress('Charging duplicate ' + label); await signedCheck(applied, label + ' duplicate', call, false);
    progress('Rejecting unsigned ' + label); await unsignedCheck(initialized, label, call);
  }
  const meta = await initialized.meta;
  progress('Charging a signed utility wrapper with a legacy report call');
  await signedCheck(initialized, 'utility.batchAll([babe.reportEquivocationUnsigned])',
    meta.tx.utility.batchAll([fixtures[1][1]]));
  progress('Verifying timestamp inherent retained normal execution');
  report.inputs = { scriptSha256: sha256(readFileSync(__filename)), snapshotSha256: sha256(readFileSync(opts.snapshot)),
    packageLockSha256: sha256(readFileSync(resolve(__dirname, 'package-lock.json'))) };
  report.checks = { exactCandidateWasmUpgradeExecuted: true, allFourUnsignedReportCallsRejectedByAllSources: true,
    allFourUnsignedReportCallsRejectedByBlockApplication: true, allFourSignedReportsSucceededAndRetainedPositiveFees: true,
    allFourDuplicateReportsFailedAndRetainedPositiveFees: true, signedUtilityWrappedLegacyReportCharged: true,
    timestampInherentRetained: true, upgradePreservesStakingClaimsLedgersAndXorIssuance: true,
    consensusRuntimeApisDisableAutomaticUnsignedSubmission: true, transactionVersionRemains131: true };
  report.status = 'passed';
}
const timeout = setTimeout(() => { report.status = 'timed_out'; save(); process.exit(2); }, 900000);
main().catch(error => { report.status = 'failed'; report.error = { message: error.message, stack: error.stack };
  console.error(error); process.exitCode = 1; }).finally(async () => { clearTimeout(timeout);
  if (cache) { const bytes = gzipSync(JSON.stringify(cache)); writeFileSync(opts.cache, bytes);
    report.readCache.savedBytes = bytes.length; report.readCache.sha256 = sha256(bytes); }
  report.finishedAt = new Date().toISOString(); save(); if (chain) await chain.close().catch(() => {});
  await destroyWorker().catch(() => {});
  console.log(JSON.stringify({ status: report.status, error: report.error?.message, candidateSha256: report.candidate?.sha256 })); });
