const {createRequire}=require('node:module');
const {writeFileSync}=require('node:fs');
const {createHash}=require('node:crypto');
const deps=createRequire('/tmp/sora-validator-investigation-20260922/package.json');
const {ApiPromise,WsProvider}=deps('@polkadot/api');
const {fullOverrideBundle}=deps('@sora-substrate/type-definitions');
const out=__dirname;
const timer=setTimeout(()=>process.exit(2),60000);
(async()=>{
 const endpoint='wss://ws.mof.sora.org';
 const api=await ApiPromise.create({provider:new WsProvider(endpoint),typesBundle:{spec:{'sora-substrate':fullOverrideBundle.spec.sora}}});
 try{
 const genesis=api.genesisHash.toHex(); if(genesis!=='0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5')throw Error('Wrong genesis');
 const hash=await api.rpc.chain.getFinalizedHead();const at=await api.at(hash);const head=await api.rpc.chain.getHeader(hash);
 const code=await api.rpc.state.getStorage('0x3a636f6465',hash);
 const report={checkedAt:new Date().toISOString(),endpoint,genesis,blockHash:hash.toHex(),blockNumber:head.number.toNumber(),runtimeVersion:(await api.rpc.state.getRuntimeVersion(hash)).toJSON(),codeSha256:createHash('sha256').update(code.unwrap().toU8a(true)).digest('hex'),rpcNodeVersion:(await api.rpc.system.version()).toString(),storage:{},errors:{},submittedTransactions:0};
 const fields=[['timestamp','now'],['system','lastRuntimeUpgrade'],['session','currentIndex'],['session','validators'],['session','disabledValidators'],['session','queuedChanged'],['session','queuedKeys'],['staking','activeEra'],['staking','currentEra'],['staking','forceEra'],['staking','bondedEras'],['staking','validatorCount'],['staking','minimumValidatorCount'],['babe','authorities'],['babe','nextAuthorities'],['babe','epochIndex'],['babe','currentSlot'],['babe','epochStart'],['babe','epochConfig'],['babe','nextEpochConfig'],['imOnline','keys'],['liveness','disabledDuringSession'],['liveness','exposureWork'],['electionProviderMultiPhase','currentPhase'],['electionProviderMultiPhase','queuedSolution'],['electionProviderMultiPhase','desiredTargets']];
 await Promise.all(fields.map(async([p,n])=>{try{const q=at.query[p]?.[n];if(!q){report.errors[p+'.'+n]='not in metadata';return;}report.storage[p+'.'+n]=(await q()).toJSON();}catch(e){report.errors[p+'.'+n]=String(e);}}));
 const session=report.storage['session.currentIndex'];
 for(const name of ['authoredBlocks','receivedHeartbeats']){report.storage['imOnline.'+name]=(await at.query.imOnline[name].entries(session)).map(([k,v])=>({key:k.args.map(x=>x.toJSON()),value:v.toJSON()}));}
 const validators=report.storage['session.validators'];
 report.validators=await Promise.all(validators.map(async(stash,index)=>{
  const controller=await at.query.staking.bonded(stash);
  return {index,stash,nextKeys:(await at.query.session.nextKeys(stash)).toJSON(),controller:controller.toJSON(),ledger:controller.isSome?(await at.query.staking.ledger(controller.unwrap())).toJSON():null,identity:at.query.identity?.identityOf?(await at.query.identity.identityOf(stash)).toHuman():null};
 }));
 writeFileSync(out+'/snapshot.json',JSON.stringify(report,null,2)+'\n');
 console.log(JSON.stringify({block:report.blockNumber,version:report.runtimeVersion.specVersion,codeSha256:report.codeSha256,nodeVersion:report.rpcNodeVersion,session,disabled:report.storage['session.disabledValidators'],validators:validators.length,authoredEntries:report.storage['imOnline.authoredBlocks'].length,forceEra:report.storage['staking.forceEra'],activeEra:report.storage['staking.activeEra'],hasLiveness:!!at.query.liveness,errors:report.errors},null,2));
 }finally{await api.disconnect();clearTimeout(timer);}
})().catch(e=>{console.error(e);process.exit(1)});
