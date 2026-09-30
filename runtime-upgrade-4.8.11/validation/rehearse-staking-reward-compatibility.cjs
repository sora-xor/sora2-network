'use strict';
// Exact candidate Wasm, pinned public state, local dispatch only.
// Usage: node rehearse-staking-reward-compatibility.cjs --wasm /absolute/candidate.wasm
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync, existsSync } = require('node:fs');
const { resolve } = require('node:path');
const { createHash } = require('node:crypto');
const { gunzipSync, gzipSync } = require('node:zlib');
const { ApiPromise } = require('@polkadot/api');
const { StorageKey, GenericExtrinsic } = require('@polkadot/types');
const { hexToU8a, u8aToHex, compactAddLength, stringToHex } = require('@polkadot/util');
const { blake2AsHex, cryptoWaitReady } = require('@polkadot/util-crypto');
const { setup, BuildBlockMode, Block, ChopsticksProvider, destroyWorker } = require('@acala-network/chopsticks-core');
const { fullOverrideBundle } = require('@sora-substrate/type-definitions');
const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
const opts = {
  endpoint: option('--endpoint', 'https://mof2.sora.org'),
  wasm: resolve(option('--wasm', resolve(__dirname, '../framenode-runtime-4.8.11.compact.compressed.wasm'))),
  snapshot: resolve(option('--snapshot', resolve(__dirname, 'snapshot.json'))),
  cache: resolve(option('--cache', '/tmp/sora-staking-reward-compatibility-public-read-cache.json.gz')),
  output: resolve(option('--output', resolve(__dirname, 'staking-reward-wasm-rehearsal.json'))),
};
const snapshot = JSON.parse(readFileSync(opts.snapshot));
const VAL = '0x020004' + '00'.repeat(29);
const MARKER = stringToHex('runtime:migrations:val_staking_rewards_published');
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const report = { status: 'running', startedAt: new Date().toISOString(), localOnly: true, endpoint: opts.endpoint,
  pinnedBlockHash: snapshot.blockHash, pinnedBlockNumber: snapshot.blockNumber,
  executor: '@acala-network/chopsticks-core@1.5.1', mockSignatureHost: true, allowUnresolvedImports: true,
  offchainWorker: false, submittedTransactions: 0, privateKeysRead: 0, realSignatures: 0, remoteRpcMethods: {},
  constraints: [
    'Public RPC supplies block-hash-pinned state. The public provider accepts only enumerated read-only methods.',
    'Only :code is manually replaced locally. Migration execution and payout changes come from this exact candidate Wasm.',
    'Headers and local extrinsics use a synthetic BABE pre-digest and mocked signature host. Consensus validity and governance-origin enactment are not tested.',
    'A normal fee-paying staking.payoutStakers extrinsic is executed locally. XOR transaction fees are recorded separately from staking reward issuance.',
    'Normal legacy claim pruning may change ledger claim history; total, active and unlocking must remain identical.',
  ], payoutChecks: [] };
let chain;
let cache;
let localApi;
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
async function snapshotEraBudgets(block) {
  return Promise.all(snapshot.eras.map(async fixture => {
    const standard = await rawQuery(block, 'staking', 'erasValidatorReward', [fixture.era]);
    const val = await rawQuery(block, 'xorFee', 'valStakingEraReward', [fixture.era]);
    const meta = await block.meta;
    const valRaw = await block.get(storageKey(meta, 'xorFee', 'valStakingEraReward', [fixture.era]).toHex());
    return { era: fixture.era, standard: standard.isSome ? standard.unwrap().toString() : null, val: val.toString(), valPresent: valRaw !== undefined && valRaw !== null };
  }));
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
  progress('Executing exact candidate Core_initialize_block and upgrade migration');
  const response = await block.call('Core_initialize_block', [header.toHex()]);
  block.pushStorageLayer().setAll(response.storageDiff);
  report.initialize = { header: header.toJSON(), slot, result: response.result, storageDiffEntries: response.storageDiff.length, runtimeLogs: response.runtimeLogs };
  const meta = await block.meta;
  const protectedPrefixes = ['claimedRewards', 'ledger'].map(name => meta.query.staking[name].keyPrefix().toHex());
  const protectedWrites = response.storageDiff.filter(([key]) => protectedPrefixes.some(prefix => key.startsWith(prefix)));
  assert.deepEqual(protectedWrites, [], 'Reward migration must preserve all claim and ledger entries');
  assert.equal(await block.get(MARKER), '0x01');
  assert.equal((await rawQuery(block, 'system', 'lastRuntimeUpgrade')).unwrap().specVersion.toNumber(), 133);
  report.migrationBudgets = { before: report.before.budgets, after: await snapshotEraBudgets(block), claimsAndLedgerWrites: protectedWrites.length };
  for (let index = 0; index < report.before.budgets.length; index++) {
    const before = report.before.budgets[index], after = report.migrationBudgets.after[index];
    assert.equal(after.val, before.val);
    assert.equal(after.valPresent, before.valPresent);
    assert.equal(after.standard, before.standard !== null && before.valPresent ? before.val : before.standard);
  }
  report.migrationBudgets.nonzeroPublishedEras = report.migrationBudgets.after.filter(row => row.standard !== null && BigInt(row.standard) > 0n).length;
  assert(report.migrationBudgets.nonzeroPublishedEras > 0);
  const beforeXor = await rawQuery(parent, 'balances', 'totalIssuance');
  const afterXor = await rawQuery(block, 'balances', 'totalIssuance');
  assert.equal(afterXor.toString(), beforeXor.toString(), 'Publishing budgets must not change XOR issuance');
  const timestamp = new GenericExtrinsic(registry, meta.tx.timestamp.set(slot * 6000)).toHex();
  const applied = await block.call('BlockBuilder_apply_extrinsic', [timestamp]);
  block.pushStorageLayer().setAll(applied.storageDiff);
  const outcome = registry.createType('ApplyExtrinsicResult', applied.result);
  assert(outcome.isOk && outcome.asOk.isOk);
  report.timestamp = { value: slot * 6000, outcome: outcome.toJSON() };
  return block;
}
async function chooseSigner(block, recipients) {
  const registry = await block.registry;
  const meta = await block.meta;
  const call = meta.tx.staking.payoutStakers(snapshot.payoutSample.validator, snapshot.payoutSample.era);
  const estimation = new GenericExtrinsic(registry, call);
  estimation.signFake('0x' + '11'.repeat(32), { blockHash: block.hash, genesisHash: snapshot.genesis, runtimeVersion: await block.runtimeVersion, nonce: 0 });
  const feeResponse = await block.call('TransactionPaymentApi_query_info', [estimation.toHex(), registry.createType('u32', estimation.encodedLength).toHex()]);
  assert.equal(feeResponse.storageDiff.length, 0);
  const feeInfo = registry.createType('RuntimeDispatchInfo', feeResponse.result);
  const minimumFree = feeInfo.partialFee.toBigInt() * 4n + 10n ** 18n;
  const validators = (await rawQuery(block, 'session', 'validators')).map(value => value.toString());
  const controllers = await Promise.all(validators.map(async address => {
    const controller = await rawQuery(block, 'staking', 'bonded', [address]);
    return controller.isSome ? controller.unwrap().toString() : address;
  }));
  const candidates = [...new Set([...(await rawQuery(block, 'council', 'members')).map(value => value.toString()), ...validators, ...controllers])];
  report.signerSelection = { candidateSource: 'Public council, session validators and their bonded controllers', estimatedPayoutFee: feeInfo.partialFee.toString(), minimumFree: minimumFree.toString(), localFeeQuery: 'TransactionPaymentApi_query_info', candidates: [] };
  for (const address of candidates) {
    if (recipients.includes(address)) continue;
    const account = await rawQuery(block, 'system', 'account', [address]);
    report.signerSelection.candidates.push({ address, free: account.data.free.toString() });
    if (account.data.free.toBigInt() > minimumFree) return { address, free: account.data.free.toString(), storageOverrides: [] };
  }
  throw new Error('No public candidate account has enough fee funding; select another funded non-recipient without mutating remote state');
}
async function rewardState(block, stash, destination) {
  const bonded = await rawQuery(block, 'staking', 'bonded', [stash]);
  const ledger = bonded.isSome ? await rawQuery(block, 'staking', 'ledger', [bonded.unwrap()]) : null;
  return { stash, destination, xor: (await rawQuery(block, 'system', 'account', [destination])).data.toJSON(),
    val: (await rawQuery(block, 'tokens', 'accounts', [destination, VAL])).toJSON(),
    ledger: ledger?.isSome ? ledger.unwrap().toJSON() : null };
}
async function verifyUnmodifiedUpstreamDerive(block) {
  // Install no custom derives. The SORA bundle supplies SCALE types only.
  await chain.onNewBlock(block);
  localApi = await ApiPromise.create({ provider: new ChopsticksProvider(chain), noInitWarn: true,
    typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } });
  const sample = snapshot.payoutSample;
  const accounts = [sample.validator, ...sample.exposure.others.map(nominator => nominator.who)];
  const eras = [localApi.registry.createType('EraIndex', sample.era)];
  progress('Checking unmodified upstream staking reward derives after actual migration');
  const rewards = await localApi.derive.staking.stakerRewardsMultiEras(accounts, eras);
  const rows = rewards.map((eras, index) => ({ account: accounts[index], eras: eras.map(reward => ({
    era: reward.era.toNumber(), eraReward: reward.eraReward.toString(), isClaimed: reward.isClaimed,
    validators: Object.fromEntries(Object.entries(reward.validators).map(([validator, values]) => [validator, { total: values.total.toString(), value: values.value.toString() }])),
    pendingTotal: Object.values(reward.validators).reduce((sum, values) => sum + BigInt(values.value.toString()), 0n).toString(),
  })) }));
  report.upstreamDerive = { package: '@polkadot/api-derive@' + JSON.parse(readFileSync(resolve(__dirname, 'node_modules/@polkadot/api-derive/package.json'))).version,
    method: 'staking.stakerRewardsMultiEras', customDerivesInstalled: false,
    stakerRewardsSourceSha256: sha256(readFileSync(require.resolve('@polkadot/api-derive/staking/stakerRewards'))), rows };
  assert(rows.some(row => row.eras.some(era => !era.isClaimed && BigInt(era.pendingTotal) > 0n)), 'Upstream derive must expose a positive unclaimed reward');
  report.upstreamDerive.positivePendingRewardVisible = true;
  await localApi.disconnect(); localApi = null;
}
async function applyPayout(before, byPage = false, duplicate = false) {
  const sample = snapshot.payoutSample;
  const registry = await before.registry;
  const meta = await before.meta;
  const call = byPage ? meta.tx.staking.payoutStakersByPage(sample.validator, sample.era, sample.page) : meta.tx.staking.payoutStakers(sample.validator, sample.era);
  const account = await rawQuery(before, 'system', 'account', [report.signer.address]);
  const extrinsic = new GenericExtrinsic(registry, call);
  extrinsic.signFake(report.signer.address, { blockHash: before.hash, genesisHash: snapshot.genesis, runtimeVersion: await before.runtimeVersion, nonce: account.nonce });
  const fakeSignature = new Uint8Array(64).fill(0xcd); fakeSignature.set([0xde, 0xad, 0xbe, 0xef]); extrinsic.signature.set(fakeSignature);
  progress('Applying local ' + (byPage ? 'staking.payoutStakersByPage' : 'staking.payoutStakers') + (duplicate ? ' duplicate' : ''));
  const response = await before.call('BlockBuilder_apply_extrinsic', [extrinsic.toHex()]);
  const shadow = new Block(chain, before.number, before.hash, before, { header: await before.header, extrinsics: [], storage: before.storage, storageDiff: Object.fromEntries(response.storageDiff) });
  const outcome = registry.createType('ApplyExtrinsicResult', response.result);
  assert(outcome.isOk, 'Payout transaction validity must pass');
  const previousEvents = await rawQuery(before, 'system', 'events');
  const events = (await rawQuery(shadow, 'system', 'events')).slice(previousEvents.length).map(({ event }) => ({ section: event.section, method: event.method, data: event.data.toJSON() }));
  const valEvents = events.filter(event => event.section === 'xorFee' && event.method === 'ValStakingRewardPaid');
  const rewardEvents = events.filter(event => event.section === 'staking' && event.method === 'Rewarded');
  const beforeVal = await rawQuery(before, 'tokens', 'totalIssuance', [VAL]);
  const afterVal = await rawQuery(shadow, 'tokens', 'totalIssuance', [VAL]);
  const beforeXor = await rawQuery(before, 'balances', 'totalIssuance');
  const afterXor = await rawQuery(shadow, 'balances', 'totalIssuance');
  const signerAfter = await rawQuery(shadow, 'system', 'account', [report.signer.address]);
  const row = { method: byPage ? 'staking.payoutStakersByPage' : 'staking.payoutStakers', duplicate, callHex: u8aToHex(call.toU8a()), outcome: outcome.toJSON(), events,
    totalValPaid: valEvents.reduce((sum, event) => sum + BigInt(event.data[4]), 0n).toString(), valIssuanceBefore: beforeVal.toString(), valIssuanceAfter: afterVal.toString(),
    xorIssuanceBefore: beforeXor.toString(), xorIssuanceAfter: afterXor.toString(), signerXorFee: (account.data.free.toBigInt() - signerAfter.data.free.toBigInt()).toString(),
    claimedPagesAfter: (await rawQuery(shadow, 'staking', 'claimedRewards', [sample.era, sample.validator])).toJSON(), storageDiffEntries: response.storageDiff.length, recipients: [] };
  report.payoutChecks.push(row); save();
  assert(afterXor.toBigInt() <= beforeXor.toBigInt(), 'VAL payout must never mint native XOR');
  if (duplicate) {
    assert(outcome.asOk.isErr);
    const error = outcome.asOk.asErr;
    assert(error.isModule);
    const decoded = registry.findMetaError(error.asModule);
    assert.equal(decoded.name, 'AlreadyClaimed');
    row.decodedError = { section: decoded.section, name: decoded.name };
    assert.equal(valEvents.length, 0); assert.equal(rewardEvents.length, 0); assert.equal(afterVal.toString(), beforeVal.toString());
  } else {
    assert(outcome.asOk.isOk);
    assert(valEvents.length > 0, 'Actual VAL payment must occur');
    assert.equal(afterVal.toBigInt() - beforeVal.toBigInt(), BigInt(row.totalValPaid));
    assert.equal(rewardEvents.length, valEvents.length, 'Every VAL reward must emit standard staking.Rewarded');
    const started = events.findIndex(event => event.section === 'staking' && event.method === 'PayoutStarted');
    const firstReward = events.findIndex(event => event.section === 'staking' && event.method === 'Rewarded');
    assert(started >= 0 && started < firstReward, 'Legacy payout event ordering must be preserved');
    for (let index = 0; index < valEvents.length; index++) {
      const paid = valEvents[index];
      assert.equal(rewardEvents[index].data[0], paid.data[0]);
      assert.equal(BigInt(rewardEvents[index].data[2]), BigInt(paid.data[4]));
      const pre = await rewardState(before, paid.data[0], paid.data[1]);
      const post = await rewardState(shadow, paid.data[0], paid.data[1]);
      assert.deepEqual(post.xor, pre.xor, 'Reward recipient native XOR balance must be unchanged');
      for (const field of ['total', 'active', 'unlocking']) assert.deepEqual(post.ledger?.[field], pre.ledger?.[field], 'VAL payout must not compound XOR staking ledger: ' + field);
      row.recipients.push({ before: pre, after: post });
    }
    row.validatorState = { before: await rewardState(before, sample.validator, sample.validator), after: await rewardState(shadow, sample.validator, sample.validator) };
    assert.deepEqual(row.validatorState.after.xor, row.validatorState.before.xor);
    for (const field of ['total', 'active', 'unlocking']) assert.deepEqual(row.validatorState.after.ledger?.[field], row.validatorState.before.ledger?.[field], 'Validator staking financial fields must remain unchanged');
    assert(row.claimedPagesAfter.includes(sample.page));
  }
  row.passed = true; save(); return shadow;
}
async function main() {
  assert.equal(snapshot.status, 'passed'); assert(existsSync(opts.wasm));
  const manifest = JSON.parse(readFileSync(resolve(__dirname, 'package.json')));
  report.dependencies = { manifest, installed: {}, lockSha256: sha256(readFileSync(resolve(__dirname, 'package-lock.json'))) };
  for (const [name, version] of Object.entries(manifest.dependencies)) {
    const installed = JSON.parse(readFileSync(resolve(__dirname, 'node_modules', name, 'package.json'))).version;
    assert.equal(installed, version, 'Installed dependency version changed: ' + name);
    report.dependencies.installed[name] = installed;
  }
  await cryptoWaitReady();
  progress('Loading pinned deployed spec-132 state through read-only public RPC');
  chain = await setup({ endpoint: opts.endpoint, block: snapshot.blockHash, registeredTypes: { typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } },
    buildBlockMode: BuildBlockMode.Manual, mockSignatureHost: true, allowUnresolvedImports: true, offchainWorker: false, processQueuedMessages: false, saveBlocks: false, runtimeLogLevel: 3, rpcTimeout: 60000 });
  const methods = new Set(['chain_getBlock', 'chain_getBlockHash', 'chain_getHeader', 'chain_getFinalizedHead', 'state_getStorage', 'state_getKeysPaged', 'state_queryStorageAt', 'state_getMetadata', 'state_getRuntimeVersion', 'state_call', 'system_properties', 'system_chain', 'system_name', 'system_version', 'rpc_methods']);
  const send = chain.api.send.bind(chain.api);
  chain.api.send = (method, ...params) => { assert(methods.has(method), 'Only read-only public RPC allowed: ' + method); report.remoteRpcMethods[method] = (report.remoteRpcMethods[method] || 0) + 1; return send(method, ...params); };
  installReadCache();
  const deployed = chain.head;
  assert.equal(deployed.number, snapshot.blockNumber);
  assert.equal((await rawQuery(deployed, 'system', 'blockHash', [0])).toHex(), snapshot.genesis);
  report.deployedVersion = await deployed.runtimeVersion; assert.equal(report.deployedVersion.specVersion, 132);
  report.deployedWasmSha256 = sha256(hexToU8a(await deployed.get('0x3a636f6465'))); assert.equal(report.deployedWasmSha256, snapshot.code.sha256);
  report.before = { marker: await deployed.get(MARKER) ?? null, budgets: await snapshotEraBudgets(deployed) }; assert.equal(report.before.marker, null);
  assert.deepEqual(report.before.budgets, snapshot.eras);
  const wasm = readFileSync(opts.wasm); report.candidate = { path: opts.wasm, bytes: wasm.length, sha256: sha256(wasm) };
  deployed.setWasm(u8aToHex(wasm)); report.candidate.runtimeVersion = await deployed.runtimeVersion;
  assert.equal(report.candidate.runtimeVersion.specVersion, 133); assert.equal(report.candidate.runtimeVersion.transactionVersion, 131);
  const initialized = await initializeUpgrade(deployed);
  const allStashes = [snapshot.payoutSample.validator, ...snapshot.payoutSample.exposure.others.map(nominator => nominator.who)];
  const destinations = await Promise.all(allStashes.map(async stash => {
    const payee = await rawQuery(initialized, 'staking', 'payee', [stash]);
    if (payee.isNone) return stash;
    const dest = payee.unwrap();
    if (dest.type === 'Account') return dest.value.toString();
    if (dest.type === 'Controller') return (await rawQuery(initialized, 'staking', 'bonded', [stash])).unwrap().toString();
    return stash;
  }));
  report.signer = await chooseSigner(initialized, destinations);
  await verifyUnmodifiedUpstreamDerive(initialized);
  const direct = await applyPayout(initialized);
  await applyPayout(direct, true, true);
  await applyPayout(direct, false, true);
  const lastRow = report.payoutChecks.at(-1);
  assert.equal(lastRow.decodedError.name, 'AlreadyClaimed');
  report.inputs = { scriptSha256: sha256(readFileSync(__filename)), snapshotSha256: sha256(readFileSync(opts.snapshot)), packageLockSha256: sha256(readFileSync(resolve(__dirname, 'package-lock.json'))) };
  report.checks = { actualCandidateWasmUpgradeExecuted: true, retainedCompletedValBudgetsPublished: true, allClaimAndLedgerMigrationEntriesPreserved: true,
    actualStandardPayoutStakersPaidVal: true, standardRewardEventsAndOrderingPreserved: true, rewardRecipientXorBalancesUnchanged: true,
    stakingLedgerBalancesUnchanged: true, noNativeXorRewardMinted: true, unmodifiedUpstreamDeriveShowsPositivePending: true,
    duplicateDirectAndByPageRejected: true, transactionVersionRemains131: true };
  report.status = 'passed';
}
const timeout = setTimeout(() => { report.status = 'timed_out'; save(); process.exit(2); }, 900000);
main().catch(error => { report.status = 'failed'; report.error = { message: error.message, stack: error.stack }; console.error(error); process.exitCode = 1; }).finally(async () => {
  clearTimeout(timeout);
  if (cache) { const bytes = gzipSync(JSON.stringify(cache)); writeFileSync(opts.cache, bytes); report.readCache.savedBytes = bytes.length; report.readCache.sha256 = sha256(bytes); }
  report.finishedAt = new Date().toISOString(); save();
  if (localApi) await localApi.disconnect().catch(() => {});
  if (chain) await chain.close().catch(() => {}); await destroyWorker().catch(() => {});
  console.log(JSON.stringify({ status: report.status, output: opts.output, error: report.error?.message, candidateSha256: report.candidate?.sha256 }));
});
