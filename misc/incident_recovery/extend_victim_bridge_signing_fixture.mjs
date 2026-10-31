#!/usr/bin/env node
/** Immutable PUBLIC FIXTURE extension. No production authorization is inferred. */
import {createReadStream} from 'node:fs';
import {readFile, writeFile, mkdir, stat} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createGunzip} from 'node:zlib';
import {Readable} from 'node:stream';
import {createRequire} from 'node:module';
import {resolve, join, dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {selectedRows} from './prepare_scheduler_review.mjs';
import {CHECKPOINT} from './capture_checkpoint.mjs';

const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
const require=createRequire(join(root,'runtime-rehearsal-sdk2609-2026-10-25/validation/package.json'));
const {Metadata,TypeRegistry}=require('@polkadot/types');
const {xxhashAsU8a,blake2AsHex}=require('@polkadot/util-crypto');
const assert=(ok,message)=>{if(!ok)throw Error(message);};
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const hx=bytes=>'0x'+Buffer.from(bytes).toString('hex');
const bytes=value=>{assert(/^0x(?:[0-9a-f]{2})*$/.test(value),'canonical hex required');return Buffer.from(value.slice(2),'hex');};
async function bounded(path){assert((await stat(path)).size<=64*1024**2,'bounded public input');const value=await readFile(path);assert(value.length<=64*1024**2,'input grew');return value;}
const encoded=value=>Buffer.from(JSON.stringify(value,null,2)+'\n');
assert(process.argv.length===4,'Usage: extend_victim_bridge_signing_fixture.mjs PRIOR_FIXTURE NEW_FIXTURE');
const [prior,output]=process.argv.slice(2).map(value=>resolve(value));
const previous=await bounded(join(prior,'FIXTURE-PROVENANCE.json')),outer=JSON.parse(previous);
const args=JSON.parse(await bounded(join(prior,'COMPOSE-ARGS.json')));
assert(outer.fixtureOnly===true&&outer.productionReady===false&&args.fixtureOnly===true&&args.productionReady===false,'public fixture only');
assert(args['evidence-commitment']===blake2AsHex(previous),'prior outer commitment mismatch');
const capture=resolve(args.capture),manifestBytes=await bounded(join(capture,'capture-complete.json')),manifest=JSON.parse(manifestBytes);
const verificationBytes=await bounded(join(capture,'../checkpoint-verification.json')),verification=JSON.parse(verificationBytes);
const metadataBytes=await bounded(join(capture,'runtime-metadata.scale')),witnessBytes=await bounded(join(capture,'../checkpoint-metadata-witness.json')),witness=JSON.parse(witnessBytes);
assert(manifest.complete===true&&manifest.checkpoint.hash===CHECKPOINT.hash&&verification.stateRootVerified===true&&verification.headerHashVerified===true&&verification.captureManifestSha256===sha(manifestBytes),'verified checkpoint capture required');
assert(witness.wasmExecutionVerified===true&&witness.metadataSha256===sha(metadataBytes)&&manifest.artifacts['runtime-metadata.scale'].sha256===sha(metadataBytes),'executed metadata witness required');
const registry=new TypeRegistry();registry.setMetadata(new Metadata(registry,metadataBytes));
const metadata=registry.metadata.toJSON();
const names=['BridgeDataSigner.Approvals','BridgeDataSigner.PendingApprovals','BridgeOutboundChannel.LatestCommitment','SubstrateBridgeOutboundChannel.LatestCommitment'];
const descriptors=names.map(storage=>{
 const [palletName,itemName]=storage.split('.'),pallet=metadata.pallets.find(p=>p.name===palletName),entry=pallet?.storage?.items.find(item=>item.name===itemName);
 assert(entry?.type?.map&&entry.type.map.hashers.every(hasher=>hasher==='Identity'),'exact native signing descriptor');
 return {storage,entry,prefix:hx(Buffer.concat([xxhashAsU8a(pallet.storage.prefix,128),xxhashAsU8a(itemName,128)]))};
});
const records=[],seen=new Set(),pages=[],counts=Object.fromEntries(names.map(name=>[name,0]));
let total=0;
for(const page of manifest.top.pages){
 if(!page.keyCount||!descriptors.some(d=>page.firstKey<=d.prefix+'f'.repeat(1024)&&page.lastKey>=d.prefix))continue;
 assert(/^pages\/top\/[0-9]{8}\.jsonl\.gz$/.test(page.file),'safe capture page path');
 const digest=createHash('sha256');
 async function* authenticatedCompressed(){for await(const chunk of createReadStream(join(capture,page.file))){digest.update(chunk);yield chunk;}}
 for await(const [key,value] of selectedRows(Readable.from(authenticatedCompressed()).pipe(createGunzip()),descriptors.map(d=>d.prefix))){
  const descriptor=descriptors.find(d=>key.startsWith(d.prefix));assert(descriptor&&!seen.has(key),'unique known signing row');seen.add(key);
  const raw=bytes(value);total+=raw.length;assert(total<=64*1024**2&&records.length<100000,'native signing evidence bound');
  const decoded=registry.createTypeUnsafe('Lookup'+descriptor.entry.type.map.value,[raw]);assert(hx(decoded.toU8a())===value,'canonical exact value codec');
  const rawKey=bytes('0x'+key.slice(descriptor.prefix.length));
  const keyDecoded=registry.createTypeUnsafe('Lookup'+descriptor.entry.type.map.key,[rawKey]);assert(hx(keyDecoded.toU8a())===hx(rawKey),'canonical exact Identity key codec');
  records.push({storage:descriptor.storage,key,valueHash:blake2AsHex(raw)});counts[descriptor.storage]++;
 }
 assert(digest.digest('hex')===page.sha256,'same-stream compressed page checksum mismatch');pages.push({file:page.file,sha256:page.sha256});
}
records.sort((a,b)=>a.key.localeCompare(b.key));
const inputs=new Map();for(const file of outer.files){
 assert(/^[a-zA-Z0-9._-]+$/.test(file.file)&&!inputs.has(file.file),'safe unique prior input');
 const content=await bounded(join(prior,file.file));assert(content.length===file.bytes&&sha(content)===file.sha256,'prior input checksum');inputs.set(file.file,{record:file,content});
}
for(const name of ['bridge-history.json','economic.json','inventory.json'])assert(inputs.has(name),'required prior input '+name);
const history=JSON.parse(inputs.get('bridge-history.json').content),economic=JSON.parse(inputs.get('economic.json').content),inventory=JSON.parse(inputs.get('inventory.json').content);
assert(history.cancelAllTerminalOutgoing===true&&Array.isArray(history.records),'complete bridge history required');
assert(economic.attackers.includes('0x64650caa38ca952fc214cbb38cb625d779d1387b52910e664012cbd7d7593a7d'),'fixed attacker registry required');
assert(['directBurns','extinguishUnissued','quarantine'].every(name=>Array.isArray(economic[name])&&!economic[name].length),'this public fixture performs no destructive attacker accounting');
const evidence={schema:'sora-nexus-public-victim-bridge-signing-extension-v1',fixtureOnly:true,productionReady:false,checkpointHash:CHECKPOINT.hash,
 priorProvenanceSha256:sha(previous),captureManifestSha256:sha(manifestBytes),checkpointVerificationSha256:sha(verificationBytes),executingMetadataSha256:sha(metadataBytes),metadataWitnessSha256:sha(witnessBytes),
 completePrefixes:descriptors.map(d=>({storage:d.storage,prefix:d.prefix,rows:counts[d.storage]})),pages,records:records.length,
 previousHistoryReviewSha256:sha(inputs.get('bridge-history.json').content),
 policy:'Cancel all checkpoint terminal and nonterminal outgoing execution without creating refunds. Preserve native signing evidence and quarantine every known signing hash. Public fixture only; external redemption and unknown historical batch membership are unresolved.',
 attackerWatch:'Fixed incident AccountId32/H160 plus explicitly approved additional identities. Native compiler must establish complete typed coverage; no beneficial ownership or external registry claim.',sources:[]};
for(const path of ['utils/incident-recovery/src/bridge_signing.rs','utils/incident-recovery/src/bridge_history.rs','utils/incident-recovery/src/economics.rs','utils/incident-recovery/src/inventory.rs','vendor/sora2-common/pallets/types/src/recovery.rs','misc/incident_recovery/extend_victim_bridge_signing_fixture.mjs'])evidence.sources.push({path,sha256:sha(await bounded(join(root,path)))});
const evidenceBytes=encoded(evidence),evidenceHash=blake2AsHex(evidenceBytes);
history.quarantineAllNonterminalOutgoing=true;history.evidenceHash=evidenceHash;
economic.attackerEvmAddresses??=[];
inventory.watchedAccounts=[...new Set([...(inventory.watchedAccounts??[]),...economic.attackers,'0xaf6fd7a11087bfbaddc5c4e1ab14ab0c592af63f',...economic.attackerEvmAddresses])].sort();
const replacements=new Map([['bridge-history.json',encoded(history)],['economic.json',encoded(economic)],['inventory.json',encoded(inventory)]]);
await mkdir(output,{recursive:false});const files=[];
for(const [name,{record,content}] of inputs){const data=replacements.get(name)??content;await writeFile(join(output,name),data,{flag:'wx'});files.push({...record,bytes:data.length,sha256:sha(data)});}
for(const [name,data] of [['bridge-signing.json',encoded({evidenceHash,cancelAllKnownSigningCommitments:true,records})],['BRIDGE-SIGNING-EXTENSION.json',evidenceBytes]]){
 assert(!inputs.has(name),'new artifact collision');await writeFile(join(output,name),data,{flag:'wx'});files.push({file:name,bytes:data.length,sha256:sha(data)});
}
outer.files=files;outer.previousFixture={path:prior,provenanceSha256:sha(previous)};outer.bridgeSigningExtension={file:'BRIDGE-SIGNING-EXTENSION.json',sha256:sha(evidenceBytes)};
const outerBytes=encoded(outer);await writeFile(join(output,'FIXTURE-PROVENANCE.json'),outerBytes,{flag:'wx'});
for(const [name,value] of Object.entries(args))if(typeof value==='string'&&value.startsWith(prior+'/'))args[name]=output+value.slice(prior.length);
args['bridge-signing']=join(output,'bridge-signing.json');args['attacker-coverage-output']=join(output,'attacker-coverage.json');args['evidence-commitment']=blake2AsHex(outerBytes);
await writeFile(join(output,'COMPOSE-ARGS.json'),encoded(args),{flag:'wx'});
console.log(JSON.stringify({output,counts,files:files.length,records:records.length,evidenceCommitment:args['evidence-commitment'],fixtureOnly:true,productionReady:false}));
