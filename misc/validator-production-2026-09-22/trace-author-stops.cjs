const {createRequire}=require('node:module');
const {readFileSync,writeFileSync}=require('node:fs');
const deps=createRequire('/tmp/sora-validator-investigation-20260922/package.json');
const {ApiPromise,WsProvider}=deps('@polkadot/api');
const {fullOverrideBundle}=deps('@sora-substrate/type-definitions');
const snap=JSON.parse(readFileSync(__dirname+'/snapshot.json'));
const completed=JSON.parse(readFileSync(__dirname+'/completed-sessions.json'));
const watched=[2,6,10,14].map(i=>snap.validators[i].stash);
const report={startedAt:new Date().toISOString(),endpoint:'wss://mof2.sora.org',referenceHash:snap.blockHash,submittedTransactions:0,upgrade:null,lastAuthors:[],olderSessions:[],slotInputs:[]};
const save=()=>writeFileSync(__dirname+'/author-stop-trace.json',JSON.stringify(report,null,2)+'\n');
const deadline=setTimeout(()=>{console.error('Trace exceeded 10 minutes');process.exit(2)},600000);
(async()=>{
 const api=await ApiPromise.create({provider:new WsProvider(report.endpoint),typesBundle:{spec:{'sora-substrate':fullOverrideBundle.spec.sora}}});
 try{
  if(api.genesisHash.toHex()!==snap.genesis || (await api.rpc.chain.getBlockHash(snap.blockNumber)).toHex()!==snap.blockHash)throw Error('Chain/reference mismatch');
  async function atBlock(n){const hash=await api.rpc.chain.getBlockHash(n);return {hash,at:await api.at(hash)}}
  async function lastAuthor(row,stash){
   const finalCount=row.validators.find(v=>v.stash===stash).blocks;
   const key=api.query.imOnline.authoredBlocks.key(row.session,stash);
   let low=row.startedBlock,high=row.lastBlock;
   async function count(n){const hash=await api.rpc.chain.getBlockHash(n);const value=await api.rpc.state.getStorage(key,hash);return value.isNone?0:api.registry.createType('u32',value.unwrap().toU8a(true)).toNumber()}
   while(low<high){const mid=Math.floor((low+high)/2);if(await count(mid)>=finalCount)high=mid;else low=mid+1;}
   const {hash,at}=await atBlock(low);const block=await api.rpc.chain.getBlock(hash);
   const item={stash,session:row.session,lastAuthoredBlock:low,hash:hash.toHex(),timestamp:(await at.query.timestamp.now()).toNumber(),countBefore:await count(low-1),countAt:await count(low),finalCount,runtimeVersion:(await api.rpc.state.getRuntimeVersion(hash)).specVersion.toNumber(),disabled:(await at.query.session.disabledValidators()).toJSON(),header:block.block.header.toJSON(),extrinsicMethods:block.block.extrinsics.map(x=>x.method.section+'.'+x.method.method),events:(await at.query.system.events()).filter(x=>['session','babe','staking','offences','system'].includes(x.event.section)).map(x=>({section:x.event.section,method:x.event.method,data:x.event.data.toJSON()}))};
   report.lastAuthors.push(item);save();console.log(JSON.stringify({stash,lastBlock:low,utc:new Date(item.timestamp).toISOString(),spec:item.runtimeVersion}));
  }
  // Locate the spec130 -> spec131 transition within the two verified snapshots.
  let low=27622680,high=snap.blockNumber;
  while(low<high){const mid=Math.floor((low+high)/2);const hash=await api.rpc.chain.getBlockHash(mid);const spec=(await api.rpc.state.getRuntimeVersion(hash)).specVersion.toNumber();if(spec>=131)high=mid;else low=mid+1;}
  const upgrade=await atBlock(low);const previous=await atBlock(low-1);
  report.upgrade={block:low,hash:upgrade.hash.toHex(),timestamp:(await upgrade.at.query.timestamp.now()).toNumber(),beforeSpec:(await api.rpc.state.getRuntimeVersion(previous.hash)).specVersion.toNumber(),afterSpec:(await api.rpc.state.getRuntimeVersion(upgrade.hash)).specVersion.toNumber(),session:(await upgrade.at.query.session.currentIndex()).toNumber(),events:(await upgrade.at.query.system.events()).filter(x=>['system','session','babe'].includes(x.event.section)).map(x=>({section:x.event.section,method:x.event.method,data:x.event.data.toJSON()}))};save();console.log(JSON.stringify({upgradeBlock:low,utc:new Date(report.upgrade.timestamp).toISOString(),beforeSpec:report.upgrade.beforeSpec,afterSpec:report.upgrade.afterSpec}));
  for(const row of completed.sessions.slice(0,2)){
   const at=await api.at(row.lastBlockHash);report.slotInputs.push({session:row.session,hash:row.lastBlockHash,randomness:(await at.query.babe.randomness()).toHex(),authorities:(await at.query.babe.authorities()).toJSON(),epochStart:(await at.query.babe.epochStart()).toJSON(),epochIndex:(await at.query.babe.epochIndex()).toJSON(),genesisSlot:(await at.query.babe.genesisSlot()).toJSON(),currentSlot:(await at.query.babe.currentSlot()).toJSON(),epochDuration:at.consts.babe.epochDuration.toJSON(),epochConfig:(await at.query.babe.epochConfig()).toJSON()});
  }save();
  for(const stash of watched){const row=completed.sessions.find(r=>r.validators.some(v=>v.stash===stash && v.blocks>0));if(row)await lastAuthor(row,stash);}
  let next=completed.sessions.at(-1).startedBlock-1;
  for(let i=0;i<40;i++){
   const {hash,at}=await atBlock(next);const session=(await at.query.session.currentIndex()).toNumber();
   const [validators,authors,epochStart,timestamp,disabled]=await Promise.all([at.query.session.validators(),at.query.imOnline.authoredBlocks.entries(session),at.query.babe.epochStart(),at.query.timestamp.now(),at.query.session.disabledValidators()]);
   const counts=new Map(authors.map(([k,v])=>[k.args[1].toString(),v.toNumber()]));
   const row={session,lastBlock:next,hash:hash.toHex(),startedBlock:epochStart[1].toNumber(),timestamp:timestamp.toNumber(),disabled:disabled.toJSON(),validators:validators.map((v,index)=>({index,stash:v.toString(),blocks:counts.get(v.toString())||0}))};
   report.olderSessions.push(row);save();
   for(const stash of watched){if(!report.lastAuthors.some(x=>x.stash===stash) && row.validators.some(v=>v.stash===stash && v.blocks>0))await lastAuthor(row,stash);}
   if(i%8===0)console.log(JSON.stringify({session,utc:new Date(row.timestamp).toISOString(),watched:watched.map(stash=>({stash,blocks:row.validators.find(v=>v.stash===stash)?.blocks??null}))}));
   next=row.startedBlock-1;
  }
  report.finishedAt=new Date().toISOString();save();
 }finally{await api.disconnect();clearTimeout(deadline)}
})().catch(e=>{console.error(e);process.exit(1)});
