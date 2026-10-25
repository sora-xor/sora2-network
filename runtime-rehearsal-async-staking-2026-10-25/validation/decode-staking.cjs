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

(async()=>{await cryptoWaitReady(); const snap=JSON.parse(fs.readFileSync(path.join(base,'snapshot.json')));const state=JSON.parse(zlib.gunzipSync(fs.readFileSync(path.join(base,'staking-public-state.json.gz'))));if(state.blockHash!==snap.blockHash||!state.complete)throw Error('invalid stored pin');const {prefix,records}=state;  const metadataBytes=fs.readFileSync(path.join(base,'deployed-metadata.scale'));
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

const file=path.join(base,'staking-public-inventory.json');const report=JSON.parse(fs.readFileSync(file));report.inventories=inventories;report.decodedAt=new Date().toISOString();report.decoding='SCALE bytes from Uint8Array, including primitive values';fs.writeFileSync(file,JSON.stringify(report,null,2)+'\n'); console.log(JSON.stringify(inventories.CurrentEra));})().catch(e=>{console.error(e);process.exitCode=1});
