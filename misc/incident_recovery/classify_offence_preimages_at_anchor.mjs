// Offline classifier of already retained exact-key retrieval. Never performs RPC.
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {createRequire} from 'node:module';
import {gunzipSync} from 'node:zlib';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {authenticateOriginal} from './prepare_offence_preimages.mjs';
import {validateChanges,validateProof,validatePlan,DENOMINATOR_KEY} from './capture_offence_preimages_at_anchor.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
export function classify({planPath,capture,output}) {
const assert=(v,m)=>{if(!v)throw Error(m);};
const sha=b=>createHash('sha256').update(b).digest('hex');
const json=v=>Buffer.from(JSON.stringify(v,null,2)+'\n');
const inputPins={};
function read(file){const full=path.resolve(file),s=fs.statSync(full);assert(s.isFile()&&s.size<=64*1024*1024,'file bound');const raw=fs.readFileSync(full);inputPins[full]=sha(raw);return raw;}
const planRaw=read(planPath),plan=JSON.parse(planRaw),planHash=sha(planRaw);validatePlan(plan);
const sourceHash=plan.source.hash;
assert(read(path.join(capture,'plan.json')).equals(planRaw),'captured plan mismatch');
for(const input of plan.inputs)assert(sha(read(input.path))===input.sha256,'planned input drift');
const manifestRaw=read(path.join(root,'artifacts/recovery-28191817/checkpoint/capture-complete.json')),manifest=JSON.parse(manifestRaw);
assert(sha(manifestRaw)==='24d0ea108f2adfbed6e543b7ce6bdb8146ce0e1ecf2b733af35ee9b9a315cc0b','capture drift');
const metadataRaw=read(path.join(root,'artifacts/recovery-28191817/checkpoint/runtime-metadata.scale'));
assert(sha(metadataRaw)==='70fb15b7f598a58fc71a174af66df68b9136a2e87ec51dcd56aa279165cd6ba4','executing metadata drift');
const page=manifest.top.pages.find(p=>p.firstKey<=DENOMINATOR_KEY&&p.lastKey>=DENOMINATOR_KEY);assert(page,'denominator page missing');
const pageRaw=read(path.join(root,'artifacts/recovery-28191817/checkpoint',page.file));assert(sha(pageRaw)===page.sha256,'denominator page hash');
const pageLines=gunzipSync(pageRaw).toString('utf8').trimEnd().split('\n').map(JSON.parse);assert(pageLines.shift().checkpointHash===plan.checkpointHash,'denominator page checkpoint');
const rows=pageLines.filter(r=>r[0]===DENOMINATOR_KEY);assert(rows.length===1&&/^0x[0-9a-f]{32}$/.test(rows[0][1]),'checkpoint denominator row');
const denominator=BigInt('0x'+Buffer.from(rows[0][1].slice(2),'hex').reverse().toString('hex'));
const require=createRequire(path.join(root,'runtime-rehearsal-async-staking-2026-10-25/validation/package.json'));
const {TypeRegistry,Metadata}=require('@polkadot/types');const registry=new TypeRegistry();registry.register(require('@sora-substrate/type-definitions').types);
const metadata=new Metadata(registry,metadataRaw);registry.setMetadata(metadata);
const reportType=metadata.asLatest.toJSON().pallets.find(p=>p.name==='Offences').storage.items.find(i=>i.name==='Reports').type.map.value;
const offenderType=registry.lookup.getSiType(reportType).def.asComposite.fields[0].type.toNumber();
function cache(method,params){const request={method,params},file=path.join(capture,'rpc',sha(json(request))+'.json'),receipt=JSON.parse(read(file));assert(receipt.planSha256===planHash&&JSON.stringify(receipt.request)===JSON.stringify(request),'RPC request join');assert(receipt.resultSha256===sha(json(receipt.result)),'RPC result digest');return receipt.result;}
assert(denominator.toString()===plan.checkpointDenominator,'checkpoint denominator differs');
assert(cache('chain_getBlockHash',[manifest.checkpoint.number])===plan.checkpointHash,'captured checkpoint identity');
assert(cache('chain_getBlockHash',[plan.source.number])===sourceHash&&JSON.stringify(cache('chain_getHeader',[sourceHash]))===JSON.stringify(plan.source.header),'captured source identity');
assert(cache('state_getStorage',[DENOMINATOR_KEY,sourceHash])===plan.source.denominatorScale,'source denominator differs');
const ratio=BigInt(plan.denominationRatio);
const matched=[],absent=[],pending=[],contradictory=[],batches=[];
for(let start=0;start<plan.requests.length;start+=plan.batchSize){
  const requests=plan.requests.slice(start,start+plan.batchSize),id=String(start/plan.batchSize).padStart(6,'0'),dir=path.join(capture,'batches',id);
  if(!fs.existsSync(path.join(dir,'receipt.json'))){pending.push(...requests.map(r=>r.id));continue;}
  const receipt=JSON.parse(read(path.join(dir,'receipt.json'))),requested=JSON.parse(read(path.join(dir,'requested-reports.json'))),history=JSON.parse(read(path.join(dir,'historical-capture.json'))),keys=[DENOMINATOR_KEY,...requests.map(r=>r.reportKey)];
  assert(JSON.stringify(requested.affected)===JSON.stringify(requests),'batch request join');
  assert(receipt.schema==='sora-nexus-offence-preimage-anchor-batch-v1'&&receipt.planSha256===planHash&&receipt.firstRequest===start&&receipt.requests===requests.length&&receipt.historicalStateProofVerified===false&&receipt.originalPreimagesAuthenticated===false&&receipt.productionReady===false,'batch receipt join');
  assert(history.states.length===1&&history.states[0].hash===sourceHash&&history.historicalStateProofVerified===false&&history.productionReady===false,'history scope');
  const source=history.states[0],values=validateChanges(cache('state_queryStorageAt',[keys,sourceHash]),keys,sourceHash),proof=validateProof(cache('state_getReadProof',[keys,sourceHash]),sourceHash);
  assert(JSON.stringify(source.values)===JSON.stringify(values)&&JSON.stringify(source.proof)===JSON.stringify(proof),'batch source bytes join');
  const valuesByKey=new Map(values[0].changes);assert(valuesByKey.get(DENOMINATOR_KEY)===plan.source.denominatorScale,'source denominator drift');
  const absentIds=requests.filter(r=>valuesByKey.get(r.reportKey)===null).map(r=>r.id);assert(JSON.stringify(receipt.absentIds)===JSON.stringify(absentIds),'absent receipt join');
  for(const r of requests){const originalValue=valuesByKey.get(r.reportKey);if(originalValue===null){absent.push({id:r.id,slot:r.slot,slotNumber:r.slotNumber});continue;}
    try{const offender=authenticateOriginal({registry,reportType,offenderType,kind:r.kind,slot:r.slot,reportId:r.id,originalValue,currentValue:r.reportValue,denominator:ratio});matched.push({reportId:r.id,offender,denominator:ratio.toString(),slot:r.slot,slotNumber:r.slotNumber,sourceBatch:id,sourceValueSha256:sha(Buffer.from(originalValue.slice(2),'hex'))});}
    catch(error){contradictory.push({id:r.id,error:String(error),sourceBatch:id});}
  }
  batches.push(id);
}
for(const[file,digest]of Object.entries(inputPins))assert(sha(fs.readFileSync(file))===digest,'input changed during classification');
const summary={schema:'sora-nexus-offence-retrieved-preimage-classification-v1',planSha256:planHash,sourceHash,sourceNumber:plan.source.number,checkpointDenominator:denominator.toString(),sourceDenominator:plan.source.denominator,completedBatches:batches,requestedReports:plan.requests.length,matchedOriginals:matched.length,absentOriginals:absent.length,pendingOriginals:pending.length,contradictoryOriginals:contradictory.length,allRequiredOriginalsAuthenticated:matched.length===plan.requests.length&&contradictory.length===0,historicalStateProofVerified:false,nativeCompilerVerificationRequired:true,productionReady:false,inputPins,matched,absent,pending,contradictory};
fs.writeFileSync(output,json(summary),{flag:'wx'});
return {...summary,inputPins:undefined,matched:undefined,absent:undefined,pending:undefined,contradictory:undefined};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(path.resolve(process.argv[1])).href){
  if(process.argv.length!==5)throw Error('Usage: classify_offence_preimages_at_anchor.mjs PLAN CAPTURE_DIRECTORY NEW_OUTPUT');
  const [planPath,capture,output]=process.argv.slice(2).map(p=>path.resolve(p));
  console.log(JSON.stringify(classify({planPath,capture,output})));
}
