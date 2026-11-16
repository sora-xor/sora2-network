#!/usr/bin/env node
/** Observed-anchor historical report-key retrieval; never submits a transaction.
 * ReportId preimage authentication is separate from historical trie/finality proof.
 * Every successful/error RPC receipt and completed batch is immutable and resumable.
 */
import {readFile, mkdir, open, link, unlink, stat} from 'node:fs/promises';
import {createHash, randomUUID} from 'node:crypto';
import {resolve, join, dirname} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {createRequire} from 'node:module';
import {CHECKPOINT, DEFAULT_ENDPOINT, ReadOnlyRpc} from './capture_checkpoint.mjs';

import {DENOMINATOR_KEY} from './capture_offence_preimages.mjs';
import {checkedHeader,denominator} from './locate_offence_denomination.mjs';
export {DENOMINATOR_KEY};
const FINAL_FACTOR = 10n ** 38n;
const REPORT_PREFIX = '0xd5c41b52a371aa36c9254ce34324f2a5b262e9238fa402540c250bc3f5d6188d';
const MAX_FILE = 64 * 1024 * 1024;
const BATCH_SIZE = 16;
const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const require = createRequire(join(root,'runtime-rehearsal-async-staking-2026-10-25/validation/package.json'));
const {xxhashAsU8a} = require('@polkadot/util-crypto');
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
export function validateRequests(requests) {
  assert(Array.isArray(requests) && requests.length > 0 && requests.length <= 4096, 'report count bound');
  const seen = new Set();
  for (const r of requests) {
    assert(isHex(r.id, 32) && isHex(r.kind, 16) && isHex(r.slot) && r.slot.length > 2 && r.slot.length <= 2050, 'report identity widths');
    const id = Buffer.from(r.id.slice(2), 'hex');
    const key = REPORT_PREFIX + Buffer.from(xxhashAsU8a(id,64)).toString('hex') + r.id.slice(2);
    assert(r.reportKey === key, 'report key hash identity');
    assert(isHex(r.reportValue) && r.reportValue.length <= 2 + 2 * 1024 * 1024 && !seen.has(r.id), 'duplicate or oversized report'); seen.add(r.id);
  }
}
export function validatePlan(plan) {
  assert(plan.schema === 'sora-nexus-offence-preimage-anchor-plan-v1' && plan.endpoint === DEFAULT_ENDPOINT && plan.checkpointHash === CHECKPOINT.hash, 'plan identity');
  assert(Number.isInteger(plan.source.number) && plan.source.number > 21_063_092 && plan.source.number < CHECKPOINT.number && isHex(plan.source.hash,32), 'historical source range');
  checkedHeader(plan.source.header,plan.source.number,plan.source.hash);
  const d = denominator(plan.source.denominatorScale);
  assert(d > 1n && d < FINAL_FACTOR && FINAL_FACTOR % d === 0n && plan.source.denominator === d.toString() && plan.checkpointDenominator === FINAL_FACTOR.toString() && plan.denominationRatio === (FINAL_FACTOR/d).toString(), 'source denomination ratio');
  assert(plan.batchSize === BATCH_SIZE && plan.historicalStateProofVerified === false && plan.productionReady === false, 'plan scope');
  assert(Array.isArray(plan.inputs) && plan.inputs.length === 3 && plan.inputs.every(x => typeof x.path === 'string' && /^[0-9a-f]{64}$/.test(x.sha256)), 'plan input bindings');
  validateRequests(plan.requests);
}
/** Request only explicit absences from a completed previous capture/classification.
 * The source is the real before-state of an already retained adjacent transition.
 */
async function derivePlan({previousPlan, classification, boundary}) {
  const [priorRaw, classRaw, boundaryRaw] = await Promise.all([bounded(previousPlan), bounded(classification), bounded(boundary)]);
  const prior = JSON.parse(priorRaw), classified = JSON.parse(classRaw), observed = JSON.parse(boundaryRaw);
  assert(['sora-nexus-offence-preimage-retrieval-plan-v1','sora-nexus-offence-preimage-anchor-plan-v1'].includes(prior.schema) && prior.endpoint === DEFAULT_ENDPOINT && prior.checkpointHash === CHECKPOINT.hash, 'prior plan identity');
  validateRequests(prior.requests);
  if (prior.schema === 'sora-nexus-offence-preimage-anchor-plan-v1') validatePlan(prior);
  for (const input of prior.inputs) assert(sha(await bounded(input.path)) === input.sha256, 'prior plan input changed');
  assert(classified.schema === 'sora-nexus-offence-retrieved-preimage-classification-v1' && classified.planSha256 === sha(priorRaw) && classified.sourceHash === prior.source.hash, 'classification plan/source join');
  assert(classified.historicalStateProofVerified === false && classified.nativeCompilerVerificationRequired === true && classified.productionReady === false && classified.checkpointDenominator === FINAL_FACTOR.toString(), 'classification scope');
  assert(classified.pendingOriginals === 0 && classified.contradictoryOriginals === 0 && Array.isArray(classified.pending) && classified.pending.length === 0 && Array.isArray(classified.contradictory) && classified.contradictory.length === 0, 'unfinished or contradictory classification');
  assert(Array.isArray(classified.absent) && classified.absent.length > 0 && classified.absentOriginals === classified.absent.length && Array.isArray(classified.matched) && classified.matchedOriginals === classified.matched.length && classified.requestedReports === prior.requests.length, 'classification coverage counts');
  const byId = new Map(prior.requests.map(r => [r.id,r])), seen = new Set();
  for (const [rows, field] of [[classified.matched,'reportId'],[classified.absent,'id']]) for (const r of rows) {
    const request = byId.get(r[field]); assert(request && !seen.has(r[field]) && request.slot === r.slot, 'classification partition/slot mismatch'); seen.add(r[field]);
  }
  assert(seen.size === prior.requests.length, 'classification coverage');
  assert(classified.inputPins && Object.keys(classified.inputPins).length > 0, 'classification input provenance absent');
  for (const [file,digest] of Object.entries(classified.inputPins)) assert(sha(await bounded(file)) === digest, 'classification evidence changed');
  assert(observed.schema === 'sora-nexus-offence-denomination-boundary-v1' && observed.checkpointHash === CHECKPOINT.hash && observed.inputGuardPassed === true && observed.historicalStateProofVerified === false && observed.productionReady === false, 'observed boundary identity/scope');
  const before = observed.before, after = observed.after;
  checkedHeader(before.header,before.number,before.hash); checkedHeader(after.header,after.number,after.hash);
  assert(before.number > prior.source.number && after.number === before.number+1 && after.header.parentHash === before.hash && before.proof.at === before.hash && after.proof.at === after.hash, 'observed boundary adjacency/chronology');
  const pre = denominator(before.denominatorScale), post = denominator(after.denominatorScale);
  assert(pre.toString() === before.denominator && post.toString() === after.denominator && post > pre && post % pre === 0n && FINAL_FACTOR % post === 0n && observed.factor === (post/pre).toString(), 'observed transition factors');
  const absentIds = new Set(classified.absent.map(r=>r.id));
  const plan = {schema:'sora-nexus-offence-preimage-anchor-plan-v1',endpoint:DEFAULT_ENDPOINT,checkpointHash:CHECKPOINT.hash,source:{number:before.number,hash:before.hash,header:before.header,version:before.version,denominator:pre.toString(),denominatorScale:before.denominatorScale},checkpointDenominator:FINAL_FACTOR.toString(),denominationRatio:(FINAL_FACTOR/pre).toString(),batchSize:BATCH_SIZE,inputs:[{path:resolve(previousPlan),sha256:sha(priorRaw)},{path:resolve(classification),sha256:sha(classRaw)},{path:resolve(boundary),sha256:sha(boundaryRaw)}],requests:prior.requests.filter(r=>absentIds.has(r.id)),historicalStateProofVerified:false,productionReady:false};
  validatePlan(plan);
  for (const input of plan.inputs) assert(sha(await bounded(input.path)) === input.sha256, 'input changed during preparation');
  return plan;
}
export async function preparePlan({previousPlan, classification, boundary, output}) {
  const plan = await derivePlan({previousPlan,classification,boundary});
  await immutable(output,json(plan)); return {output:resolve(output),sourceNumber:plan.source.number,sourceHash:plan.source.hash,sourceDenominator:plan.source.denominator,denominationRatio:plan.denominationRatio,requests:plan.requests.length,batches:Math.ceil(plan.requests.length/BATCH_SIZE),productionReady:false};
}
export function validateChanges(value, keys, sourceHash) {
  assert(Array.isArray(value) && value.length === 1 && value[0].block === sourceHash && Array.isArray(value[0].changes), 'historical storage response anchor');
  const expected = new Set(keys), seen = new Set();
  assert(value[0].changes.length === keys.length, 'historical storage coverage');
  for (const row of value[0].changes) {
    assert(Array.isArray(row) && row.length === 2 && expected.has(row[0]) && !seen.has(row[0]), 'unexpected or duplicate storage response'); seen.add(row[0]);
    assert(row[1] === null || isHex(row[1]) && row[1].length <= 2 + 2 * 1024 * 1024, 'historical record encoding bound');
  }
  return value;
}
export function validateProof(value, sourceHash) {
  assert(value?.at === sourceHash && Array.isArray(value.proof) && value.proof.length > 0 && value.proof.length <= 65536, 'historical proof response anchor');
  let size = 0; for (const node of value.proof) { assert(isHex(node) && node.length > 2, 'historical proof node'); size += (node.length-2)/2; assert(size <= 32*1024*1024, 'historical proof byte bound'); }
  return value;
}
export async function capture({planPath, output, maxRpc = 8, timeoutMs = 900_000, rpc}) {
  assert(Number.isInteger(maxRpc) && maxRpc >= 1 && maxRpc <= 4096 && Number.isInteger(timeoutMs) && timeoutMs >= 60_000 && timeoutMs <= 3_600_000, 'request/time budget');
  const planRaw = await bounded(planPath), planHash = sha(planRaw), plan = JSON.parse(planRaw); validatePlan(plan); const sourceHash=plan.source.hash, sourceNumber=plan.source.number;
  for (const input of plan.inputs) assert(sha(await bounded(input.path)) === input.sha256, 'plan input changed');
  const derived = await derivePlan({previousPlan:plan.inputs[0].path,classification:plan.inputs[1].path,boundary:plan.inputs[2].path});
  assert(json(derived).equals(json(plan)), 'plan differs from observed boundary and absent request derivation');
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
    assert(await read('chain_getBlockHash',[sourceNumber]) === sourceHash, 'historical hash mismatch');
    assert(JSON.stringify(await read('chain_getHeader',[sourceHash])) === JSON.stringify(plan.source.header), 'retained historical header mismatch');
    assert(await read('state_getStorage',[DENOMINATOR_KEY,sourceHash]) === plan.source.denominatorScale, 'historical denomination mismatch');
    for (let index=0;index<plan.requests.length;index+=BATCH_SIZE) {
      const rows=plan.requests.slice(index,index+BATCH_SIZE), keys=[DENOMINATOR_KEY,...rows.map(r=>r.reportKey)], dir=join(output,'batches',String(index/BATCH_SIZE).padStart(6,'0'));
      const values=validateChanges(await read('state_queryStorageAt',[keys,sourceHash]),keys,sourceHash);
      assert(new Map(values[0].changes).get(DENOMINATOR_KEY) === plan.source.denominatorScale, 'batch denominator mismatch');
      const proof=validateProof(await read('state_getReadProof',[keys,sourceHash]),sourceHash);
      const absentIds=rows.filter(r=>new Map(values[0].changes).get(r.reportKey)===null).map(r=>r.id);
      const content={'requested-reports.json':{affected:rows},'historical-capture.json':{schema:'sora-nexus-historical-offence-preimage-capture-v1',checkpoint:CHECKPOINT,endpoint:DEFAULT_ENDPOINT,states:[{...plan.source,denominator:plan.source.denominatorScale,values,proof}],historicalStateProofVerified:false,productionReady:false},'receipt.json':{schema:'sora-nexus-offence-preimage-anchor-batch-v1',planSha256:planHash,firstRequest:index,requests:rows.length,absentIds,historicalStateProofVerified:false,originalPreimagesAuthenticated:false,productionReady:false}};
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
    const result={schema:'sora-nexus-offence-preimage-anchor-run-v1',planSha256:planHash,requestsMade,completedBatches:completed,totalBatches:Math.ceil(plan.requests.length/BATCH_SIZE),absentReportsInCompletedBatches:absent,budgetExhausted:error===budget,error:error&&error!==budget?String(error):null,inputGuardPassed,complete:inputGuardPassed&&completed===Math.ceil(plan.requests.length/BATCH_SIZE),historicalStateProofVerified:false,originalPreimagesAuthenticated:false,productionReady:false};
    await immutable(join(output,'runs',randomUUID()+'.json'),json(result)); await unlink(lockPath);
    if(error && error!==budget)throw error; return result;
  }
}

if(process.argv[1] && import.meta.url===pathToFileURL(resolve(process.argv[1])).href) {
  const [mode,...args]=process.argv.slice(2);
  if(mode==='prepare') { assert(args.length===4,'prepare PREVIOUS_PLAN CLASSIFICATION ACTUAL_BOUNDARY NEW_PLAN'); console.log(JSON.stringify(await preparePlan({previousPlan:resolve(args[0]),classification:resolve(args[1]),boundary:resolve(args[2]),output:resolve(args[3])}))); }
  else { assert(mode==='capture' && args.length===3,'capture PLAN OUTPUT MAX_RPC'); console.log(JSON.stringify(await capture({planPath:resolve(args[0]),output:resolve(args[1]),maxRpc:Number(args[2])}))); }
}
