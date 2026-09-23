const {createRequire}=require('node:module');const {readFileSync,writeFileSync}=require('node:fs');
const deps=createRequire('/tmp/sora-validator-investigation-20260922/package.json');
const {ApiPromise,WsProvider}=deps('@polkadot/api');const {fullOverrideBundle}=deps('@sora-substrate/type-definitions');
const snap=JSON.parse(readFileSync(__dirname+'/snapshot.json'));const timer=setTimeout(()=>process.exit(2),180000);
(async()=>{const endpoint='wss://mof2.sora.org';const api=await ApiPromise.create({provider:new WsProvider(endpoint),typesBundle:{spec:{'sora-substrate':fullOverrideBundle.spec.sora}}});
try{
 if(api.genesisHash.toHex()!==snap.genesis)throw Error('Genesis mismatch');if((await api.rpc.chain.getBlockHash(snap.blockNumber)).toHex()!==snap.blockHash)throw Error('Snapshot hash mismatch');
 let start=snap.storage['babe.epochStart'][1];const output={checkedAt:new Date().toISOString(),endpoint,snapshotBlock:snap.blockNumber,snapshotHash:snap.blockHash,sessions:[],submittedTransactions:0};
 for(let i=0;i<8;i++){
  const hash=await api.rpc.chain.getBlockHash(start-1);const at=await api.at(hash);const session=(await at.query.session.currentIndex()).toNumber();
  const [validators,disabled,authors,beats,epochStart,era,keys,authorities]=await Promise.all([at.query.session.validators(),at.query.session.disabledValidators(),at.query.imOnline.authoredBlocks.entries(session),at.query.imOnline.receivedHeartbeats.entries(session),at.query.babe.epochStart(),at.query.staking.activeEra(),at.query.session.queuedKeys(),at.query.babe.authorities()]);
  const bh=await api.rpc.chain.getBlockHash(start);const boundary=await api.at(bh);const events=await boundary.query.system.events();
  const a=new Map(authors.map(([k,v])=>[k.args[1].toString(),v.toNumber()]));
  const row={session,lastBlock:start-1,lastBlockHash:hash.toHex(),boundaryBlock:start,boundaryHash:bh.toHex(),startedBlock:epochStart[1].toNumber(),activeEra:era.toJSON(),disabled:disabled.toJSON(),heartbeats:beats.map(([k,v])=>({key:k.args.map(x=>x.toJSON()),value:v.toJSON()})),validators:validators.map((s,index)=>({index,stash:s.toString(),blocks:a.get(s.toString())||0})),authorities:authorities.toJSON(),queuedKeys:keys.toJSON(),events:events.filter(x=>['staking','session','imOnline','offences','electionProviderMultiPhase'].includes(x.event.section)).map(x=>({section:x.event.section,method:x.event.method,data:x.event.data.toJSON()})),afterDisabled:(await boundary.query.session.disabledValidators()).toJSON(),afterHistory:boundary.query.liveness?(await boundary.query.liveness.disabledDuringSession()).toJSON():null,specVersion:(await api.rpc.state.getRuntimeVersion(hash)).specVersion.toNumber()};
  output.sessions.push(row);writeFileSync(__dirname+'/completed-sessions.json',JSON.stringify(output,null,2)+'\n');console.log(JSON.stringify({session,lastBlock:row.lastBlock,spec:row.specVersion,era:row.activeEra,disabled:row.disabled,missing:row.validators.filter(v=>!v.blocks).map(v=>v.stash),events:row.events.map(e=>e.section+'.'+e.method)}));
  start=row.startedBlock;
 }
}finally{await api.disconnect();clearTimeout(timer);}})().catch(e=>{console.error(e);process.exit(1)});
