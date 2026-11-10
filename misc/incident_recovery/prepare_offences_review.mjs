#!/usr/bin/env node
/** Offline complete Offences review from independently verified captured state. */
import {createReadStream} from 'node:fs';
import {readFile,mkdir,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createGunzip} from 'node:zlib';
import {createRequire} from 'node:module';
import {resolve,join,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {selectedRows} from './prepare_scheduler_review.mjs';
import {CHECKPOINT} from './capture_checkpoint.mjs';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
const capture=resolve(process.argv[2]??join(root,'artifacts/recovery-28191817/checkpoint'));
const out=resolve(process.argv[3]??join(root,'artifacts/recovery-28191817/offences-review-v1'));
const sha=b=>createHash('sha256').update(b).digest('hex');const hex=b=>'0x'+Buffer.from(b).toString('hex');
const assert=(x,message)=>{if(!x)throw Error(message);};
const require=createRequire(join(root,'runtime-rehearsal-async-staking-2026-10-25/validation/package.json'));
const{xxhashAsU8a,blake2AsU8a}=require('@polkadot/util-crypto');
const raw=await readFile(join(capture,'capture-complete.json')),manifest=JSON.parse(raw);
const verifiedRaw=await readFile(join(capture,'../checkpoint-verification.json')),verified=JSON.parse(verifiedRaw);
const witnessRaw=await readFile(join(capture,'../checkpoint-metadata-witness.json')),witness=JSON.parse(witnessRaw);
const metadata=await readFile(join(capture,'runtime-metadata.scale'));
assert(manifest.checkpoint.hash===CHECKPOINT.hash&&manifest.complete&&verified.stateRootVerified&&verified.captureManifestSha256===sha(raw)&&verified.checkpointHash===CHECKPOINT.hash,'complete independently verified checkpoint required');
assert(witness.wasmExecutionVerified&&witness.checkpointHash===CHECKPOINT.hash&&witness.metadataSha256===sha(metadata)&&manifest.artifacts['runtime-metadata.scale'].sha256===sha(metadata),'executing metadata witness required');
const descriptors=['Reports','ConcurrentReportsIndex'].map(item=>({storage:'Offences.'+item,prefix:hex(Buffer.concat([xxhashAsU8a('Offences',128),xxhashAsU8a(item,128)]))}));
const records=[],pages=[],counts=Object.fromEntries(descriptors.map(d=>[d.storage,0]));let total=0;
for(const page of manifest.top.pages){
 if(!page.keyCount||!descriptors.some(d=>page.firstKey<=d.prefix+'f'.repeat(1024)&&page.lastKey>=d.prefix))continue;
 assert(/^pages\/top\/[0-9]{8}\.jsonl\.gz$/.test(page.file),'page path');const path=join(capture,page.file),h=createHash('sha256');for await(const chunk of createReadStream(path))h.update(chunk);assert(h.digest('hex')===page.sha256,'capture page hash mismatch');pages.push({file:page.file,sha256:page.sha256});
 for await(const[key,value]of selectedRows(createReadStream(path).pipe(createGunzip()),descriptors.map(d=>d.prefix))){
  assert(/^0x(?:[0-9a-f]{2})*$/.test(value),'canonical hex');const bytes=Buffer.from(value.slice(2),'hex');total+=bytes.length;assert(bytes.length<=1024*1024&&total<=64*1024*1024&&records.length<100000,'offences graph bounds');const d=descriptors.find(d=>key.startsWith(d.prefix));records.push({storage:d.storage,key,valueHash:hex(blake2AsU8a(bytes,256))});counts[d.storage]++;
 }
}
records.sort((a,b)=>a.key.localeCompare(b.key));assert(records.every((r,i)=>i===0||records[i-1].key<r.key),'duplicate offence row');
const sources=[];for(const file of ['misc/incident_recovery/prepare_offences_review.mjs','misc/incident_recovery/prepare_scheduler_review.mjs','utils/incident-recovery/src/offences.rs'])sources.push({file,sha256:sha(await readFile(join(root,file)))});
const evidence={schema:'sora-nexus-offences-graph-evidence-v1',checkpointHash:CHECKPOINT.hash,captureManifestSha256:sha(raw),nativeVerificationSha256:sha(verifiedRaw),executingMetadataWitnessSha256:sha(witnessRaw),metadataSha256:sha(metadata),sources,pages,counts,totalEncodedBytes:total,recordsSha256:sha(JSON.stringify(records)),nativeGraphVerificationPerformed:false,productionReady:false};
const evidenceBytes=JSON.stringify(evidence,null,2)+'\n';const review={evidenceHash:hex(blake2AsU8a(Buffer.from(evidenceBytes),256)),records};await mkdir(out,{recursive:false});await writeFile(join(out,'evidence.json'),evidenceBytes);await writeFile(join(out,'review.json'),JSON.stringify(review,null,2)+'\n');console.log(JSON.stringify({out,counts,totalEncodedBytes:total,productionReady:false}));
