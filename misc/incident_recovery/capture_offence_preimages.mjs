#!/usr/bin/env node
/** Exact historical report-key retrieval; never submits a transaction.
 * ReportId preimage authentication is separate from historical trie/finality proof.
 * Every successful/error RPC receipt and completed batch is immutable and resumable.
 */
import {readFile, mkdir, open, link, unlink, stat} from 'node:fs/promises';
import {createHash, randomUUID} from 'node:crypto';
import {resolve, join, dirname} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {createRequire} from 'node:module';
import {CHECKPOINT, DEFAULT_ENDPOINT, ReadOnlyRpc} from './capture_checkpoint.mjs';

export const SOURCE_NUMBER = 21_063_092;
export const SOURCE_HASH = '0xf47edd8d25943b4c11d0b07f722c85aecb33525f6a70139c41195e2672a0b62f';
export const DENOMINATOR_KEY = '0x349246788a5a1a2b4c7f5025f3620fdf88fbfb377e1c57a4a58beac21d5b56d6';
const REPORT_PREFIX = '0xd5c41b52a371aa36c9254ce34324f2a5b262e9238fa402540c250bc3f5d6188d';
const MAX_FILE = 64 * 1024 * 1024;
const BATCH_SIZE = 16;
const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const require = createRequire(join(root,'runtime-rehearsal-async-staking-2026-10-25/validation/package.json'));
const {blake2AsU8a} = require('@polkadot/util-crypto');
const sha = raw => createHash('sha256').update(raw).digest('hex');
const json = value => Buffer.from(JSON.stringify(value, null, 2) + '\n');
const assert = (value, message) => { if (!value) throw Error(message); };
const isHex = (v, width) => typeof v === 'string' && /^0x(?:[0-9a-f]{2})*$/.test(v) && (width === undefined || v.length === width * 2 + 2);
async function bounded(file) { const s = await stat(file); assert(s.isFile() && s.size <= MAX_FILE, 'input file bound'); const raw = await readFile(file); assert(raw.length <= MAX_FILE, 'input grew beyond bound'); return raw; }
async function exists(file) { try { await stat(file); return true; } catch (error) { if (error.code === 'ENOENT') return false; throw error; } }
async function immutable(file, raw) {
  const temporary = file + '.writing-' + process.pid + '-' + randomUUID();
  const handle = await open(temporary, 'wx', 0o600);
  try { await handle.writeFile(raw); await handle.sync(); } finally { await handle.close(); }
  try { await link(temporary, file); } finally { await unlink(temporary); }
}
function identity(plan) {
  assert(plan.schema === 'sora-nexus-offence-preimage-retrieval-plan-v1' && plan.endpoint === DEFAULT_ENDPOINT && plan.checkpointHash === CHECKPOINT.hash, 'plan identity');
  assert(plan.source.number === SOURCE_NUMBER && plan.source.hash === SOURCE_HASH && Number(BigInt(plan.source.header.number)) === SOURCE_NUMBER, 'fixed historical source');
  assert(isHex(plan.source.header.stateRoot, 32) && plan.batchSize === BATCH_SIZE && plan.historicalStateProofVerified === false && plan.productionReady === false, 'plan scope');
  assert(Array.isArray(plan.requests) && plan.requests.length > 0 && plan.requests.length <= 4096, 'report count bound');
  const seen = new Set();
  for (const r of plan.requests) {
    assert(isHex(r.id, 32) && isHex(r.kind, 16) && isHex(r.slot) && r.slot.length > 2 && r.slot.length <= 2050, 'report identity widths');
    assert(isHex(r.reportKey, 72) && r.reportKey.startsWith(REPORT_PREFIX) && r.reportKey.endsWith(r.id.slice(2)), 'report key identity');
    assert(isHex(r.reportValue) && r.reportValue.length <= 2 + 2 * 1024 * 1024 && !seen.has(r.reportKey), 'duplicate or oversized report'); seen.add(r.reportKey);
  }
}
export async function preparePlan({affected, sourceCapture, baseReview, output}) {
  const [affectedRaw, sourceRaw, reviewRaw] = await Promise.all([bounded(affected), bounded(sourceCapture), bounded(baseReview)]);
  const requests = JSON.parse(affectedRaw).affected, history = JSON.parse(sourceRaw), review = JSON.parse(reviewRaw);
  assert(history.schema === 'sora-nexus-historical-offence-preimage-capture-v1' && history.endpoint === DEFAULT_ENDPOINT && history.checkpoint.hash === CHECKPOINT.hash, 'retained historical source identity');
  const source = history.states.find(s => s.number === SOURCE_NUMBER && s.hash === SOURCE_HASH);
  assert(source && source.proof.at === SOURCE_HASH && source.denominator === null, 'retained pre-denomination source');
  const records = new Map(review.records.map(r => [r.key, r])); assert(records.size === review.records.length, 'duplicate base review');
  for (const r of requests) {
    const record = records.get(r.reportKey);
    assert(record?.storage === 'Offences.Reports' && record.valueHash === '0x' + Buffer.from(blake2AsU8a(Buffer.from(r.reportValue.slice(2), 'hex'),256)).toString('hex'), 'checkpoint report differs from base review');
  }
  const plan = {schema:'sora-nexus-offence-preimage-retrieval-plan-v1', endpoint:DEFAULT_ENDPOINT, checkpointHash:CHECKPOINT.hash, source:{number:SOURCE_NUMBER, hash:SOURCE_HASH, header:source.header, version:source.version}, batchSize:BATCH_SIZE, inputs:[{path:resolve(affected),sha256:sha(affectedRaw)},{path:resolve(sourceCapture),sha256:sha(sourceRaw)},{path:resolve(baseReview),sha256:sha(reviewRaw)}], requests, historicalStateProofVerified:false, productionReady:false};
  identity(plan); await immutable(output, json(plan)); return {output:resolve(output),requests:requests.length,batches:Math.ceil(requests.length/BATCH_SIZE),productionReady:false};
}
export function validateChanges(value, keys) {
  assert(Array.isArray(value) && value.length === 1 && value[0].block === SOURCE_HASH && Array.isArray(value[0].changes), 'historical storage response anchor');
  const expected = new Set(keys), seen = new Set();
  assert(value[0].changes.length === keys.length, 'historical storage coverage');
  for (const row of value[0].changes) {
    assert(Array.isArray(row) && row.length === 2 && expected.has(row[0]) && !seen.has(row[0]), 'unexpected or duplicate storage response'); seen.add(row[0]);
    assert(row[1] === null || isHex(row[1]) && row[1].length <= 2 + 2 * 1024 * 1024, 'historical record encoding bound');
  }
  return value;
}
export function validateProof(value) {
  assert(value?.at === SOURCE_HASH && Array.isArray(value.proof) && value.proof.length > 0 && value.proof.length <= 65536, 'historical proof response anchor');
  let size = 0; for (const node of value.proof) { assert(isHex(node) && node.length > 2, 'historical proof node'); size += (node.length-2)/2; assert(size <= 32*1024*1024, 'historical proof byte bound'); }
  return value;
}
export async function capture({planPath, output, maxRpc = 8, timeoutMs = 900_000, rpc}) {
  assert(Number.isInteger(maxRpc) && maxRpc >= 1 && maxRpc <= 4096 && Number.isInteger(timeoutMs) && timeoutMs >= 60_000 && timeoutMs <= 3_600_000, 'request/time budget');
  const planRaw = await bounded(planPath), planHash = sha(planRaw), plan = JSON.parse(planRaw); identity(plan);
  for (const input of plan.inputs) assert(sha(await bounded(input.path)) === input.sha256, 'plan input changed');
  await mkdir(output, {recursive:true});
  for (const directory of ['rpc','batches','runs']) await mkdir(join(output,directory), {recursive:true});
  const copied = join(output,'plan.json'); if (await exists(copied)) assert((await bounded(copied)).equals(planRaw),'output bound to another plan'); else await immutable(copied,planRaw);
  const lockPath = join(output,'LOCK.json'); const lock = await open(lockPath,'wx',0o600);
  await lock.writeFile(json({pid:process.pid,planSha256:planHash,startedAt:new Date().toISOString()})); await lock.sync(); await lock.close();
  const ownRpc = !rpc; rpc ??= new ReadOnlyRpc(DEFAULT_ENDPOINT,{timeoutMs});
  let requestsMade = 0, completed = 0, absent = 0, error = null;
  const budget = new Error('request budget exhausted');
  async function read(method, params) {
    const request = {method,params}, key = sha(json(request)), file = join(output,'rpc',key+'.json');
    if (await exists(file)) { const prior=JSON.parse(await bounded(file)); assert(prior.planSha256===planHash && JSON.stringify(prior.request)===JSON.stringify(request) && Object.hasOwn(prior,'result'),'cached request identity'); assert(prior.resultSha256===sha(json(prior.result)),'cached response bytes changed'); return prior.result; }
    if(requestsMade>=maxRpc) throw budget; requestsMade++;
    try { const result=await rpc.call(method,params); await immutable(file,json({planSha256:planHash,request,result,resultSha256:sha(json(result))})); return result; }
    catch(error) { await immutable(join(output,'rpc',key+'.error-'+randomUUID()+'.json'),json({planSha256:planHash,request,error:String(error),at:new Date().toISOString()})); throw error; }
  }
  try {
    assert(await read('chain_getBlockHash',[CHECKPOINT.number]) === CHECKPOINT.hash, 'checkpoint hash mismatch');
    assert(await read('chain_getBlockHash',[SOURCE_NUMBER]) === SOURCE_HASH, 'historical hash mismatch');
    assert(JSON.stringify(await read('chain_getHeader',[SOURCE_HASH])) === JSON.stringify(plan.source.header), 'retained historical header mismatch');
    assert(await read('state_getStorage',[DENOMINATOR_KEY,SOURCE_HASH]) === null, 'historical denomination must be absent');
    for (let index=0;index<plan.requests.length;index+=BATCH_SIZE) {
      const rows=plan.requests.slice(index,index+BATCH_SIZE), keys=[DENOMINATOR_KEY,...rows.map(r=>r.reportKey)], dir=join(output,'batches',String(index/BATCH_SIZE).padStart(6,'0'));
      const values=validateChanges(await read('state_queryStorageAt',[keys,SOURCE_HASH]),keys);
      assert(new Map(values[0].changes).get(DENOMINATOR_KEY) === null, 'batch denominator mismatch');
      const proof=validateProof(await read('state_getReadProof',[keys,SOURCE_HASH]));
      const absentIds=rows.filter(r=>new Map(values[0].changes).get(r.reportKey)===null).map(r=>r.id);
      const content={'requested-reports.json':{affected:rows},'historical-capture.json':{schema:'sora-nexus-historical-offence-preimage-capture-v1',checkpoint:CHECKPOINT,endpoint:DEFAULT_ENDPOINT,states:[{...plan.source,denominator:null,values,proof}],historicalStateProofVerified:false,productionReady:false},'receipt.json':{schema:'sora-nexus-offence-preimage-retrieval-batch-v1',planSha256:planHash,firstRequest:index,requests:rows.length,absentIds,historicalStateProofVerified:false,originalPreimagesAuthenticated:false,productionReady:false}};
      await mkdir(dir,{recursive:true});
      for(const[name,value]of Object.entries(content)) {const file=join(dir,name), raw=json(value);if(await exists(file))assert((await bounded(file)).equals(raw),'completed batch changed');else await immutable(file,raw);}
      completed++; absent+=absentIds.length;
    }
  } catch(caught) { error=caught; }
  finally {
    if(ownRpc)rpc.close();
    let inputGuardPassed=false;
    try {
      assert(sha(await bounded(planPath))===planHash && (await bounded(copied)).equals(planRaw),'plan changed during capture');
      for(const input of plan.inputs)assert(sha(await bounded(input.path))===input.sha256,'plan input changed during capture');
      inputGuardPassed=true;
    } catch(caught) { error=caught; }
    const result={schema:'sora-nexus-offence-preimage-retrieval-run-v1',planSha256:planHash,requestsMade,completedBatches:completed,totalBatches:Math.ceil(plan.requests.length/BATCH_SIZE),absentReportsInCompletedBatches:absent,budgetExhausted:error===budget,error:error&&error!==budget?String(error):null,inputGuardPassed,complete:inputGuardPassed&&completed===Math.ceil(plan.requests.length/BATCH_SIZE),historicalStateProofVerified:false,originalPreimagesAuthenticated:false,productionReady:false};
    await immutable(join(output,'runs',randomUUID()+'.json'),json(result)); await unlink(lockPath);
    if(error && error!==budget)throw error; return result;
  }
}

if(process.argv[1] && import.meta.url===pathToFileURL(resolve(process.argv[1])).href) {
  const [mode,...args]=process.argv.slice(2);
  if(mode==='prepare') { assert(args.length===4,'prepare AFFECTED SOURCE_CAPTURE BASE_REVIEW NEW_PLAN'); console.log(JSON.stringify(await preparePlan({affected:resolve(args[0]),sourceCapture:resolve(args[1]),baseReview:resolve(args[2]),output:resolve(args[3])}))); }
  else { assert(mode==='capture' && args.length===3,'capture PLAN OUTPUT MAX_RPC'); console.log(JSON.stringify(await capture({planPath:resolve(args[0]),output:resolve(args[1]),maxRpc:Number(args[2])}))); }
}
