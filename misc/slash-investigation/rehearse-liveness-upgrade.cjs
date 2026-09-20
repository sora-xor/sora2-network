/* Initialization-only, local Wasm rehearsal. No transactions or private keys.
 * Dependencies reuse the prior local Chopsticks installation; its artifacts and
 * immutable read cache are only read. Override --dependencies if it moved.
 * Usage: node rehearse-liveness-upgrade.cjs --wasm /absolute/candidate.wasm
 */
'use strict';
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync, mkdirSync, existsSync } = require('node:fs');
const { resolve, dirname } = require('node:path');
const { createRequire } = require('node:module');
const { gunzipSync } = require('node:zlib');
const { createHash } = require('node:crypto');
const argv = process.argv.slice(2);
const option = (name, fallback) => {
  const index = argv.indexOf(name);
  return index < 0 ? fallback : argv[index + 1];
};
const dependencies = resolve(option('--dependencies', '/tmp/sora-payout-runtime-hunt/package.json'));
const localRequire = createRequire(dependencies);
const { StorageKey } = localRequire('@polkadot/types');
const { u8aToHex, hexToU8a } = localRequire('@polkadot/util');
const { setup, BuildBlockMode, Block, destroyWorker } = localRequire('@acala-network/chopsticks-core');
const { newHeader } = localRequire('@acala-network/chopsticks-core/blockchain/block-builder');
const { fullOverrideBundle } = localRequire('@sora-substrate/type-definitions');
const opts = {
  endpoint: option('--endpoint', 'https://mof2.sora.org'),
  block: option('--block', '0xedc2506edcb75c79c12d26a8c89a2473677a8aff67b3e05eb310573bdbf2a049'),
  wasm: option('--wasm', null),
  cache: resolve(option('--read-cache', '/tmp/sora-payout-runtime-hunt/rehearsal-read-cache.json.gz')),
  output: resolve(option('--output', resolve(__dirname, 'liveness-wasm-rehearsal.json'))),
};
assert(opts.wasm, '--wasm is required');
assert(/^0x[0-9a-f]{64}$/i.test(opts.block), 'A pinned upstream block hash is required');
const report = {
  startedAt: new Date().toISOString(), status: 'running', endpoint: opts.endpoint,
  pinnedBlockHash: opts.block, executor: '@acala-network/chopsticks-core@1.5.1',
  dependencyManifest: dependencies, localOnly: true, submittedTransactions: 0,
  executedExtrinsics: 0, realSignatures: 0, mockSignatureHost: true,
  allowUnresolvedImports: true, offchainWorker: false, remoteRpcMethods: {},
  constraints: [
    'Runs the candidate Core_initialize_block against pinned archive state after a local :code overlay; no governance dispatch, consensus verification, block finalization or network submission is exercised.',
    'Stops immediately after initialization and checks its storage diff; does not advance unrelated bridge multi-block migrations.',
    'The prior cache contains only block-hash-scoped immutable upstream reads and is never modified. New remote reads are cached only in this process.',
    'Mock signatures and unresolved imports are executor settings only. Offchain workers are disabled; no extrinsics are executed.',
    'This release Wasm runs normal runtime-upgrade hooks, not try-runtime pre/post assertions.',
  ],
};
let chain;
const save = () => {
  mkdirSync(dirname(opts.output), { recursive: true });
  writeFileSync(opts.output, JSON.stringify(report, null, 2) + '\n');
};
const progress = stage => {
  report.stage = stage;
  console.log(new Date().toISOString(), stage);
  save();
};
async function rawQuery(block, pallet, entry, args = []) {
  const meta = await block.meta;
  const query = meta.query[pallet][entry];
  const key = new StorageKey(meta.registry, [query, args]);
  const raw = await block.get(key.toHex());
  if (query.meta.modifier.isOptional) {
    return meta.registry.createType('Option<' + key.outputType + '>', hexToU8a(raw ? '0x01' + raw.slice(2) : '0x00'));
  }
  return meta.registry.createType(key.outputType, raw ? hexToU8a(raw) : undefined);
}
async function context(block) {
  const [session, active, current, bonded] = await Promise.all([
    rawQuery(block, 'session', 'currentIndex'), rawQuery(block, 'staking', 'activeEra'),
    rawQuery(block, 'staking', 'currentEra'), rawQuery(block, 'staking', 'bondedEras'),
  ]);
  return {
    session: session.toNumber(), activeEra: active.unwrap().index.toNumber(),
    plannedEra: current.unwrap().toNumber(),
    eras: [...new Set([...bonded.map(pair => pair[0].toNumber()), current.unwrap().toNumber()])].sort((a, b) => a - b),
  };
}
function installReadCache() {
  const cache = existsSync(opts.cache)
    ? JSON.parse(gunzipSync(readFileSync(opts.cache)).toString())
    : { format: 1, values: {}, keyRanges: {} };
  assert.equal(cache.format, 1);
  report.readCache = {
    path: opts.cache, readOnly: true, existingValues: Object.keys(cache.values).length,
    storageHits: 0, keyPageHits: 0, fetchedValues: 0, fetchedKeyPages: 0,
  };
  const scoped = (hash, value) => hash + ':' + value;
  const getStorage = chain.api.getStorage.bind(chain.api);
  const getKeys = chain.api.getKeysPaged.bind(chain.api);
  const batch = async (keys, hash) => {
    if (!keys.length) return;
    try {
      const result = await chain.api.send('state_queryStorageAt', [keys, hash], true);
      const changes = new Map(result[0]?.changes || []);
      for (const key of keys) {
        assert(changes.has(key), 'Batch response omitted ' + key);
        cache.values[scoped(hash, key)] = changes.get(key);
        report.readCache.fetchedValues++;
      }
    } catch (error) {
      if (keys.length < 2 || !(error.code === -32008 || /Response is too big|Exceeded max limit/.test(error.message))) throw error;
      const middle = Math.floor(keys.length / 2);
      await batch(keys.slice(0, middle), hash);
      await batch(keys.slice(middle), hash);
    }
  };
  chain.api.getStorage = async (key, hash) => {
    if (!hash) return getStorage(key, hash);
    const lookup = scoped(hash, key);
    if (Object.hasOwn(cache.values, lookup)) {
      report.readCache.storageHits++;
      return cache.values[lookup];
    }
    const value = await getStorage(key, hash);
    cache.values[lookup] = value ?? null;
    report.readCache.fetchedValues++;
    return value;
  };
  chain.api.getKeysPaged = async (prefix, count, start, hash) => {
    if (!hash || prefix.startsWith('0x3a6368696c645f73746f726167653a')) return getKeys(prefix, count, start, hash);
    const ranges = cache.keyRanges[scoped(hash, prefix)] ||= [];
    const normalized = start || prefix;
    for (const range of ranges) {
      if (normalized < range.start || (!range.complete && normalized > range.end)) continue;
      const remaining = range.keys.filter(key => key > normalized);
      if (remaining.length >= count || range.complete) {
        report.readCache.keyPageHits++;
        return remaining.slice(0, count);
      }
    }
    const fetchCount = prefix.length < 66 ? count : Math.max(1000, count);
    const keys = await getKeys(prefix, fetchCount, normalized, hash);
    const missing = keys.filter(key => !Object.hasOwn(cache.values, scoped(hash, key)));
    for (let i = 0; i < missing.length; i += 256) await batch(missing.slice(i, i + 256), hash);
    ranges.push({ start: normalized, end: keys.at(-1) || normalized, complete: keys.length < fetchCount, keys });
    report.readCache.fetchedKeyPages++;
    return keys.slice(0, count);
  };
}
async function exposureCounts(block, expectedContext) {
  const meta = await block.meta;
  const overviews = meta.query.staking.erasStakersOverview;
  const eras = [];
  for (const era of expectedContext.eras) {
    const legacyPrefix = u8aToHex(meta.query.staking.erasStakers.keyPrefix(era));
    const legacy = await block.getKeysPaged({ prefix: legacyPrefix, startKey: legacyPrefix, pageSize: 1 });
    assert.equal(legacy.length, 0, 'Legacy exposure keys must prevent cache seeding');
    const prefix = u8aToHex(overviews.keyPrefix(era));
    let startKey = prefix;
    let validators = 0;
    let nominationEdges = 0;
    for (;;) {
      const keys = await block.getKeysPaged({ prefix, startKey, pageSize: 128 });
      for (const keyHex of keys) {
        const key = new StorageKey(meta.registry, keyHex);
        key.setMeta(overviews.meta);
        const raw = await block.get(keyHex);
        const overview = meta.registry.createType(key.outputType, hexToU8a(raw));
        nominationEdges += overview.nominatorCount.toNumber();
        validators++;
        assert(validators <= 4096, 'Independent scan is bounded');
      }
      if (keys.length < 128) break;
      startKey = keys.at(-1);
    }
    eras.push({ era, validators, nominationEdges });
  }
  return {
    eras, overviewEntries: eras.reduce((sum, era) => sum + era.validators, 0),
    maxNominationEdges: Math.max(...eras.map(era => era.nominationEdges)),
    maxValidators: Math.max(...eras.map(era => era.validators)),
  };
}
async function main() {
  progress('Loading pinned archive state');
  chain = await setup({
    endpoint: opts.endpoint, block: opts.block,
    registeredTypes: { typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } },
    buildBlockMode: BuildBlockMode.Manual, mockSignatureHost: true,
    allowUnresolvedImports: true, offchainWorker: false, processQueuedMessages: false,
    saveBlocks: false, runtimeLogLevel: 3, rpcTimeout: 120000,
  });
  const readonlyMethods = new Set([
    'chain_getBlock', 'chain_getBlockHash', 'chain_getHeader', 'chain_getFinalizedHead',
    'state_getStorage', 'state_getKeysPaged', 'state_queryStorageAt', 'state_getMetadata',
    'state_getRuntimeVersion', 'state_call', 'system_properties', 'system_chain',
    'system_name', 'system_version', 'rpc_methods',
  ]);
  const upstreamSend = chain.api.send.bind(chain.api);
  chain.api.send = async (method, ...args) => {
    assert(readonlyMethods.has(method), 'Upstream RPC must be explicitly read-only: ' + method);
    report.remoteRpcMethods[method] = (report.remoteRpcMethods[method] || 0) + 1;
    return upstreamSend(method, ...args);
  };
  installReadCache();
  const previous = chain.head;
  report.before = {
    number: previous.number, runtimeVersion: await previous.runtimeVersion,
    context: await context(previous),
    lastRuntimeUpgrade: (await rawQuery(previous, 'system', 'lastRuntimeUpgrade')).toJSON(),
  };
  assert.equal(report.before.runtimeVersion.specVersion, 130);
  const wasm = readFileSync(opts.wasm);
  report.candidate = {
    path: resolve(opts.wasm), bytes: wasm.length,
    sha256: createHash('sha256').update(wasm).digest('hex'),
  };
  previous.setWasm(u8aToHex(wasm));
  report.candidate.runtimeVersion = await previous.runtimeVersion;
  assert.equal(report.candidate.runtimeVersion.specVersion, 131);
  const header = await newHeader(previous);
  report.initializedHeader = header.toJSON();
  const initialized = new Block(chain, previous.number + 1, header.hash.toHex(), previous, {
    header, extrinsics: [], storage: previous.storage,
  });
  progress('Executing candidate Core_initialize_block and normal runtime-upgrade hooks');
  const response = await initialized.call('Core_initialize_block', [header.toHex()]);
  initialized.pushStorageLayer().setAll(response.storageDiff);
  report.initialize = {
    result: response.result, storageDiffEntries: response.storageDiff.length,
    runtimeLogs: response.runtimeLogs,
  };
  progress('Checking initialized Liveness cache against retained exposure metadata');
  const expectedContext = await context(initialized);
  const cache = await rawQuery(initialized, 'liveness', 'exposureWork');
  const history = await rawQuery(initialized, 'liveness', 'disabledDuringSession');
  const upgrade = await rawQuery(initialized, 'system', 'lastRuntimeUpgrade');
  report.after = {
    number: initialized.number, lastRuntimeUpgrade: upgrade.toJSON(),
    context: expectedContext, exposureWork: cache.toJSON(), disabledDuringSession: history.toJSON(),
  };
  assert.equal(upgrade.unwrap().specVersion.toNumber(), 131, 'Actual runtime-upgrade detection must advance');
  assert(cache.isSome, 'Bounded runtime-upgrade hook must seed equivocation exposure work');
  const cached = cache.unwrap().toJSON();
  assert.deepEqual(cached.context, expectedContext, 'Exposure work must carry the exact current context');
  report.independentExposureCounts = await exposureCounts(initialized, expectedContext);
  assert.equal(cached.maxNominationEdges, report.independentExposureCounts.maxNominationEdges);
  assert.equal(cached.maxValidators, report.independentExposureCounts.maxValidators);
  assert.equal(expectedContext.session, report.before.context.session, 'Pinned fixture must not rotate session');
  assert(history.isNone, 'An incomplete upgrade session must retain offline-penalty grace');
  report.checks = {
    runtimeUpgradeExecuted: true, exposureCacheSeeded: true, cacheContextFresh: true,
    exposureCountsMatchIndependentScan: true, incompleteSessionGracePreserved: true,
  };
  report.status = 'passed';
}
main().catch(error => {
  report.status = 'failed';
  report.error = { message: error.message, stack: error.stack };
  process.exitCode = 1;
  console.error(error);
}).finally(async () => {
  report.finishedAt = new Date().toISOString();
  save();
  if (chain) await chain.close().catch(() => {});
  await destroyWorker().catch(() => {});
  console.log(JSON.stringify({ status: report.status, output: opts.output, error: report.error?.message }));
});
