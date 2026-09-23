/* Local runtime author execution replay. Never submits transactions.
 * Synthetic SecondaryPlain digests avoid requiring a validator's private VRF key.
 * This checks runtime execution only, not BABE leader eligibility, seals or consensus.
 */
'use strict';
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync, existsSync } = require('node:fs');
const { createRequire } = require('node:module');
const { createHash } = require('node:crypto');
const { gunzipSync } = require('node:zlib');
const { resolve } = require('node:path');
const deps = createRequire('/tmp/sora-author-replay-20260922/package.json');
const { StorageKey, GenericExtrinsic } = deps('@polkadot/types');
const { hexToU8a, compactAddLength } = deps('@polkadot/util');
const { setup, BuildBlockMode, Block, destroyWorker } = deps('@acala-network/chopsticks-core');
const { fullOverrideBundle } = deps('@sora-substrate/type-definitions');
const snapshot = JSON.parse(readFileSync(resolve(__dirname, 'snapshot.json')));
const opts = { cache: resolve(__dirname, 'nonexistent-author-replay-cache.json.gz') };
const output = resolve(__dirname, 'author-wasm-replay.json');
const report = {
 startedAt: new Date().toISOString(), status: 'running', localOnly: true,
 endpoint: 'https://mof2.sora.org', pinnedBlockHash: snapshot.blockHash,
 pinnedBlockNumber: snapshot.blockNumber, executor: '@acala-network/chopsticks-core@1.5.1',
 expectedWasmSha256: 'db948406c5f22d4923b2760019de53bcd0ef756ed05accaf5156ca3041988447',
 mockSignatureHost: true, allowUnresolvedImports: true, offchainWorker: false,
 submittedTransactions: 0, privateKeysRead: 0, signedExtrinsics: 0, remoteRpcMethods: {},
 constraints: [
  'Each author runs independently from the same pinned finalized state; only one local child block per author.',
  'Synthetic SecondaryPlain BABE pre-digest; the live epoch permits PrimaryAndSecondaryVRFSlots, so these synthetic headers are NOT consensus-valid blocks.',
  'Checks Core_initialize_block, one unsigned timestamp inherent, and BlockBuilder_finalize_block; does not validate slot eligibility, VRF, block seal, node keystore, network propagation or import.',
  'Executor mock signatures and allowUnresolvedImports match the prior committed rehearsal; unsupported invoked host imports are failures, not permission to bypass them.',
  'No code, balances, disablements, session keys or other chain storage are manually overlaid. Only normal local runtime call storage diffs are applied.'
 ], cases: []
};
let chain;
const save = () => writeFileSync(output, JSON.stringify(report, null, 2) + '\n');
const progress = stage => {report.stage=stage;save();console.log(new Date().toISOString(),stage);};
async function rawQuery(block, pallet, entry, args=[]) {
 const meta=await block.meta; const query=meta.query[pallet][entry];
 const key=new StorageKey(meta.registry,[query,args]); const raw=await block.get(key.toHex());
 if(query.meta.modifier.isOptional) return meta.registry.createType('Option<'+key.outputType+'>',hexToU8a(raw?'0x01'+raw.slice(2):'0x00'));
 return meta.registry.createType(key.outputType,raw?hexToU8a(raw):undefined);
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

async function main() {
 progress('Loading pinned deployed runtime and read-only state');
 chain=await setup({endpoint:report.endpoint,block:report.pinnedBlockHash,
  registeredTypes:{typesBundle:{spec:{'sora-substrate':fullOverrideBundle.spec.sora}}},
  buildBlockMode:BuildBlockMode.Manual,mockSignatureHost:true,allowUnresolvedImports:true,
  offchainWorker:false,processQueuedMessages:false,saveBlocks:false,runtimeLogLevel:3,rpcTimeout:60000});
 const methods=new Set(['chain_getBlock','chain_getBlockHash','chain_getHeader','chain_getFinalizedHead','state_getStorage','state_getKeysPaged','state_queryStorageAt','state_getMetadata','state_getRuntimeVersion','state_call','system_properties','system_chain','system_name','system_version','rpc_methods']);
 const send=chain.api.send.bind(chain.api);
 chain.api.send=async(method,...args)=>{assert(methods.has(method),'Only read-only RPC allowed: '+method);report.remoteRpcMethods[method]=(report.remoteRpcMethods[method]||0)+1;return send(method,...args);};
 installReadCache();
 const parent=chain.head;
 assert.equal(parent.number,snapshot.blockNumber);
 const wasm=hexToU8a(await parent.get('0x3a636f6465'));
 report.wasmSha256=createHash('sha256').update(wasm).digest('hex');
 assert.equal(report.wasmSha256,report.expectedWasmSha256);
 report.runtimeVersion=await parent.runtimeVersion;
 assert.equal(report.runtimeVersion.specVersion,131);
 const meta=await parent.meta;
 const registry=meta.registry;
 const slot=Number(snapshot.storage['babe.currentSlot'])+1;
 const timestamp=Number(snapshot.storage['timestamp.now'])+6000;
 const session=snapshot.storage['session.currentIndex'];
 assert.equal((await rawQuery(parent,'babe','currentSlot')).toNumber(),slot-1);
 assert.equal((await rawQuery(parent,'timestamp','now')).toNumber(),timestamp-6000);
 report.parentDisabled=(await rawQuery(parent,'session','disabledValidators')).toJSON();
 report.parentHistory=(await rawQuery(parent,'liveness','disabledDuringSession')).toJSON();
 for(const index of [0,2,6,10,14]) {
  const stash=snapshot.validators[index].stash;
  const item={index,stash,role:index===0?'producing control':'missing validator',status:'running'};
  report.cases.push(item);
  progress('Local runtime block for authority index '+index);
  try {
   const predigest=registry.createType('RawBabePreDigest',{SecondaryPlain:{authorityIndex:index,slotNumber:slot}});
   const digest=registry.createType('DigestItem',{PreRuntime:['BABE',compactAddLength(predigest.toU8a())]});
   const header=registry.createType('Header',{parentHash:parent.hash,number:parent.number+1,stateRoot:'0x'+'00'.repeat(32),extrinsicsRoot:'0x'+'00'.repeat(32),digest:{logs:[digest]}});
   item.syntheticHeader=header.toJSON();
   const block=new Block(chain,parent.number+1,header.hash.toHex(),parent,{header,extrinsics:[],storage:parent.storage});
   const beforeAuthored=(await rawQuery(parent,'imOnline','authoredBlocks',[session,stash])).toNumber();
   let response=await block.call('Core_initialize_block',[header.toHex()]);
   block.pushStorageLayer().setAll(response.storageDiff);
   item.initialize={result:response.result,storageDiffEntries:response.storageDiff.length,runtimeLogs:response.runtimeLogs};
   item.resolvedAuthor=(await rawQuery(block,'authorship','author')).toJSON();
   item.authoredBefore=beforeAuthored;
   item.authoredAfterInitialize=(await rawQuery(block,'imOnline','authoredBlocks',[session,stash])).toNumber();
   assert.equal(item.resolvedAuthor,stash,'runtime must resolve the supplied author index');
   assert.equal(item.authoredAfterInitialize,beforeAuthored+1);
   assert.equal((await rawQuery(block,'session','currentIndex')).toNumber(),session);
   const inherent=new GenericExtrinsic(registry,meta.tx.timestamp.set(timestamp)).toHex();
   response=await block.call('BlockBuilder_apply_extrinsic',[inherent]);
   block.pushStorageLayer().setAll(response.storageDiff);
   const outcome=registry.createType('ApplyExtrinsicResult',response.result);
   item.timestamp={outcome:outcome.toJSON(),storageDiffEntries:response.storageDiff.length,runtimeLogs:response.runtimeLogs};
   assert(outcome.isOk&&outcome.asOk.isOk,'timestamp inherent must dispatch successfully');
   response=await block.call('BlockBuilder_finalize_block',[]);
   block.pushStorageLayer().setAll(response.storageDiff);
   item.finalize={header:registry.createType('Header',response.result).toJSON(),storageDiffEntries:response.storageDiff.length,runtimeLogs:response.runtimeLogs};
   item.disabledAfter=(await rawQuery(block,'session','disabledValidators')).toJSON();
   item.status='passed';
  } catch(error) {
   item.status='failed';item.error={message:error.message,stack:error.stack};save();
   throw error;
  }
  save();
 }
 report.status='passed';
}
const timeout=setTimeout(()=>{report.status='timed_out';save();process.exit(2);},600000);
main().catch(error=>{report.status='failed';report.error={message:error.message,stack:error.stack};process.exitCode=1;console.error(error);}).finally(async()=>{
 clearTimeout(timeout);report.finishedAt=new Date().toISOString();save();
 if(chain)await chain.close().catch(()=>{});await destroyWorker().catch(()=>{});
 console.log(JSON.stringify({status:report.status,output,cases:report.cases.map(({index,status,error})=>({index,status,error:error?.message}))}));
});
