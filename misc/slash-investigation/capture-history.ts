import {readFileSync,writeFileSync} from 'node:fs';
import {dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {ApiPromise,WsProvider} from '@polkadot/api';
import {fullOverrideBundle} from '@sora-substrate/type-definitions';
const out=process.env.SLASH_EVIDENCE_DIR ?? dirname(fileURLToPath(import.meta.url));
const snap=JSON.parse(readFileSync(out+'/snapshot.json','utf8'));
const timer=setTimeout(()=>process.exit(2),180000);
async function main(){
 const api=await ApiPromise.create({provider:new WsProvider('wss://ws.mof.sora.org'),typesBundle:{spec:{'sora-substrate':fullOverrideBundle.spec.sora}}});
 try{
 const at=await api.at(snap.blockHash);const current=snap.storage['session.currentIndex'];
 const kind='0x'+Buffer.from('im-online:offlin').toString('hex');
 const eras=snap.storage['staking.bondedEras'];const first=eras[1][1];
 const sessions=Array.from({length:current-first},(_,i)=>first+i);
 const slots=sessions.map(n=>'0x'+Buffer.from(api.createType('u32',n).toU8a()).toString('hex'));
 const ids=await at.query.offences.concurrentReportsIndex.multi(slots.map(s=>[kind,s]));
 const unique=[...new Set(ids.flatMap(v=>v.map(x=>x.toHex())))];
 const records=await at.query.offences.reports.multi(unique);
 const report:any={checkedAt:new Date().toISOString(),blockHash:snap.blockHash,offenceKind:'im-online:offlin',sessions:sessions.map((n,i)=>({session:n,reportIds:ids[i].toJSON()})),reports:Object.fromEntries(unique.map((id,i)=>[id,records[i].toJSON()])),submittedTransactions:0};
 writeFileSync(out+'/history.json',JSON.stringify(report,null,2)+'\n');console.log('offline reports',unique.length,'sessions',sessions.length);
 const [epochStart,epochIndex,queuedKeys]=await Promise.all([at.query.babe.epochStart(),at.query.babe.epochIndex(),at.query.session.queuedKeys()]);
 report.epochStart=epochStart.toJSON(); report.epochIndex=epochIndex.toJSON();report.queuedKeys=queuedKeys.toJSON();
 const start=epochStart[1].toNumber();
 report.boundary=[];
 for(const block of [start-1,start]){
  const hash=await api.rpc.chain.getBlockHash(block); let h; try{h=await api.at(hash)}catch(e){report.boundary.push({block,hash:hash.toHex(),error:String(e)});continue;}
  const [session,events,authors,beats,validators,disabled,keys]=await Promise.all([h.query.session.currentIndex(),h.query.system.events(),h.query.imOnline.authoredBlocks.entries(),h.query.imOnline.receivedHeartbeats.entries(),h.query.session.validators(),h.query.session.disabledValidators(),h.query.imOnline.keys()]);
  report.boundary.push({block,hash:hash.toHex(),session:session.toJSON(),events:events.filter(x=>['staking','offences','imOnline','session'].includes(x.event.section)).map(x=>({section:x.event.section,method:x.event.method,data:x.event.data.toJSON()})),authors:authors.map(([k,v])=>({key:k.args.map(x=>x.toJSON()),value:v.toJSON()})),heartbeats:beats.map(([k,v])=>({key:k.args.map(x=>x.toJSON()),value:v.toJSON()})),validators:validators.toJSON(),disabled:disabled.toJSON(),keys:keys.toJSON()});
 }
 writeFileSync(out+'/history.json',JSON.stringify(report,null,2)+'\n');console.log('boundary',JSON.stringify(report.boundary.map(b=>({block:b.block,session:b.session,events:b.events,authors:b.authors?.length,heartbeats:b.heartbeats?.length,error:b.error})),null,2));
 }finally{await api.disconnect();clearTimeout(timer)}
}main().catch(e=>{console.error(e);process.exit(1)});
