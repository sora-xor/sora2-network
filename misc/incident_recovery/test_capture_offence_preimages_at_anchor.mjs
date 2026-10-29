import {test} from 'node:test';
import {existsSync} from 'node:fs';
import assert from 'node:assert/strict';
import {mkdtemp,writeFile,readFile,readdir,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {createRequire} from 'node:module';
import {capture,preparePlan,validateChanges,validateProof,DENOMINATOR_KEY} from './capture_offence_preimages_at_anchor.mjs';
import {CHECKPOINT,DEFAULT_ENDPOINT} from './capture_checkpoint.mjs';
const require=createRequire(resolve('runtime-rehearsal-async-staking-2026-10-25/validation/package.json'));
const {xxhashAsU8a}=require('@polkadot/util-crypto');
const {TypeRegistry}=require('@polkadot/types');
const SOURCE_NUMBER=22_000_000;
const sha=b=>createHash('sha256').update(b).digest('hex');
const denom='0x64000000000000000000000000000000';
const hex=b=>'0x'+Buffer.from(b).toString('hex');
const header={number:'0x'+SOURCE_NUMBER.toString(16),parentHash:'0x'+'11'.repeat(32),stateRoot:'0x'+'22'.repeat(32),extrinsicsRoot:'0x'+'33'.repeat(32),digest:{logs:[]}};
const SOURCE_HASH=new TypeRegistry().createType('Header',header).hash.toHex();
async function fixture(t,count=20) {
  const dir=await mkdtemp(join(tmpdir(),'offence-capture-test-'));t.after(()=>rm(dir,{recursive:true,force:true}));
  const requests=Array.from({length:count+1},(_,i)=>{const id='0x'+i.toString(16).padStart(64,'0');return{id,kind:'0x'+'44'.repeat(16),slot:'0x01000000',reportKey:'0xd5c41b52a371aa36c9254ce34324f2a5b262e9238fa402540c250bc3f5d6188d'+Buffer.from(xxhashAsU8a(Buffer.from(id.slice(2),'hex'),64)).toString('hex')+id.slice(2),reportValue:'0x010203'};});
  const previousPlan=join(dir,'prior.json'),classification=join(dir,'classification.json'),boundary=join(dir,'boundary.json'),planPath=join(dir,'plan.json'),output=join(dir,'captured'),evidence=join(dir,'evidence.json');
  await writeFile(evidence,'retained source evidence');
  const prior={schema:'sora-nexus-offence-preimage-retrieval-plan-v1',endpoint:DEFAULT_ENDPOINT,checkpointHash:CHECKPOINT.hash,source:{number:21_063_092,hash:'0x'+'88'.repeat(32)},inputs:[{path:evidence,sha256:sha(await readFile(evidence))}],requests};
  await writeFile(previousPlan,JSON.stringify(prior));
  const report={schema:'sora-nexus-offence-retrieved-preimage-classification-v1',planSha256:sha(await readFile(previousPlan)),sourceHash:prior.source.hash,checkpointDenominator:(10n**38n).toString(),requestedReports:requests.length,matchedOriginals:1,absentOriginals:count,pendingOriginals:0,contradictoryOriginals:0,matched:[{reportId:requests.at(-1).id,slot:requests.at(-1).slot}],absent:requests.slice(0,-1).map(r=>({id:r.id,slot:r.slot})),pending:[],contradictory:[],historicalStateProofVerified:false,nativeCompilerVerificationRequired:true,productionReady:false,inputPins:{[evidence]:sha(await readFile(evidence))}};
  await writeFile(classification,JSON.stringify(report));
  const afterHeader={...header,number:'0x'+(SOURCE_NUMBER+1).toString(16),parentHash:SOURCE_HASH},afterHash=new TypeRegistry().createType('Header',afterHeader).hash.toHex();
  const observed={schema:'sora-nexus-offence-denomination-boundary-v1',checkpointHash:CHECKPOINT.hash,inputGuardPassed:true,historicalStateProofVerified:false,productionReady:false,factor:'100',before:{number:SOURCE_NUMBER,hash:SOURCE_HASH,header,version:{specVersion:114},denominator:'100',denominatorScale:denom,proof:{at:SOURCE_HASH,proof:['0x01']}},after:{number:SOURCE_NUMBER+1,hash:afterHash,header:afterHeader,denominator:'10000',denominatorScale:'0x10270000000000000000000000000000',proof:{at:afterHash,proof:['0x01']}}};
  await writeFile(boundary,JSON.stringify(observed));
  await preparePlan({previousPlan,classification,boundary,output:planPath});return{dir,requests:requests.slice(0,-1),planPath,output,affected:classification,previousPlan,classification,boundary};
}
function mocked({failQuery=false,missing=true,drift=false}={}) {
  const calls=[];return{calls,async call(method,params){calls.push({method,params});
    if(method==='chain_getBlockHash')return params[0]===CHECKPOINT.number?CHECKPOINT.hash:SOURCE_HASH;
    if(method==='chain_getHeader')return drift?{...header,stateRoot:'0x'+'99'.repeat(32)}:header;
    if(method==='state_getStorage')return denom;
    if(method==='state_getReadProof')return{at:SOURCE_HASH,proof:['0x01']};
    assert.equal(method,'state_queryStorageAt');if(failQuery)throw Error('retained synthetic provider failure');
    return[{block:SOURCE_HASH,changes:params[0].map((key,i)=>[key,key===DENOMINATOR_KEY?denom:missing&&i===2?null:'0x010203'])}];
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
  const keys=[DENOMINATOR_KEY,'0xaa'];const good=[{block:SOURCE_HASH,changes:[[keys[0],null],[keys[1],'0x00']]}];assert.deepEqual(validateChanges(good,keys,SOURCE_HASH),good);
  for(const bad of [[{...good[0],block:'0x'+'00'.repeat(32)}],[{...good[0],changes:[good[0].changes[0]]}],[{...good[0],changes:[good[0].changes[0],good[0].changes[0]]}]])assert.throws(()=>validateChanges(bad,keys,SOURCE_HASH));
  assert.throws(()=>validateProof({at:SOURCE_HASH,proof:[]},SOURCE_HASH));assert.throws(()=>validateProof({at:'0x'+'00'.repeat(32),proof:['0x01']},SOURCE_HASH));
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

test('new anchor plan includes only absent exact prior request bytes and the observed ratio',async t=>{
  const f=await fixture(t,2),plan=JSON.parse(await readFile(f.planPath));
  assert.deepEqual(plan.requests,f.requests);assert.equal(plan.source.hash,SOURCE_HASH);assert.equal(plan.denominationRatio,(10n**36n).toString());assert.equal(plan.inputs.length,3);
});
test('ambiguous incomplete or mismatched classification cannot select a new plan',async t=>{
  const f=await fixture(t,2),valid=JSON.parse(await readFile(f.classification));
  for (const change of [r=>r.pendingOriginals=1,r=>r.contradictoryOriginals=1,r=>r.absent.push(r.absent[0]),r=>r.absent[0].slot='0x02000000',r=>r.absent[0].id='0x'+'ff'.repeat(32),r=>r.sourceHash='0x'+'ff'.repeat(32)]){
    const value=structuredClone(valid);change(value);await writeFile(f.classification,JSON.stringify(value));
    await assert.rejects(preparePlan({...f,output:join(f.dir,'invalid-plan.json')}));
  }
});
test('fabricated or non-adjacent transition and stale denominator are rejected',async t=>{
  const f=await fixture(t,2),valid=JSON.parse(await readFile(f.boundary));
  for(const change of [r=>r.before.hash='0x'+'ff'.repeat(32),r=>r.after.number+=1,r=>r.before.denominator='1',r=>r.factor='3',r=>r.inputGuardPassed=false]){
    const value=structuredClone(valid);change(value);await writeFile(f.boundary,JSON.stringify(value));
    await assert.rejects(preparePlan({...f,output:join(f.dir,'invalid-plan.json')}));
  }
});
test('manually changed but structurally valid plan cannot retrieve an already matched report',async t=>{
  const f=await fixture(t,2),plan=JSON.parse(await readFile(f.planPath)),prior=JSON.parse(await readFile(f.previousPlan));
  plan.requests[0]=prior.requests.at(-1);await writeFile(f.planPath,JSON.stringify(plan));
  const rpc=mocked();await assert.rejects(capture({...f,rpc,maxRpc:8}),/absent request derivation/);assert.equal(rpc.calls.length,0);
});
test('offline classifier joins actual capture bytes and authenticates the source-to-checkpoint ratio',{skip:!existsSync(resolve('artifacts/recovery-28191817/checkpoint/runtime-metadata.scale'))},async t=>{
  const {classify}=await import('./classify_offence_preimages_at_anchor.mjs');
  const {Metadata}=require('@polkadot/types'),{compactToU8a}=require('@polkadot/util'),{blake2AsU8a}=require('@polkadot/util-crypto');
  const registry=new TypeRegistry();registry.register(require('@sora-substrate/type-definitions').types);
  const metadata=new Metadata(registry,await readFile(resolve('artifacts/recovery-28191817/checkpoint/runtime-metadata.scale')));registry.setMetadata(metadata);
  const reportType=metadata.asLatest.toJSON().pallets.find(p=>p.name==='Offences').storage.items.find(i=>i.name==='Reports').type.map.value;
  const original=registry.createTypeUnsafe('Lookup'+reportType,[{offender:['0x'+'77'.repeat(32),{total:'200',own:'200',others:[]}],reporters:[]}]);
  const current=registry.createTypeUnsafe('Lookup'+reportType,[{offender:['0x'+'77'.repeat(32),{total:'0',own:'0',others:[]}],reporters:[]}]);
  const f=await fixture(t,1),prior=JSON.parse(await readFile(f.previousPlan)),classified=JSON.parse(await readFile(f.classification));
  const row=prior.requests[0];row.id=hex(blake2AsU8a(Buffer.concat([Buffer.from(row.kind.slice(2),'hex'),compactToU8a(4),Buffer.from(row.slot.slice(2),'hex'),original.offender.toU8a()]),256));
  row.reportKey='0xd5c41b52a371aa36c9254ce34324f2a5b262e9238fa402540c250bc3f5d6188d'+Buffer.from(xxhashAsU8a(Buffer.from(row.id.slice(2),'hex'),64)).toString('hex')+row.id.slice(2);row.reportValue=current.toHex();
  await writeFile(f.previousPlan,JSON.stringify(prior));classified.planSha256=sha(await readFile(f.previousPlan));classified.absent[0].id=row.id;await writeFile(f.classification,JSON.stringify(classified));
  f.planPath=join(f.dir,'auth-plan.json');await preparePlan({...f,output:f.planPath});
  const rpc=mocked(),originalCall=rpc.call;rpc.call=async function(method,params){if(method==='state_queryStorageAt')return[{block:SOURCE_HASH,changes:[[DENOMINATOR_KEY,denom],[row.reportKey,original.toHex()]]}];return originalCall.call(this,method,params);};
  await capture({...f,rpc,maxRpc:6});
  const report=classify({planPath:f.planPath,capture:f.output,output:join(f.dir,'classified.json')});
  assert.equal(report.matchedOriginals,1);assert.equal(report.absentOriginals,0);assert.equal(report.contradictoryOriginals,0);assert.equal(report.sourceDenominator,'100');assert.equal(report.historicalStateProofVerified,false);assert.equal(report.productionReady,false);
  const data=JSON.parse(await readFile(join(f.dir,'classified.json')));assert.equal(data.matched[0].denominator,(10n**36n).toString());assert.equal(data.matched[0].offender,original.offender.toHex());
});
