import assert from 'node:assert/strict';
import {test} from 'node:test';
import {mkdtemp, readFile, readdir, rm, unlink} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {captureBurnHistory, nativeProofSession, validateProof, validateBlock, EVENTS_KEY, CODE_PREFIX_KEY} from './capture_burn_history.mjs';
import {CHECKPOINT, sha256} from './capture_checkpoint.mjs';
import {gunzipSync} from 'node:zlib';

const h = n => `0x${n.toString(16).padStart(64, '0')}`;
test('storage proofs reject absent/wrong-block/duplicate evidence rather than treating it as no events', () => {
  for (const proof of [null, {at:h(1),proof:['0x00']}, {at:CHECKPOINT.hash,proof:[]},
    {at:CHECKPOINT.hash,proof:['0x00','0x00']}, {at:CHECKPOINT.hash,proof:['0xA0']}]) {
    assert.throws(() => validateProof(proof, CHECKPOINT.hash));
  }
  assert.deepEqual(validateProof({at:CHECKPOINT.hash, proof:['0x00']}, CHECKPOINT.hash), [Buffer.from([0])]);
  assert.throws(() => validateBlock({block:{header:{number:CHECKPOINT.number,parentHash:h(1)},extrinsics:[]}}, CHECKPOINT.number, CHECKPOINT.hash, h(2)));
});

test('incremental capture requests events and raw colon path while preserving explicit cached code references', async () => {
  const dir = await mkdtemp(join(tmpdir(),'sora-nexus-cached-burn-'));
  const reads = [], checks = [];
  const code = sha256(Buffer.from([1])), event = sha256(Buffer.from([2]));
  const rpc = {endpoint:'ws://localhost/',async call(method,params) {
    if (method==='chain_getBlockHash') return params[0]===CHECKPOINT.number ? CHECKPOINT.hash : CHECKPOINT.parentHash;
    if (method==='chain_getHeader') return {number:CHECKPOINT.number-1,parentHash:h(1)};
    if (method==='chain_getBlock') return {block:{header:{number:CHECKPOINT.number,parentHash:CHECKPOINT.parentHash},extrinsics:[]}};
    if (method==='state_getReadProof') {reads.push(params[0]);return {at:params[1],proof:params[0].includes(EVENTS_KEY)?['0x02']:['0x01']};}
    throw new Error('Unexpected RPC');
  }};
  const proofVerifier={async verify(request) {checks.push(request);return {verified:true,proof:request.initial?[code]:[code,event]};}};
  try {
    const options={output:dir,first:CHECKPOINT.number,maxPages:1,rpc,proofVerifier};
    assert.equal((await captureBurnHistory(options)).complete,true);
    assert.deepEqual(reads,[['0x3a636f6465'],[EVENTS_KEY,CODE_PREFIX_KEY]]);
    assert.equal(checks[0].initial,true);assert.equal(checks[1].initial,false);
    const page=JSON.parse(gunzipSync(await readFile(join(dir,'pages',`${CHECKPOINT.number}.json.gz`))));
    assert.deepEqual(page.records[0].proof,[code,event]);
    checks.length=0;reads.length=0;await captureBurnHistory(options);
    assert.deepEqual(checks,[]);assert.deepEqual(reads,[]);
  } finally {await rm(dir,{recursive:true,force:true});}
});

test('missing current-root code path requires a full retry even without an upgrade digest', async () => {
  for (const fullSucceeds of [true,false]) {
    const dir=await mkdtemp(join(tmpdir(),'sora-nexus-burn-fallback-'));const reads=[];let checked=0;
    const rpc={endpoint:'ws://localhost/',async call(method,params) {
      if(method==='chain_getBlockHash')return params[0]===CHECKPOINT.number?CHECKPOINT.hash:CHECKPOINT.parentHash;
      if(method==='chain_getHeader')return {number:CHECKPOINT.number-1,parentHash:h(1)};
      if(method==='chain_getBlock')return {block:{header:{number:CHECKPOINT.number,parentHash:CHECKPOINT.parentHash,digest:{logs:[]}},extrinsics:[]}};
      if(method==='state_getReadProof'){reads.push(params[0]);return {at:params[1],proof:params[0].length===2&&params[0].includes('0x3a636f6465')?['0x02','0x03']:['0x01']};}
      throw new Error('Unexpected RPC');
    }};
    const proofVerifier={async verify(request){
      if(request.initial)return {verified:true,proof:request.proof};
      checked+=1;
      return checked===1||!fullSucceeds?{verified:false,needFullProof:true}:{verified:true,proof:request.proof};
    }};
    try {
      const operation=captureBurnHistory({output:dir,first:CHECKPOINT.number,maxPages:1,rpc,proofVerifier});
      if(fullSucceeds)assert.equal((await operation).complete,true);
      else {await assert.rejects(operation,/remains incomplete/);await assert.rejects(readFile(join(dir,'burn-history-manifest.json')),/ENOENT/);}
      assert.deepEqual(reads,[['0x3a636f6465'],[EVENTS_KEY,CODE_PREFIX_KEY],[EVENTS_KEY,'0x3a636f6465']]);
      assert.equal(checked,2);
    } finally {await rm(dir,{recursive:true,force:true});}
  }
});

test('incremental helper requires an explicitly pinned binary', async () => {
  await assert.rejects(nativeProofSession('/does/not/exist',undefined,'/tmp'),/SHA-256 is required/);
  await assert.rejects(captureBurnHistory({output:'/tmp',verifier:'/does/not/exist'}),/required together/);
});
test('repairing a page hole authenticates later cached pages before downloading again', async () => {
  const dir = await mkdtemp(join(tmpdir(),'sora-nexus-burn-hole-'));
  const first = CHECKPOINT.number - 47;
  const blockHash = n => n === CHECKPOINT.number ? CHECKPOINT.hash : h(n);
  const height = hash => hash === CHECKPOINT.hash ? CHECKPOINT.number : Number(BigInt(hash));
  let proofReads = 0;
  const rpc = {endpoint:'ws://localhost/',async call(method,params) {
    if(method==='chain_getBlockHash')return blockHash(params[0]);
    if(method==='chain_getHeader')return {number:height(params[0]),parentHash:blockHash(height(params[0])-1)};
    if(method==='chain_getBlock')return {block:{header:{number:height(params[0]),parentHash:blockHash(height(params[0])-1)},extrinsics:[]}};
    if(method==='state_getReadProof'){proofReads+=1;return {at:params[1],proof:['0x01']};}
    throw new Error('Unexpected RPC');
  }};
  try {
    await captureBurnHistory({output:dir,first,maxPages:3,rpc});
    await unlink(join(dir,'pages',`${first}.json.gz`));
    await unlink(join(dir,'pages',`${first+32}.json.gz`));
    await unlink(join(dir,'burn-history-manifest.json'));
    const cached = await readFile(join(dir,'pages',`${first+16}.json.gz`));
    let previous, checks=0;
    const proofVerifier={async verify(request) {
      assert.equal(request.initial,previous===undefined);
      if(previous!==undefined)assert.equal(request.header.number,previous+1);
      previous=request.header.number;checks+=1;
      return {verified:true,proof:request.proof};
    }};
    proofReads=0;
    const result=await captureBurnHistory({output:dir,first,maxPages:2,rpc,proofVerifier});
    assert.equal(result.complete,true);assert.equal(result.downloadedPages,2);
    assert.equal(proofReads,32);assert.equal(checks,49);
    assert.deepEqual(await readFile(join(dir,'pages',`${first+16}.json.gz`)),cached);
  } finally {await rm(dir,{recursive:true,force:true});}
});
test('bounded diagnostic capture resumes immutable pages and deduplicates code proof nodes', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'sora-nexus-burn-capture-'));
  const calls = [];
  const rpc = {endpoint:'ws://localhost/', async call(method, params) {
    calls.push([method,params]);
    if (method === 'chain_getBlockHash') return params[0] === CHECKPOINT.number ? CHECKPOINT.hash : CHECKPOINT.parentHash;
    if (method === 'chain_getHeader') return {number:CHECKPOINT.number-1,parentHash:h(1)};
    if (method === 'chain_getBlock') return {block:{header:{number:CHECKPOINT.number,parentHash:CHECKPOINT.parentHash},extrinsics:[]}};
    if (method === 'state_getReadProof') return {at:params[1],proof:params[0].includes(EVENTS_KEY) ? ['0x01','0x02'] : ['0x01']};
    throw new Error(`Unexpected read ${method}`);
  }};
  try {
    const options = {output:dir, first:CHECKPOINT.number, maxPages:1, rpc};
    const first = await captureBurnHistory(options);
    assert.equal(first.complete,true); assert.equal(first.coverageComplete,false);
    assert.equal(first.independentProofVerification,false);
    assert.equal((await readdir(join(dir,'objects'))).length,2);
    const manifest = await readFile(join(dir,'burn-history-manifest.json'));
    calls.length = 0;
    const second = await captureBurnHistory(options);
    assert.equal(second.downloadedPages,0);
    assert.deepEqual(calls.map(([method])=>method),['chain_getBlockHash']);
    assert.deepEqual(await readFile(join(dir,'burn-history-manifest.json')),manifest);
    await assert.rejects(captureBurnHistory({...options,maxPages:0}),/max-pages/);
    await assert.rejects(captureBurnHistory({...options,first:CHECKPOINT.number-1}),/immutable capture differs/);
  } finally {await rm(dir,{recursive:true,force:true});}
});
