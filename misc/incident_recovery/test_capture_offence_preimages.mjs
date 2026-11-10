import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,writeFile,readFile,readdir,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {createRequire} from 'node:module';
import {capture,preparePlan,validateChanges,validateProof,SOURCE_HASH,SOURCE_NUMBER,DENOMINATOR_KEY} from './capture_offence_preimages.mjs';
import {CHECKPOINT,DEFAULT_ENDPOINT} from './capture_checkpoint.mjs';
const require=createRequire(resolve('runtime-rehearsal-async-staking-2026-10-25/validation/package.json'));
const {blake2AsU8a}=require('@polkadot/util-crypto');
const hex=b=>'0x'+Buffer.from(b).toString('hex');
const header={number:'0x'+SOURCE_NUMBER.toString(16),parentHash:'0x'+'11'.repeat(32),stateRoot:'0x'+'22'.repeat(32),extrinsicsRoot:'0x'+'33'.repeat(32),digest:{logs:[]}};
async function fixture(t,count=20) {
  const dir=await mkdtemp(join(tmpdir(),'offence-capture-test-'));t.after(()=>rm(dir,{recursive:true,force:true}));
  const requests=Array.from({length:count},(_,i)=>{const id='0x'+i.toString(16).padStart(64,'0');return{id,kind:'0x'+'44'.repeat(16),slot:'0x01000000',reportKey:'0xd5c41b52a371aa36c9254ce34324f2a5b262e9238fa402540c250bc3f5d6188d'+'55'.repeat(8)+id.slice(2),reportValue:'0x010203'};});
  const affected=join(dir,'affected.json'),sourceCapture=join(dir,'source.json'),baseReview=join(dir,'review.json'),planPath=join(dir,'plan.json'),output=join(dir,'captured');
  await writeFile(affected,JSON.stringify({affected:requests}));await writeFile(sourceCapture,JSON.stringify({schema:'sora-nexus-historical-offence-preimage-capture-v1',endpoint:DEFAULT_ENDPOINT,checkpoint:CHECKPOINT,states:[{number:SOURCE_NUMBER,hash:SOURCE_HASH,header,version:{specVersion:114},denominator:null,proof:{at:SOURCE_HASH,proof:['0x01']}}]}));
  await writeFile(baseReview,JSON.stringify({records:requests.map(r=>({storage:'Offences.Reports',key:r.reportKey,valueHash:hex(blake2AsU8a(Buffer.from(r.reportValue.slice(2),'hex'),256))}))}));
  await preparePlan({affected,sourceCapture,baseReview,output:planPath});return{dir,requests,planPath,output,affected};
}
function mocked({failQuery=false,missing=true,drift=false}={}) {
  const calls=[];return{calls,async call(method,params){calls.push({method,params});
    if(method==='chain_getBlockHash')return params[0]===CHECKPOINT.number?CHECKPOINT.hash:SOURCE_HASH;
    if(method==='chain_getHeader')return drift?{...header,stateRoot:'0x'+'99'.repeat(32)}:header;
    if(method==='state_getStorage')return null;
    if(method==='state_getReadProof')return{at:SOURCE_HASH,proof:['0x01']};
    assert.equal(method,'state_queryStorageAt');if(failQuery)throw Error('retained synthetic provider failure');
    return[{block:SOURCE_HASH,changes:params[0].map((key,i)=>[key,key===DENOMINATOR_KEY||missing&&i===2?null:'0x010203'])}];
  }};
}
test('bounded exact-key batches retain nulls, proof bytes and honest scope',async t=>{
  const f=await fixture(t),rpc=mocked();const r=await capture({...f,rpc,maxRpc:8});assert.equal(r.complete,true);assert.equal(r.requestsMade,8);assert.equal(r.completedBatches,2);assert.equal(r.absentReportsInCompletedBatches,2);assert.equal(r.originalPreimagesAuthenticated,false);assert.equal(r.historicalStateProofVerified,false);assert.equal(r.productionReady,false);
  const history=JSON.parse(await readFile(join(f.output,'batches/000000/historical-capture.json')));assert.deepEqual(history.states[0].proof,{at:SOURCE_HASH,proof:['0x01']});assert.equal(history.states[0].values[0].changes.length,17);
});
test('budget resume reuses immutable exact hash receipts and completes remaining proof',async t=>{
  const f=await fixture(t),rpc=mocked();const first=await capture({...f,rpc,maxRpc:5});assert.equal(first.budgetExhausted,true);assert.equal(first.completedBatches,0);
  const before=await readFile(join(f.output,'plan.json'));const second=await capture({...f,rpc,maxRpc:3});assert.equal(second.complete,true);assert.equal(second.requestsMade,3);assert.equal(rpc.calls.length,8);assert.deepEqual(await readFile(join(f.output,'plan.json')),before);
});
test('provider errors are retained and retries never overwrite their evidence',async t=>{
  const f=await fixture(t);await assert.rejects(capture({...f,rpc:mocked({failQuery:true}),maxRpc:8}),/provider failure/);
  const before=(await readdir(join(f.output,'rpc'))).filter(n=>n.includes('.error-'));assert.equal(before.length,1);
  assert.equal((await capture({...f,rpc:mocked(),maxRpc:4})).complete,true);assert.deepEqual((await readdir(join(f.output,'rpc'))).filter(n=>n.includes('.error-')),before);
});
test('retained header mismatch stops before report retrieval',async t=>{
  const f=await fixture(t),rpc=mocked({drift:true});await assert.rejects(capture({...f,rpc,maxRpc:8}),/header mismatch/);assert.equal(rpc.calls.length,3);
});
test('altered plan input refuses all RPC calls',async t=>{
  const f=await fixture(t),rpc=mocked();await writeFile(f.affected,'{}');await assert.rejects(capture({...f,rpc,maxRpc:8}),/input changed/);assert.equal(rpc.calls.length,0);
});
test('partial batch files are checked and never replaced',async t=>{
  const f=await fixture(t),rpc=mocked();await capture({...f,rpc,maxRpc:8});const file=join(f.output,'batches/000000/receipt.json');await writeFile(file,'{}');await assert.rejects(capture({...f,rpc,maxRpc:8}),/batch changed/);assert.equal(await readFile(file,'utf8'),'{}');
});
test('storage rows require exact block/key coverage and proof result remains structural only',()=>{
  const keys=[DENOMINATOR_KEY,'0xaa'];const good=[{block:SOURCE_HASH,changes:[[keys[0],null],[keys[1],'0x00']]}];assert.deepEqual(validateChanges(good,keys),good);
  for(const bad of [[{...good[0],block:'0x'+'00'.repeat(32)}],[{...good[0],changes:[good[0].changes[0]]}],[{...good[0],changes:[good[0].changes[0],good[0].changes[0]]}]])assert.throws(()=>validateChanges(bad,keys));
  assert.throws(()=>validateProof({at:SOURCE_HASH,proof:[]}));assert.throws(()=>validateProof({at:'0x'+'00'.repeat(32),proof:['0x01']}));
});
test('active or interrupted lock remains fail closed without overwriting it',async t=>{
  const f=await fixture(t);await capture({...f,rpc:mocked(),maxRpc:4});const file=join(f.output,'LOCK.json');await writeFile(file,'interrupted owner evidence');const rpc=mocked();await assert.rejects(capture({...f,rpc,maxRpc:8}),/EEXIST/);assert.equal(rpc.calls.length,0);assert.equal(await readFile(file,'utf8'),'interrupted owner evidence');
});
test('changed incomplete cached response fails its retained content digest',async t=>{
  const f=await fixture(t);await capture({...f,rpc:mocked(),maxRpc:5});
  const dir=join(f.output,'rpc');for(const name of await readdir(dir)){const file=join(dir,name),x=JSON.parse(await readFile(file));if(x.request.method==='state_queryStorageAt'){x.result[0].changes[1][1]='0xffff';await writeFile(file,JSON.stringify(x));}}
  const rpc=mocked();await assert.rejects(capture({...f,rpc,maxRpc:8}),/response bytes changed/);assert.equal(rpc.calls.length,0);
});
test('input drift during capture is recorded and cannot report completion',async t=>{
  const f=await fixture(t,1),rpc=mocked();const original=rpc.call;rpc.call=async function(method,params){const result=await original.call(this,method,params);if(method==='state_getReadProof')await writeFile(f.affected,'{}');return result;};
  await assert.rejects(capture({...f,rpc,maxRpc:8}),/input changed during capture/);
  const runs=await readdir(join(f.output,'runs'));const result=JSON.parse(await readFile(join(f.output,'runs',runs[0])));assert.equal(result.inputGuardPassed,false);assert.equal(result.complete,false);
});
