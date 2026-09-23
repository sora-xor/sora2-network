/* Local Wasm execution only. No chain writes, private keys, or signed blocks.
 * Usage: node rehearse-babe-repair.cjs --wasm /absolute/candidate.wasm
 */
'use strict';
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync, existsSync } = require('node:fs');
const { createRequire } = require('node:module');
const { createHash } = require('node:crypto');
const { gunzipSync, gzipSync } = require('node:zlib');
const { dirname, resolve } = require('node:path');
const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
const repositoryRoot = resolve(__dirname, '../..');
const localManifest = resolve(__dirname, 'package.json');
const dependencyManifest = resolve(option('--dependencies', process.env.SORA_REPAIR_DEPENDENCIES
  || (existsSync(localManifest) ? localManifest : '/tmp/sora-author-replay-20260922/package.json')));
const deps = createRequire(dependencyManifest);
const expectedDependencies = {
  '@acala-network/chopsticks-core': '1.5.1',
  '@babel/runtime': '7.28.4',
  '@polkadot/api': '16.5.6',
  '@polkadot/keyring': '14.0.3',
  '@sora-substrate/type-definitions': '1.27.7',
};
const installedDependencies = {};
for (const [name, version] of Object.entries(expectedDependencies)) {
  const manifest = resolve(dirname(dependencyManifest), 'node_modules', name, 'package.json');
  installedDependencies[name] = JSON.parse(readFileSync(manifest)).version;
  assert.equal(installedDependencies[name], version, 'Unexpected installed dependency version: ' + name);
}
const { StorageKey, GenericExtrinsic } = deps('@polkadot/types');
const { hexToU8a, u8aToHex, compactAddLength } = deps('@polkadot/util');
const { xxhashAsHex, blake2AsHex, cryptoWaitReady } = deps('@polkadot/util-crypto');
const { setup, BuildBlockMode, Block, destroyWorker } = deps('@acala-network/chopsticks-core');
const { fullOverrideBundle } = deps('@sora-substrate/type-definitions');
const snapshotPath = resolve(option('--snapshot', resolve(__dirname, 'snapshot.json')));
const snapshot = JSON.parse(readFileSync(snapshotPath));
const opts = {
  endpoint: option('--endpoint', 'https://mof2.sora.org'),
  wasm: resolve(option('--wasm', resolve(repositoryRoot, 'runtime-upgrade-4.8.10/framenode-runtime-4.8.10.compact.compressed.wasm'))),
  cache: resolve(option('--read-cache', '/tmp/sora-babe-repair-20260922-read-cache.json.gz')),
  cacheOutput: resolve(option('--write-cache', '/tmp/sora-babe-repair-20260922-read-cache.json.gz')),
  output: resolve(option('--output', resolve(__dirname, 'babe-repair-wasm-rehearsal.json'))),
  epochOutput: resolve(option('--epoch-output', resolve(__dirname, 'babe-repair-runtime-epochs.json'))),
};
const VRF = 'PrimaryAndSecondaryVRFSlots';
const PLAIN = 'PrimaryAndSecondaryPlainSlots';
const report = {
  startedAt: new Date().toISOString(), status: 'running', localOnly: true,
  endpoint: opts.endpoint, pinnedBlockHash: snapshot.blockHash, pinnedBlockNumber: snapshot.blockNumber,
  executor: '@acala-network/chopsticks-core@1.5.1', dependencyManifest,
  mockSignatureHost: true, allowUnresolvedImports: true, offchainWorker: false,
  submittedTransactions: 0, privateKeysRead: 0, signedExtrinsics: 0, remoteRpcMethods: {},
  constraints: [
    'The only manually overlaid chain storage is local :code. All subsequent changes come from actual candidate Wasm runtime calls.',
    'Three consecutive local block numbers use increasing slots: a mid-epoch upgrade, then the next two epoch boundaries. Intervening slots are empty; this is not a replay of their historical production.',
    'Synthetic unsealed SecondaryPlain pre-digests select the correct secondary authority, but no real block signature, VRF, client import, or network consensus is tested.',
    'The initial and first-boundary runtime storage still advertises VRF; these synthetic Plain headers deliberately exercise runtime execution, not consensus validity.',
    'Mock signatures and unresolved-import support are executor settings. An invoked unsupported host import is a failure; no runtime guard is bypassed.',
    'Remote requests are restricted to read-only RPC. Local timestamp inherents are executed, but never signed or submitted.',
    'Skipping intervening production and election preparation can leave an election snapshot unavailable. This run verifies BABE transitions, not a complete validator election.',
  ], blocks: [],
};
let chain;
let cache;
let markerKey;
const save = () => writeFileSync(opts.output, JSON.stringify(report, null, 2) + '\n');
const progress = stage => { report.stage = stage; save(); console.log(new Date().toISOString(), stage); };
const sha256 = value => createHash('sha256').update(value).digest('hex');

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

function installReadCache() {
  cache = existsSync(opts.cache)
    ? JSON.parse(gunzipSync(readFileSync(opts.cache)).toString())
    : { format: 1, values: {}, keyRanges: {} };
  assert.equal(cache.format, 1);
  report.readCache = {
    path: opts.cache, outputPath: opts.cacheOutput,
    content: 'Block-hash-scoped public upstream storage reads only',
    existingValues: Object.keys(cache.values).length, storageHits: 0, keyPageHits: 0,
    fetchedValues: 0, fetchedKeyPages: 0,
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

async function babeApis(block) {
  const registry = await block.registry;
  registry.register({
    RehearsalBabeConfiguration: {
      slotDuration: 'u64', epochLength: 'u64', c: '(u64,u64)',
      authorities: 'Vec<(AuthorityId,u64)>', randomness: 'H256', allowedSlots: 'AllowedSlots',
    },
    RehearsalEpoch: {
      epochIndex: 'u64', startSlot: 'u64', duration: 'u64',
      authorities: 'Vec<(AuthorityId,u64)>', randomness: 'H256', config: 'BabeEpochConfiguration',
    },
    RehearsalNextEpochDescriptor: {
      authorities: 'Vec<(AuthorityId,u64)>', randomness: 'H256',
    },
  });
  const out = {};
  for (const [label, method, type] of [
    ['configuration', 'BabeApi_configuration', 'RehearsalBabeConfiguration'],
    ['currentEpoch', 'BabeApi_current_epoch', 'RehearsalEpoch'],
    ['nextEpoch', 'BabeApi_next_epoch', 'RehearsalEpoch'],
  ]) {
    const response = await block.call(method, []);
    assert.equal(response.storageDiff.length, 0, method + ' must be read-only');
    const value = registry.createType(type, response.result);
    assert.equal(value.toHex(), response.result, 'Decode must consume exact SCALE bytes');
    out[label] = { raw: response.result, value: value.toJSON() };
  }
  return out;
}

async function state(block) {
  const out = { markerRaw: await block.get(markerKey) ?? null, storage: {}, runtimeApi: await babeApis(block) };
  for (const [pallet, name] of [
    ['babe', 'epochConfig'], ['babe', 'nextEpochConfig'], ['babe', 'pendingEpochConfigChange'],
    ['babe', 'epochIndex'], ['babe', 'currentSlot'], ['session', 'currentIndex'],
    ['session', 'disabledValidators'], ['system', 'lastRuntimeUpgrade'],
  ]) out.storage[pallet + '.' + name] = (await rawQuery(block, pallet, name)).toJSON();
  return out;
}

function secondaryIndex(epoch, slot) {
  const encodedSlot = Buffer.alloc(8);
  encodedSlot.writeBigUInt64LE(BigInt(slot));
  const hash = blake2AsHex(Buffer.concat([Buffer.from(epoch.randomness.slice(2), 'hex'), encodedSlot]), 256);
  return Number(BigInt(hash) % BigInt(epoch.authorities.length));
}

function configIs(value, allowedSlots) {
  assert.deepEqual(value, { c: [1, 4], allowedSlots });
}

function babeConsensus(header) {
  return header.digest.logs
    .filter(item => item.isConsensus && item.asConsensus[0].toHex() === '0x42414245')
    .map(item => {
      const bytes = Buffer.from(item.asConsensus[1].toU8a(true));
      return { variant: bytes[0], raw: '0x' + bytes.toString('hex') };
    });
}

async function checkNextEpochAnnouncement(block, row) {
  const registry = await block.registry;
  const announcements = row.finalizedConsensus.filter(item => item.variant === 1);
  assert.equal(announcements.length, 1, 'Exactly one NextEpochData announcement is required');
  const raw = '0x' + announcements[0].raw.slice(4);
  const descriptor = registry.createType('RehearsalNextEpochDescriptor', raw);
  assert.equal(descriptor.toHex(), raw, 'NextEpochData must decode without trailing bytes');
  const next = row.afterFinalize.runtimeApi.nextEpoch.value;
  assert.deepEqual(descriptor.toJSON(), { authorities: next.authorities, randomness: next.randomness },
    'NextEpochData must advertise the same authorities and randomness as the next-epoch API');
}

async function executeBlock(parent, slot, label, expectedEpoch) {
  const registry = await parent.registry;
  const meta = await parent.meta;
  const index = secondaryIndex(expectedEpoch, slot);
  const predigest = registry.createType('RawBabePreDigest', { SecondaryPlain: { authorityIndex: index, slotNumber: slot } });
  const digest = registry.createType('DigestItem', { PreRuntime: ['BABE', compactAddLength(predigest.toU8a())] });
  const header = registry.createType('Header', {
    parentHash: parent.hash, number: parent.number + 1, stateRoot: '0x' + '00'.repeat(32),
    extrinsicsRoot: '0x' + '00'.repeat(32), digest: { logs: [digest] },
  });
  const block = new Block(chain, parent.number + 1, header.hash.toHex(), parent, { header, extrinsics: [], storage: parent.storage });
  const row = { label, number: block.number, slot, authorityIndex: index, status: 'running', initializedHeader: header.toJSON() };
  report.blocks.push(row);
  progress('Executing ' + label + ' Core_initialize_block');
  let response = await block.call('Core_initialize_block', [header.toHex()]);
  block.pushStorageLayer().setAll(response.storageDiff);
  row.initialize = { result: response.result, storageDiffEntries: response.storageDiff.length, runtimeLogs: response.runtimeLogs };
  row.afterInitialize = await state(block);
  row.consensusAtInitialize = babeConsensus({ digest: await rawQuery(block, 'system', 'digest') });
  save();
  const timestamp = slot * 6000;
  const inherent = new GenericExtrinsic(registry, meta.tx.timestamp.set(timestamp)).toHex();
  response = await block.call('BlockBuilder_apply_extrinsic', [inherent]);
  block.pushStorageLayer().setAll(response.storageDiff);
  const outcome = registry.createType('ApplyExtrinsicResult', response.result);
  row.timestamp = { timestamp, outcome: outcome.toJSON(), storageDiffEntries: response.storageDiff.length, runtimeLogs: response.runtimeLogs };
  assert(outcome.isOk && outcome.asOk.isOk, 'Timestamp inherent must succeed');
  progress('Finalizing ' + label);
  response = await block.call('BlockBuilder_finalize_block', []);
  block.pushStorageLayer().setAll(response.storageDiff);
  const finalizedHeader = registry.createType('Header', response.result);
  row.finalize = { header: finalizedHeader.toJSON(), headerHex: finalizedHeader.toHex(), hash: finalizedHeader.hash.toHex(), storageDiffEntries: response.storageDiff.length, runtimeLogs: response.runtimeLogs };
  row.finalizedConsensus = babeConsensus(finalizedHeader);
  row.afterFinalize = await state(block);
  assert.deepEqual(row.afterFinalize, row.afterInitialize, 'BABE transition must survive finalization unchanged');
  row.status = 'passed';
  save();
  return new Block(chain, block.number, finalizedHeader.hash.toHex(), parent, { header: finalizedHeader, extrinsics: [inherent], storage: block.storage });
}

async function main() {
  assert(existsSync(opts.wasm), 'Candidate Wasm does not exist: ' + opts.wasm);
  await cryptoWaitReady();
  assert.equal(secondaryIndex({ randomness: '0x' + '03'.repeat(32), authorities: Array(1000) }, 100), 167,
    'Secondary assignment must match the pinned SDK test vector');
  markerKey = xxhashAsHex('Babe', 128) + xxhashAsHex('MainnetPlainConfigRepairV1', 128).slice(2);
  progress('Loading pinned deployed state through read-only RPC');
  chain = await setup({ endpoint: opts.endpoint, block: snapshot.blockHash,
    registeredTypes: { typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } },
    buildBlockMode: BuildBlockMode.Manual, mockSignatureHost: true, allowUnresolvedImports: true,
    offchainWorker: false, processQueuedMessages: false, saveBlocks: false, runtimeLogLevel: 3, rpcTimeout: 60000 });
  const methods = new Set(['chain_getBlock', 'chain_getBlockHash', 'chain_getHeader', 'chain_getFinalizedHead',
    'state_getStorage', 'state_getKeysPaged', 'state_queryStorageAt', 'state_getMetadata', 'state_getRuntimeVersion',
    'state_call', 'system_properties', 'system_chain', 'system_name', 'system_version', 'rpc_methods']);
  const send = chain.api.send.bind(chain.api);
  chain.api.send = async (method, ...params) => {
    assert(methods.has(method), 'Only read-only RPC allowed: ' + method);
    report.remoteRpcMethods[method] = (report.remoteRpcMethods[method] || 0) + 1;
    return send(method, ...params);
  };
  installReadCache();
  let parent = chain.head;
  assert.equal(parent.number, snapshot.blockNumber);
  assert.equal((await rawQuery(parent, 'system', 'blockHash', [0])).toHex(), snapshot.genesis);
  report.deployedVersion = await parent.runtimeVersion;
  const lockPath = resolve(dependencyManifest, '..', 'package-lock.json');
  report.dependencies = {
    manifest: JSON.parse(readFileSync(dependencyManifest)),
    installed: installedDependencies,
    lockSha256: existsSync(lockPath) ? sha256(readFileSync(lockPath)) : null,
  };
  assert.equal(report.deployedVersion.specVersion, 131);
  report.deployedWasmSha256 = sha256(hexToU8a(await parent.get('0x3a636f6465')));
  assert.equal(report.deployedWasmSha256, 'db948406c5f22d4923b2760019de53bcd0ef756ed05accaf5156ca3041988447');
  report.before = await state(parent);
  assert.equal(report.before.markerRaw, null);
  configIs(report.before.storage['babe.epochConfig'], VRF);
  configIs(report.before.storage['babe.nextEpochConfig'], VRF);
  assert.equal(report.before.storage['babe.pendingEpochConfigChange'], null);
  const originalEpoch = report.before.runtimeApi.currentEpoch.value;
  const firstBoundary = originalEpoch.startSlot + originalEpoch.duration;
  const currentSlot = report.before.storage['babe.currentSlot'];
  assert(currentSlot + 1 < firstBoundary, 'Upgrade fixture must be inside an epoch');
  const wasm = readFileSync(opts.wasm);
  report.candidate = { path: opts.wasm, bytes: wasm.length, sha256: sha256(wasm) };
  parent.setWasm(u8aToHex(wasm));
  report.candidate.runtimeVersion = await parent.runtimeVersion;
  assert.equal(report.candidate.runtimeVersion.specVersion, 132);
  assert.equal(report.candidate.runtimeVersion.transactionVersion, 131);
  parent = await executeBlock(parent, currentSlot + 1, 'mid-epoch-upgrade', originalEpoch);
  const afterUpgrade = report.blocks.at(-1).afterFinalize;
  assert.equal(afterUpgrade.markerRaw, '0x01');
  assert.equal(afterUpgrade.storage['system.lastRuntimeUpgrade'].specVersion, 132);
  configIs(afterUpgrade.storage['babe.epochConfig'], VRF);
  configIs(afterUpgrade.storage['babe.nextEpochConfig'], VRF);
  assert.deepEqual(afterUpgrade.runtimeApi, report.before.runtimeApi,
    'Scheduling the repair must preserve all three epoch API payloads until the first boundary');
  assert.deepEqual(afterUpgrade.storage['babe.pendingEpochConfigChange'], { v1: { c: [1, 4], allowedSlots: PLAIN } });
  assert.equal(report.blocks.at(-1).finalizedConsensus.length, 0, 'No mid-epoch BABE consensus announcement');
  parent = await executeBlock(parent, firstBoundary, 'first-epoch-boundary', afterUpgrade.runtimeApi.nextEpoch.value);
  const first = report.blocks.at(-1);
  configIs(first.afterFinalize.storage['babe.epochConfig'], VRF);
  configIs(first.afterFinalize.storage['babe.nextEpochConfig'], PLAIN);
  assert.equal(first.afterFinalize.storage['babe.pendingEpochConfigChange'], null);
  assert.equal(first.afterFinalize.runtimeApi.currentEpoch.value.epochIndex, originalEpoch.epochIndex + 1);
  assert.deepEqual(first.afterFinalize.runtimeApi.currentEpoch, afterUpgrade.runtimeApi.nextEpoch,
    'The first boundary must activate exactly the previously advertised next epoch');
  await checkNextEpochAnnouncement(parent, first);
  assert.deepEqual(first.finalizedConsensus.filter(x => x.variant === 3), [{ variant: 3, raw: '0x03010100000000000000040000000000000001' }]);
  parent = await executeBlock(parent, firstBoundary + originalEpoch.duration, 'second-epoch-boundary', first.afterFinalize.runtimeApi.nextEpoch.value);
  const second = report.blocks.at(-1);
  assert.equal(second.afterFinalize.runtimeApi.currentEpoch.value.epochIndex, originalEpoch.epochIndex + 2);
  assert.deepEqual(second.afterFinalize.runtimeApi.currentEpoch, first.afterFinalize.runtimeApi.nextEpoch,
    'The second boundary must activate exactly the previously advertised next epoch');
  configIs(second.afterFinalize.storage['babe.epochConfig'], PLAIN);
  configIs(second.afterFinalize.storage['babe.nextEpochConfig'], PLAIN);
  assert.equal(second.afterFinalize.storage['babe.pendingEpochConfigChange'], null);
  await checkNextEpochAnnouncement(parent, second);
  assert.equal(second.finalizedConsensus.filter(x => x.variant === 3).length, 0, 'Announcement must not repeat');
  for (const name of ['configuration', 'currentEpoch', 'nextEpoch']) {
    const value = second.afterFinalize.runtimeApi[name].value;
    configIs(name === 'configuration' ? { c: value.c, allowedSlots: value.allowedSlots } : value.config, PLAIN);
  }
  report.checks = {
    actualWasmUpgradeExecuted: true, markerSet: true, activeAndNextInitiallyPreserved: true,
    firstBoundaryAnnouncedPlainExactlyOnce: true, secondBoundaryActivatedPlain: true,
    allThreeBabeApisAgree: true, allThreeTimestampsAndFinalizationsPassed: true,
    announcedEpochPayloadsMatchRuntimeApis: true, transactionVersionRemains131: true,
  };
  report.electionSnapshotUnavailable = report.blocks.some(row =>
    row.initialize.runtimeLogs.some(entry => entry.message.includes('SnapshotUnavailable')));
  report.inputs = {
    scriptPath: __filename, scriptSha256: sha256(readFileSync(__filename)),
    snapshotPath, snapshotSha256: sha256(readFileSync(snapshotPath)),
  };
  writeFileSync(opts.epochOutput, JSON.stringify({
    localOnly: true, sourcePinnedBlockHash: snapshot.blockHash, candidate: report.candidate,
    syntheticHeadersAreUnsealed: true,
    stages: [{ label: 'before', ...report.before.runtimeApi }, ...report.blocks.map(row => ({
      label: row.label, number: row.number, slot: row.slot, header: row.finalize.header,
      headerHex: row.finalize.headerHex, ...row.afterFinalize.runtimeApi,
    }))],
  }, null, 2) + '\n');
  report.epochOutput = opts.epochOutput;
  report.status = 'passed';
}

const timeout = setTimeout(() => { report.status = 'timed_out'; save(); process.exit(2); }, 900000);
main().catch(error => {
  report.status = 'failed'; report.error = { message: error.message, stack: error.stack };
  process.exitCode = 1; console.error(error);
}).finally(async () => {
  clearTimeout(timeout);
  if (cache) {
    const bytes = gzipSync(JSON.stringify(cache));
    writeFileSync(opts.cacheOutput, bytes);
    report.readCache.savedBytes = bytes.length;
    report.readCache.sha256 = sha256(bytes);
  }
  report.finishedAt = new Date().toISOString(); save();
  if (chain) await chain.close().catch(() => {});
  await destroyWorker().catch(() => {});
  console.log(JSON.stringify({ status: report.status, output: opts.output, error: report.error?.message }));
});
