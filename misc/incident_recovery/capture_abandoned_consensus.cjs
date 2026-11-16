#!/usr/bin/env node
// Read-only public header collection; no signing or transaction submission.
const fs = require('node:fs');
const path = require('node:path');
const { parseArgs } = require('node:util');
const options = parseArgs({options:{output:{type:'string'}},allowPositionals:false}).values;
if (!options.output) throw new Error('--output is required');
const root = path.resolve(options.output);
fs.mkdirSync(root, {recursive:false});
const endpoint = 'wss://mof2.sora.org';
const checkpoint = 28191817;
const checkpointHash = '0x7f7c253fa72f32e7b03f32bdc9ae456fd7174fb131b689be6d876006124129f0';
const startedAt = new Date().toISOString();
const ws = new WebSocket(endpoint);
let nextId = 0;
const pending = new Map();
ws.addEventListener('message', e => {
  for (const row of [].concat(JSON.parse(String(e.data)))) {
    const p = pending.get(row.id); if (!p) continue;
    pending.delete(row.id); clearTimeout(p.timer);
    row.error ? p.reject(new Error(JSON.stringify(row.error))) : p.resolve(row.result);
  }
});
function rejectPending(error) {
 for(const item of pending.values()){clearTimeout(item.timer);item.reject(error);}
 pending.clear();
}
ws.addEventListener('close',()=>rejectPending(new Error('RPC connection closed')));
ws.addEventListener('error',()=>rejectPending(new Error('RPC connection failed')));
function rpc(method, params=[]) {
 return new Promise((resolve,reject)=> {
  const id=++nextId;
  const timer=setTimeout(()=>{pending.delete(id);reject(new Error('RPC timed out after ten minutes: '+method));},600000);
  pending.set(id,{resolve,reject,timer});ws.send(JSON.stringify({jsonrpc:'2.0',id,method,params}));
 });
}
function write(name,value) { fs.writeFileSync(path.join(root,name),JSON.stringify(value,null,2)+'\n',{flag:'wx'}); }
(async()=>{
 await new Promise((resolve,reject)=>{ws.addEventListener('open',resolve,{once:true});ws.addEventListener('error',reject,{once:true});});
 const finalized=await rpc('chain_getFinalizedHead');
 const head=await rpc('chain_getHeader');
 const number=parseInt(head.number,16);
 if(!Number.isSafeInteger(number) || number<checkpoint || number-checkpoint>100000)throw new Error('unexpected abandoned head distance');
 const headHash=await rpc('chain_getBlockHash',[number]);
 const cp=await rpc('chain_getHeader',[checkpointHash]);
 const ids={};
 for(const [label,hash] of [['checkpoint',checkpointHash],['finalized',finalized],['best',headHash]])
  ids[label]=await rpc('state_call',['GrandpaApi_current_set_id','0x',hash]);
 write('started.json',{endpoint,startedAt,headHash,head,finalized,checkpointHash,checkpointHeader:cp,setIdCalls:ids,readOnly:true,productionReady:false});
 const records=[];
 for(let start=checkpoint+1;start<=number;start+=8){
  const rows=await Promise.all(Array.from({length:Math.min(8,number-start+1)},async(_,i)=>{
   const n=start+i;const hash=await rpc('chain_getBlockHash',[n]);const header=await rpc('chain_getHeader',[hash]);return{number:n,hash,header};
  }));
  records.push(...rows);
  if(records.length%128===0)process.stderr.write(`Captured ${records.length}/${number-checkpoint} headers\n`);
 }
 const after=await rpc('chain_getHeader');const finalizedAfter=await rpc('chain_getFinalizedHead');
 if(JSON.stringify(after)!==JSON.stringify(head)||finalizedAfter!==finalized)throw new Error('public head changed during capture');
 write('headers.json',records);
 write('completion.json',{schema:'sora-nexus-public-abandoned-consensus-capture-v1',endpoint,startedAt,finishedAt:new Date().toISOString(),headHash,finalized,headers:records.length,headsUnchanged:true,headerHashesVerified:false,clientAuthoritySetsVerified:false,productionReady:false});
 ws.close();
})().catch(error=>{write('failure.json',{error:String(error),productionReady:false});process.stderr.write(String(error)+'\n');rejectPending(error);ws.close();process.exitCode=1;});
