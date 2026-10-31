'use strict';
// Locate a witnessed adjacent absent->present boundary, not the first-ever creation.
const assert=require('node:assert/strict'),fs=require('node:fs'),{createRequire}=require('node:module');
const validation='/Users/takemiyamakoto/dev/sora2-network/runtime-rehearsal-async-staking-2026-10-25/validation';
const req=createRequire(validation+'/package.json');
const {ApiPromise,WsProvider}=req('@polkadot/api');
const {fullOverrideBundle,types}=req('@sora-substrate/type-definitions');
const {blake2AsHex}=req('@polkadot/util-crypto');
const snapshot=JSON.parse(fs.readFileSync(validation+'/snapshot.json'));
const source=JSON.parse(fs.readFileSync(__dirname+'/full-recovery.json'));
const missing=new Set(source.unresolved.map(x=>x.hash));
const report={status:'running',endpoint:'wss://mof2.sora.org',blockHash:snapshot.blockHash,blockNumber:snapshot.blockNumber,
 submittedTransactions:0,privateKeysRead:0,roundLimit:25,concurrency:4,rpcMethods:{},transitions:[],payloads:[],observations:[]};
const allowed=new Set(['chain_getBlock','chain_getBlockHash','chain_getHeader','chain_getFinalizedHead','state_getStorage',
 'state_queryStorageAt','state_getMetadata','state_getRuntimeVersion','state_call','system_properties','system_chain','system_name',
 'system_version','rpc_methods','state_subscribeRuntimeVersion','state_unsubscribeRuntimeVersion']);
let api;const save=()=>fs.writeFileSync(__dirname+'/archive-recovery.json',JSON.stringify(report,null,2)+'\n');
async function main(){
 const provider=new WsProvider(report.endpoint),send=provider.send.bind(provider);
 provider.send=async(method,params,...rest)=>{assert(allowed.has(method),'Forbidden RPC '+method);report.rpcMethods[method]=(report.rpcMethods[method]||0)+1;return send(method,params,...rest);};
 api=await ApiPromise.create({provider,noInitWarn:true,typesBundle:{spec:{'sora-substrate':fullOverrideBundle.spec.sora}}});
 assert.equal(api.genesisHash.toHex(),snapshot.genesis);
 assert.equal((await api.rpc.chain.getBlockHash(snapshot.blockNumber)).toHex(),snapshot.blockHash);
 const pending=source.pending.filter(p=>missing.has(p.hash));
 const bounds=pending.map(p=>({...p,low:0,high:snapshot.blockNumber}));
 const blockHashes=new Map([[0,api.genesisHash.toHex()],[snapshot.blockNumber,snapshot.blockHash]]);
 async function read(height,items){
  if(!blockHashes.has(height))blockHashes.set(height,(await api.rpc.chain.getBlockHash(height)).toHex());
  const blockHash=blockHashes.get(height);
  const result=await provider.send('state_queryStorageAt',[items.map(x=>x.key),blockHash]);
  const values=new Map(result.flatMap(x=>x.changes));
  assert.equal(values.size,items.length);
  report.observations.push({height,blockHash,values:[...values]});
  return values;
 }
 const genesis=await read(0,bounds);assert([...genesis.values()].every(value=>value===null),'Genesis must lack pending operations');
 const pinned=await read(snapshot.blockNumber,bounds);for(const row of bounds)assert.equal(pinned.get(row.key),row.rawValue);
 for(let round=0;round<25;round++){
  const grouped=new Map();
  for(const row of bounds)if(row.high-row.low>1){const height=Math.floor((row.low+row.high)/2);if(!grouped.has(height))grouped.set(height,[]);grouped.get(height).push(row);}
  if(!grouped.size)break;
  const groups=[...grouped];
  for(let offset=0;offset<groups.length;offset+=4)await Promise.all(groups.slice(offset,offset+4).map(async([height,items])=>{
   const values=await read(height,items);for(const row of items)if(values.get(row.key)===null)row.low=height;else row.high=height;
  }));
  report.rounds=round+1;report.bounds=bounds.map(({hash,low,high})=>({hash,low,high}));save();console.log('ROUND',round+1,JSON.stringify(report.bounds));
 }
 assert(bounds.every(x=>x.high-x.low===1),'Search did not converge within round bound');
 const blocks=new Map();
 for(const row of bounds){
  const before=await read(row.low,[row]),after=await read(row.high,[row]);
  assert.equal(before.get(row.key),null);assert(after.get(row.key));
  const hash=blockHashes.get(row.high);
  report.transitions.push({hash:row.hash,key:row.key,before:{height:row.low,blockHash:blockHashes.get(row.low),value:null},after:{height:row.high,blockHash:hash,value:after.get(row.key)}});
  if(!blocks.has(hash))blocks.set(hash,{height:row.high,value:await provider.send('chain_getBlock',[hash])});
 }
 report.blocks=[];
 for(const [blockHash,{height,value:block}] of blocks){
  const historical=await api.at(blockHash);historical.registry.register({...types,Timepoint:'BridgeTimepoint'});
  const metadata=await api.rpc.state.getMetadata(blockHash);
  const blockProof={blockHash,height,header:block.block.header,extrinsics:block.block.extrinsics,metadataHex:metadata.toHex(),decodeErrors:[]};
  report.blocks.push(blockProof);
  for(let index=0;index<block.block.extrinsics.length;index++){
   const extrinsicHex=block.block.extrinsics[index];
   try{
    const ext=historical.registry.createType('Extrinsic',extrinsicHex);assert.equal(ext.toHex(),extrinsicHex,'Full historical extrinsic roundtrip');const call=ext.method;
    if(call.section==='bridgeMultisig'&&call.method==='asMulti'){
     const callHex=call.args[2].toHex(),hash=blake2AsHex(callHex,256);
     if(missing.has(hash)){
      report.payloads.push({account:call.args[0].toString(),hash,callHex,callBytes:(callHex.length-2)/2,
       source:{kind:'archived-pending-creation-extrinsic',reference:'bridge-archived-payload-source.json',blockHash,height,extrinsicIndex:index,extrinsicHex,
        boundary:'A verified adjacent absent-to-present storage transition; no first-ever creation claim.'}});
      console.log('RECOVERED',hash,callHex.length/2-1,height,index);
     }
    }
   }catch(error){blockProof.decodeErrors.push({index,error:error.message});}
  }
 }
 report.recoveredCount=new Set(report.payloads.map(p=>p.hash)).size;
 report.unresolved=[...missing].filter(hash=>!report.payloads.some(p=>p.hash===hash));report.status='passed';save();
}
const timer=setTimeout(()=>{report.status='timed_out';save();process.exit(2);},600000);
main().catch(error=>{report.status='failed';report.error=error.stack;process.exitCode=1;}).finally(async()=>{clearTimeout(timer);if(api)await api.disconnect();save();console.log(JSON.stringify({status:report.status,recovered:report.recoveredCount,unresolved:report.unresolved,error:report.error}));});
