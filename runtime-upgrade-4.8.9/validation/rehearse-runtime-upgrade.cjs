/* Local-only runtime rehearsal. Upstream RPC supplies pinned read-only state.
 * No private keys, real signatures, network submissions, or host offchain workers.
 * Usage: node rehearse-runtime-upgrade.cjs --wasm /absolute/candidate.wasm
 *   --block 0x... --output /absolute/rehearsal.json [--inspect-only]
 */
'use strict';
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync, mkdirSync, existsSync, renameSync } = require('node:fs');
const { dirname, resolve } = require('node:path');
const { gzipSync, gunzipSync } = require('node:zlib');
const { createHash } = require('node:crypto');
const { ApiPromise } = require('@polkadot/api');
const { StorageKey } = require('@polkadot/types');
const { stringToHex, u8aToHex, hexToU8a } = require('@polkadot/util');
const { xxhashAsHex } = require('@polkadot/util-crypto');
const { setup, BuildBlockMode, ChopsticksProvider, buildBlock, Block, destroyWorker } = require('@acala-network/chopsticks-core');
const { fullOverrideBundle } = require('@sora-substrate/type-definitions');
const argv = process.argv.slice(2);
const option = (name, fallback) => { const at = argv.indexOf(name); return at < 0 ? fallback : argv[at + 1]; };
const opts = {
  endpoint: option('--endpoint', 'wss://ws.mof.sora.org'),
  block: option('--block', '0xedc2506edcb75c79c12d26a8c89a2473677a8aff67b3e05eb310573bdbf2a049'),
  wasm: option('--wasm', null),
  output: resolve(option('--output', '/tmp/sora-payout-runtime-hunt/wasm-rehearsal.json')),
  inspectOnly: argv.includes('--inspect-only'),
  cache: resolve(option('--cache', '/tmp/sora-payout-runtime-hunt/rehearsal-read-cache.json.gz'))
};
assert(/^0x[0-9a-f]{64}$/i.test(opts.block), 'A pinned block hash is required');
assert(opts.inspectOnly || opts.wasm, '--wasm is required for an upgrade rehearsal');
const registeredTypes = { typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } };
const VAL = '0x020004' + '00'.repeat(29);
const REMAP_MARKER = stringToHex('runtime:migrations:staking_reward_points_stash_remapped');
const report = {
  startedAt: new Date().toISOString(), endpoint: opts.endpoint, pinnedBlockHash: opts.block,
  executor: '@acala-network/chopsticks-core@1.5.1', status: 'running',
  localOnly: true, submittedTransactions: 0, realSignatures: 0,
  mockSignatureHost: true, allowUnresolvedImports: true, offchainWorker: false,
  remoteRpcMethods: {}, phases: [], payoutChecks: [], constraints: [
    'This executes candidate Wasm in Chopsticks against pinned remote state; it is not a validator-consensus or governance-origin rehearsal.',
    'Runtime replacement uses a local :code overlay. The next Core_initialize_block performs normal on_runtime_upgrade detection.',
    'Signatures are faked only inside the local executor; no private key or live transaction submission is used.',
    'The release Wasm does not include try-runtime pre/post assertions; normal migration execution and observed state are checked.',
    'allowUnresolvedImports is enabled because deployed runtime130 imports offchain ecdsa_public_keys unsupported by this executor; offchain workers remain disabled.'
  ]
};
let chain;
let api;
let saveReadCache = () => {};
function save() { mkdirSync(dirname(opts.output), { recursive: true }); writeFileSync(opts.output, JSON.stringify(report, null, 2) + '\n'); }
function progress(message) { console.log(new Date().toISOString(), message); report.stage = message; save(); }
const plain = (x) => x == null ? null : typeof x.toJSON === 'function' ? x.toJSON() : x;
const rawVersionKey = (name) => xxhashAsHex(name, 128) + xxhashAsHex(':__STORAGE_VERSION__:', 128).slice(2);
async function rawQuery(block, pallet, entry, args = []) {
  const meta = await block.meta;
  const query = meta.query[pallet][entry];
  const key = new StorageKey(meta.registry, [query, args]);
  const raw = await block.get(key.toHex());
  return meta.registry.createType(key.outputType, raw ? hexToU8a(raw) : undefined);
}
async function storageSnapshot(block) {
  const names = ['Staking', 'EthBridge', 'VestedRewards', 'Kensetsu', 'Polkamarkt', 'XorFee'];
  const storageVersions = Object.fromEntries(await Promise.all(names.map(async name => {
    const raw = await block.get(rawVersionKey(name));
    return [name, { hex: raw || null, version: raw ? Number(BigInt('0x' + Buffer.from(hexToU8a(raw)).reverse().toString('hex'))) : null }];
  })));
  return {
    blockNumber: block.number, blockHash: block.hash,
    runtimeVersion: await block.runtimeVersion,
    lastRuntimeUpgrade: plain(await rawQuery(block, 'system', 'lastRuntimeUpgrade')),
    rewardPointsRemapMarker: await block.get(REMAP_MARKER) || null,
    storageVersions
  };
}
async function connectLocalApi() {
  return ApiPromise.create({ provider: new ChopsticksProvider(chain), ...registeredTypes, noInitWarn: true });
}
async function selectPayout() {
  const active = (await api.query.staking.activeEra()).unwrap().index.toNumber();
  for (let era = active - 1; era >= active - 5; era--) {
    const native = await api.query.staking.erasValidatorReward(era);
    const val = await api.query.xorFee.valStakingEraReward(era);
    if (native.isNone || !native.unwrap().isZero() || val.isZero()) continue;
    const points = await api.query.staking.erasRewardPoints(era);
    for (const [validator, amount] of points.individual) {
      if (amount.isZero()) continue;
      const [overview, claims, bonded, payee] = await Promise.all([
        api.query.staking.erasStakersOverview(era, validator),
        api.query.staking.claimedRewards(era, validator),
        api.query.staking.bonded(validator),
        api.query.staking.payee(validator)
      ]);
      if (overview.isNone || bonded.isNone || payee.isNone || payee.unwrap().isNone) continue;
      const pageCount = Math.max(1, overview.unwrap().pageCount.toNumber());
      const claimed = claims.map(x => x.toNumber());
      const page = Array.from({length: pageCount}, (_, i) => i).find(i => !claimed.includes(i));
      if (page === undefined) continue;
      return { era, validator: validator.toString(), page, pageCount, nativeEraReward: native.unwrap().toString(), valEraReward: val.toString(), claimedPagesBefore: claimed, overview: overview.unwrap().toJSON(), rewardPoints: amount.toString(), totalRewardPoints: points.total.toString() };
    }
  }
  throw new Error('No unpaid, funded VAL payout page found in five completed eras');
}
async function chooseSigner(sample) {
  const council = api.query.council?.members ? await api.query.council.members() : [];
  const candidates = [sample.validator, ...council.map(x => x.toString())];
  for (const address of candidates) {
    const account = await api.query.system.account(address);
    if (account.data.free.toBigInt() > 10n ** 21n) return { address, free: account.data.free.toString(), nonce: account.nonce.toString(), storageOverrides: [] };
  }
  // Top up one public account only in the local storage overlay if no candidate
  // has sufficient free XOR. Preserve all existing account fields.
  const address = sample.validator;
  const account = await api.query.system.account(address);
  const previous = account.toJSON();
  const updated = { ...previous, data: { ...previous.data, free: '1000000000000000000000000000000' } };
  const key = api.query.system.account.key(address);
  const encoded = api.registry.createType(account.toRawType(), updated).toHex();
  chain.head.pushStorageLayer().set(key, encoded);
  return { address, free: updated.data.free, nonce: account.nonce.toString(), storageOverrides: [{ key, previous, updated, reason: 'local fee funding only' }] };
}
function fakeExtrinsic(call, address, nonce, runtimeVersion, registry, blockHash) {
  const extrinsic = registry.createType('GenericExtrinsic', registry.createType('Call', call.method.toHex()));
  extrinsic.signFake(address, { blockHash, genesisHash: blockHash, runtimeVersion, nonce });
  const signature = new Uint8Array(64).fill(0xcd);
  signature.set([0xde, 0xad, 0xbe, 0xef]);
  extrinsic.signature.set(signature);
  return extrinsic.toHex();
}
async function payoutDryRun(label, call, sample, signer) {
  progress('Dry-running ' + label + ' against upgraded local state');
  const before = chain.head;
  const registry = await before.registry;
  const result = await chain.dryRunExtrinsic({ call: call.method.toHex(), address: signer.address });
  const shadow = new Block(chain, before.number, before.hash, before, { header: await before.header, extrinsics: [], storage: before.storage, storageDiff: Object.fromEntries(result.storageDiff) });
  const events = await rawQuery(shadow, 'system', 'events');
  const simplified = events.map(({ event }) => ({ section: event.section, method: event.method, data: event.data.toJSON() }));
  const valEvents = simplified.filter(e => e.section === 'xorFee' && e.method === 'ValStakingRewardPaid');
  const beforeIssuance = await rawQuery(before, 'tokens', 'totalIssuance', [VAL]);
  const afterIssuance = await rawQuery(shadow, 'tokens', 'totalIssuance', [VAL]);
  const beforeXor = await rawQuery(before, 'balances', 'totalIssuance');
  const afterXor = await rawQuery(shadow, 'balances', 'totalIssuance');
  const claims = await rawQuery(shadow, 'staking', 'claimedRewards', [sample.era, sample.validator]);
  const entry = {
    label, callHex: call.method.toHex(), outcome: result.outcome.toJSON(), outcomeHuman: result.outcome.toHuman(),
    events: simplified, valRewardEventCount: valEvents.length,
    totalValPaid: valEvents.reduce((sum, e) => sum + BigInt(e.data[4]), 0n).toString(),
    valIssuanceBefore: beforeIssuance.toString(), valIssuanceAfter: afterIssuance.toString(),
    xorIssuanceBefore: beforeXor.toString(), xorIssuanceAfter: afterXor.toString(),
    claimedPagesAfter: claims.toJSON(), storageDiffEntries: result.storageDiff.length
  };
  report.payoutChecks.push(entry); save();
  assert(result.outcome.isOk && result.outcome.asOk.isOk, label + ': extrinsic must dispatch successfully');
  assert(valEvents.length > 0, label + ': VAL payment event missing');
  assert.equal(afterIssuance.toBigInt() - beforeIssuance.toBigInt(), BigInt(entry.totalValPaid), label + ': VAL issuance must equal reward events');
  assert(afterXor.toBigInt() <= beforeXor.toBigInt(), label + ': zero native staking budget must not mint XOR');
  assert(claims.some(x => x.toNumber() === sample.page), label + ': page must be claimed');
  entry.passed = true; save();
  return entry;
}
// Read-through cache for immutable upstream state only. Every entry is scoped
// to its exact block hash; local writes remain in Chopsticks' storage layers.
// Fetch values in the same batches as keys to avoid thousands of serial RPCs.
function installReadCache() {
  const cacheData = existsSync(opts.cache)
    ? JSON.parse(gunzipSync(readFileSync(opts.cache)).toString())
    : { format: 1, values: {}, keyRanges: {} };
  assert.equal(cacheData.format, 1, 'Unknown read-cache format');
  report.readCache = { path: opts.cache, existingValues: Object.keys(cacheData.values).length,
    storageHits: 0, keyPageHits: 0, fetchedValues: 0, fetchedKeyPages: 0, responseLimitSplits: 0 };
  const scoped = (hash, value) => hash + ':' + value;
  saveReadCache = () => {
    mkdirSync(dirname(opts.cache), {recursive:true});
    const temporary = opts.cache + '.tmp';
    writeFileSync(temporary, gzipSync(Buffer.from(JSON.stringify(cacheData))));
    renameSync(temporary, opts.cache);
  };
  const getStorage = chain.api.getStorage.bind(chain.api);
  const getKeys = chain.api.getKeysPaged.bind(chain.api);
  const inflight = new Map();
  const requestBatch = async (keys,hash) => {
    try { return (await chain.api.send('state_queryStorageAt',[keys,hash],true))[0]?.changes || []; }
    catch (error) {
      if (keys.length < 2 || !(error.code === -32008 || /Response is too big|Exceeded max limit/.test(error.message))) throw error;
      report.readCache.responseLimitSplits++;
      const mid = Math.floor(keys.length/2);
      return [...await requestBatch(keys.slice(0,mid),hash), ...await requestBatch(keys.slice(mid),hash)];
    }
  };
  const batch = async (keys, hash) => {
    const missing = keys.filter(key => !Object.hasOwn(cacheData.values, scoped(hash,key)));
    for (let i=0; i<missing.length; i+=256) {
      const chunk = missing.slice(i,i+256);
      const changes = new Map(await requestBatch(chunk,hash));
      for (const key of chunk) {
        assert(changes.has(key), 'Batch response omitted requested key ' + key);
        cacheData.values[scoped(hash,key)] = changes.get(key);
      }
      report.readCache.fetchedValues += chunk.length;
    }
  };
  chain.api.getStorage = async (key, hash) => {
    if (!hash) return getStorage(key,hash);
    const cacheKey = scoped(hash,key);
    if (Object.hasOwn(cacheData.values, cacheKey)) {
      report.readCache.storageHits++;
      return cacheData.values[cacheKey];
    }
    if (inflight.has(cacheKey)) return inflight.get(cacheKey);
    const pending = getStorage(key,hash).then(value => {
      cacheData.values[cacheKey] = value ?? null;
      report.readCache.fetchedValues++;
      return value;
    }).finally(() => inflight.delete(cacheKey));
    inflight.set(cacheKey,pending);
    return pending;
  };
  chain.api.getKeysPaged = async (prefix, count, start, hash) => {
    if (!hash || prefix.startsWith('0x3a6368696c645f73746f726167653a')) return getKeys(prefix,count,start,hash);
    const rangeKey = scoped(hash,prefix);
    const ranges = cacheData.keyRanges[rangeKey] ||= [];
    const normalizedStart = start || prefix;
    for (const range of ranges) {
      if (normalizedStart < range.start || (!range.complete && normalizedStart > range.end)) continue;
      const remaining = range.keys.filter(key => key > normalizedStart);
      if (remaining.length >= count || range.complete) {
        report.readCache.keyPageHits++;
        return remaining.slice(0,count);
      }
    }
    // Pallet-wide prefix existence checks need only one key. Prefetch map
    // iterations, whose prefix contains both pallet and storage item hashes.
    const fetchCount = prefix.length < 66 ? count : Math.max(1000,count);
    const fetched = await getKeys(prefix, fetchCount, normalizedStart, hash);
    await batch(fetched,hash);
    ranges.push({start:normalizedStart,end:fetched.at(-1)||normalizedStart,complete:fetched.length<fetchCount,keys:fetched});
    report.readCache.fetchedKeyPages++;
    saveReadCache();
    console.log(new Date().toISOString(), 'Cached upstream key page', prefix, fetched.length,
      'values total', report.readCache.fetchedValues);
    return fetched.slice(0,count);
  };
  report.constraints.push('Immutable upstream reads use a persistent cache keyed by block hash; key-page values are batch-prefetched. Local state transitions are never inserted into that cache.');
}
async function main() {
  progress('Loading pinned mainnet state into local Chopsticks executor');
  chain = await setup({ endpoint: opts.endpoint, block: opts.block, registeredTypes, buildBlockMode: BuildBlockMode.Manual, mockSignatureHost: true, allowUnresolvedImports: true, offchainWorker: false, processQueuedMessages: false, saveBlocks: false, runtimeLogLevel: 3, rpcTimeout: 120000 });
  const upstreamSend = chain.api.send.bind(chain.api);
  chain.api.send = async (method, ...args) => {
    assert(!/^(author_|dev_|offchain_|engine_)/.test(method), 'Forbidden upstream mutation RPC: ' + method);
    report.remoteRpcMethods[method] = (report.remoteRpcMethods[method] || 0) + 1;
    return upstreamSend(method, ...args);
  };
  installReadCache();
  report.before = await storageSnapshot(chain.head);
  assert.equal(report.before.runtimeVersion.specVersion, 130, 'Expected mainnet runtime130');
  progress('Pinned runtime130 metadata and storage decoded');
  if (opts.inspectOnly) { report.status = 'inspection-passed'; return; }
  const wasm = readFileSync(opts.wasm);
  report.candidate = { path: resolve(opts.wasm), bytes: wasm.length, sha256: createHash('sha256').update(wasm).digest('hex') };
  chain.head.setWasm(u8aToHex(wasm));
  report.candidate.runtimeVersion = await chain.head.runtimeVersion;
  assert.equal(report.candidate.runtimeVersion.specVersion, 131);
  assert.equal(report.candidate.runtimeVersion.transactionVersion, 131);
  progress('Executing first runtime131 block and normal runtime-upgrade hooks');
  const emptyParams = { transactions: [], downwardMessages: [], upwardMessages: {}, horizontalMessages: {} };
  const [upgraded, pending] = await buildBlock(chain.head, chain.getInherents(), emptyParams, {
    onApplyExtrinsicError: (_extrinsic, error) => { throw new Error('Upgrade block transaction validity failure: ' + error); },
    onPhaseApplied: (phase, response) => {
      report.phases.push({phase, result: response.result, storageDiffEntries: response.storageDiff.length, runtimeLogs: response.runtimeLogs});
      progress('Applied upgrade block phase ' + phase);
    }
  });
  assert.equal(pending.length, 0);
  await chain.onNewBlock(upgraded);
  report.after = await storageSnapshot(upgraded);
  assert.equal(report.after.lastRuntimeUpgrade.specVersion, 131, 'System LastRuntimeUpgrade must advance to131');
  assert.equal(report.after.rewardPointsRemapMarker, '0x01', 'Reward point remapping must complete');
  for (const [name, expected] of Object.entries({Staking:16, VestedRewards:4, Kensetsu:6, Polkamarkt:7, XorFee:3})) assert.equal(report.after.storageVersions[name].version, expected, name + ' storage version');
  assert([3,4].includes(report.after.storageVersions.EthBridge.version), 'EthBridge should be migrating3→4 or complete4');
  // pallet-migrations deliberately freezes ordinary calls until its bounded
  // bridge-index conversion completes. Advance empty local blocks first.
  report.migrationBlocks = [];
  for (let index=0; index<=64; index++) {
    const current = chain.head;
    const migrationMeta = await current.meta;
    const cursorKey = new StorageKey(migrationMeta.registry,[migrationMeta.query.multiBlockMigrations.cursor,[]]).toHex();
    const cursorRaw = await current.get(cursorKey);
    const cursor = cursorRaw ? await rawQuery(current, 'multiBlockMigrations', 'cursor') : null;
    const bridgeRaw = await current.get(rawVersionKey('EthBridge'));
    const bridgeVersion = bridgeRaw ? Number(BigInt('0x' + Buffer.from(hexToU8a(bridgeRaw)).reverse().toString('hex'))) : null;
    report.migrationBlocks.push({blockNumber:current.number,blockHash:current.hash,cursor:plain(cursor),ethBridgeStorageVersion:bridgeVersion});
    save(); saveReadCache();
    if (!cursorRaw && bridgeVersion === 4) break;
    assert(index < 64, 'Bridge migration did not complete within64 local blocks');
    progress('Advancing empty local block for bridge migration: ' + (index+1));
    await chain.newBlock({transactions:[]});
  }
  report.afterMigrations = await storageSnapshot(chain.head);
  assert.equal(report.afterMigrations.storageVersions.EthBridge.version,4);
  api = await connectLocalApi();
  report.upgradeEvents = (await api.query.system.events()).map(({ event }) => ({section: event.section, method: event.method, data: event.data.toJSON()}));
  const sample = await selectPayout();
  report.payoutSample = sample;
  const signer = await chooseSigner(sample);
  report.signer = signer;
  const payout = api.tx.staking.payoutStakersByPage(sample.validator, sample.era, sample.page);
  const direct = await payoutDryRun('staking.payoutStakersByPage', payout, sample, signer);
  const legacy = await payoutDryRun('staking.payoutStakers', api.tx.staking.payoutStakers(sample.validator, sample.era), sample, signer);
  const batch = await payoutDryRun('utility.batchAll(payoutStakersByPage)', api.tx.utility.batchAll([payout]), sample, signer);
  assert.equal(direct.totalValPaid, legacy.totalValPaid);
  assert.equal(direct.totalValPaid, batch.totalValPaid, 'Direct and nested payout must mint identical VAL');
  // Commit the same mocked extrinsic only to a locally built block, then confirm
  // the actual candidate rejects the duplicate without any reward events.
  const registry = await chain.head.registry;
  const nonce = (await api.query.system.account(signer.address)).nonce;
  const fake = fakeExtrinsic(payout, signer.address, nonce, await chain.head.runtimeVersion, registry, chain.head.hash);
  progress('Applying payout in one local block, then checking duplicate rejection');
  const paidBlock = await chain.newBlock({ transactions: [fake] });
  const paidAt = await api.at(paidBlock.hash);
  const paidEvents = (await paidAt.query.system.events()).map(({ event }) => ({section:event.section, method:event.method, data:event.data.toJSON()}));
  assert(paidEvents.some(e => e.section === 'xorFee' && e.method === 'ValStakingRewardPaid'), 'Committed local block must include payout');
  const duplicate = await chain.dryRunExtrinsic({call: payout.method.toHex(), address: signer.address});
  assert(duplicate.outcome.isOk && duplicate.outcome.asOk.isErr, 'Duplicate payout must fail dispatch');
  const duplicateError = duplicate.outcome.asOk.asErr;
  const decodedError = duplicateError.isModule ? registry.findMetaError(duplicateError.asModule) : null;
  assert.equal(decodedError?.name, 'AlreadyClaimed');
  report.duplicateCheck = { outcome: duplicate.outcome.toJSON(), decodedError: decodedError && {section:decodedError.section,name:decodedError.name}, paidBlockNumber: paidBlock.number, paidBlockEvents: paidEvents, passed:true };
  report.status = 'passed';
}
main().catch(error => { report.status='failed'; report.error={message:error.message,stack:error.stack}; console.error(error); process.exitCode=1; }).finally(async () => {
  report.finishedAt=new Date().toISOString(); save(); saveReadCache();
  if (api) await api.disconnect().catch(()=>{});
  if (chain) await chain.close().catch(()=>{});
  await destroyWorker().catch(()=>{});
  console.log(JSON.stringify({status:report.status,output:opts.output,error:report.error?.message}));
});
