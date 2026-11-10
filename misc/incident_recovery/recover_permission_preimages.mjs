#!/usr/bin/env node
/** Offline candidate preimages from typed checkpoint records. Every resulting
 * component is rehashed against its exact captured Permissions storage key. */
import{createReadStream}from'node:fs';
import{readFile,writeFile}from'node:fs/promises';
import{createHash}from'node:crypto';
import{createRequire}from'node:module';
import{resolve,join}from'node:path';
import{createGunzip}from'node:zlib';
import{pathToFileURL}from'node:url';
import{CHECKPOINT}from'./capture_checkpoint.mjs';
import{lines,parseTyped}from'./independent_attacker_inventory.mjs';
const assert=(c,m)=>{if(!c)throw new Error(m)};
const hex=b=>'0x'+Buffer.from(b).toString('hex');
const sha=b=>createHash('sha256').update(b).digest('hex');

export function recoverComponents(key,descriptors,candidates,hash) {
  const {prefix,keyTypes}=descriptors;assert(key.startsWith(prefix),'permission prefix mismatch');
  const bytes=Buffer.from(key.slice(prefix.length),'hex');assert(bytes.length===64&&keyTypes.length===2,'permission key shape differs');
  const result=[];
  for(let index=0;index<2;index++) {
    const digest=hex(bytes.subarray(index*32,index*32+32));const matches=candidates[index].get(digest)??[];
    assert(matches.length<=1,'ambiguous hashed preimage collision');if(matches.length!==1)return null;
    assert(hex(hash(Buffer.from(matches[0].slice(2),'hex')) )===digest,'candidate does not rehash');result.push(matches[0]);
  }
  return result;
}

async function run({capture,dependencies,output}) {
  const require=createRequire(resolve(dependencies));const{Metadata,TypeRegistry}=require('@polkadot/types');const{blake2AsU8a,xxhashAsU8a}=require('@polkadot/util-crypto');
  const manifestBytes=await readFile(join(capture,'capture-complete.json')),manifest=JSON.parse(manifestBytes);
  const verification=JSON.parse(await readFile(join(capture,'../checkpoint-verification.json'),'utf8'));
  assert(verification.stateRootVerified&&verification.headerHashVerified&&verification.stateRoot===CHECKPOINT.stateRoot&&verification.captureManifestSha256===sha(manifestBytes),'verified capture mismatch');
  const metadataBytes=await readFile(join(capture,'runtime-metadata.scale'));const witness=JSON.parse(await readFile(join(capture,'../checkpoint-metadata-witness.json'),'utf8'));
  assert(witness.wasmExecutionVerified&&witness.checkpointHash===CHECKPOINT.hash&&witness.metadataSha256===sha(metadataBytes),'executed metadata witness mismatch');
  const registry=new TypeRegistry();registry.register(require('@sora-substrate/type-definitions').types);const metadata=new Metadata(registry,metadataBytes);registry.setMetadata(metadata);
  const json=metadata.asLatest.toJSON(),types=new Map(json.lookup.types.map(t=>[t.id,t.type]));const descriptors=new Map();
  const wanted=new Set(['System.Account','Tokens.Accounts','Assets.AssetOwners','Assets.AssetInfosV2','DEXManager.DEXInfos','Permissions.Owners','Permissions.Permissions']);
  for(const p of json.pallets)if(p.storage)for(const e of p.storage.items){const name=`${p.name}.${e.name}`;if(!wanted.has(name))continue;const map=e.type.map;assert(map,'expected map');const keyTypes=map.hashers.length===1?[map.key]:types.get(map.key).def.tuple;const prefix=hex(Buffer.concat([xxhashAsU8a(p.storage.prefix,128),xxhashAsU8a(e.name,128)]));descriptors.set(name,{name,prefix,hashers:map.hashers,keyTypes,valueType:map.value});}
  assert(descriptors.size===wanted.size,'checkpoint permission source schemas incomplete');
  for(const name of ['Permissions.Owners','Permissions.Permissions'])assert(descriptors.get(name).hashers.every(h=>h==='Blake2_256'),'permission hasher differs');
  const accountCandidates=new Map(),scopeCandidates=new Map(),permissionCandidates=new Map();
  const add=(map,bytes)=>{const digest=hex(blake2AsU8a(bytes,256)),value=hex(bytes);const old=map.get(digest)??[];if(!old.includes(value))old.push(value);assert(old.length===1,'candidate hash collision');map.set(digest,old);};
  const addScope=(bytes)=>add(scopeCandidates,Buffer.concat([Buffer.from([0]),blake2AsU8a(bytes,512)]));
  add(scopeCandidates,Buffer.from([1]));
  // Concrete public permission constants in the checkpoint source. Any other
  // permission ID remains unresolved, not guessed from a broad numeric range.
  // ID 1 is historical TRANSFER, proven by 4a46ab3fd^:pallets/permissions/src/lib.rs.
  for(let id=1;id<=13;id++){const bytes=Buffer.alloc(4);bytes.writeUInt32LE(id);add(permissionCandidates,bytes);}
  const pending=[],counts={},pageEvidence=[];
  for(const page of manifest.top.pages){if(!page.keyCount)continue;const selected=[...descriptors.values()].some(d=>page.firstKey<=d.prefix+'f'.repeat(256)&&page.lastKey>=d.prefix);if(!selected)continue;
    const path=resolve(capture,page.file);assert(path.startsWith(resolve(capture)+'/'),'page path escape');const checksum=createHash('sha256');for await(const chunk of createReadStream(path))checksum.update(chunk);assert(checksum.digest('hex')===page.sha256,'page checksum mismatch');pageEvidence.push({file:page.file,sha256:page.sha256});
    let header=true;for await(const row of lines(createReadStream(path).pipe(createGunzip()))){if(header){assert(row.line&&JSON.parse(row.line).checkpointHash===CHECKPOINT.hash,'page checkpoint mismatch');header=false;continue;}const key=row.line?JSON.parse(row.line)[0]:row.prefix.match(/^\["(0x[0-9a-f]+)",/)?.[1];assert(key,'key missing');const descriptor=[...descriptors.values()].find(d=>key.startsWith(d.prefix));if(!descriptor)continue;assert(row.line,'unexpected oversized source record');const [,valueHex]=JSON.parse(row.line),value=Buffer.from(valueHex.slice(2),'hex');counts[descriptor.name]=(counts[descriptor.name]??0)+1;
      if(descriptor.name.startsWith('Permissions.')){pending.push({trie:'top',key,descriptor});if(descriptor.name==='Permissions.Owners'){const codec=registry.createTypeUnsafe(`Lookup${descriptor.valueType}`,[value]);assert(Buffer.from(codec.toU8a()).equals(value),'owner vector roundtrip');const owners=codec.toJSON();assert(Array.isArray(owners),'owner vector');for(const owner of owners){const bytes=registry.createTypeUnsafe('Lookup0',[owner]).toU8a();assert(bytes.length===32,'owner AccountId width');add(accountCandidates,bytes);}}continue;}
      let offset=32;const components=[];for(let i=0;i<descriptor.hashers.length;i++){const hasher=descriptor.hashers[i],width={Blake2_128Concat:16,Twox64Concat:8,Identity:0}[hasher];assert(width!==undefined,'source map hash-only');const bytes=Buffer.from(key.slice(2),'hex');const decoded=parseTyped(bytes.subarray(offset+width),descriptor.keyTypes[i],types);const component=bytes.subarray(offset+width,offset+width+decoded.consumed);const digest=hasher==='Blake2_128Concat'?blake2AsU8a(component,128):hasher==='Twox64Concat'?xxhashAsU8a(component,64):Buffer.alloc(0);assert(Buffer.from(digest).equals(bytes.subarray(offset,offset+width)),'source key hash mismatch');offset+=width+component.length;components.push(component);const type=types.get(descriptor.keyTypes[i]);if(type.path?.at(-1)==='AccountId32'){assert(component.length===32,'account width');add(accountCandidates,component);}if(type.path?.at(-1)==='AssetId32'||descriptor.name==='DEXManager.DEXInfos')addScope(component);}
      assert(offset===(key.length-2)/2,'source key trailing bytes');
      if(descriptor.name==='Assets.AssetOwners'){const decoded=parseTyped(value,descriptor.valueType,types);assert(decoded.consumed===value.length&&value.length===32,'asset owner codec');add(accountCandidates,value);}
    }
  }
  const keyPreimages=[],unresolved=[];
  for(const row of pending){const candidates=row.descriptor.name==='Permissions.Owners'?[permissionCandidates,scopeCandidates]:[accountCandidates,scopeCandidates];const components=recoverComponents(row.key,row.descriptor,candidates,b=>blake2AsU8a(b,256));if(components){for(let i=0;i<2;i++){const bytes=Buffer.from(components[i].slice(2),'hex'),decoded=parseTyped(bytes,row.descriptor.keyTypes[i],types);assert(decoded.consumed===bytes.length,'permission preimage did not decode exactly');}keyPreimages.push({trie:'top',key:row.key,components});}else unresolved.push({storage:row.descriptor.name,key:row.key,missingComponents:candidates.map((m,i)=>!m.has('0x'+row.key.slice(row.descriptor.prefix.length+i*64,row.descriptor.prefix.length+(i+1)*64)))});}
  const legacy=JSON.parse(await readFile(new URL('./checkpoint_legacy_schema.json',import.meta.url)));
  const report={schema:'sora-nexus-permission-preimages-v1',checkpointHash:CHECKPOINT.hash,stateRoot:CHECKPOINT.stateRoot,captureManifestSha256:sha(manifestBytes),metadataSha256:sha(metadataBytes),scriptSha256:sha(await readFile(new URL(import.meta.url))),sourceCounts:counts,candidateCounts:{accounts:accountCandidates.size,scopes:scopeCandidates.size,permissionIds:permissionCandidates.size},recovered:keyPreimages.length,unresolved,pageEvidence,sourceProvenance:['pallets/permissions/src/lib.rs: Owners, Permissions, Scope','common/src/permissions.rs: PermissionId constants','4a46ab3fd316a09c47d82f9cb9e8452c6fc65726^:pallets/permissions/src/lib.rs historical TRANSFER=1','common/src/lib.rs: hash = Blake2b-512(SCALE input)','pallets/dex-manager/src/lib.rs: Scope::Limited(hash(dex_id))'],productionReady:false};
  const options={includePinnedCheckpointLegacy:true,watchedAccounts:['0x64650caa38ca952fc214cbb38cb625d779d1387b52910e664012cbd7d7593a7d','0xaf6fd7a11087bfbaddc5c4e1ab14ab0c592af63f'],keyPreimages,dispositions:legacy.storage.filter(s=>s.disposition==='retain_historical').map(s=>({trie:s.trie,storage:s.name,action:s.disposition,reason:s.reason,evidenceHash:'0x'+sha(Buffer.from(JSON.stringify(s)))}))};
  await writeFile(output,JSON.stringify(options,null,2)+'\n',{flag:'wx'});await writeFile(output+'.report.json',JSON.stringify(report,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify({output,recovered:keyPreimages.length,unresolved:unresolved.length,candidates:report.candidateCounts}));
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){const args=process.argv.slice(2),options={};for(let i=0;i<args.length;i+=2){assert(['--capture','--dependencies','--output'].includes(args[i])&&args[i+1],'--capture DIR --dependencies PACKAGE_JSON --output FILE required');options[args[i].slice(2)]=args[i+1];}assert(options.capture&&options.dependencies&&options.output,'missing arguments');await run(options);}
