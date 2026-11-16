#!/usr/bin/env node
/** Complete ordered native HASHI namespace commitments. Public fixture only. */
import {createReadStream} from 'node:fs';
import {readFile,writeFile,mkdir,stat} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createGunzip} from 'node:zlib';
import {Readable} from 'node:stream';
import {createRequire} from 'node:module';
import {resolve,join,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {selectedRows} from './prepare_scheduler_review.mjs';
import {CHECKPOINT} from './capture_checkpoint.mjs';

const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
const require=createRequire(join(root,'runtime-rehearsal-sdk2609-2026-10-25/validation/package.json'));
const {Metadata,TypeRegistry}=require('@polkadot/types');
const {xxhashAsU8a,blake2AsU8a,cryptoWaitReady}=require('@polkadot/util-crypto');
await cryptoWaitReady();
const assert=(ok,message)=>{if(!ok)throw Error(message);};
const sha=data=>createHash('sha256').update(data).digest('hex');
const hx=data=>'0x'+Buffer.from(data).toString('hex');
const blake=data=>Buffer.from(blake2AsU8a(data,256));
const bytes=value=>{assert(/^0x(?:[0-9a-f]{2})*$/.test(value),'canonical hex');return Buffer.from(value.slice(2),'hex');};
const encoded=value=>Buffer.from(JSON.stringify(value,null,2)+'\n');
async function bounded(path){assert((await stat(path)).size<=64*1024**2,'bounded public file');const data=await readFile(path);assert(data.length<=64*1024**2,'input grew');return data;}
function scaleKey(key){
 assert(key.length>0&&key.length<=512,'native storage key bound');
 if(key.length<64)return Buffer.concat([Buffer.from([key.length<<2]),key]);
 const length=Buffer.alloc(2);length.writeUInt16LE((key.length<<2)|1);return Buffer.concat([length,key]);
}
assert(process.argv.length===4,'Usage: extend_victim_hashi_fixture.mjs PRIOR_FIXTURE NEW_FIXTURE');
const [prior,output]=process.argv.slice(2).map(value=>resolve(value));
const priorBytes=await bounded(join(prior,'FIXTURE-PROVENANCE.json')),outer=JSON.parse(priorBytes);
const args=JSON.parse(await bounded(join(prior,'COMPOSE-ARGS.json')));
assert(outer.fixtureOnly===true&&outer.productionReady===false&&args.fixtureOnly===true&&args.productionReady===false,'public fixture only');
assert(args['evidence-commitment']===hx(blake(priorBytes)),'prior fixture commitment');
const capture=resolve(args.capture),manifestBytes=await bounded(join(capture,'capture-complete.json')),manifest=JSON.parse(manifestBytes);
const verificationBytes=await bounded(join(capture,'../checkpoint-verification.json')),verification=JSON.parse(verificationBytes);
const witnessBytes=await bounded(join(capture,'../checkpoint-metadata-witness.json')),witness=JSON.parse(witnessBytes),metadataBytes=await bounded(join(capture,'runtime-metadata.scale'));
assert(manifest.complete===true&&manifest.checkpoint.hash===CHECKPOINT.hash&&verification.stateRootVerified===true&&verification.headerHashVerified===true&&verification.captureManifestSha256===sha(manifestBytes),'verified exact checkpoint required');
assert(witness.wasmExecutionVerified===true&&witness.metadataSha256===sha(metadataBytes)&&manifest.artifacts['runtime-metadata.scale'].sha256===sha(metadataBytes),'executed metadata witness required');
const registry=new TypeRegistry();registry.setMetadata(new Metadata(registry,metadataBytes));
const metadata=registry.metadata.toJSON(),pallet=metadata.pallets.find(p=>p.name==='EthBridge');
assert(pallet?.storage?.prefix==='EthBridge','native HASHI metadata');
const names=['Requests','RequestStatuses','RequestApprovals','RequestApprovers','RequestSubmissionHeight','RequestsQueue','AccountRequests'];
const descriptors=names.map(name=>{
 assert(pallet.storage.items.some(entry=>entry.name===name),'missing exact native prefix '+name);
 const storage='EthBridge.'+name;
 return {storage,prefix:hx(Buffer.concat([xxhashAsU8a('EthBridge',128),xxhashAsU8a(name,128)])),records:0,lastKey:null,
 commitment:blake(Buffer.from('sora-nexus-hashi-prefix-v1:'+storage)),valueBytes:0};
});
const pages=[];let totalBytes=0,totalRecords=0;
for(const page of manifest.top.pages){
 if(!page.keyCount||!descriptors.some(d=>page.firstKey<=d.prefix+'f'.repeat(1024)&&page.lastKey>=d.prefix))continue;
 assert(/^pages\/top\/[0-9]{8}\.jsonl\.gz$/.test(page.file),'safe capture page');
 const digest=createHash('sha256');
 async function* authenticatedCompressed(){for await(const chunk of createReadStream(join(capture,page.file))){digest.update(chunk);yield chunk;}}
 for await(const [key,value] of selectedRows(Readable.from(authenticatedCompressed()).pipe(createGunzip()),descriptors.map(d=>d.prefix))){
  const d=descriptors.find(item=>key.startsWith(item.prefix));assert(d&&(d.lastKey===null||key>d.lastKey),'strict canonical namespace key order');
  const rawKey=bytes(key),rawValue=bytes(value);totalBytes+=rawValue.length;totalRecords++;
  assert(totalBytes<=512*1024**2&&totalRecords<=1_000_000,'complete HASHI namespace bounds');
  d.commitment=blake(Buffer.concat([d.commitment,scaleKey(rawKey),blake(rawValue)]));
  d.lastKey=key;d.records++;d.valueBytes+=rawValue.length;
 }
 assert(digest.digest('hex')===page.sha256,'same-stream page checksum mismatch');pages.push({file:page.file,sha256:page.sha256});
}
const prefixes=descriptors.map(d=>({storage:d.storage,records:d.records,commitment:hx(d.commitment)}));
const evidence={schema:'sora-nexus-public-victim-native-hashi-extension-v1',fixtureOnly:true,productionReady:false,checkpointHash:CHECKPOINT.hash,
 priorProvenanceSha256:sha(priorBytes),captureManifestSha256:sha(manifestBytes),checkpointVerificationSha256:sha(verificationBytes),executingMetadataSha256:sha(metadataBytes),metadataWitnessSha256:sha(witnessBytes),
 prefixes,pages,totalRecords,totalValueBytes:totalBytes,
 commitmentCodec:'Initial Blake2-256(UTF8(sora-nexus-hashi-prefix-v1: + exact storage name)); each row in ascending raw-key order: Blake2-256(SCALE(previous [u8;32], key Vec<u8>, Blake2-256(value) [u8;32])).',
 policy:'Permanently stop every captured native outgoing request without refunds or native signature/status mutation. The compiler must validate every source namespace and outgoing wrapper identity; this helper does not infer ownership or external redemption.',
 identity:'Network/map-key request hash plus native stored wrapper hash. No financial-payload hash is recomputed, and no original denomination preimage is invented.',sources:[]};
for(const path of ['utils/incident-recovery/src/bridge_hashi.rs','pallets/eth-bridge/src/requests/mod.rs','pallets/eth-bridge/src/recovery.rs','vendor/sora2-common/pallets/types/src/recovery.rs','misc/incident_recovery/extend_victim_hashi_fixture.mjs'])evidence.sources.push({path,sha256:sha(await bounded(join(root,path)))});
const evidenceBytes=encoded(evidence),evidenceHash=hx(blake(evidenceBytes));
await mkdir(output,{recursive:false});const files=[],seen=new Set();
for(const record of outer.files){
 assert(/^[a-zA-Z0-9._-]+$/.test(record.file)&&!seen.has(record.file),'safe unique prior input');seen.add(record.file);
 const data=await bounded(join(prior,record.file));assert(data.length===record.bytes&&sha(data)===record.sha256,'exact prior artifact');
 await writeFile(join(output,record.file),data,{flag:'wx'});files.push(record);
}
for(const [name,data] of [['bridge-hashi.json',encoded({evidenceHash,cancelAllNativeOutgoing:true,prefixes})],['HASHI-EXTENSION.json',evidenceBytes]]){
 assert(!seen.has(name),'new artifact collision');await writeFile(join(output,name),data,{flag:'wx'});files.push({file:name,bytes:data.length,sha256:sha(data)});
}
outer.files=files;outer.previousFixture={path:prior,provenanceSha256:sha(priorBytes)};outer.nativeHashiExtension={file:'HASHI-EXTENSION.json',sha256:sha(evidenceBytes)};
const outerBytes=encoded(outer);await writeFile(join(output,'FIXTURE-PROVENANCE.json'),outerBytes,{flag:'wx'});
for(const [name,value] of Object.entries(args))if(typeof value==='string'&&value.startsWith(prior+'/'))args[name]=output+value.slice(prior.length);
args['bridge-hashi']=join(output,'bridge-hashi.json');args['evidence-commitment']=hx(blake(outerBytes));
await writeFile(join(output,'COMPOSE-ARGS.json'),encoded(args),{flag:'wx'});
console.log(JSON.stringify({output,totalRecords,totalValueBytes:totalBytes,prefixes,evidenceCommitment:args['evidence-commitment'],files:files.length,fixtureOnly:true,productionReady:false}));
