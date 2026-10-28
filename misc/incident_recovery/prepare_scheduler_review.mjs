#!/usr/bin/env node
/** Produce finite custody evidence from an already independently verified checkpoint.
 * Offline only. Never decodes or executes the historical scheduled RuntimeCalls. */
import {createReadStream} from 'node:fs';
import {readFile, writeFile, mkdir} from 'node:fs/promises';
import {createGunzip} from 'node:zlib';
import {createHash} from 'node:crypto';
import {createRequire} from 'node:module';
import {resolve, join, dirname} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {parseTyped} from './independent_attacker_inventory.mjs';
import {CHECKPOINT, MAX_PAGE_BYTES} from './capture_checkpoint.mjs';

const MAX_ROW = 64 * 1024 * 1024 + 16384;
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const hex = bytes => `0x${Buffer.from(bytes).toString('hex')}`;
const bytes = value => { if (!/^0x(?:[0-9a-f]{2})*$/.test(value)) throw Error('noncanonical hex'); return Buffer.from(value.slice(2), 'hex'); };
const assert = (value, message) => { if (!value) throw Error(message); };

// Skip unrelated oversized rows while retaining bounded matching opaque preimages.
export async function* selectedRows(stream, prefixes) {
  let parts=[], size=0, total=0, retain=null, head=Buffer.alloc(0);
  for await (const chunk of stream) {
    total += chunk.length; assert(total <= MAX_PAGE_BYTES, 'page decompression bound');
    let start=0;
    while (start<chunk.length) {
      const newline=chunk.indexOf(10,start), part=chunk.subarray(start,newline<0?chunk.length:newline);
      size+=part.length;
      if (retain===null) {
        head=Buffer.concat([head,part.subarray(0,Math.max(0,4096-head.length))]);
        const match=/^\["(0x[0-9a-f]+)","0x/.exec(head.toString('utf8'));
        if(match) retain=prefixes.some(prefix=>match[1].startsWith(prefix));
        else if(head.length>=4096||newline>=0) retain=false;
      }
      if(retain!==false) { assert(size<=MAX_ROW,'matching record exceeds 32 MiB decoded bound'); parts.push(part); }
      if(newline>=0) {
        if(retain) { const row=JSON.parse(Buffer.concat(parts).toString('utf8')); assert(Array.isArray(row)&&row.length===2,'invalid row'); yield row; }
        parts=[];size=0;retain=null;head=Buffer.alloc(0);start=newline+1;
      } else break;
    }
  }
  assert(size===0,'unterminated row');
}
export function opaqueByteVector(value) {
  const mode=value[0]&3, width=mode<3?1<<mode:(value[0]>>>2)+5;
  assert(width<=5&&value.length>=width,'opaque byte-vector compact width');
  let count=0n;
  if(mode<3) { for(let i=width-1;i>=0;i--)count=(count<<8n)|BigInt(value[i]);count>>=2n; }
  else { assert(value[width-1]!==0,'noncanonical compact');for(let i=width-1;i>=1;i--)count=(count<<8n)|BigInt(value[i]); }
  assert(mode===0||count>=[0n,64n,16384n,1073741824n][mode],'noncanonical compact');
  assert(count===BigInt(value.length-width),'opaque byte-vector length');
  return Number(count);
}
export async function main(args) {
  const options={};for(let i=0;i<args.length;i+=2){assert(args[i]?.startsWith('--')&&args[i+1],'expected --name value');options[args[i].slice(2)]=args[i+1];}
  const workspace=resolve(options.workspace??join(dirname(fileURLToPath(import.meta.url)),'../..'));
  const capture=resolve(options.capture??join(workspace,'artifacts/recovery-28191817/checkpoint'));
  const out=resolve(options.out??join(workspace,'artifacts/recovery-28191817/scheduler-review-v1'));
  const verificationPath=resolve(options.verification??join(capture,'../checkpoint-verification.json'));
  const witnessPath=resolve(options['metadata-witness']??join(capture,'../checkpoint-metadata-witness.json'));
  const raw=await readFile(join(capture,'capture-complete.json')), manifest=JSON.parse(raw);
  const verifiedRaw=await readFile(verificationPath), verified=JSON.parse(verifiedRaw);
  const witnessRaw=await readFile(witnessPath), witness=JSON.parse(witnessRaw);
  assert(verified.captureManifestSha256===sha(raw)&&verified.stateRootVerified===true&&manifest.complete===true,'independently verified complete capture required');
  assert(manifest.checkpoint.hash===CHECKPOINT.hash&&verified.checkpointHash===CHECKPOINT.hash&&witness.checkpointHash===CHECKPOINT.hash,'wrong checkpoint');
  const metadataBytes=await readFile(join(capture,'runtime-metadata.scale'));
  assert(sha(metadataBytes)===manifest.artifacts['runtime-metadata.scale'].sha256&&sha(metadataBytes)===witness.metadataSha256&&witness.wasmExecutionVerified===true,'executing-code metadata witness required');
  const require=createRequire(join(workspace,'runtime-rehearsal-async-staking-2026-10-25/validation/package.json'));
  const {Metadata,TypeRegistry}=require('@polkadot/types');
  const {xxhashAsU8a,blake2AsU8a}=require('@polkadot/util-crypto');
  const registry=new TypeRegistry();registry.register(require('@sora-substrate/type-definitions').types);
  const metadata=new Metadata(registry,metadataBytes);registry.setMetadata(metadata);const json=metadata.asLatest.toJSON();
  const types=new Map(json.lookup.types.map(t=>[t.id,t.type]));
  const descriptors=[];
  for(const pallet of json.pallets) if(pallet.storage) for(const entry of pallet.storage.items) {
    if(!['Scheduler','Multisig','BridgeMultisig','Preimage','System'].includes(pallet.name))continue;
    if(pallet.name==='BridgeMultisig'&&entry.name!=='Accounts'||pallet.name==='System'&&entry.name!=='Account')continue;
    const prefix=hex(Buffer.concat([xxhashAsU8a(pallet.storage.prefix,128),xxhashAsU8a(entry.name,128)]));
    descriptors.push({storage:`${pallet.name}.${entry.name}`,prefix,entry,valueType:entry.type.plain??entry.type.map.value});
  }
  const descriptor=name=>{const d=descriptors.find(d=>d.storage===name);assert(d,`missing ${name}`);return d;};
  const pages=new Map(), rows=new Map();
  async function readPrefixes(prefixes) {
    for(const page of manifest.top.pages) {
      if(!page.keyCount||!prefixes.some(prefix=>page.firstKey<=prefix+'f'.repeat(1024)&&page.lastKey>=prefix))continue;
      assert(/^pages\/top\/[0-9]{8}\.jsonl\.gz$/.test(page.file),'unexpected capture page path');
      const pagePath=join(capture,page.file), digest=createHash('sha256');
      for await(const chunk of createReadStream(pagePath))digest.update(chunk);
      assert(digest.digest('hex')===page.sha256,'capture page hash mismatch');pages.set(page.file,{file:page.file,sha256:page.sha256});
      for await(const [key,value] of selectedRows(createReadStream(pagePath).pipe(createGunzip()),prefixes)) {
        const d=descriptors.find(d=>key.startsWith(d.prefix));
        if(!d) { rows.set(key,{storage:'storage-version-or-unknown',key,value,valueHash:hex(blake2AsU8a(bytes(value),256))});continue; }
        const rawValue=bytes(value);let decoded=null,references=[];
        if(d.storage==='Preimage.PreimageFor') decoded={opaqueCallPreimage:true,bytes:opaqueByteVector(rawValue)};
        else {
          const parsed=parseTyped(rawValue,d.valueType,types,{render:true,watched:{has:()=>true}});
          assert(parsed.consumed===rawValue.length,'typed row trailing bytes');decoded=parsed.value;references=parsed.references;
          const codec=registry.createTypeUnsafe('Lookup'+d.valueType,[rawValue]);assert(Buffer.from(codec.toU8a()).equals(rawValue),'noncanonical typed row');
        }
        rows.set(key,{storage:d.storage,key,valueHash:hex(blake2AsU8a(rawValue,256)),encodedBytes:rawValue.length,decoded,references,...(rawValue.length<1024?{value}: {})});
      }
    }
  }
  await readPrefixes([hex(xxhashAsU8a('Scheduler',128)),hex(xxhashAsU8a('Multisig',128)),descriptor('BridgeMultisig.Accounts').prefix]);
  const agendas=[...rows.values()].filter(r=>r.storage==='Scheduler.Agenda');assert(agendas.length===6,'unexpected agenda inventory');
  const callHashes=[...new Set(agendas.map(r=>{const v=bytes(r.value);assert(v.length===76&&v[36]===2,'unexpected scheduled lookup envelope');return hex(v.subarray(37,69));}))];
  await readPrefixes(['Preimage.StatusFor','Preimage.RequestStatusFor','Preimage.PreimageFor'].flatMap(name=>callHashes.map(hash=>descriptor(name).prefix+hash.slice(2))));
  const providers=[...new Set([...rows.values()].filter(r=>r.storage==='Preimage.StatusFor'||r.storage==='Preimage.RequestStatusFor').flatMap(r=>r.references.filter(x=>x.kind==='account_id32').map(x=>x.identity)))].sort();
  const providerKeys=providers.map(provider=>descriptor('System.Account').prefix+hex(blake2AsU8a(bytes(provider),128)).slice(2)+provider.slice(2));
  await readPrefixes(providerKeys);
  const sourcePaths=['misc/incident_recovery/prepare_scheduler_review.mjs','misc/incident_recovery/independent_attacker_inventory.mjs','utils/incident-recovery/src/scheduler_controls.rs','vendor/sora2-common/pallets/multisig/src/lib.rs'];
  const sources=[];for(const file of sourcePaths)sources.push({file,sha256:sha(await readFile(join(workspace,file)))});
  const bridgeAccounts=[...rows.values()].filter(r=>r.storage==='BridgeMultisig.Accounts').map(r=>{
    const rawKey=bytes(r.key),account=rawKey.subarray(40);assert(rawKey.length===72&&rawKey.subarray(32,40).equals(Buffer.from(xxhashAsU8a(account,64))),'bridge account key hash');
    return {...r,account:hex(account),disposition:'retained; no inference of compromised ownership',derivationProofRequired:'creator, creation block and nonce plus checkpoint controller proofs'};
  });
  const evidence={schema:'sora-nexus-scheduler-custody-evidence-v1',checkpointHash:CHECKPOINT.hash,stateRoot:manifest.checkpoint.stateRoot,captureManifestSha256:sha(raw),nativeVerificationSha256:sha(verifiedRaw),executingMetadataWitnessSha256:sha(witnessRaw),metadataSha256:sha(metadataBytes),sources,pages:[...pages.values()].sort((a,b)=>a.file.localeCompare(b.file)),callHashes:callHashes.sort(),providers,providerKeys,descriptors,rows:[...rows.values()].sort((a,b)=>a.key.localeCompare(b.key)),bridgeAccounts,productionReady:false,historicalCallsExecuted:false,refundsCreated:0};
  const evidenceRaw=JSON.stringify(evidence,null,2)+'\n';
  const records=[...rows.values()].filter(r=>['Scheduler.Agenda','Scheduler.IncompleteSince','Preimage.StatusFor','Preimage.RequestStatusFor','Preimage.PreimageFor','System.Account'].includes(r.storage)).map(({storage,key,valueHash})=>({storage,key,valueHash})).sort((a,b)=>a.key.localeCompare(b.key));
  const review={evidenceHash:hex(blake2AsU8a(Buffer.from(evidenceRaw),256)),records};
  await mkdir(out,{recursive:false});await writeFile(join(out,'evidence.json'),evidenceRaw);await writeFile(join(out,'review.json'),JSON.stringify(review,null,2)+'\n');
  process.stdout.write(JSON.stringify({out,records:records.length,pages:pages.size,providers:providers.length,bridgeAccounts:bridgeAccounts.length,evidenceSha256:sha(evidenceRaw),productionReady:false})+'\n');
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href)main(process.argv.slice(2)).catch(error=>{console.error(error.message);process.exitCode=1;});
