const {createRequire}=require('node:module');const {readFileSync,writeFileSync}=require('node:fs');
const deps=createRequire('/tmp/sora-validator-investigation-20260922/package.json');const {ApiPromise,WsProvider}=deps('@polkadot/api');const {fullOverrideBundle}=deps('@sora-substrate/type-definitions');
const snap=JSON.parse(readFileSync(__dirname+'/snapshot.json'));const output={checkedAt:new Date().toISOString(),genesis:snap.genesis,submittedTransactions:0,states:[],headers:[]};
const save=()=>writeFileSync(__dirname+'/babe-config-evidence.json',JSON.stringify(output,null,2)+'\n');const timer=setTimeout(()=>process.exit(2),180000);
(async()=>{const api=await ApiPromise.create({provider:new WsProvider('wss://mof2.sora.org'),typesBundle:{spec:{'sora-substrate':fullOverrideBundle.spec.sora}}});try{
if(api.genesisHash.toHex()!==snap.genesis)throw Error('Wrong chain');
const latestHash=await api.rpc.chain.getFinalizedHead();const latest=await api.rpc.chain.getHeader(latestHash);
for(const [label,hash] of [['reference',snap.blockHash],['latest',latestHash.toHex()]]){
 const at=await api.at(hash);const row={label,hash,block:(await api.rpc.chain.getHeader(hash)).number.toNumber(),storage:{},rawStorage:{},runtimeApi:{},errors:{}};
 for(const name of ['epochConfig','nextEpochConfig','pendingEpochConfigChange','authorities','randomness','epochIndex','genesisSlot','currentSlot']){if(!at.query.babe[name])continue;row.storage[name]=(await at.query.babe[name]()).toJSON();row.rawStorage[name]=(await api.rpc.state.getStorage(at.query.babe[name].key(),hash)).toHex();}
 for(const name of ['configuration','currentEpoch','nextEpoch']){try{row.runtimeApi[name]=(await at.call.babeApi[name]()).toJSON()}catch(e){row.errors[name]=e.message}}
 try{row.configurationRaw=(await api.rpc.state.call('BabeApi_configuration','0x',hash)).toHex()}catch(e){row.errors.configurationRaw=e.message}
 row.epochConfigType=at.query.babe.epochConfig.meta.toHuman();
 output.states.push(row);save();console.log(JSON.stringify({label,block:row.block,epochConfig:row.storage.epochConfig,raw:row.rawStorage.epochConfig,configuration:row.runtimeApi.configuration,currentEpoch:row.runtimeApi.currentEpoch,errors:row.errors}));
}
for(let i=0;i<20;i++){
 const n=latest.number.toNumber()-i;const hash=await api.rpc.chain.getBlockHash(n);const header=await api.rpc.chain.getHeader(hash);
 const digest=header.digest.logs.find(x=>x.isPreRuntime&&x.asPreRuntime[0].toHex()==='0x42414245');const raw=digest?digest.asPreRuntime[1].toU8a(true):null;
 output.headers.push({block:n,hash:hash.toHex(),header:header.toJSON(),babePreDigest:raw?'0x'+Buffer.from(raw).toString('hex'):null,variant:raw?.[0],authorityIndex:raw?Buffer.from(raw).readUInt32LE(1):null,slot:raw?Buffer.from(raw).readBigUInt64LE(5).toString():null});
}
save();console.log(JSON.stringify({headers:output.headers.map(x=>({block:x.block,variant:x.variant,index:x.authorityIndex}))}));
}finally{await api.disconnect();clearTimeout(timer)}})().catch(e=>{console.error(e);process.exit(1)});
