import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,writeFile,readFile,readdir,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {createRequire} from 'node:module';
import {prepare,locate,findBoundary,denominator,checkedHeader} from './locate_offence_denomination.mjs';
import {CHECKPOINT,DEFAULT_ENDPOINT} from './capture_checkpoint.mjs';
import {DENOMINATOR_KEY} from './capture_offence_preimages.mjs';
const require=createRequire(resolve('runtime-rehearsal-async-staking-2026-10-25/validation/package.json'));const{TypeRegistry}=require('@polkadot/types');
const header={parentHash:'0xf47edd8d25943b4c11d0b07f722c85aecb33525f6a70139c41195e2672a0b62f',number:'0x14165b5',stateRoot:'0x85daa47a4d6dd5cf96ba7613b9b9a006708f621df2a63a8749fb56bfd2f314c7',extrinsicsRoot:'0xd688bbb62a0ac9510ca51a913d59d07a7370c29dc64b0cd5e2e4648ffc04bf0b',digest:{logs:['0x0642414245b50101180000007e2c5d110000000088c2983d177f524f40f0b237fe76df8bc6661ba786e8b397ff9a5a67e7b4a802a898221c75770d54f20dff2cda350a06bb954b0bbff775d3bff18462aba4110f03e17a23a8453d231f357817744f2149138991202a63844df60595b788911a05','0x054241424501018c27dc9df17ddf9b121aad9b90484a4f777891b9c3514e2a4c13fa7126cb351e2e676f6d3b369c3496ef71eb256e908d1c58d5ee1780ee7ab72c84aedd83c38c']}};
const start=21_063_093,startHash='0xc23f5689016474fb5f0dc07651ba11e35d328cdb0ea43c2f634412e30bed76cc';
const scale=n=>'0x'+Buffer.from(n.toString(16).padStart(32,'0'),'hex').reverse().toString('hex');
test('first positive transition is found exactly, without assuming factor or upgrade height',async()=>{
  const seen=[];const result=await findBoundary({number:10,denominator:'100'},{number:1000,denominator:'1000000'},async number=>{seen.push(number);return{number,denominator:number<137?'100':number<600?'10000':'1000000'};});
  assert.equal(result.before.number,136);assert.equal(result.after.number,137);assert.equal(result.after.denominator,'10000');assert.ok(seen.length<=10);
});
test('adjacent search performs no extra probe; nonmonotonic and incompatible values fail',async()=>{
  const result=await findBoundary({number:10,denominator:'100'},{number:11,denominator:'10000'},()=>assert.fail('unexpected probe'));assert.equal(result.before.number,10);
  for(const value of ['0','1','99','101','100000000'])await assert.rejects(findBoundary({number:10,denominator:'100'},{number:1000,denominator:'10000'},async number=>({number,denominator:value})),/nonmonotonic|incompatible/);
});
test('denominator is exact unsigned little-endian and header hash is recomputed',()=>{
  assert.equal(denominator(null),1n);assert.equal(denominator(scale(100n)),100n);for(const raw of ['0x00',scale(0n),'100'])assert.throws(()=>denominator(raw));
  assert.deepEqual(checkedHeader(header,start,startHash),header);assert.throws(()=>checkedHeader({...header,stateRoot:'0x'+'aa'.repeat(32)},start,startHash),/hash differs/);
});
async function fixture(t){const dir=await mkdtemp(join(tmpdir(),'denom-locator-test-'));t.after(()=>rm(dir,{recursive:true,force:true}));const source=join(dir,'source.json'),planPath=join(dir,'plan.json'),output=join(dir,'capture');await writeFile(source,JSON.stringify({schema:'sora-nexus-historical-offence-preimage-capture-v1',endpoint:DEFAULT_ENDPOINT,checkpoint:CHECKPOINT,states:[{number:start,hash:startHash,header,denominator:scale(100n),proof:{at:startHash,proof:['0x01']}}]}));await prepare({source,output:planPath});return{source,planPath,output};}
function mock({drift=false,fail=false,absentEvents=false}={}){
  const calls=[],transition=start+17,known=new Map([[startHash,start],[CHECKPOINT.hash,CHECKPOINT.number]]),registry=new TypeRegistry();
  function h(number){if(number===start)return header;return{parentHash:number===transition?hash(number-1):'0x'+'11'.repeat(32),number:'0x'+number.toString(16),stateRoot:'0x'+'22'.repeat(32),extrinsicsRoot:'0x'+'33'.repeat(32),digest:{logs:[]}};}
  function hash(number){if(number===start)return startHash;if(number===CHECKPOINT.number)return CHECKPOINT.hash;const value=registry.createType('Header',h(number)).hash.toHex();known.set(value,number);return value;}
  return{calls,transition,async call(method,params){calls.push({method,params});if(method==='chain_getBlockHash')return hash(params[0]);const at=method==='state_getStorage'||method==='state_getReadProof'?params[1]:params[0],number=known.get(at);assert.notEqual(number,undefined);
    if(method==='state_getStorage')return params[0]===DENOMINATOR_KEY?scale(number<transition?100n:10n**38n):absentEvents?null:'0x00';
    if(method==='chain_getHeader')return drift?{...h(number),stateRoot:'0x'+'99'.repeat(32)}:h(number);
    if(method==='state_getRuntimeVersion')return{specVersion:115,transactionVersion:115};
    if(method==='chain_getBlock')return{block:{header:h(number),extrinsics:['0x00']},justifications:null};
    if(method==='state_getReadProof'){if(fail)throw Error('synthetic proof failure');return{at,proof:['0x01']};}
    assert.equal(method,'state_getMetadata');return'0x00';
  }};
}
test('budgeted capture resumes exact cached probes and retains adjacent headers bodies events proofs',async t=>{
  const f=await fixture(t),rpc=mock();const first=await locate({...f,rpc,maxRpc:8});assert.equal(first.budgetExhausted,true);assert.equal(first.complete,false);
  const last=await locate({...f,rpc,maxRpc:100});assert.equal(last.complete,true);assert.equal(last.inputGuardPassed,true);
  const b=JSON.parse(await readFile(join(f.output,'boundary.json')));assert.equal(b.before.number,rpc.transition-1);assert.equal(b.after.number,rpc.transition);assert.equal(b.after.header.parentHash,b.before.hash);assert.equal(b.factor,(10n**36n).toString());
  assert.equal(b.historicalStateProofVerified,false);assert.equal(b.denominationEventDecoded,false);assert.equal(b.rootCallDecoded,false);assert.equal(b.before.block.block.extrinsics[0],'0x00');assert.equal(b.after.events,'0x00');
  const again=await locate({...f,rpc,maxRpc:1});assert.equal(again.requestsMade,0);assert.equal(again.complete,true);
});
test('changed final header and failed proof remain failures with retained evidence',async t=>{
  for(const options of [{drift:true},{fail:true}]){const f=await fixture(t),rpc=mock(options);await assert.rejects(locate({...f,rpc,maxRpc:100}),/header hash|proof failure/);await assert.rejects(readFile(join(f.output,'boundary.json')),/ENOENT/);if(options.fail)assert.ok((await readdir(join(f.output,'rpc'))).some(n=>n.includes('.error-')));}
});
test('source drift is rejected before RPC and incomplete cache alteration fails',async t=>{
  const f=await fixture(t),rpc=mock();await locate({...f,rpc,maxRpc:8});const files=await readdir(join(f.output,'rpc'));const file=join(f.output,'rpc',files[0]),value=JSON.parse(await readFile(file));value.result=null;await writeFile(file,JSON.stringify(value));await assert.rejects(locate({...f,rpc,maxRpc:8}),/cached RPC/);
  await writeFile(f.source,'{}');const clean=mock();await assert.rejects(locate({...f,rpc:clean,maxRpc:8}),/source changed/);assert.equal(clean.calls.length,0);
});
test('absent events remain explicit absence without claiming event or root-call verification',async t=>{
  const f=await fixture(t);assert.equal((await locate({...f,rpc:mock({absentEvents:true}),maxRpc:100})).complete,true);
  const b=JSON.parse(await readFile(join(f.output,'boundary.json')));assert.equal(b.before.events,null);assert.equal(b.after.events,null);assert.equal(b.denominationEventDecoded,false);assert.equal(b.rootCallDecoded,false);
});
