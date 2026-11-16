#!/usr/bin/env node
/** Read-only, offline graph/reference diagnostics from the independently verified checkpoint. */
import {createReadStream} from 'node:fs';
import {readFile,writeFile} from 'node:fs/promises';
import {createGunzip} from 'node:zlib';
import {createHash} from 'node:crypto';
import {createRequire} from 'node:module';
import {resolve,join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {lines} from './independent_attacker_inventory.mjs';
import {CHECKPOINT} from './capture_checkpoint.mjs';
const assert=(ok,message)=>{if(!ok)throw new Error(message);};
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const hx=bytes=>`0x${Buffer.from(bytes).toString('hex')}`;
const bytes=text=>{assert(/^0x(?:[0-9a-f]{2})*$/.test(text),'noncanonical hex');return Buffer.from(text.slice(2),'hex');};
export function validateGraph(forward,reverse,accounts) {
  const counts=new Map();
  for(const [owner,referrer] of forward) {
    assert(reverse.get(referrer)?.filter(entry=>entry===owner).length===1,'forward/reverse mismatch');
    for(const id of [owner,referrer])counts.set(id,(counts.get(id)??0)+1);
  }
  for(const [referrer,owners] of reverse) {
    assert(new Set(owners).size===owners.length,'duplicate reverse entry');
    for(const owner of owners)assert(forward.get(owner)===referrer,'orphan reverse entry');
  }
  for(const [id,count]of counts) {const account=accounts.get(id);assert(account&&account.providers>0&&account.consumers>=count,'reference custody inconsistent');}
  return counts;
}
export async function run({capture,dependencies,output}) {
  assert(capture&&dependencies&&output,'--capture --dependencies PACKAGE_JSON --output required');
  const require=createRequire(resolve(dependencies));const {Metadata,TypeRegistry}=require('@polkadot/types');
  const {blake2AsU8a,xxhashAsU8a}=require('@polkadot/util-crypto');
  const load=async file=>{const raw=await readFile(join(capture,file));return {value:JSON.parse(raw),sha256:sha(raw)};};
  const manifest=await load('capture-complete.json'), verification=await load('../checkpoint-verification.json'), metadataWitness=await load('../checkpoint-metadata-witness.json');
  assert(manifest.value.complete&&manifest.value.checkpoint.hash===CHECKPOINT.hash&&verification.value.stateRootVerified&&verification.value.headerHashVerified&&verification.value.stateRoot===CHECKPOINT.stateRoot&&verification.value.captureManifestSha256===manifest.sha256,'verified exact checkpoint required');
  const metadataBytes=await readFile(join(capture,'runtime-metadata.scale'));
  assert(metadataWitness.value.wasmExecutionVerified&&metadataWitness.value.checkpointHash===CHECKPOINT.hash&&metadataWitness.value.metadataSha256===sha(metadataBytes)&&manifest.value.artifacts['runtime-metadata.scale'].sha256===sha(metadataBytes),'executed metadata mismatch');
  const registry=new TypeRegistry();registry.register(require('@sora-substrate/type-definitions').types);registry.setMetadata(new Metadata(registry,metadataBytes));
  const metadata=registry.metadata.toJSON();const descriptors=new Map();
  for(const pallet of metadata.pallets)if(pallet.storage)for(const entry of pallet.storage.items){const name=`${pallet.name}.${entry.name}`;if(['Referrals.Referrers','Referrals.Referrals','System.Account'].includes(name)){assert(entry.type.map?.hashers.length===1&&entry.type.map.hashers[0]==='Blake2_128Concat','unexpected key hasher');descriptors.set(name,{name,entry,prefix:hx(Buffer.concat([xxhashAsU8a(pallet.storage.prefix,128),xxhashAsU8a(entry.name,128)]))});}}
  assert(descriptors.size===3,'missing referral metadata');
  const prefix=name=>descriptors.get(name).prefix;
  const key=(name,id)=>hx(Buffer.concat([bytes(prefix(name)),blake2AsU8a(bytes(id),128),bytes(id)]));
  const seenPages=new Map(), records=[];
  const readRows=async(wanted,accept)=>{
    for(const page of manifest.value.top.pages){if(!page.keyCount||!wanted.some(p=>page.firstKey<=p+'f'.repeat(256)&&page.lastKey>=p))continue;
      const path=resolve(capture,page.file);assert(path.startsWith(resolve(capture)+'/'),'capture path escape');
      const digest=createHash('sha256');for await(const chunk of createReadStream(path))digest.update(chunk);assert(digest.digest('hex')===page.sha256,'capture checksum mismatch');seenPages.set(page.file,page.sha256);
      let header=true;for await(const row of lines(createReadStream(path).pipe(createGunzip()))){if(header){assert(row.line&&JSON.parse(row.line).checkpointHash===CHECKPOINT.hash,'capture page identity');header=false;continue;}if(!row.line){assert(!wanted.some(p=>row.prefix.startsWith(`["${p}`)),'oversized relevant record');continue;}const [k,v]=JSON.parse(row.line);accept(k,v);}
    }
  };
  const forward=new Map(),reverse=new Map();
  const decode=(name,k,v)=>{const d=descriptors.get(name),id=`0x${k.slice(-64)}`;assert(key(name,id)===k,'noncanonical account key');const value=registry.createTypeUnsafe(`Lookup${d.entry.type.map.value}`,[bytes(v)]);assert(hx(value.toU8a())===v,'noncanonical typed value');records.push({storage:name,key:k,valueHash:hx(blake2AsU8a(bytes(v),256))});return [id,value];};
  await readRows([prefix('Referrals.Referrers'),prefix('Referrals.Referrals')],(k,v)=>{if(k.startsWith(prefix('Referrals.Referrers'))){const [id,value]=decode('Referrals.Referrers',k,v);assert(!forward.has(id),'duplicate forward row');forward.set(id,hx(value.toU8a()));}else if(k.startsWith(prefix('Referrals.Referrals'))){const[id,value]=decode('Referrals.Referrals',k,v);assert(!reverse.has(id),'duplicate reverse row');reverse.set(id,[...value].map(account=>hx(account.toU8a())));}});
  const ids=new Set([...forward.keys(),...forward.values()]),wanted=new Set([...ids].map(id=>key('System.Account',id))),accounts=new Map();
  await readRows([...wanted],(k,v)=>{if(!wanted.has(k))return;const[id,value]=decode('System.Account',k,v);const json=value.toJSON();assert(!accounts.has(id),'duplicate AccountInfo row');accounts.set(id,{providers:Number(json.providers),consumers:Number(json.consumers),sufficients:Number(json.sufficients)});});
  const counts=validateGraph(forward,reverse,accounts);
  const result={schema:'sora-nexus-checkpoint-referral-custody-v1',checkpointHash:CHECKPOINT.hash,captureManifestSha256:manifest.sha256,stateVerificationSha256:verification.sha256,metadataWitnessSha256:metadataWitness.sha256,metadataSha256:sha(metadataBytes),scriptSha256:sha(await readFile(new URL(import.meta.url))),forwardEdges:forward.size,reverseRows:reverse.size,referenceAccounts:accounts.size,graphConsistent:true,nativeReferencesCoverGraph:true,referenceOriginsInferred:false,minimumConsumers:Object.fromEntries(counts),accounts:Object.fromEntries(accounts),reviewRecords:records.sort((a,b)=>a.key.localeCompare(b.key)),pages:[...seenPages].map(([file,sha256])=>({file,sha256})),productionReady:false};
  await writeFile(output,JSON.stringify(result,null,2)+'\n',{flag:'wx'});return {forwardEdges:forward.size,reverseRows:reverse.size,referenceAccounts:accounts.size,graphConsistent:true,nativeReferencesCoverGraph:true,productionReady:false};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){const args={};for(let i=2;i<process.argv.length;i+=2){assert(process.argv[i].startsWith('--')&&process.argv[i+1],'invalid arguments');args[process.argv[i].slice(2)]=process.argv[i+1];}console.log(JSON.stringify(await run(args)));}
