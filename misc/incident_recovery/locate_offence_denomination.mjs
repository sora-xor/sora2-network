#!/usr/bin/env node
/** Locate one next positive cumulative-denomination transition using exact-key reads.
 * Root owns real RPC launches. Cache and proof retention do not establish finality.
 */
import {readFile,stat,mkdir,open,link,unlink} from 'node:fs/promises';
import {createHash,randomUUID} from 'node:crypto';
import {createRequire} from 'node:module';
import {resolve,dirname,join} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {CHECKPOINT,DEFAULT_ENDPOINT,ReadOnlyRpc} from './capture_checkpoint.mjs';
import {DENOMINATOR_KEY} from './capture_offence_preimages.mjs';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
const require=createRequire(join(root,'runtime-rehearsal-async-staking-2026-10-25/validation/package.json'));
const {TypeRegistry}=require('@polkadot/types');const {xxhashAsU8a}=require('@polkadot/util-crypto');
const EVENTS_KEY='0x'+Buffer.concat([xxhashAsU8a('System',128),xxhashAsU8a('Events',128)]).toString('hex');
const FINAL_FACTOR=10n**38n;
const json=v=>Buffer.from(JSON.stringify(v,null,2)+'\n');const sha=b=>createHash('sha256').update(b).digest('hex');
const need=(v,m)=>{if(!v)throw Error(m);};
const isHex=(v,n)=>typeof v==='string'&&/^0x(?:[0-9a-f]{2})*$/.test(v)&&(n===undefined||v.length===2+2*n);
async function read(file){const s=await stat(file);need(s.isFile()&&s.size<=64*1024*1024,'file bound');const raw=await readFile(file);need(raw.length<=64*1024*1024,'file grew beyond bound');return raw;}
async function exists(file){try{await stat(file);return true;}catch(e){if(e.code==='ENOENT')return false;throw e;}}
async function write(file,raw){const tmp=file+'.writing-'+process.pid+'-'+randomUUID(),fd=await open(tmp,'wx',0o600);try{await fd.writeFile(raw);await fd.sync();}finally{await fd.close();}try{await link(tmp,file);}finally{await unlink(tmp);}}
export function denominator(value){if(value===null)return 1n;need(isHex(value,16),'denominator must be raw u128');const n=BigInt('0x'+Buffer.from(value.slice(2),'hex').reverse().toString('hex'));need(n>0n,'denominator must be positive');return n;}
export function checkedHeader(header,number,hash){need(header&&Number(BigInt(header.number))===number,'header height differs');const registry=new TypeRegistry();need(registry.createType('Header',header).hash.toHex()===hash,'header hash differs');return header;}
function planIdentity(plan){need(plan.schema==='sora-nexus-offence-denomination-search-v1'&&plan.endpoint===DEFAULT_ENDPOINT&&plan.checkpointHash===CHECKPOINT.hash,'search plan identity');need(Number.isInteger(plan.lower.number)&&plan.lower.number>=21_063_093&&plan.lower.number<CHECKPOINT.number&&isHex(plan.lower.hash,32),'lower anchor');checkedHeader(plan.lower.header,plan.lower.number,plan.lower.hash);const d=denominator(plan.lower.denominatorScale);need(d.toString()===plan.lower.denominator&&d>1n&&d<FINAL_FACTOR&&FINAL_FACTOR%d===0n,'lower cumulative factor');need(plan.historicalStateProofVerified===false&&plan.productionReady===false,'search plan scope');}
export async function prepare({source,output}){
  const raw=await read(source),value=JSON.parse(raw);let lower;
  if(value.schema==='sora-nexus-historical-offence-preimage-capture-v1'){
    need(value.checkpoint.hash===CHECKPOINT.hash&&value.endpoint===DEFAULT_ENDPOINT,'historical capture source');
    const state=value.states.find(s=>s.number===21_063_093&&s.hash==='0xc23f5689016474fb5f0dc07651ba11e35d328cdb0ea43c2f634412e30bed76cc');need(state&&state.proof.at===state.hash,'first denomination anchor');
    const d=denominator(state.denominator);need(d===100n,'first observed denomination');lower={number:state.number,hash:state.hash,header:state.header,denominator:d.toString(),denominatorScale:state.denominator};
  }else{
    need(value.schema==='sora-nexus-offence-denomination-boundary-v1'&&value.checkpointHash===CHECKPOINT.hash&&value.inputGuardPassed===true&&value.historicalStateProofVerified===false,'prior observed boundary');
    const state=value.after;need(state.number===value.before.number+1&&state.header.parentHash===value.before.hash,'prior adjacent boundary');
    lower={number:state.number,hash:state.hash,header:state.header,denominator:state.denominator,denominatorScale:state.denominatorScale};
  }
  const plan={schema:'sora-nexus-offence-denomination-search-v1',endpoint:DEFAULT_ENDPOINT,checkpointHash:CHECKPOINT.hash,lower,source:{path:resolve(source),sha256:sha(raw)},historicalStateProofVerified:false,productionReady:false};planIdentity(plan);await write(output,json(plan));return{output:resolve(output),lower:lower.number,denominator:lower.denominator,productionReady:false};
}
/** A successful search establishes adjacent observed values, never original balances. */
export async function findBoundary(lower,upper,probe){
  const initial=BigInt(lower.denominator);need(initial>0n&&BigInt(upper.denominator)>initial&&upper.number>lower.number,'search interval');
  const observed=new Map([[lower.number,lower],[upper.number,upper]]);let left=lower,right=upper;
  while(right.number-left.number>1){
    const number=left.number+Math.floor((right.number-left.number)/2),middle=await probe(number);need(middle.number===number,'probe height');const d=BigInt(middle.denominator);
    need(d>=BigInt(left.denominator)&&d<=BigInt(right.denominator)&&d%initial===0n&&BigInt(upper.denominator)%d===0n,'nonmonotonic or incompatible cumulative denominator');observed.set(number,middle);
    if(d===initial)left=middle;else right=middle;
  }
  return{before:left,after:right,observations:[...observed.values()].sort((a,b)=>a.number-b.number)};
}
function proof(value,hash){need(value?.at===hash&&Array.isArray(value.proof)&&value.proof.length>0&&value.proof.length<=65536,'proof anchor/count');let size=0;for(const node of value.proof){need(isHex(node)&&node.length>2,'proof node');size+=(node.length-2)/2;need(size<=32*1024*1024,'proof byte bound');}return value;}
export async function locate({planPath,output,maxRpc=8,timeoutMs=900_000,rpc}){
  need(Number.isInteger(maxRpc)&&maxRpc>=1&&maxRpc<=4096&&Number.isInteger(timeoutMs)&&timeoutMs>=60_000&&timeoutMs<=3_600_000,'RPC request/time budget');
  const planRaw=await read(planPath),plan=JSON.parse(planRaw),planHash=sha(planRaw);planIdentity(plan);need(sha(await read(plan.source.path))===plan.source.sha256,'source changed');
  await mkdir(output,{recursive:true});for(const d of ['rpc','runs'])await mkdir(join(output,d),{recursive:true});
  const copy=join(output,'plan.json');if(await exists(copy))need((await read(copy)).equals(planRaw),'output plan differs');else await write(copy,planRaw);
  const lockPath=join(output,'LOCK.json'),lock=await open(lockPath,'wx',0o600);await lock.writeFile(json({pid:process.pid,planSha256:planHash}));await lock.sync();await lock.close();
  const ownRpc=!rpc;rpc??=new ReadOnlyRpc(DEFAULT_ENDPOINT,{timeoutMs});let calls=0,boundary=null,error=null,inputGuardPassed=false;const budget=Error('request budget exhausted');
  async function query(method,params){const request={method,params},id=sha(json(request)),file=join(output,'rpc',id+'.json');if(await exists(file)){const r=JSON.parse(await read(file));need(r.planSha256===planHash&&JSON.stringify(r.request)===JSON.stringify(request)&&r.resultSha256===sha(json(r.result)),'cached RPC identity/content differs');return r.result;}if(calls>=maxRpc)throw budget;calls++;try{const result=await rpc.call(method,params);await write(file,json({planSha256:planHash,request,result,resultSha256:sha(json(result))}));return result;}catch(e){await write(join(output,'rpc',id+'.error-'+randomUUID()+'.json'),json({planSha256:planHash,request,error:String(e)}));throw e;}}
  async function probe(number){const hash=await query('chain_getBlockHash',[number]);need(isHex(hash,32),'block hash width');const raw=await query('state_getStorage',[DENOMINATOR_KEY,hash]);return{number,hash,denominator:denominator(raw).toString(),denominatorScale:raw};}
  try{
    const lower=await probe(plan.lower.number),upper=await probe(CHECKPOINT.number);
    need(lower.hash===plan.lower.hash&&lower.denominatorScale===plan.lower.denominatorScale,'lower state differs');need(upper.hash===CHECKPOINT.hash&&BigInt(upper.denominator)===FINAL_FACTOR,'checkpoint state differs');
    const found=await findBoundary(lower,upper,probe);need(BigInt(found.after.denominator)>BigInt(found.before.denominator),'no next transition');
    for(const side of ['before','after']){
      const state=found[side];state.header=checkedHeader(await query('chain_getHeader',[state.hash]),state.number,state.hash);state.version=await query('state_getRuntimeVersion',[state.hash]);
      const block=await query('chain_getBlock',[state.hash]);need(JSON.stringify(block?.block?.header)===JSON.stringify(state.header)&&Array.isArray(block.block.extrinsics)&&block.block.extrinsics.length<=100_000&&block.block.extrinsics.every(e=>isHex(e)&&e.length<=4*1024*1024),'block header/body differs');state.block=block;
      state.events=await query('state_getStorage',[EVENTS_KEY,state.hash]);need(state.events===null||isHex(state.events)&&state.events.length<=64*1024*1024,'events raw bytes/absence');state.proof=proof(await query('state_getReadProof',[[DENOMINATOR_KEY,EVENTS_KEY],state.hash]),state.hash);
      const metadata=await query('state_getMetadata',[state.hash]);need(isHex(metadata)&&metadata.length<=32*1024*1024,'metadata raw bytes');state.metadataScale=metadata;
    }
    need(found.after.header.parentHash===found.before.hash,'transition headers not adjacent');
    boundary={schema:'sora-nexus-offence-denomination-boundary-v1',checkpointHash:CHECKPOINT.hash,planSha256:planHash,...found,factor:(BigInt(found.after.denominator)/BigInt(found.before.denominator)).toString(),historicalStateProofVerified:false,denominationEventDecoded:false,rootCallDecoded:false,nativeCompilerVerificationRequired:true,productionReady:false};
  }catch(e){error=e;}
  finally{
    if(ownRpc)rpc.close();try{need(sha(await read(planPath))===planHash&&(await read(copy)).equals(planRaw)&&sha(await read(plan.source.path))===plan.source.sha256,'input changed during locator');inputGuardPassed=true;}catch(e){error=e;}
    if(boundary&&inputGuardPassed&&!error){boundary.inputGuardPassed=true;const file=join(output,'boundary.json'),raw=json(boundary);if(await exists(file))need((await read(file)).equals(raw),'completed boundary differs');else await write(file,raw);}
    const report={schema:'sora-nexus-offence-denomination-search-run-v1',planSha256:planHash,requestsMade:calls,budgetExhausted:error===budget,error:error&&error!==budget?String(error):null,inputGuardPassed,complete:Boolean(boundary&&inputGuardPassed&&!error),historicalStateProofVerified:false,productionReady:false};
    await write(join(output,'runs',randomUUID()+'.json'),json(report));await unlink(lockPath);if(error&&error!==budget)throw error;return report;
  }
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){const[mode,...args]=process.argv.slice(2);if(mode==='prepare'){need(args.length===2,'prepare SOURCE_CAPTURE_OR_BOUNDARY NEW_PLAN');console.log(JSON.stringify(await prepare({source:resolve(args[0]),output:resolve(args[1])})));}else{need(mode==='locate'&&args.length===3,'locate PLAN OUTPUT MAX_RPC');console.log(JSON.stringify(await locate({planPath:resolve(args[0]),output:resolve(args[1]),maxRpc:Number(args[2])})));}}
