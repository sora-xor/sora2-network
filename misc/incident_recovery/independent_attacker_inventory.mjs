#!/usr/bin/env node
/** Independent, offline typed-reference cross-check. No RPC or ownership inference. */
import {createReadStream} from 'node:fs';
import {readFile, writeFile} from 'node:fs/promises';
import {resolve, join, dirname} from 'node:path';
import {createRequire} from 'node:module';
import {createHash} from 'node:crypto';
import {createGunzip} from 'node:zlib';
import {pathToFileURL} from 'node:url';
import {CHECKPOINT, MAX_PAGE_BYTES} from './capture_checkpoint.mjs';

const WATCHED = new Set(['0x64650caa38ca952fc214cbb38cb625d779d1387b52910e664012cbd7d7593a7d', '0xaf6fd7a11087bfbaddc5c4e1ab14ab0c592af63f']);
const MAX_RECORD = 1024 * 1024;
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
const assert = (condition, message) => { if (!condition) throw new Error(message); };
const hex = (bytes) => `0x${Buffer.from(bytes).toString('hex')}`;

/** Stream pages without retaining oversized historical records. */
export async function* lines(stream) {
  let pending = Buffer.alloc(0), tail = Buffer.alloc(0), oversize = false, lineSize = 0, total = 0;
  for await (const chunk of stream) {
    total += chunk.length; assert(total <= MAX_PAGE_BYTES, 'decompression bound exceeded');
    let start = 0;
    while (start < chunk.length) {
      const end = chunk.indexOf(10, start); const part = chunk.subarray(start, end < 0 ? chunk.length : end);
      lineSize += part.length;
      tail=Buffer.concat([tail,part.subarray(Math.max(0,part.length-4096))]).subarray(-4096);
      if (!oversize && pending.length + part.length <= MAX_RECORD * 2 + 16384) pending = Buffer.concat([pending, part]);
      else if (!oversize) { pending = Buffer.concat([pending, part.subarray(0, Math.max(0, 4096 - pending.length))]).subarray(0,4096); oversize = true; }
      if (end >= 0) { yield {line: oversize ? null : pending.toString('utf8'), prefix: pending.subarray(0,4096).toString('utf8'), bytes: lineSize, tail:tail.toString('utf8')}; pending = Buffer.alloc(0); tail=Buffer.alloc(0); oversize = false; lineSize = 0; start = end + 1; }
      else break;
    }
  }
  assert(lineSize === 0, 'unterminated capture row');
}

function compact(bytes, position, maxBytes=16) {
  assert(position < bytes.length, 'compact value absent');
  const first = bytes[position], mode = first & 3;
  const length = mode < 3 ? 1 << mode : (first >>> 2) + 5;
  assert(length <= maxBytes + 1 && position + length <= bytes.length, 'compact width/truncation rejected');
  let value = 0n;
  if (mode < 3) { for(let i=length-1;i>=0;i--) value = (value << 8n) | BigInt(bytes[position+i]); value >>= 2n; }
  else { assert(bytes[position+length-1] !== 0, 'noncanonical compact'); for(let i=length-1;i>=1;i--) value = (value << 8n) | BigInt(bytes[position+i]); }
  assert(mode===0 || value >= [0n,64n,16384n,1073741824n][mode], 'noncanonical compact');
  return [value, length];
}

/** Bounded allocation-free schema traversal; JSON rendering runs only on hits. */
export function parseTyped(bytes, typeId, types, {render=false, path=[], watched=WATCHED}={}) {
  assert(bytes.length <= MAX_RECORD, 'typed value exceeds 1 MiB');
  const state = {position:0, nodes:65536, references:[]};
  function take(count) { assert(Number.isSafeInteger(count) && count>=0 && state.position+count<=bytes.length, 'truncated typed value'); const start=state.position; state.position+=count; return bytes.subarray(start,state.position); }
  function integer(data, signed=false) { let value=0n; for(let i=data.length-1;i>=0;i--) value=(value<<8n)|BigInt(data[i]); if(signed && data.at(-1)&128) value-=1n<<BigInt(data.length*8); return value.toString(); }
  function visit(id, currentPath, depth) {
    assert(depth<=64 && state.nodes-->0, 'typed depth/node budget exceeded');
    const type=types.get(id); assert(type, `missing type ${id}`); const start=state.position; const def=type.def;
    const identity=type.path?.at(-1)==='AccountId32' && type.path.some(p=>p==='sp_core'||p==='sp_runtime') ? 'account_id32' : type.path?.at(-1)==='H160' ? 'h160_address_reference' : null;
    let output;
    const fields=(entries)=>{
      const named=entries.length>0 && entries.every(f=>f.name!==null); const values=render?(named?{}:[]):null;
      for(let i=0;i<entries.length;i++){ const f=entries[i]; const v=visit(f.type,[...currentPath,f.name??String(i)],depth+1); if(render){if(named)values[f.name]=v;else values.push(v);} }
      return values;
    };
    if(def.composite) output=fields(def.composite.fields);
    else if(def.variant){const index=take(1)[0], variant=def.variant.variants.find(v=>v.index===index);assert(variant,'unknown variant');output=render?{variant:variant.name,fields:fields(variant.fields)}:fields(variant.fields);}
    else if(def.tuple) output=fields(def.tuple.map(type=>({name:null,type})));
    else if(def.sequence||def.array){let count,element;if(def.sequence){const [n,len]=compact(bytes,state.position,4);take(len);count=Number(n);element=def.sequence.type;}else{count=def.array.len;element=def.array.type;} if(types.get(element)?.def.primitive==='U8'){const data=take(count);output=render?hex(data):null;}else{assert(count<=state.nodes,'sequence allocation budget');output=render?[]:null;for(let i=0;i<count;i++){const v=visit(element,[...currentPath,String(i)],depth+1);if(render)output.push(v);}}}
    else if(def.compact){const [n,len]=compact(bytes,state.position);take(len);output=render?n.toString():null;}
    else if(def.bitSequence){const [n,len]=compact(bytes,state.position,4);take(len);const store=types.get(def.bitSequence.bitStoreType)?.def?.primitive;const width={U8:1,U16:2,U32:4,U64:8}[store];assert(width,'unsupported bit store');const size=Math.ceil(Number(n)/(width*8))*width;output=render?hex(take(size)):void take(size);}
    else if(def.primitive){const primitive=def.primitive;if(primitive==='Str'){const [n,len]=compact(bytes,state.position,4);take(len);const data=take(Number(n));new TextDecoder('utf-8',{fatal:true}).decode(data);output=render?data.toString('utf8'):null;}else{const width={Bool:1,Char:4,U8:1,I8:1,U16:2,I16:2,U32:4,I32:4,U64:8,I64:8,U128:16,I128:16,U256:32,I256:32}[primitive];assert(width,'unsupported primitive');const data=take(width);if(primitive==='Bool'){assert(data[0]<=1,'noncanonical bool');output=Boolean(data[0]);}else output=render?integer(data,primitive.startsWith('I')):null;}}
    else throw new Error(`unsupported definition ${Object.keys(def)}`);
    if(identity){const encoded=bytes.subarray(start,state.position);assert(encoded.length===(identity==='account_id32'?32:20),'identity width mismatch');const address=hex(encoded);if(watched.has(address))state.references.push({identity:address,kind:identity,typeId:id,typePath:type.path.join('::'),path:currentPath});if(render)output=address;}
    else if(render && ['H256','AssetId32'].includes(type.path?.at(-1))) output=hex(bytes.subarray(start,state.position));
    return output;
  }
  const value=visit(typeId,path,0); return {consumed:state.position,references:state.references,value};
}

/** Oversized preimages stay opaque. Examine only the source-proven outer
 * envelope; never decode the pending call, search its bytes, or infer its owner.
 * The complete JSON row is already bound to independently verified page/root
 * evidence; retained prefix/tail are bounded to 4096 bytes each. */
export function inspectOversizedPreimage(row,key,descriptor,types,watched=WATCHED) {
  const start=`["${key}","0x`;
  assert(row.prefix.startsWith(start)&&row.tail.endsWith('"]'),'noncanonical oversized capture envelope');
  const encodedBytes=(row.bytes-key.length-9)/2;
  assert(Number.isSafeInteger(encodedBytes)&&encodedBytes>MAX_RECORD&&encodedBytes<=64*1024*1024,'oversized preimage bounds');
  const head=Buffer.from(row.prefix.slice(start.length,start.length+32),'hex');
  if(descriptor.name==='Preimage.PreimageFor') {
    let id=descriptor.valueType;
    for(let depth=0;depth<16;depth++) { const def=types.get(id)?.def; if(def?.sequence&&types.get(def.sequence.type)?.def.primitive==='U8') break; assert(def?.composite?.fields.length===1,'preimage is not reviewed byte vector');id=def.composite.fields[0].type; }
    assert(types.get(id)?.def.sequence&&types.get(types.get(id).def.sequence.type)?.def.primitive==='U8','opaque preimage shape mismatch');
    const [count,width]=compact(head,0,4);assert(BigInt(encodedBytes-width)===count,'opaque preimage compact length mismatch');
    return {encodedBytes,opaqueBytes:Number(count),references:[],value:{opaqueCallPreimage:true,bytes:Number(count)},action:'retain exact opaque bytes; separate request-status/deposit accounting remains reviewed'};
  }
  assert(descriptor.name==='Democracy.Preimages'&&types.get(descriptor.valueType)?.path.at(-1)==='LegacyDemocracyPreimageStatus','unreviewed oversized encoding');
  assert(head[0]===1,'oversized legacy preimage must be Available');
  const [count,width]=compact(head,1,4);const suffixLength=encodedBytes-1-width-Number(count);
  assert(suffixLength===53||suffixLength===57,'legacy preimage envelope length mismatch');
  const tailHex=row.tail.slice(0,-2);const suffix=Buffer.from(tailHex.slice(-suffixLength*2),'hex');
  const provider=parseTyped(suffix.subarray(0,32),0,types,{watched,path:['value','Available','provider'],render:true});
  const deposit=parseTyped(suffix.subarray(32,48),6,types,{watched,render:true}).value;
  assert(suffix[52]===(suffixLength===53?0:1),'legacy expiry option mismatch');
  return {encodedBytes,opaqueBytes:Number(count),references:provider.references,value:{variant:'Available',provider:provider.value,deposit,since:suffix.readUInt32LE(48),expiry:suffixLength===53?null:suffix.readUInt32LE(53)},action:'pending authorization and provider deposit require explicit disposition; no call execution or nominal refund'};
}

async function run({capture, dependencies, output, inventoryOptions}) {
  const require=createRequire(resolve(dependencies)); const {Metadata,TypeRegistry}=require('@polkadot/types');
  const {blake2AsU8a,xxhashAsU8a}=require('@polkadot/util-crypto');
  const registry=new TypeRegistry();registry.register(require('@sora-substrate/type-definitions').types);
  const manifestBytes=await readFile(join(capture,'capture-complete.json')), manifest=JSON.parse(manifestBytes);
  const verified=JSON.parse(await readFile(join(capture,'../checkpoint-verification.json'),'utf8'));
  assert(verified.stateRootVerified && verified.headerHashVerified && verified.stateRoot===CHECKPOINT.stateRoot && verified.checkpointHash===CHECKPOINT.hash && verified.captureManifestSha256===sha256(manifestBytes),'independent state verification evidence mismatch');
  assert(manifest.complete && manifest.checkpoint.hash===CHECKPOINT.hash && manifest.children.length===0,'unsupported/incomplete capture');
  const metadataBytes=await readFile(join(capture,'runtime-metadata.scale'));
  assert(sha256(metadataBytes)===manifest.artifacts['runtime-metadata.scale'].sha256,'metadata artifact hash mismatch');
  const metadataWitness=JSON.parse(await readFile(join(capture,'../checkpoint-metadata-witness.json'),'utf8'));
  assert(metadataWitness.wasmExecutionVerified && metadataWitness.checkpointHash===CHECKPOINT.hash && metadataWitness.stateRoot===CHECKPOINT.stateRoot && metadataWitness.metadataSha256===sha256(metadataBytes),'executed metadata witness mismatch');
  const metadata=new Metadata(registry,metadataBytes);registry.setMetadata(metadata);
  const json=metadata.asLatest.toJSON(), types=new Map(json.lookup.types.map(t=>[t.id,t.type]));
  const descriptors=new Map();
  for(const pallet of json.pallets)if(pallet.storage)for(const entry of pallet.storage.items){const prefix=Buffer.concat([xxhashAsU8a(pallet.storage.prefix,128),xxhashAsU8a(entry.name,128)]);const map=entry.type.map;const components=map?(map.hashers.length===1?[map.key]:types.get(map.key)?.def.tuple):[];assert(!map||components?.length===map.hashers.length,'metadata hashers/key shape mismatch');const descriptor={name:`${pallet.name}.${entry.name}`,prefix:hex(prefix),hashers:map?.hashers??[],keyTypes:components,valueType:map?.value??entry.type.plain};assert(!descriptors.has(descriptor.prefix),'metadata prefix collision');descriptors.set(descriptor.prefix,descriptor);}
  const u16Type=[...types].find(([_,type])=>type.def.primitive==='U16')?.[0];assert(u16Type!==undefined,'u16 storage-version type missing');
  for(const pallet of json.pallets){const prefix=hex(Buffer.concat([xxhashAsU8a(pallet.name,128),xxhashAsU8a(':__STORAGE_VERSION__:',128)]));assert(!descriptors.has(prefix),'storage-version prefix collision');descriptors.set(prefix,{name:`${pallet.name}.__STORAGE_VERSION__`,prefix,hashers:[],keyTypes:[],valueType:u16Type});}
  const legacyBytes=await readFile(new URL('./checkpoint_legacy_schema.json',import.meta.url));const legacy=JSON.parse(legacyBytes);assert(legacy.metadataBlake2===hex(blake2AsU8a(metadataBytes,256))&&legacy.originalTypeCount===types.size,'legacy schema metadata mismatch');
  for(const type of legacy.types){assert(type.id===types.size,'legacy type ID mismatch');let def;if(type.kind==='tuple')def={tuple:type.fields};else if(type.kind==='sequence')def={sequence:{type:type.type}};else if(type.kind==='composite')def={composite:{fields:type.fields}};else if(type.kind==='variant')def={variant:{variants:type.variants}};else throw new Error('unknown legacy shape');types.set(type.id,{path:['sora_nexus_checkpoint_legacy',type.name],def});}
  for(const storage of legacy.storage){assert(!descriptors.has(storage.prefix),'legacy prefix collision');descriptors.set(storage.prefix,storage);}
  const hashKey=(hasher,bytes)=>{switch(hasher){case'Identity':return bytes;case'Blake2_128':return Buffer.from(blake2AsU8a(bytes,128));case'Blake2_256':return Buffer.from(blake2AsU8a(bytes,256));case'Twox128':return Buffer.from(xxhashAsU8a(bytes,128));case'Twox256':return Buffer.from(xxhashAsU8a(bytes,256));case'Blake2_128Concat':return Buffer.concat([blake2AsU8a(bytes,128),bytes]);case'Twox64Concat':return Buffer.concat([xxhashAsU8a(bytes,64),bytes]);default:throw new Error('opaque map key requires explicit preimage');}};
  const hashLengths={Identity:0,Blake2_128Concat:16,Twox64Concat:8};
  const opaqueWidths={Blake2_128:16,Blake2_256:32,Twox128:16,Twox256:32};const hashedAccountProbes={};
  const optionsBytes=inventoryOptions?await readFile(inventoryOptions):null;const supplied=optionsBytes?JSON.parse(optionsBytes):{};const preimages=new Map((supplied.keyPreimages??[]).map(p=>[p.key,p.components]));const seenPreimages=new Set();assert(preimages.size===(supplied.keyPreimages??[]).length,'duplicate preimage proof');
  const codeBytes=await readFile(join(capture,'runtime-code.wasm'));assert(sha256(codeBytes)===manifest.artifacts['runtime-code.wasm'].sha256&&sha256(codeBytes)==='de7173a9e0137a32265b341354383958d85beef81103571e6b0aa09622ef2221','deployed code evidence mismatch');
  const prefixes={}, watchedReferences=[], streamedOpaque=[];let total=0, pages=0;const failures=[];
  for(const page of manifest.top.pages){
    const path=resolve(capture,page.file);assert(path.startsWith(resolve(capture)+'/'),'page path escapes capture');
    const checksum=createHash('sha256');for await(const chunk of createReadStream(path))checksum.update(chunk);assert(checksum.digest('hex')===page.sha256,'page checksum mismatch');
    let first=true,count=0;
    for await(const row of lines(createReadStream(path).pipe(createGunzip()))){if(first){assert(row.line,'oversized page header');const header=JSON.parse(row.line);assert(header.checkpointHash===CHECKPOINT.hash&&header.pageIndex===pages,'page identity mismatch');first=false;continue;}count++;total++;
      const keyText=row.line?JSON.parse(row.line)[0]:row.prefix.match(/^\["(0x[0-9a-f]+)",/)?.[1];assert(keyText,'record key missing');
      const descriptor=(descriptors.get(keyText.slice(0,66))??descriptors.get(keyText));const name=descriptor?.name??`unknown:${keyText.slice(0,66)}`;
      const summary=prefixes[name]??={entries:0,decoded:0,failed:0};summary.entries++;
      try{if(keyText==='0x3a636f6465'){const start=`["${keyText}","0x`;assert(row.bytes===keyText.length+9+codeBytes.length*2&&row.prefix.startsWith(start+codeBytes.subarray(0,64).toString('hex'))&&row.tail.endsWith(codeBytes.subarray(-64).toString('hex')+'"]'),'code artifact envelope mismatch');assert((supplied.opaqueRetentions??[]).some(p=>p.key===keyText&&p.valueHash===hex(blake2AsU8a(codeBytes,256))),'exact deployed-code retention required');streamedOpaque.push({storage:'Runtime.Code',key:keyText,encodedBytes:codeBytes.length,valueHash:hex(blake2AsU8a(codeBytes,256)),references:[],value:{historicalRuntimeCode:true},action:'retain exact deployed code; full code bytes independently matched by verified capture reader'});summary.decoded++;continue;}if(!row.line){assert(descriptor,'unknown oversized storage');const streamed=inspectOversizedPreimage(row,keyText,descriptor,types);streamedOpaque.push({storage:name,key:keyText,pageSha256:page.sha256,...streamed});if(streamed.references.length)watchedReferences.push({storage:name,key:keyText,valueScale:null,valueDecoded:streamed.value,references:streamed.references,pageSha256:page.sha256,interpretation:'Typed identity reference only; source-proven opaque call bytes remain unchanged.'});summary.decoded++;continue;}const [keyHex,valueHex]=JSON.parse(row.line);const key=Buffer.from(keyHex.slice(2),'hex'),value=Buffer.from(valueHex.slice(2),'hex');assert(descriptor,'unknown storage encoding');
        let offset=(descriptor.prefix.length-2)/2;const references=[],components=[];let keyDecodingError=null,valueDecodingError=null;
        try{for(let i=0;i<descriptor.hashers.length;i++){const hasher=descriptor.hashers[i],hashLength=hashLengths[hasher];if(hashLength===undefined){const width=opaqueWidths[hasher];assert(width&&offset+width<=key.length,'opaque key width/truncation');const proof=preimages.get(keyHex);if(proof){assert(proof.length===descriptor.hashers.length,'preimage components mismatch');const original=Buffer.from(proof[i].slice(2),'hex');const parsed=parseTyped(original,descriptor.keyTypes[i],types,{path:[`key[${i}]`]});assert(parsed.consumed===original.length&&hashKey(hasher,original).equals(key.subarray(offset,offset+width)),'supplied preimage codec/hash mismatch');components.push(original);references.push(...parsed.references);seenPreimages.add(keyHex);offset+=width;continue;}const type=types.get(descriptor.keyTypes[i]);if(type.path?.at(-1)==='AccountId32'&&type.path.some(p=>p==='sp_core'||p==='sp_runtime')){for(const identity of WATCHED){if(identity.length!==66)continue;const probe=hashedAccountProbes[`${name}:${i}:${identity}`]??={storage:name,keyComponent:i,typeId:descriptor.keyTypes[i],hasher,identity,examinedEntries:0,matchedEntries:0};probe.examinedEntries++;if(hashKey(hasher,Buffer.from(identity.slice(2),'hex')).equals(key.subarray(offset,offset+width))){probe.matchedEntries++;references.push({identity,kind:'account_id32_hashed_key_preimage_match',typeId:descriptor.keyTypes[i],typePath:type.path.join('::'),path:[`key[${i}]`]});}}}offset+=width;keyDecodingError='opaque map key requires explicit preimage';continue;}const decoded=parseTyped(key.subarray(offset+hashLength),descriptor.keyTypes[i],types,{path:[`key[${i}]`]});const original=key.subarray(offset+hashLength,offset+hashLength+decoded.consumed);assert(hashKey(hasher,original).equals(key.subarray(offset,offset+hashLength+decoded.consumed)),'key hasher mismatch');offset+=hashLength+decoded.consumed;components.push(original);references.push(...decoded.references);}assert(offset===key.length,'storage key trailing bytes');}catch(error){keyDecodingError=error.message;}
        try{const decoded=parseTyped(value,descriptor.valueType,types,{path:['value']});assert(decoded.consumed===value.length,'storage value trailing bytes');references.push(...decoded.references);}catch(error){valueDecodingError=error.message;}
        if(references.length){if(!valueDecodingError&&descriptor.valueType<legacy.originalTypeCount){const codec=registry.createTypeUnsafe(`Lookup${descriptor.valueType}`,[value]);assert(Buffer.from(codec.toU8a()).equals(value),'matched value failed independent codec roundtrip');}watchedReferences.push({storage:name,key:keyHex,keyDecoded:components.map((bytes,index)=>parseTyped(bytes,descriptor.keyTypes[index],types,{render:true,watched:new Set()}).value),valueHash:hex(blake2AsU8a(value,256)),valueScale:valueHex,valueDecoded:valueDecodingError?null:parseTyped(value,descriptor.valueType,types,{render:true,watched:new Set()}).value,keyDecodingError,valueDecodingError,references,interpretation:'Typed reference only; beneficial ownership/control requires reviewed pallet semantics.'});}
        assert(!keyDecodingError&&!valueDecodingError,[keyDecodingError,valueDecodingError].filter(Boolean).join('; '));summary.decoded++;
      }catch(error){summary.failed++;if(failures.filter(f=>f.storage===name).length<3)failures.push({storage:name,key:keyText,reason:error.message});}
    }
    assert(count===page.keyCount,'page entry count mismatch');pages++;
    if(pages%250===0)process.stdout.write(JSON.stringify({pages,total,typedHits:watchedReferences.length})+'\n');
  }
  assert(total===manifest.top.keyCount,'capture total mismatch');assert(seenPreimages.size===preimages.size,'unused or unresolved supplied preimage proof');
  const report={schema:'sora-nexus-independent-typed-watch-v1',legacySchemaSha256:sha256(legacyBytes),inventoryOptionsSha256:optionsBytes?sha256(optionsBytes):null,suppliedPreimagesVerified:seenPreimages.size,streamedOpaque,hashedAccountProbes:Object.values(hashedAccountProbes),checkpointHash:CHECKPOINT.hash,stateRoot:CHECKPOINT.stateRoot,captureManifestSha256:sha256(manifestBytes),metadataSha256:sha256(metadataBytes),scriptSha256:sha256(await readFile(new URL(import.meta.url))),watchedAccounts:[...WATCHED],topEntries:total,childTries:0,prefixes,failures,watchedReferences,productionReady:false,limitations:'Metadata is shared evidence; this independent traversal cannot prove beneficial ownership or absent interests in opaque/unknown records. Finality/history and metadata execution witness remain separate gates.'};
  await writeFile(output,JSON.stringify(report,null,2)+'\n',{flag:'wx'});process.stdout.write(JSON.stringify({complete:true,output,total,typedHits:watchedReferences.length,failedEntries:Object.values(prefixes).reduce((sum,p)=>sum+p.failed,0)})+'\n');
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){const args=process.argv.slice(2), options={};for(let i=0;i<args.length;i+=2){assert(['--capture','--dependencies','--output','--inventory-options'].includes(args[i])&&args[i+1],'--capture DIR --dependencies PACKAGE_JSON --output FILE required');options[args[i].slice(2).replace(/-([a-z])/g,(_,c)=>c.toUpperCase())]=args[i+1];}assert(options.capture&&options.dependencies&&options.output,'missing arguments');await run(options);}
