#!/usr/bin/env node
/** Exact byte-preservation review for unresolved BridgeProxy ownership anomalies. */
import {readFile,writeFile,mkdir,stat} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {gunzipSync} from 'node:zlib';
import {createRequire} from 'node:module';
import {resolve,join,dirname} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {CHECKPOINT} from './capture_checkpoint.mjs';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
const require=createRequire(join(root,'runtime-rehearsal-sdk2609-2026-10-25/validation/package.json'));
const {Metadata,TypeRegistry}=require('@polkadot/types');
const {xxhashAsU8a,blake2AsU8a}=require('@polkadot/util-crypto');
const assert=(ok,m)=>{if(!ok)throw Error(m);},hx=b=>'0x'+Buffer.from(b).toString('hex');
const bytes=h=>{assert(/^0x(?:[0-9a-f]{2})*$/.test(h),'canonical hex');return Buffer.from(h.slice(2),'hex');};
const sha=b=>createHash('sha256').update(b).digest('hex'),blake=b=>hx(blake2AsU8a(b,256));
async function bounded(p,n=64*1024**2){assert((await stat(p)).size<=n,'input size');const b=await readFile(p);assert(b.length<=n,'input grew');return b;}
export async function prepare(baseReview,output){
 const capture=join(root,'artifacts/recovery-28191817/checkpoint');
 const baseBytes=await bounded(baseReview),review=JSON.parse(baseBytes);
 assert(review.cancelAllTerminalOutgoing===true&&!review.preservedAnomalies?.length,'complete base review without pre-existing exceptions');
 const manifestBytes=await bounded(join(capture,'capture-complete.json')),manifest=JSON.parse(manifestBytes);
 const verification=JSON.parse(await bounded(join(capture,'../checkpoint-verification.json')));
 const metadataBytes=await bounded(join(capture,'runtime-metadata.scale'));
 const witness=JSON.parse(await bounded(join(capture,'../checkpoint-metadata-witness.json')));
 assert(manifest.complete&&manifest.checkpoint.hash===CHECKPOINT.hash&&verification.stateRootVerified&&verification.headerHashVerified&&verification.captureManifestSha256===sha(manifestBytes)&&witness.wasmExecutionVerified&&witness.metadataSha256===sha(metadataBytes),'verified checkpoint and executing metadata required');
 const registry=new TypeRegistry();registry.setMetadata(new Metadata(registry,metadataBytes));
 const metadata=registry.metadata.toJSON(),pallet=metadata.pallets.find(p=>p.name==='BridgeProxy');
 const descriptors=['Senders','Transactions'].map(name=>{const e=pallet.storage.items.find(e=>e.name===name);return{name,entry:e,prefix:hx(Buffer.concat([xxhashAsU8a(pallet.storage.prefix,128),xxhashAsU8a(name,128)])),types:metadata.lookup.types.find(t=>t.id===e.type.map.key).type.def.tuple};});
 const reviewed=new Map(review.records.map(r=>[r.key,r]));assert(reviewed.size===review.records.length,'duplicate review');
 const senders=new Map(),transactions=[],pages=[];let records=0;
 for(const page of manifest.top.pages){
  if(!page.keyCount||!descriptors.some(d=>page.firstKey<=d.prefix+'f'.repeat(1024)&&page.lastKey>=d.prefix))continue;
  assert(/^pages\/top\/[0-9]{8}\.jsonl\.gz$/.test(page.file),'page path');
  const compressed=await bounded(join(capture,page.file),256*1024**2);assert(sha(compressed)===page.sha256,'page hash');pages.push({file:page.file,sha256:page.sha256});
  const lines=gunzipSync(compressed,{maxOutputLength:256*1024**2}).toString().trimEnd().split('\n');const header=JSON.parse(lines.shift());assert(header.checkpointHash===CHECKPOINT.hash&&header.keyCount===lines.length,'page identity');
  for(const line of lines){const[k,v]=JSON.parse(line);const d=descriptors.find(d=>k.startsWith(d.prefix));if(!d)continue;
   const row=reviewed.get(k);assert(row?.storage==='BridgeProxy.'+d.name&&row.valueHash===blake(bytes(v)),'exact base review');records++;
   let tail=bytes('0x'+k.slice(d.prefix.length)),parts=[];
   for(const[index,hasher]of d.entry.type.map.hashers.entries()){
    assert(['Blake2_128Concat','Twox64Concat'].includes(hasher),'key hasher');const size=hasher==='Twox64Concat'?8:16;
    const part=registry.createTypeUnsafe('Lookup'+d.types[index],[tail.subarray(size)]),encoded=Buffer.from(part.toU8a()),digest=hasher==='Twox64Concat'?xxhashAsU8a(encoded,64):blake2AsU8a(encoded,128);
    assert(tail.subarray(0,size).equals(Buffer.from(digest))&&tail.subarray(size,size+encoded.length).equals(encoded),'key codec/hash');tail=tail.subarray(size+encoded.length);parts.push(part);
   }
   assert(!tail.length,'key trailing bytes');const value=registry.createTypeUnsafe('Lookup'+d.entry.type.map.value,[bytes(v)]);assert(hx(value.toU8a())===v,'value codec');
   const network=d.name==='Senders'?parts[0]:parts[0][0],owner=d.name==='Senders'?value:parts[0][1],message=parts[1];
   const identity=hx(network.toU8a())+':'+message.toHex(),record={key:k,value:v,hash:row.valueHash,owner:hx(owner.toU8a()),identity,networkScale:hx(network.toU8a()),messageId:message.toHex()};
   if(d.name==='Senders'){assert(!senders.has(identity),'duplicate sender');senders.set(identity,record);}else transactions.push({...record,codec:value});
  }
 }
 assert(records===review.records.length&&senders.size===transactions.length,'complete base graph coverage');
 const anomalies=[],seen=new Set();for(const transaction of transactions){
  const sender=senders.get(transaction.identity);assert(sender?.owner===transaction.owner&&!seen.has(transaction.identity),'pair owner/identity');seen.add(transaction.identity);
  const native=transaction.codec,local=native.direction.isInbound?native.dest:native.source;
  if(local.isSora&&hx(local.asSora.toU8a())===transaction.owner)continue;
  anomalies.push({senderKey:sender.key,senderValueHash:sender.hash,transactionKey:transaction.key,transactionValueHash:transaction.hash,reason:'nativeOwnerMismatch',networkScale:transaction.networkScale,messageId:transaction.messageId});
 }
 assert(anomalies.length>0&&anomalies.length<=256,'bounded nonempty exception set');
 const evidence={schema:'sora-nexus-bridge-unresolved-anomaly-preservation-v1',fixtureOnly:true,productionReady:false,checkpointHash:CHECKPOINT.hash,baseReviewSha256:sha(baseBytes),baseEvidenceHash:review.evidenceHash,captureManifestSha256:sha(manifestBytes),metadataSha256:sha(metadataBytes),pages,pairedRequests:transactions.length,anomalies,
  disposition:'Preserve the exact sender index and request bytes; quarantine refunds/outbound recreation without interpreting financial ownership, amounts or direction. Require separate reconciliation before production completion.',
  sourceInterpretation:'Current native owner invariant fails. The retained generic_account_v2 migration translated the older GenericAccount variant order; these bytes are consistent with historical layout misinterpretation, but do not prove the original request. No old-layout reconstruction is attempted.',
  nativeStatusAndInboundGuardsComplete:false,sources:[]};
 for(const path of ['pallets/bridge-proxy/src/lib.rs','pallets/bridge-proxy/src/migrations.rs','vendor/sora2-common/pallets/types/src/lib.rs','vendor/sora2-common/pallets/types/src/recovery.rs','utils/incident-recovery/src/bridge_history.rs','misc/incident_recovery/prepare_bridge_history_anomalies.mjs'])evidence.sources.push({path,sha256:sha(await bounded(join(root,path)))});
 const evidenceBytes=Buffer.from(JSON.stringify(evidence,null,2)+'\n'),evidenceHash=blake(evidenceBytes);
 review.evidenceHash=evidenceHash;review.preservedAnomalies=anomalies.map(({networkScale,messageId,...row})=>({...row,sourceEvidenceHash:evidenceHash}));
 await mkdir(output,{recursive:false});await writeFile(join(output,'evidence.json'),evidenceBytes,{flag:'wx'});await writeFile(join(output,'review.json'),JSON.stringify(review,null,2)+'\n',{flag:'wx'});
 return{output,anomalies:anomalies.length,pairedRequests:transactions.length,evidenceHash,productionReady:false};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){assert(process.argv.length===4,'Usage: prepare_bridge_history_anomalies.mjs BASE_REVIEW NEW_OUTPUT_DIRECTORY');console.log(JSON.stringify(await prepare(resolve(process.argv[2]),resolve(process.argv[3]))));}
