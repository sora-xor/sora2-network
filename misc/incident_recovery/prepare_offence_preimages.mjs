#!/usr/bin/env node
/** Bind self-authenticating historical offence identities to a complete checkpoint review.
 * Historical headers/proofs are retained as source evidence. Admission depends on the
 * checkpoint ReportId hash and the native compiler's independent denomination check.
 */
import {createReadStream} from 'node:fs';
import {readFile,stat,mkdir,writeFile} from 'node:fs/promises';
import {createGunzip} from 'node:zlib';
import {createHash} from 'node:crypto';
import {createRequire} from 'node:module';
import {resolve,join,dirname} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {selectedRows} from './prepare_scheduler_review.mjs';
import {CHECKPOINT,DEFAULT_ENDPOINT} from './capture_checkpoint.mjs';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
const require=createRequire(join(root,'runtime-rehearsal-async-staking-2026-10-25/validation/package.json'));
const {Metadata,TypeRegistry}=require('@polkadot/types');
const {xxhashAsU8a,blake2AsU8a}=require('@polkadot/util-crypto');
const {compactToU8a}=require('@polkadot/util');
const sha=b=>createHash('sha256').update(b).digest('hex');
const hex=b=>'0x'+Buffer.from(b).toString('hex');
const bytes=h=>{if(typeof h!=='string'||!/^0x(?:[0-9a-f]{2})*$/.test(h))throw Error('canonical hex required');return Buffer.from(h.slice(2),'hex');};
const assert=(condition,message)=>{if(!condition)throw Error(message);};
async function bounded(file,limit=64*1024*1024){assert((await stat(file)).size<=limit,'input byte bound');const b=await readFile(file);assert(b.length<=limit,'input grew past bound');return b;}
export function authenticateOriginal({registry,reportType,offenderType,kind,slot,reportId,originalValue,currentValue,denominator}){
  assert(denominator>1n,'nontrivial positive denomination required');
  const original=registry.createTypeUnsafe('Lookup'+reportType,[bytes(originalValue)]);
  assert(hex(original.toU8a())===originalValue,'historical report codec mismatch');
  const offender=Buffer.from(original.offender.toU8a());
  const encoded=Buffer.concat([bytes(kind),compactToU8a(bytes(slot).length),bytes(slot),offender]);
  assert(hex(blake2AsU8a(encoded,256))===reportId,'original offender does not hash to checkpoint ReportId');
  const prior=original.offender[1];
  const own=BigInt(prior.own.toString())/denominator;
  const others=Array.from(prior.others,n=>({who:n.who.toHex(),value:(BigInt(n.value.toString())/denominator).toString()}));
  const total=others.reduce((sum,n)=>sum+BigInt(n.value),own);
  const after=registry.createTypeUnsafe('Lookup'+offenderType,[[original.offender[0].toHex(),{total:total.toString(),own:own.toString(),others}]]);
  const current=registry.createTypeUnsafe('Lookup'+reportType,[bytes(currentValue)]);
  assert(hex(current.toU8a())===currentValue&&hex(after.toU8a())===hex(current.offender.toU8a()),'denomination does not preserve exact checkpoint account order and values');
  return hex(offender);
}
export async function prepare({baseReview,affected,historicalCapture,output}){
  const reviewBytes=await bounded(baseReview),review=JSON.parse(reviewBytes);
  const affectedBytes=await bounded(affected,4*1024*1024),requests=JSON.parse(affectedBytes).affected;
  const historicalBytes=await bounded(historicalCapture),history=JSON.parse(historicalBytes);
  assert(Array.isArray(requests)&&requests.length>0&&requests.length<=64,'1..64 explicit historical preimage requests required');
  assert(history.schema==='sora-nexus-historical-offence-preimage-capture-v1'&&history.endpoint===DEFAULT_ENDPOINT&&history.checkpoint.hash===CHECKPOINT.hash,'historical capture identity');
  const capture=join(root,'artifacts/recovery-28191817/checkpoint');
  const manifestBytes=await bounded(join(capture,'capture-complete.json')),manifest=JSON.parse(manifestBytes);
  const verified=JSON.parse(await bounded(join(capture,'../checkpoint-verification.json')));
  const metadataBytes=await bounded(join(capture,'runtime-metadata.scale'));
  const witness=JSON.parse(await bounded(join(capture,'../checkpoint-metadata-witness.json')));
  assert(manifest.checkpoint.hash===CHECKPOINT.hash&&manifest.complete&&verified.stateRootVerified&&verified.captureManifestSha256===sha(manifestBytes)&&witness.wasmExecutionVerified&&witness.metadataSha256===sha(metadataBytes),'independently verified checkpoint and executing metadata required');
  const registry=new TypeRegistry();registry.register(require('@sora-substrate/type-definitions').types);
  const metadata=new Metadata(registry,metadataBytes);registry.setMetadata(metadata);
  const mj=metadata.asLatest.toJSON(),reports=mj.pallets.find(p=>p.name==='Offences').storage.items.find(i=>i.name==='Reports');
  const reportType=reports.type.map.value,offenderType=registry.lookup.getSiType(reportType).def.asComposite.fields[0].type.toNumber();
  const denominatorKey=hex(Buffer.concat([xxhashAsU8a('Denomination',128),xxhashAsU8a('Denominator',128)]));
  const wanted=[denominatorKey,...requests.map(r=>r.reportKey)],captured=new Map(),pages=[];
  for(const page of manifest.top.pages){
    if(!page.keyCount||!wanted.some(k=>page.firstKey<=k&&page.lastKey>=k))continue;
    assert(/^pages\/top\/[0-9]{8}\.jsonl\.gz$/.test(page.file),'capture page path');const file=join(capture,page.file),h=createHash('sha256');for await(const part of createReadStream(file))h.update(part);assert(h.digest('hex')===page.sha256,'capture page hash');pages.push({file:page.file,sha256:page.sha256});
    for await(const[k,v]of selectedRows(createReadStream(file).pipe(createGunzip()),wanted))if(wanted.includes(k))captured.set(k,v);
  }
  assert(captured.size===new Set(wanted).size,'missing authenticated checkpoint rows');
  const decodeDenominator=v=>{assert(bytes(v).length===16,'denominator u128 width');return BigInt('0x'+bytes(v).reverse().toString('hex'));};
  const cumulative=decodeDenominator(captured.get(denominatorKey));
  const source=history.states[0];assert(source&&source.number<CHECKPOINT.number&&Number(BigInt(source.header.number))===source.number&&source.proof.at===source.hash,'historical source anchor');
  assert(source.values.length===1&&source.values[0].block===source.hash,'historical storage response block');
  const sourceRows=new Map(source.values[0].changes);
  const prior=sourceRows.get(denominatorKey)===null?1n:decodeDenominator(sourceRows.get(denominatorKey));
  assert(prior>0n&&cumulative%prior===0n,'checkpoint cumulative denomination ratio');const factor=cumulative/prior;
  const seen=new Set(),preimages=[];
  for(const r of requests){
    assert(!seen.has(r.id)&&bytes(r.id).length===32&&bytes(r.kind).length===16&&bytes(r.slot).length>0&&bytes(r.slot).length<=1024,'duplicate or invalid report identity');seen.add(r.id);
    const key=hex(Buffer.concat([xxhashAsU8a('Offences',128),xxhashAsU8a('Reports',128),xxhashAsU8a(bytes(r.id),64),bytes(r.id)]));
    assert(key===r.reportKey&&captured.get(key)===r.reportValue,'request must match exact checkpoint report');
    const reviewed=review.records.filter(e=>e.key===key);assert(reviewed.length===1&&reviewed[0].storage==='Offences.Reports'&&reviewed[0].valueHash===hex(blake2AsU8a(bytes(r.reportValue),256)),'report not bound by base review');
    const originalValue=sourceRows.get(key);assert(originalValue&&bytes(originalValue).length<=1024*1024,'missing/bounded original record');
    const offender=authenticateOriginal({registry,reportType,offenderType,kind:r.kind,slot:r.slot,reportId:r.id,originalValue,currentValue:r.reportValue,denominator:factor});
    preimages.push({reportId:r.id,offender,denominator:factor.toString()});
  }
  const evidence={schema:'sora-nexus-offence-original-preimage-evidence-v1',checkpointHash:CHECKPOINT.hash,baseReviewSha256:sha(reviewBytes),baseEvidenceHash:review.evidenceHash,historicalCaptureSha256:sha(historicalBytes),requestsSha256:sha(affectedBytes),metadataSha256:sha(metadataBytes),captureManifestSha256:sha(manifestBytes),pages,sourceBlock:{number:source.number,hash:source.hash,stateRoot:source.header.stateRoot},checkpointDenominator:cumulative.toString(),sourceDenominator:prior.toString(),reportIds:preimages.map(p=>p.reportId),authentication:'Each original offender hashes to its retained checkpoint ReportId, and source denomination reproduces the exact checkpoint ordered account graph and financial values.',historicalStateProofVerified:false,nativeCompilerVerificationRequired:true,productionReady:false,sources:[]};
  for(const file of ['misc/incident_recovery/prepare_offence_preimages.mjs','utils/incident-recovery/src/offences.rs','pallets/denomination/src/lib.rs'])evidence.sources.push({file,sha256:sha(await bounded(join(root,file)))});
  const evidenceBytes=Buffer.from(JSON.stringify(evidence,null,2)+'\n'),evidenceHash=hex(blake2AsU8a(evidenceBytes,256));
  review.evidenceHash=evidenceHash;review.originalOffenders=preimages.map(p=>({...p,sourceEvidenceHash:evidenceHash}));
  await mkdir(output,{recursive:false});
  for(const[file,value]of[['evidence.json',evidenceBytes],['review.json',Buffer.from(JSON.stringify(review,null,2)+'\n')],['historical-capture.json',historicalBytes],['requested-reports.json',affectedBytes]])await writeFile(join(output,file),value,{flag:'wx'});
  return {output,originalPreimages:preimages.length,records:review.records.length,evidenceHash,sourceBlock:source.number,productionReady:false};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  assert(process.argv.length===6,'Usage: prepare_offence_preimages.mjs BASE_REVIEW AFFECTED_REPORTS HISTORICAL_CAPTURE NEW_OUTPUT_DIRECTORY');
  const [baseReview,affected,historicalCapture,output]=process.argv.slice(2).map(p=>resolve(p));
  console.log(JSON.stringify(await prepare({baseReview,affected,historicalCapture,output})));
}
