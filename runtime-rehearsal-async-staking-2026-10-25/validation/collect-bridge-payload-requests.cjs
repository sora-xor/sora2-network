'use strict';
// Public archived-state reconstruction. Never signs, submits, or reads credentials.
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const {createRequire}=require('node:module');
const validation='/Users/takemiyamakoto/dev/sora2-network/runtime-rehearsal-async-staking-2026-10-25/validation';
const requireValidation=createRequire(path.join(validation,'package.json'));
const {ApiPromise,WsProvider}=requireValidation('@polkadot/api');
const {fullOverrideBundle}=requireValidation('@sora-substrate/type-definitions');
const {blake2AsHex}=requireValidation('@polkadot/util-crypto');
const snapshot=JSON.parse(fs.readFileSync(path.join(validation,'snapshot.json')));
const cap=300000,pageSize=512,network=0;
const output=__dirname;
const sha256=value=>crypto.createHash('sha256').update(value).digest('hex');
const report={status:'running',endpoint:'wss://mof2.sora.org',blockHash:snapshot.blockHash,blockNumber:snapshot.blockNumber,
  requestsCap:cap,pageSize,requestCount:0,requestKinds:{},requestScanComplete:false,pending:[],recovered:{},rpcMethods:{},
  requestsAtPendingTimepoints:0,submittedTransactions:0,privateKeysRead:0,startedAt:new Date().toISOString()};
const allowed=new Set(['chain_getBlock','chain_getBlockHash','chain_getHeader','chain_getFinalizedHead','state_getStorage',
 'state_queryStorageAt','state_getKeysPaged','state_getMetadata','state_getRuntimeVersion','state_call','system_properties',
 'system_chain','system_name','system_version','rpc_methods','state_subscribeRuntimeVersion','state_unsubscribeRuntimeVersion']);
const rawStorage=new Map(), matched=new Map();let api;
const rawPath=path.join(output,'request-pages.jsonl');
function save(){
 report.unresolved=report.pending.filter(p=>!report.recovered[p.hash]).map(p=>({hash:p.hash,when:p.when}));
 report.scriptSha256=sha256(fs.readFileSync(__filename));
 fs.writeFileSync(path.join(output,'full-recovery.json'),JSON.stringify(report,null,2)+'\n');
 fs.writeFileSync(path.join(output,'matched-requests.json'),JSON.stringify([...matched.values()],null,2)+'\n');
}
async function main(){
 const provider=new WsProvider(report.endpoint),send=provider.send.bind(provider);
 provider.send=async(method,params,...rest)=>{
  assert(allowed.has(method),'Forbidden RPC '+method);report.rpcMethods[method]=(report.rpcMethods[method]||0)+1;
  const result=await send(method,params,...rest);
  if(method==='state_queryStorageAt')for(const change of result)for(const [key,value] of change.changes)rawStorage.set(key,value);
  if(method==='state_getStorage')rawStorage.set(params[0],result);
  return result;
 };
 api=await ApiPromise.create({provider,noInitWarn:true,typesBundle:{spec:{'sora-substrate':fullOverrideBundle.spec.sora}}});
 assert.equal(api.genesisHash.toHex(),snapshot.genesis);
 assert.equal((await api.rpc.chain.getBlockHash(snapshot.blockNumber)).toHex(),snapshot.blockHash);
 const version=(await api.rpc.state.getRuntimeVersion(snapshot.blockHash)).toJSON();
 assert.deepEqual(version,snapshot.runtimeVersion);
 assert.equal(api.runtimeVersion.specVersion.toNumber(),version.specVersion);
 const at=await api.at(snapshot.blockHash);
 const id=(await at.query.ethBridge.bridgeAccount(network)).unwrap();report.account=id.toString();
 report.peers=(await at.query.bridgeMultisig.accounts(id)).unwrap().signatories.map(p=>p.toString());
 let startKey;
 for(let pages=0;pages<80;pages++){
  const page=await at.query.bridgeMultisig.multisigs.entriesPaged({args:[id],pageSize:128,...(startKey?{startKey}:{})});
  for(const [key,value] of page){assert.equal(value.unwrap().toHex(),rawStorage.get(key.toHex()));report.pending.push({key:key.toHex(),hash:key.args[1].toHex(),rawValue:rawStorage.get(key.toHex()),...value.unwrap().toJSON()});}
  if(page.length<128){report.pendingScanComplete=true;break;}startKey=page.at(-1)[0].toHex();
 }
 assert(report.pendingScanComplete,'Pending scan truncated');
 fs.writeFileSync(path.join(output,'pending-raw-storage.json'),JSON.stringify([...rawStorage])+'\n');
 const wanted=new Set(report.pending.map(p=>p.hash));
 const timepoints=new Set(report.pending.map(p=>JSON.stringify(p.when)));
 const lookup=at.registry.createLookupType(at.query.ethBridge.requests.meta.type.asMap.value);
 const decode=raw=>at.registry.createType(lookup,raw);
 function retain(key,value){
  const raw=rawStorage.get(key.toHex());assert.equal(value.toHex(),raw);
  const row={key:key.toHex(),hash:key.args[1].toHex(),rawValue:raw,kind:value.type};
  matched.set(row.hash,row);return row;
 }
 function timepoint(value){
  if(value.isIncoming)return value.asIncoming[0].value.get('timepoint');
  if(value.isLoadIncoming){const load=value.asLoadIncoming;return(load.isTransaction?load.asTransaction:load.asMeta[0]).get('timepoint');}
  if(value.isOutgoing)return value.asOutgoing[0].value.get('timepoint');
  throw Error('Unexpected request kind');
 }
 fs.writeFileSync(rawPath,'');startKey=undefined;
 while(report.requestCount<cap){
  rawStorage.clear();const count=Math.min(pageSize,cap-report.requestCount);
  const page=await at.query.ethBridge.requests.entriesPaged({args:[network],pageSize:count,...(startKey?{startKey}:{})});
  if(startKey&&page.length)assert(page[0][0].toHex()>startKey,'Pagination must advance');
  const rawPage=[];
  for(const [key,optional] of page){
   const value=optional.unwrap();assert.equal(value.toHex(),rawStorage.get(key.toHex()));
   rawPage.push([key.toHex(),value.toHex()]);report.requestKinds[value.type]=(report.requestKinds[value.type]||0)+1;
   if(timepoints.has(JSON.stringify(timepoint(value).toJSON())))retain(key,value);
  }
  fs.appendFileSync(rawPath,JSON.stringify({startKey:startKey||null,rows:rawPage})+'\n');
  report.requestCount+=page.length;report.requestsAtPendingTimepoints=matched.size;
  if(page.length<count){report.requestScanComplete=true;break;}
  startKey=page.at(-1)[0].toHex();report.requestCursor=startKey;
  if(report.requestCount%8192===0){save();console.log('SCAN',report.requestCount,matched.size);}
 }
 save();
 function check(call,bytes,source){
  const hash=blake2AsHex(bytes,256);if(!wanted.has(hash))return;
  if(report.recovered[hash]){assert.equal(report.recovered[hash].callHex,bytes);return;}
  report.recovered[hash]={account:report.account,hash,callHex:bytes,callBytes:(bytes.length-2)/2,source:{kind:'retained-historical-request-reconstruction',reference:'matched-requests.json',...source},
   currentCallHex:call.method.toHex(),currentMethod:call.method.section+'.'+call.method.method,
   derivation:'All bytes after the first pallet index byte equal the call encoded with the pinned deployed registry.'};
 }
 function variants(call,source){
  const current=call.method.toHex();check(call,current,{...source,palletIndex:Number.parseInt(current.slice(2,4),16)});
  check(call,'0x1e'+current.slice(4),{...source,palletIndex:30});
 }
 const sourceRows=[...matched.values()];
 for(const row of sourceRows){
  const value=decode(row.rawValue);
  variants(api.tx.ethBridge.finalizeIncomingRequest(row.hash,network),{requestKey:row.key});
  if(!value.isIncoming)continue;
  const incoming=value.asIncoming[0];
  variants(api.tx.ethBridge.registerIncomingRequest(incoming),{requestKey:row.key});
  const inner=incoming.value;const loadHash=(inner.get('txHash')||inner.get('initialRequestHash')).toHex();
  let loadRow=matched.get(loadHash);
  if(!loadRow){
   const loadValue=await at.query.ethBridge.requests(network,loadHash);
   if(loadValue.isSome){const key=api.createType('StorageKey',at.query.ethBridge.requests.key(network,loadHash));loadRow={key:key.toHex(),hash:loadHash,rawValue:loadValue.unwrap().toHex(),kind:loadValue.unwrap().type};assert.equal(loadRow.rawValue,rawStorage.get(key.toHex()));matched.set(loadHash,loadRow);}
  }
  if(!loadRow||!decode(loadRow.rawValue).isLoadIncoming)continue;
  const load=decode(loadRow.rawValue).asLoadIncoming;
  variants(api.tx.ethBridge.importIncomingRequest(load,{Ok:incoming}),{requestKey:row.key,loadRequestKey:loadRow.key,loadHash});
 }
 report.recoveredCount=Object.keys(report.recovered).length;
 report.requestPagesSha256=sha256(fs.readFileSync(rawPath));
 report.requestPagesBytes=fs.statSync(rawPath).size;
 report.status=report.requestScanComplete?'passed':'truncated';report.completedAt=new Date().toISOString();save();
 fs.writeFileSync(path.join(output,'bridge-payload-evidence.json'),JSON.stringify({payloads:Object.values(report.recovered)},null,2)+'\n');
}
const timer=setTimeout(()=>{report.status='timed_out';save();process.exit(2);},900000);
main().catch(error=>{report.status='failed';report.error=error.stack;process.exitCode=1;}).finally(async()=>{clearTimeout(timer);if(api)await api.disconnect();save();console.log(JSON.stringify({status:report.status,requestCount:report.requestCount,matched:matched.size,recovered:report.recoveredCount,unresolved:report.unresolved.length,error:report.error}));});
