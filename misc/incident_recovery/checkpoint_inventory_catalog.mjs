/** Complete key-only catalog of an already root-verified checkpoint capture.
 * Page-range hints are not authenticated by the native reader, so they are
 * deliberately unused. Values remain opaque here; native inventory admission
 * separately rechecks every codec, supplied key preimage and retention proof. */
import {createReadStream} from 'node:fs';
import {lstat} from 'node:fs/promises';
import {createHash} from 'node:crypto';
const need=(condition,message)=>{if(!condition)throw Error(message);};
const key=value=>typeof value==='string'&&/^0x(?:[0-9a-f]{2})+$/.test(value)&&value.length<=2050;
const digest=value=>typeof value==='string'&&/^0x[0-9a-f]{64}$/.test(value)&&!/^0x0+$/.test(value);
/** Large capture pages are hashed incrementally; never allocate their full file. */
export async function streamDigest(path,maximum) {
  const info=await lstat(path);
  need(info.isFile()&&!info.isSymbolicLink()&&info.size<=maximum,`unaliased bounded capture file required: ${path} (${info.size} bytes; maximum ${maximum})`);
  const hash=createHash('sha256');let count=0;
  for await(const part of createReadStream(path)){count+=part.length;need(count<=maximum,`capture file exceeded bound while reading: ${path}`);hash.update(part);}
  need(count===info.size,`capture file changed while reading: ${path}`);return hash.digest('hex');
}
export const OPAQUE_REASONS=Object.freeze({
  'Preimage.PreimageFor':'Retain exact opaque historical call bytes; current RuntimeCall decoding/execution is forbidden and request/deposit state is separately inventoried.',
  'unknown:0x3a636f6465':'Retain the exact deployed spec-134 runtime code; the immutable substitute and final activation are separate authenticated operations.',
});

export function sourceBase(options,legacy,preimagePrefix) {
  need(options.includePinnedCheckpointLegacy===true&&!(options.additionalStorage??[]).length,'source-pinned legacy codecs required; custom schemas forbidden');
  const allowed=new Map(legacy.storage.filter(row=>row.disposition==='retain_historical').map(row=>[row.name,row.reason]));
  for(const entry of Object.entries(OPAQUE_REASONS))allowed.set(...entry);
  const seen=new Set();
  for(const row of options.dispositions??[]) {
    need(row.trie==='top'&&row.action==='retain_historical'&&allowed.get(row.storage)===row.reason
      &&digest(row.evidenceHash)&&!seen.has(row.storage),'baseline must contain only source-defined immutable retentions');
    seen.add(row.storage);
  }
  need(seen.has('Preimage.PreimageFor')&&seen.has('unknown:0x3a636f6465'),'immutable preimage/code dispositions required');
  const retained=new Set();
  for(const row of options.opaqueRetentions??[]) {
    need(row.trie==='top'&&key(row.key)&&(row.key==='0x3a636f6465'||row.key.startsWith(preimagePrefix))
      &&digest(row.valueHash)&&digest(row.evidenceHash)&&!retained.has(row.key),'unexpected or duplicate opaque retention');
    retained.add(row.key);
  }
  need(retained.has('0x3a636f6465'),'deployed code retention required');
  need(Array.isArray(options.keyPreimages)&&Array.isArray(options.watchedAccounts),'native key-preimage/watch inputs required');
  return {includePinnedCheckpointLegacy:true,keyPreimages:structuredClone(options.keyPreimages),
    opaqueRetentions:structuredClone(options.opaqueRetentions),watchedAccounts:[...options.watchedAccounts]};
}

/** readPage must hash-check the compressed file and return bounded line records. */
export async function catalogCapture(manifest,descriptors,readPage,onPage=()=>{}) {
  need(manifest.complete===true&&manifest.children.length===0&&manifest.top.trie==='top'
    &&manifest.top.pageCount===manifest.top.pages.length&&manifest.top.pages.length>0,'complete top-only capture required');
  const rows=new Map();let cursor=null,total=0,terminal=false;
  for(const [index,page]of manifest.top.pages.entries()) {
    need(!terminal&&page.file===`pages/top/${String(index).padStart(8,'0')}.jsonl.gz`
      &&Number.isSafeInteger(page.keyCount)&&page.keyCount>=0&&page.keyCount<=1000,'page identity/count differs');
    let header,firstKey=null,count=0;
    for await(const line of readPage(page,index)) {
      if(!header) {
        need(line.line,'missing or oversized page header');header=JSON.parse(line.line);
        need(header.schema==='sora-nexus-checkpoint-page-v1'&&header.checkpointHash===manifest.checkpoint.hash
          &&header.trie==='top'&&header.pageIndex===index&&header.startKey===cursor
          &&Number.isSafeInteger(header.pageSize)&&header.pageSize>0&&header.pageSize<=1000
          &&header.keyCount===page.keyCount&&header.keyCount<=header.pageSize,'page header/cursor differs');
        continue;
      }
      const storageKey=line.prefix.match(/^\["(0x[0-9a-f]+)","0x/)?.[1];
      need(key(storageKey)&&(cursor===null||storageKey>cursor),'unordered, duplicate or malformed captured key');
      const descriptor=descriptors.get(storageKey)??descriptors.get(storageKey.slice(0,66));
      need(descriptor,`unknown source storage prefix ${storageKey}`);
      const existing=rows.get(descriptor.name)??{trie:'top',storage:descriptor.name,prefix:descriptor.prefix,entries:0};
      need(existing.prefix===descriptor.prefix,'storage descriptor name collision');existing.entries++;rows.set(descriptor.name,existing);
      firstKey??=storageKey;cursor=storageKey;count++;total++;
    }
    need(header&&count===page.keyCount&&header.lastKey===cursor&&header.terminal===(count===0),'page coverage differs');
    terminal=header.terminal;
    onPage({file:page.file,sha256:page.sha256,keyCount:count,firstKey,lastKey:count?cursor:null});
  }
  need(terminal&&total===manifest.top.keyCount,'capture traversal incomplete');
  return [...rows.values()].sort((a,b)=>a.storage.localeCompare(b.storage));
}

export function inventoryFromCatalog(base,catalog,mutable,evidenceHash) {
  need(digest(evidenceHash),'catalog evidence commitment required');
  const names=new Set();
  const dispositions=catalog.map(row=>{
    need(row.trie==='top'&&row.entries>0&&!names.has(row.storage),'duplicate or empty catalog prefix');names.add(row.storage);
    const mutableReason=mutable[row.storage];
    return {trie:'top',storage:row.storage,action:mutableReason?'mutable_ownership':'retain_historical',
      reason:mutableReason??OPAQUE_REASONS[row.storage]??'Preserve checkpoint bytes unless an exact native adapter resolves mapped ownership; mapped mutable references are not waived by this retention.',evidenceHash};
  });
  return {...structuredClone(base),dispositions};
}
