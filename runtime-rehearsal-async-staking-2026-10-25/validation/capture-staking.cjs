'use strict';
const fs = require('node:fs');
const path = require('node:path');
const zlib = require('node:zlib');
const {createHash} = require('node:crypto');
const base = __dirname;
const {WsProvider} = require(path.join(base,'node_modules/@polkadot/api'));
const {Metadata,TypeRegistry} = require(path.join(base,'node_modules/@polkadot/types'));
const {cryptoWaitReady,xxhashAsHex} = require(path.join(base,'node_modules/@polkadot/util-crypto'));
const sha = x => createHash('sha256').update(x).digest('hex');
(async()=>{
 await cryptoWaitReady();
 const snap=JSON.parse(fs.readFileSync(path.join(base,'snapshot.json')));
 if(snap.endpoint!=='wss://mof2.sora.org') throw Error('unapproved endpoint');
 const provider=new WsProvider(snap.endpoint); await provider.isReady;
 const allowed=new Set(['chain_getBlockHash','state_getKeysPaged','state_queryStorageAt']);
 const counts={}; const send=(m,p)=>{if(!allowed.has(m))throw Error('forbidden method');counts[m]=(counts[m]||0)+1;return provider.send(m,p)};
 try {
  if(await send('chain_getBlockHash',[snap.blockNumber])!==snap.blockHash)throw Error('pin mismatch');
  const prefix=xxhashAsHex('Staking',128); let last;const keys=[];
  for(;;){const page=await send('state_getKeysPaged',[prefix,500,last??null,snap.blockHash]);if(!page.length)break;
   if(page.some(k=>!k.startsWith(prefix))||(last&&page[0]<=last))throw Error('invalid key page');keys.push(...page);last=page.at(-1);if(keys.length>1000000)throw Error('unexpected key count');
   if(page.length<500)break;
  }
  console.log('Pinned full Staking keys:',keys.length);
  const records=[];
  for(let offset=0;offset<keys.length;offset+=800){
   const batches=[];for(let n=offset;n<Math.min(offset+800,keys.length);n+=200)batches.push(keys.slice(n,n+200));
   const values=await Promise.all(batches.map(batch=>send('state_queryStorageAt',[batch,snap.blockHash])));
   values.forEach((v,i)=>{if(v.length!==1||v[0].block!==snap.blockHash||v[0].changes.length!==batches[i].length)throw Error('invalid value batch');records.push(...v[0].changes)});
   console.log('Values:',records.length);
  }
  const metadataBytes=fs.readFileSync(path.join(base,'deployed-metadata.scale'));
  if(sha(metadataBytes)!==snap.metadata.sha256)throw Error('metadata hash');
  const registry=new TypeRegistry();const metadata=new Metadata(registry,metadataBytes);registry.setMetadata(metadata);
  const pallet=metadata.asLatest.pallets.find(p=>p.name.toString()==='Staking');
  const inventories={};
  for(const item of pallet.storage.unwrap().items){
   const name=item.name.toString(); const storagePrefix=prefix+xxhashAsHex(name,128).slice(2);
   const entries=records.filter(([k])=>k.startsWith(storagePrefix)); const type=item.type.isMap?item.type.asMap.value:item.type.asPlain;
   let maxBytes=0,maxOthers=0,maxReporters=0,maxEntries=0;const samples=[];
   for(const [key,value] of entries){if(value===null)throw Error('missing enumerated key');maxBytes=Math.max(maxBytes,(value.length-2)/2);
    const decoded=registry.createTypeUnsafe('Lookup'+type,[Buffer.from(value.slice(2),'hex')]).toJSON();
    if(decoded?.others)maxOthers=Math.max(maxOthers,decoded.others.length);
    if(Array.isArray(decoded)){maxEntries=Math.max(maxEntries,decoded.length);for(const entry of decoded){maxOthers=Math.max(maxOthers,entry?.others?.length??0);maxReporters=Math.max(maxReporters,entry?.reporters?.length??0)}}
    if(name==='UnappliedSlashes'||['ActiveEra','CurrentEra','CurrentPlannedSession','BondedEras'].includes(name))samples.push({key,value:decoded});
   }
   inventories[name]={count:entries.length,maxBytes,maxOthers,maxReporters,maxEntries,...(samples.length?{samples}:{})};
  }
  const output={status:'captured-complete-pinned-staking',endpoint:snap.endpoint,blockHash:snap.blockHash,blockNumber:snap.blockNumber,prefix,complete:true,keys:keys.length,records};
  const file=path.join(base,'staking-public-state.json.gz');fs.writeFileSync(file,zlib.gzipSync(JSON.stringify(output)));
  const report={status:'passed',createdAt:new Date().toISOString(),blockHash:snap.blockHash,blockNumber:snap.blockNumber,endpoint:snap.endpoint,complete:true,totalKeys:keys.length,metadataSha256:sha(metadataBytes),stateFile:path.basename(file),stateSha256:sha(fs.readFileSync(file)),rpcMethods:counts,submittedTransactions:0,inventories};
  fs.writeFileSync(path.join(base,'staking-public-inventory.json'),JSON.stringify(report,null,2)+'\n');
  console.log('Capture complete');
 } finally {await provider.disconnect()}
})().catch(e=>{console.error(e);process.exitCode=1});
