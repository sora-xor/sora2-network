import {writeFileSync} from 'node:fs';
import {ApiPromise, WsProvider} from '@polkadot/api';
import {fullOverrideBundle} from '@sora-substrate/type-definitions';
import {mkdirSync} from 'node:fs';
import {dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
const timer=setTimeout(()=>process.exit(2),180000);
const out=process.env.SLASH_EVIDENCE_DIR ?? dirname(fileURLToPath(import.meta.url));
mkdirSync(out,{recursive:true});
async function main(){
 const api=await ApiPromise.create({provider:new WsProvider('wss://ws.mof.sora.org'),typesBundle:{spec:{'sora-substrate':fullOverrideBundle.spec.sora}}});
 try{
 const hash=await api.rpc.chain.getFinalizedHead(); const at=await api.at(hash);
 const fields=[['staking','activeEra'],['staking','currentEra'],['staking','unappliedSlashes'],['staking','validatorSlashInEra'],['staking','bondedEras'],['session','currentIndex'],['session','validators'],['session','disabledValidators'],['imOnline','authoredBlocks'],['imOnline','receivedHeartbeats'],['imOnline','keys'],['staking','offenceQueue'],['staking','offenceQueueEras'],['staking','unappliedSlashesEra']];
 const report:any={checkedAt:new Date().toISOString(),endpoint:'wss://ws.mof.sora.org',blockHash:hash.toHex(),blockNumber:(await api.rpc.chain.getHeader(hash)).number.toNumber(),version:(await api.rpc.state.getRuntimeVersion(hash)).toJSON(),timestamp:(await at.query.timestamp.now()).toString(),consts:{staking:Object.fromEntries(Object.entries(at.consts.staking).map(([k,v])=>[k,v.toJSON()]))},storage:{},errors:{},submittedTransactions:0};
 await Promise.all(fields.map(async ([pallet,name])=>{const q=at.query[pallet]?.[name]; if(!q){report.errors[pallet+'.'+name]='not in metadata';return;} try{report.storage[pallet+'.'+name]=q.meta.type.isPlain?(await q()).toJSON():(await q.entries()).map(([k,v])=>({key:k.args.map(x=>x.toJSON()),value:v.toJSON()}));}catch(e){report.errors[pallet+'.'+name]=String(e);} writeFileSync(out+'/snapshot.partial.json',JSON.stringify(report,null,2)); console.log('captured '+pallet+'.'+name); }));
 writeFileSync(out+'/snapshot.json',JSON.stringify(report,null,2)+'\n');
 console.log(JSON.stringify({blockNumber:report.blockNumber,version:report.version,storage:Object.fromEntries(Object.entries(report.storage).map(([k,v])=>[k,Array.isArray(v)?{count:v.length,sample:v.slice(0,2)}:v])),errors:report.errors},null,2));
 }finally{await api.disconnect();clearTimeout(timer);}
}
main().catch(e=>{console.error(e);process.exit(1)});
