 'use strict';
// Exact candidate Wasm, pinned public state, local dispatch only; no real keys.
const assert = require('node:assert/strict');
const execution = require('./runtime-execution.cjs');
const applyExtrinsic = (block, encoded) => execution.applyExtrinsic(block, encoded, report);
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
  candidateSpec: Number(option('--candidate-spec', '137')),
  wasm: resolve(option('--wasm', resolve(__dirname, '../candidate.compact.compressed.wasm'))),
  snapshot: resolve(option('--snapshot', resolve(__dirname, 'snapshot.json'))),
  cache: resolve(option('--cache', resolve(__dirname, '.public-read-cache.json.gz'))),
  syntheticFixtures: args.includes('--synthetic-fixtures'),
  output: resolve(option('--output', resolve(__dirname, 'equivocation-fee-wasm-rehearsal.json'))),
};
const snapshot = JSON.parse(readFileSync(opts.snapshot));
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const fakeSignature = new Uint8Array(64).fill(0xcd); fakeSignature.set([0xde, 0xad, 0xbe, 0xef]);
const report = { status: 'running', startedAt: new Date().toISOString(), localOnly: true,
  endpoint: opts.endpoint, pinnedBlockHash: snapshot.blockHash, pinnedBlockNumber: snapshot.blockNumber,
  executor: '@acala-network/chopsticks-core@1.5.2', mockSignatureHost: true, allowUnresolvedImports: true,
  offchainWorker: false, submittedTransactions: 0, privateKeysRead: 0, realSignatures: 0, remoteRpcMethods: {},
  unsignedChecks: [], signedChecks: [], localOnlySyntheticEvidence: true,
  syntheticFixturesEnabled: opts.syntheticFixtures, fixtureOverrides: [],
  constraints: [
    'Only enumerated read-only public RPC supplies block-hash-pinned state.',
    'An inherent-only upgrade block is finalized with timestamp only; signed fixtures start in the next AllExtrinsics block. InvalidTransaction raw diffs are recorded and rolled back as in the node block builder.',
    'Only :code is replaced before upgrade initialization. Explicit --synthetic-fixtures may subsequently fund a local payer when the bounded public-account search has insufficient spendable XOR; every account and issuance write is disclosed.',
    'Synthetic headers, reports, and extrinsics use the mocked signature host. Real cryptographic verification is covered by native tests; consensus block validity and governance enactment are outside this rehearsal.',
    'Every transaction branch starts from the same initialized local state; slashing/offence changes remain inside its local branch.',
  ] };
let chain, cache, restoreRpcGuard;
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
async function initializeUpgrade(parent, phase = 'upgrade') {
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
  progress(phase === 'upgrade' ? 'Executing exact candidate Core_initialize_block and on-runtime-upgrade hooks' : 'Initializing the next normal block after migration completion');
  const response = await block.call('Core_initialize_block', [header.toHex()]);
  block.pushStorageLayer().setAll(response.storageDiff);
  const mode = execution.inclusionMode(response.result);
  const initialization = { header: header.toJSON(), slot, result: response.result, inclusionMode: mode,
    storageDiffEntries: response.storageDiff.length, runtimeLogs: response.runtimeLogs };
  if (phase === 'upgrade') report.initialize = initialization;
  else report.postUpgradeInitialize = initialization;
  if (phase !== 'upgrade') assert.equal(mode, 'AllExtrinsics', 'Next block must allow normal transactions');
  assert.equal(header.parentHash.toHex(), parent.hash);
  assert.equal(header.number.toNumber(), parent.number + 1);
  assert.equal(slot, (await rawQuery(parent, 'babe', 'currentSlot')).toNumber() + 1);
  assert.equal((await block.runtimeVersion).specVersion, opts.candidateSpec, 'Candidate code must survive block rollover');
  assert.equal((await rawQuery(block, 'system', 'lastRuntimeUpgrade')).unwrap().specVersion.toNumber(), opts.candidateSpec);
  const meta = await block.meta;
  const protectedPrefixes = ['claimedRewards', 'ledger'].map(name => meta.query.staking[name].keyPrefix().toHex());
  const protectedWrites = response.storageDiff.filter(([key]) => protectedPrefixes.some(prefix => key.startsWith(prefix)));
  assert.deepEqual(protectedWrites, [], 'Upgrade hooks must preserve all staking claim and ledger entries');
  assert.equal((await rawQuery(block, 'balances', 'totalIssuance')).toString(),
    (await rawQuery(parent, 'balances', 'totalIssuance')).toString(), 'Upgrade hooks must preserve native XOR issuance');
  initialization.stakingClaimAndLedgerWrites = protectedWrites.length;
  initialization.nativeXorIssuancePreserved = true;
  const timestamp = new GenericExtrinsic(registry, meta.tx.timestamp.set(slot * 6000)).toHex();
  const applied = await applyExtrinsic(block, timestamp);
  block.pushStorageLayer().setAll(applied.storageDiff);
  const outcome = registry.createType('ApplyExtrinsicResult', applied.result);
  assert(outcome.isOk && outcome.asOk.isOk);
  const timestampEvidence = { value: slot * 6000, outcome: outcome.toJSON() };
  if (mode === 'AllExtrinsics') {
    report.timestamp = timestampEvidence;
    assert((await rawQuery(block, 'multiBlockMigrations', 'cursor')).isNone, 'Signed fixtures require no pending multi-block migration');
    report.signedFixtureBlock = { number: block.number, hash: block.hash, parentHash: header.parentHash.toHex(),
      slot, inclusionMode: mode, migrationCursorNone: true, candidateSpecVersion: opts.candidateSpec };
    return block;
  }
  assert.equal(phase, 'upgrade', 'Only the first upgrade block may be inherent-only');
  report.upgradeTimestamp = timestampEvidence;
  const beforeEvents = await rawQuery(block, 'system', 'events');
  const cursorBefore = await rawQuery(block, 'multiBlockMigrations', 'cursor');
  assert(cursorBefore.isSome, 'Inherent-only upgrade block must have an active migration cursor');
  progress('Finalizing the inherent-only upgrade block through actual candidate Wasm');
  const finalized = await block.call('BlockBuilder_finalize_block', []);
  block.pushStorageLayer().setAll(finalized.storageDiff);
  const finalizedHeader = registry.createType('Header', finalized.result);
  assert.equal(finalizedHeader.number.toNumber(), block.number);
  assert.equal(finalizedHeader.parentHash.toHex(), parent.hash);
  const cursorAfter = await rawQuery(block, 'multiBlockMigrations', 'cursor');
  assert(cursorAfter.isNone, 'Pinned current-state migration must finish before normal fixtures');
  const migrationEvents = (await rawQuery(block, 'system', 'events')).slice(beforeEvents.length)
    .map(({ event }) => ({ section: event.section, method: event.method, data: event.data.toJSON() }));
  assert(migrationEvents.some(event => event.section === 'multiBlockMigrations' && event.method === 'UpgradeCompleted'));
  const protectedFinalizationWrites = finalized.storageDiff.filter(([key]) => protectedPrefixes.some(prefix => key.startsWith(prefix)));
  assert.deepEqual(protectedFinalizationWrites, [], 'Current-state migration finalization must preserve staking claim and ledger entries');
  assert.equal((await rawQuery(block, 'balances', 'totalIssuance')).toString(), (await rawQuery(parent, 'balances', 'totalIssuance')).toString());
  report.upgradeFinalization = { actualCandidateWasm: true, timestampOnly: true, signedExtrinsicsApplied: 0,
    header: finalizedHeader.toJSON(), hash: finalizedHeader.hash.toHex(), cursorBefore: cursorBefore.toJSON(),
    cursorAfter: cursorAfter.toJSON(), events: migrationEvents, storageDiffEntries: finalized.storageDiff.length,
    stakingClaimAndLedgerWrites: protectedFinalizationWrites.length, nativeXorIssuancePreserved: true,
    stateRootConsensusVerified: false, scope: 'Local partial-backend execution; finalized header linkage is coherent, but this is not a consensus/state-root validation.' };
  const completed = new Block(chain, block.number, finalizedHeader.hash.toHex(), block, {
    header: finalizedHeader, extrinsics: [timestamp], storage: block.storage,
  });
  return initializeUpgrade(completed, 'postUpgrade');
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
  execution.assertSignedFixtureBlock(report, block);
  const tx = new GenericExtrinsic(registry, call);
  tx.signFake(report.signer.address, { blockHash: block.hash, genesisHash: snapshot.genesis,
    runtimeVersion: report.candidate.runtimeVersion, nonce: account.nonce });
  tx.signature.set(fakeSignature);
  return tx;
}
async function chooseSigner(block, fixtures) {
  const registry = await block.registry;
  const spendable = candidate => {
    const frozen = candidate.data.frozen?.toBigInt() ?? candidate.data.feeFrozen?.toBigInt() ?? 0n;
    return candidate.data.free.toBigInt() > frozen ? candidate.data.free.toBigInt() - frozen : 0n;
  };
  report.signer = { address: snapshot.feePayerCandidates[0] };
  const account = await rawQuery(block, 'system', 'account', [report.signer.address]);
  let largestFee = 0n;
  for (const [, fixture] of fixtures) {
    const tx = signedExtrinsic(registry, fixture, block, account);
    const feeResponse = await block.call('TransactionPaymentApi_query_info', [tx.toHex(), u8aToHex(registry.createType('u32', tx.encodedLength).toU8a())]);
    const fee = registry.createType('RuntimeDispatchInfo', feeResponse.result).partialFee.toBigInt();
    assert(fee > 0n); if (fee > largestFee) largestFee = fee;
  }
  const minimum = largestFee * 2n + 10n ** 18n;
  report.signerSelection = { largestEstimatedReportFee: largestFee.toString(), reportQuotes: fixtures.length,
    minimumSpendable: minimum.toString() }; save();
  const select = async address => {
    const candidate = await rawQuery(block, 'system', 'account', [address]);
    if (spendable(candidate) <= minimum) return false;
    report.signer = { address, free: candidate.data.free.toString(), spendable: spendable(candidate).toString(),
      minimumSpendable: minimum.toString(), selectedFromPublicAccounts: true, storageOverrides: [] };
    return true;
  };
  for (const address of snapshot.feePayerCandidates) {
    if (await select(address)) return block;
  }
  // The public validators/council may have enough free XOR but insufficient
  // spendable XOR after staking locks. Search a bounded public page at the same
  // pinned block without funding or otherwise changing any account.
  const meta = await block.meta;
  const prefix = meta.query.system.account.keyPrefix().toHex();
  const keys = await chain.api.getKeysPaged(prefix, 256, prefix, snapshot.blockHash);
  report.signerSelection.publicAccountScan = { pinnedBlockHash: snapshot.blockHash,
    maximumAccountsExamined: 256, cachePrefetchLimit: 1000, returnedKeys: keys.length, examined: 0 };
  for (const key of keys) {
    assert.equal(key.length, prefix.length + 32 + 64, 'System account key must have Blake2_128Concat AccountId32 layout');
    const address = registry.createType('AccountId', '0x' + key.slice(-64)).toString();
    report.signerSelection.publicAccountScan.examined++;
    if (await select(address)) return block;
  }
  assert(opts.syntheticFixtures, 'Bounded public payers lack spendable funding; rerun with --synthetic-fixtures for disclosed local-only funding');
  assert(report.initialize.nativeXorIssuancePreserved, 'Synthetic funding must follow the unmodified upgrade preservation checks');
  const address = snapshot.feePayerCandidates[0];
  const before = await rawQuery(block, 'system', 'account', [address]);
  const after = before.toJSON();
  const added = minimum + 10n ** 18n;
  after.data.free = (before.data.free.toBigInt() + added).toString();
  const oldIssuance = (await rawQuery(block, 'balances', 'totalIssuance')).toBigInt();
  const rows = [
    ['system', 'account', [address], after],
    ['balances', 'totalIssuance', [], (oldIssuance + added).toString()],
  ];
  const writes = [];
  for (const [pallet, name, params, json] of rows) {
    const key = storageKey(meta, pallet, name, params);
    const value = u8aToHex(registry.createType(key.outputType, json).toU8a());
    writes.push([key.toHex(), value]);
    report.fixtureOverrides.push({ kind: 'equivocationPayerFunding', fixtureOnly: true, pallet, name,
      key: key.toHex(), value, parentStateBefore: await block.get(key.toHex()) ?? null });
  }
  const funded = new Block(chain, block.number, block.hash, block, {
    header: await block.header, extrinsics: [], storage: block.storage,
  });
  funded.pushStorageLayer().setAll(writes);
  const fundedAccount = await rawQuery(funded, 'system', 'account', [address]);
  const restored = fundedAccount.toJSON(); restored.data.free = before.toJSON().data.free;
  assert.deepEqual(restored, before.toJSON(), 'Funding changes only free balance, preserving nonce, locks and account references');
  assert.equal((await rawQuery(funded, 'balances', 'totalIssuance')).toBigInt() - oldIssuance, added);
  assert(spendable(fundedAccount) > minimum);
  report.signer = { address, free: fundedAccount.data.free.toString(), spendable: spendable(fundedAccount).toString(),
    minimumSpendable: minimum.toString(), selectedFromPublicAccounts: true, syntheticFunding: true,
    storageOverrides: writes.map(([key]) => key) };
  report.syntheticPayerFunding = { passed: true, fixtureOnly: true, afterUpgradePreservationChecks: true,
    account: address, before: before.toJSON(), after: fundedAccount.toJSON(), added: added.toString(),
    issuanceBefore: oldIssuance.toString(), issuanceAfter: (oldIssuance + added).toString(),
    nonceFrozenAndReferencesPreserved: true, issuanceDeltaMatchesFunding: true };
  save(); return funded;
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
  const response = await applyExtrinsic(block, tx.toHex());
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
  const response = await applyExtrinsic(block, tx.toHex());
  const applied = registry.createType('ApplyExtrinsicResult', response.result);
  if (applied.isErr) {
    report.failedSignedAdmission = { method: label, outcome: applied.toJSON(),
      quotedFee: quote.partialFee.toString(), signerBefore: before.toJSON() };
    save();
  }
  assert(applied.isOk, label + ' funded signed extrinsic must pass transaction validity: ' + JSON.stringify(applied.toJSON()));
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
  assert.equal(opts.endpoint, snapshot.endpoint, 'Endpoint must equal the captured approved endpoint');
  assert.equal(new URL(opts.endpoint).protocol, 'wss:', 'Only the guarded secure WebSocket transport is permitted');
  report.dependencies = require('./validate-dependencies.cjs').validateDependencies(__dirname);
  await cryptoWaitReady(); progress('Loading pinned deployed public state');
  assert(Number.isSafeInteger(opts.candidateSpec) && opts.candidateSpec > snapshot.runtimeVersion.specVersion, 'Candidate spec must strictly advance captured production version');
  restoreRpcGuard = require('./readonly-rpc.cjs').installReadonlyRpc(report);
  chain = await setup({ endpoint: opts.endpoint, block: snapshot.blockHash,
    registeredTypes: { typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } },
    buildBlockMode: BuildBlockMode.Manual, mockSignatureHost: true, allowUnresolvedImports: true,
    offchainWorker: false, processQueuedMessages: false, saveBlocks: false, runtimeLogLevel: 3, rpcTimeout: 60000 });
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
  assert.equal(report.candidate.runtimeVersion.specVersion, opts.candidateSpec);
  assert.equal(report.candidate.runtimeVersion.transactionVersion, 133);
  let initialized = await initializeUpgrade(deployed);
  progress('Generating synthetic report fixtures using real current-session ownership proofs');
  const fixtures = await reportFixtures(initialized);
  initialized = await chooseSigner(initialized, fixtures);
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
    packageLockSha256: sha256(readFileSync(resolve(__dirname, 'package-lock.json'))), rpcGuardSha256: sha256(readFileSync(resolve(__dirname, 'readonly-rpc.cjs'))), dependencyValidatorSha256: sha256(readFileSync(resolve(__dirname, 'validate-dependencies.cjs'))) };
  report.checks = { exactCandidateWasmUpgradeExecuted: true, allFourUnsignedReportCallsRejectedByAllSources: true,
    allFourUnsignedReportCallsRejectedByBlockApplication: true, allFourSignedReportsSucceededAndRetainedPositiveFees: true,
    allFourDuplicateReportsFailedAndRetainedPositiveFees: true, signedUtilityWrappedLegacyReportCharged: true,
    timestampInherentRetained: true, upgradePreservesStakingClaimsLedgersAndXorIssuance: true,
    consensusRuntimeApisDisableAutomaticUnsignedSubmission: true, transactionVersionAdvancedTo133: true };
  report.status = 'passed';
}
const timeout = setTimeout(() => { report.status = 'timed_out'; save(); process.exit(2); }, 900000);
main().catch(error => { report.status = 'failed'; report.error = { message: error.message, stack: error.stack };
  console.error(error); process.exitCode = 1; }).finally(async () => {
  clearTimeout(timeout);
  try {
    if (cache) { const bytes = gzipSync(JSON.stringify(cache)); writeFileSync(opts.cache, bytes);
      report.readCache.savedBytes = bytes.length; report.readCache.sha256 = sha256(bytes); }
    if (chain) await chain.close();
    await destroyWorker();
  } catch (error) {
    report.status = 'failed'; report.cleanupError = { message: error.message, stack: error.stack };
    process.exitCode = 1;
  } finally {
    if (restoreRpcGuard) restoreRpcGuard();
    report.finishedAt = new Date().toISOString(); save();
  }
  console.log(JSON.stringify({ status: report.status, error: report.error?.message || report.cleanupError?.message, candidateSha256: report.candidate?.sha256 }));
});
