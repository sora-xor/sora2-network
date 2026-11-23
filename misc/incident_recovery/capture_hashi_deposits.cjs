#!/usr/bin/env node
'use strict';
// SORA Nexus: resumable provider-observed source inventory, never a credit proof.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const ENDPOINT = 'https://ethereum-rpc.publicnode.com';
const PROVIDERS = Object.freeze({
  publicnode:Object.freeze({endpoint:ENDPOINT,source:'https://www.publicnode.com/'}),
  drpc:Object.freeze({endpoint:'https://eth.drpc.org/',source:'https://drpc.org/docs/ethereum-api'}),
  tenderly:Object.freeze({endpoint:'https://mainnet.gateway.tenderly.co',source:'https://docs.tenderly.co/node-rpc/overview#public-endpoint-limits'}),
});
const GENESIS = '0xd4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3';
const CHECKPOINT = '0x7f7c253fa72f32e7b03f32bdc9ae456fd7174fb131b689be6d876006124129f0';
const TOPIC = '0x85c0fa492ded927d3acca961da52b0dda1debb06d8c27fe189315f06bb6e26c8';
const CONTRACTS = Object.freeze([
  {storage:'EthBridge.BridgeContractAddress',kind:'bridge',address:'0x313416870a4da6f12505a550b67bb73c8e21d5d3'},
  {storage:'EthBridge.XorMasterContractAddress',kind:'xor-master',address:'0xc08edf13be9b9cc584c5da8004ce7e6be63c1316'},
  {storage:'EthBridge.ValMasterContractAddress',kind:'val-master',address:'0xd1eeb2f30016fffd746233ee12c486e7ca8efef1'},
]);
const LIMIT = Object.freeze({file:64*1024*1024,logs:10000,data:1024*1024,span:50000,pages:100000,requests:500000,timeout:900000});
const QUOTA_HEADERS = Object.freeze(['x-tdly-egress-limit','x-tdly-egress-remaining','x-tdly-egress-reset-timestamp','x-tdly-limit','x-tdly-remaining','x-tdly-reset-timestamp','retry-after']);
const KERNEL = path.join(__dirname,'evm');
const KERNEL_PINS = Object.freeze({'kernel.cjs':'cdc206e8755aa247695c0982735b5444fccd9bcbbca9adcf8e45515006c18bb2',
  'dependencies.json':'7e9f1f9553c5f4371540cfde74a1e31255afc827de24bcc85a10e308b8cd7f2f',
  'source-provenance.json':'1ba16c2358ba71d7762acd687a44bc0fe1156707dddebaad00676bf8548f209f'});
const need=(value,message)=>{if(!value)throw Error(message);};
const sha=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const json=value=>Buffer.from(JSON.stringify(value)+'\n');
const hex=(value,width)=>{need(typeof value==='string'&&/^0x(?:[0-9a-f]{2})*$/.test(value),'canonical bytes required');
  const bytes=Buffer.from(value.slice(2),'hex');need(width===undefined||bytes.length===width,'byte width differs');return bytes;};
const quantity=value=>{need(typeof value==='string'&&/^0x(?:0|[1-9a-f][0-9a-f]*)$/.test(value),'canonical RPC quantity required');
  const n=BigInt(value);need(n<=BigInt(Number.MAX_SAFE_INTEGER),'RPC quantity exceeds safe inventory bound');return Number(n);};
const tag=number=>'0x'+number.toString(16);
function read(file,bound=LIMIT.file){
  need(path.isAbsolute(file)&&path.resolve(file)===file&&fs.realpathSync(file)===file,'canonical unaliased input required');
  const fd=fs.openSync(file,fs.constants.O_RDONLY|fs.constants.O_NOFOLLOW);
  try{const stat=fs.fstatSync(fd);need(stat.isFile()&&stat.size<=bound,'file bound');const bytes=fs.readFileSync(fd);need(bytes.length===stat.size,'input changed');return bytes;}finally{fs.closeSync(fd);}
}
function write(file,bytes){
  const temporary=file+'.writing-'+process.pid+'-'+crypto.randomUUID();
  const fd=fs.openSync(temporary,fs.constants.O_WRONLY|fs.constants.O_CREAT|fs.constants.O_EXCL|fs.constants.O_NOFOLLOW,0o600);
  try{fs.writeFileSync(fd,bytes);fs.fsyncSync(fd);}finally{fs.closeSync(fd);}
  // Linking is an atomic exclusive commit; unlike rename it cannot overwrite.
  // An interrupted temporary file is retained and never counts as a committed page.
  try{fs.linkSync(temporary,file);}finally{fs.unlinkSync(temporary);}
}
function pin(file){const bytes=read(file);return {file,sha256:sha(bytes),bytes:bytes.length};}
function kernel(){for(const [file,digest]of Object.entries(KERNEL_PINS))need(sha(read(path.join(KERNEL,file)))===digest,'fixed EVM kernel changed');
  return require(path.join(KERNEL,'kernel.cjs'));}
function rpcHeader(block,k){
  need(block&&typeof block==='object','RPC header missing');
  const fixed=(name,width)=>hex(block[name],width);
  const integer=name=>{need(typeof block[name]==='string'&&/^0x(?:0|[1-9a-f][0-9a-f]*)$/.test(block[name]),'header quantity missing');return k.integer(BigInt(block[name]));};
  const fields=[fixed('parentHash',32),fixed('sha3Uncles',32),fixed('miner',20),fixed('stateRoot',32),
    fixed('transactionsRoot',32),fixed('receiptsRoot',32),fixed('logsBloom',256),integer('difficulty'),integer('number'),
    integer('gasLimit'),integer('gasUsed'),integer('timestamp'),fixed('extraData'),fixed('mixHash',32),fixed('nonce',8)];
  const optional=['baseFeePerGas','withdrawalsRoot','blobGasUsed','excessBlobGas','parentBeaconBlockRoot','requestsHash'];
  let missing=false;
  for(const [i,name]of optional.entries()){
    if(block[name]===undefined||block[name]===null){missing=true;continue;}
    need(!missing,'header extension gap');fields.push([0,2,3].includes(i)?integer(name):fixed(name,32));
  }
  const raw=k.encode(fields),decoded=k.header(raw);
  need(k.hx(decoded.hash)===block.hash,'RPC header hash differs from canonical RLP');
  return {number:quantity(block.number),hash:block.hash,parentHash:block.parentHash,timestamp:quantity(block.timestamp),header:k.hx(raw)};
}
function envelope(bytes,id){
  const text=bytes.toString('utf8'),value=JSON.parse(text);
  need(Buffer.from(text).equals(bytes)&&[JSON.stringify(value),JSON.stringify(value)+'\n'].includes(text),'RPC response must be unambiguous compact JSON');
  need(value&&value.jsonrpc==='2.0'&&value.id===id&&Object.hasOwn(value,'result')!==Object.hasOwn(value,'error'),'RPC response envelope differs');
  if(Object.hasOwn(value,'error')){const error=Error('RPC error: '+JSON.stringify(value.error));error.rpcError=value.error;throw error;}
  need(value.result!==null,'RPC returned null');return value.result;
}
function originalRpc(file,method,params,endpoint=ENDPOINT,source){const bytes=read(file),row=JSON.parse(bytes);
  need(row.endpoint===endpoint&&(!source||row.source===source)&&row.httpStatus===200&&row.request.method===method&&JSON.stringify(row.request.params)===JSON.stringify(params),'retained identity RPC differs');
  return {value:envelope(Buffer.from(row.responseText),row.request.id),responseHeaders:row.responseHeaders,pin:{file,sha256:sha(bytes),bytes:bytes.length}};
}
async function checkFrozenHead(rpc,frozen,k){
  const reply=await rpc('eth_getBlockByNumber',[tag(frozen.number),false]),observed=rpcHeader(reply.value,k);
  if(observed.number!==frozen.number||observed.hash!==frozen.hash){
    const error=Error('frozen head canonical hash drift by block height');error.headDrift={expected:frozen,observed,request:reply.request,response:reply.response};throw error;
  }
  return {number:observed.number,hash:observed.hash,request:reply.request,response:reply.response};
}
function providerAdmission(provider,directory,frozen,k){
  need(Object.hasOwn(PROVIDERS,provider),'provider is not in the fixed allowlist');
  const selected=PROVIDERS[provider];
  if(provider==='publicnode'){
    need(directory===undefined,'PublicNode does not accept an alternate-provider probe');
    return {provider,endpoint:selected.endpoint,source:selected.source,probe:null,inputs:[]};
  }
  need(typeof directory==='string',provider+' requires its separately retained capability probe');
  const probeThrough=provider==='tenderly'?49999:99;
  const calls=[['eth_chainId',[]],['eth_getBlockByNumber',['0x0',false]],
    ['eth_getBlockByNumber',[tag(frozen.number),false]],
    ['eth_getLogs',[{address:CONTRACTS.map(c=>c.address),fromBlock:'0x0',toBlock:tag(probeThrough)}]]];
  const rows=calls.map(([method,params],i)=>originalRpc(path.join(directory,`rpc-${i}.json`),method,params,selected.endpoint,selected.source));
  need(rows[0].value==='0x1','alternate provider chain differs');
  const genesis=rpcHeader(rows[1].value,k),head=rpcHeader(rows[2].value,k);
  need(genesis.number===0&&genesis.hash===GENESIS,'alternate provider genesis differs');
  need(head.number===frozen.number&&head.hash===frozen.hash&&head.header===frozen.header,'alternate provider frozen header differs');
  const logBound=provider==='tenderly'?3000:LIMIT.logs;
  const logs=validateLogs(rows[3].value,0,probeThrough,CONTRACTS,logBound);
  if(provider==='tenderly')for(const row of rows)quotaFromHeaders(row.responseHeaders);
  return {provider,endpoint:selected.endpoint,source:selected.source,inputs:rows.map(row=>row.pin),
    probe:{fromBlock:0,throughBlock:probeThrough,logCount:logs.length,fullArchiveCapabilityEstablished:false}};
}
function derivePlan({registryFile,verificationFile,identityDirectory,provider='publicnode',providerProbeDirectory}){
  const k=kernel(),registryBytes=read(registryFile),registry=JSON.parse(registryBytes),verificationBytes=read(verificationFile),verification=JSON.parse(verificationBytes);
  need(registry.schema==='sora-nexus-checkpoint-bridge-source-registry-v1'&&registry.checkpointHash===CHECKPOINT,'checkpoint registry differs');
  need(verification.stateRootVerified===true&&verification.headerHashVerified===true&&verification.checkpointHash===CHECKPOINT
    &&verification.stateRoot===registry.stateRoot&&verification.captureManifestSha256===registry.captureManifestSha256
    &&sha(verificationBytes)===registry.stateVerificationSha256,'native capture verification does not bind source registry');
  const contracts=CONTRACTS.map(contract=>{
    const rows=registry.configuration.filter(row=>row.storage===contract.storage);
    need(rows.length===1&&rows[0].valueScale===contract.address&&rows[0].valueDecoded===contract.address,'exact checkpoint contract differs');
    need(contract.kind!=='bridge'||rows[0].network==='0x0200000000','HASHI internal network differs');
    return {...contract,checkpointRecord:rows[0]};
  });
  const chain=originalRpc(path.join(identityDirectory,'eth-1.json'),'eth_chainId',[]);need(chain.value==='0x1','external chain must be Ethereum mainnet');
  const head=originalRpc(path.join(identityDirectory,'eth-2.json'),'eth_getBlockByNumber',['finalized',false]);
  const frozen=rpcHeader(head.value,k),codePins=[];
  for(const [i,contract]of contracts.entries()){
    const code=originalRpc(path.join(identityDirectory,`eth-${i+3}.json`),'eth_getCode',[contract.address,{blockHash:frozen.hash,requireCanonical:true}]);
    need(hex(code.value).length>0,'configured contract has no observed code');contract.frozenCodeSha256=sha(hex(code.value));codePins.push(code.pin);
  }
  const providerBinding=providerAdmission(provider,providerProbeDirectory,frozen,k);
  const dependency=JSON.parse(read(path.join(KERNEL,'dependencies.json')));
  const dependencyResolver=require('node:module').createRequire(path.resolve(KERNEL,dependency.resolutionPackage));
  const dependencyRoot=path.dirname(fs.realpathSync(dependencyResolver.resolve('@noble/hashes/sha3')));
  const dependencyInputs=dependency.files.map(item=>{const itemPin=pin(path.join(dependencyRoot,item.file));need(itemPin.sha256===item.sha256,'loaded Keccak dependency changed');return itemPin;});
  const inputs=[pin(registryFile),pin(verificationFile),chain.pin,head.pin,...codePins,...providerBinding.inputs,pin(__filename),pin(process.execPath),
    pin(path.resolve(KERNEL,dependency.resolutionPackage)),pin(path.resolve(KERNEL,dependency.lockfile)),
    ...dependencyInputs,
    ...Object.keys(KERNEL_PINS).map(name=>pin(path.join(KERNEL,name)))];
  return {schema:'sora-nexus-hashi-provider-inventory-plan-v2',provider,endpoint:providerBinding.endpoint,providerSource:providerBinding.source,capabilityProbe:providerBinding.probe,nativeNetwork:0,chainId:'1',genesisHash:GENESIS,
    checkpointHash:CHECKPOINT,frozenHead:frozen,contracts,fromBlock:0,maximumBlockSpan:LIMIT.span,
    responseLogCountExclusive:provider==='tenderly'?3000:LIMIT.logs,
    quotaPolicy:provider==='tenderly'?{requiredHeaders:QUOTA_HEADERS.slice(0,6),minimumEgressReserveBytes:2*LIMIT.file,reservedFinalHeadRequests:1}:null,
    admissionInputs:{registryFile,verificationFile,identityDirectory,provider,...(providerProbeDirectory?{providerProbeDirectory}:{})},
    queryPolicy:'all logs at the three exact checkpoint addresses from genesis; Deposit locators are distinguished without decoding a receipt or inventing a credit',
    discoveryPolicy:'no deployment-height binary-search assumption; current addresses scanned over their entire lifetime including possible redeployments; historical alternate bridge addresses require separate native configuration history',
    finality:'provider-observed finalized tag; no consensus proof or new quorum rule',inputs,
    historicalContractsExhaustivelyDiscovered:false,peerAuthenticationVerified:false,productionReady:false};
}
function prepare({output,...inputs}){
  const plan=derivePlan(inputs);
  need(fs.realpathSync(path.dirname(output))===path.dirname(output),'canonical output parent required');write(output,json(plan));return plan;
}
function validatePlan(plan){
  need(plan.schema==='sora-nexus-hashi-provider-inventory-plan-v2'&&Object.hasOwn(PROVIDERS,plan.provider)&&plan.endpoint===PROVIDERS[plan.provider].endpoint&&plan.chainId==='1'&&plan.genesisHash===GENESIS
    &&plan.checkpointHash===CHECKPOINT&&plan.nativeNetwork===0&&plan.fromBlock===0&&plan.maximumBlockSpan===LIMIT.span,'fixed source plan differs');
  need(plan.contracts.length===3&&plan.contracts.every((c,i)=>c.address===CONTRACTS[i].address&&c.kind===CONTRACTS[i].kind),'fixed contract set differs');
  need(Number.isSafeInteger(plan.frozenHead.number)&&plan.frozenHead.number>0,'frozen head number');hex(plan.frozenHead.hash,32);
  need(plan.productionReady===false&&plan.historicalContractsExhaustivelyDiscovered===false&&plan.peerAuthenticationVerified===false,'capture cannot promote verification flags');
  for(const p of plan.inputs){const bytes=read(p.file);need(bytes.length===p.bytes&&sha(bytes)===p.sha256,'plan input changed: '+p.file);}
  need(JSON.stringify(derivePlan(plan.admissionInputs))===JSON.stringify(plan),'plan differs from independently rederived source admissions');
}
function validateLogs(logs,from,through,contracts=CONTRACTS,logBound=LIMIT.logs){
  need(Number.isSafeInteger(logBound)&&logBound>0&&logBound<=LIMIT.logs,'provider log bound');
  need(Array.isArray(logs)&&logs.length<logBound,'log count at provider truncation/safety bound');const seen=new Set(),blocks=new Map(),transactions=new Map();
  const locators=logs.map(log=>{
    need(log&&contracts.some(c=>c.address===log.address)&&log.removed===false,'log source or removed flag differs');
    const number=quantity(log.blockNumber),transactionIndex=quantity(log.transactionIndex),logIndex=quantity(log.logIndex);
    need(number>=from&&number<=through,'log outside requested range');hex(log.blockHash,32);hex(log.transactionHash,32);hex(log.address,20);
    need(Array.isArray(log.topics)&&log.topics.length<=4,'log topic bound');log.topics.forEach(t=>hex(t,32));
    const data=hex(log.data);need(data.length<=LIMIT.data,'log data bound');
    const id=log.blockHash+':'+logIndex;need(!seen.has(id),'duplicate source log locator');seen.add(id);
    need(!blocks.has(number)||blocks.get(number)===log.blockHash,'conflicting block hashes within one range');blocks.set(number,log.blockHash);
    const transactionPosition=log.blockHash+':'+transactionIndex;
    need(!transactions.has(log.transactionHash)||transactions.get(log.transactionHash)===transactionPosition,'conflicting transaction locator');transactions.set(log.transactionHash,transactionPosition);
    const deposit=log.topics[0]===TOPIC;
    if(deposit)need(log.topics.length===1&&data.length===128&&data.subarray(64,76).every(b=>b===0),'malformed native Deposit log');
    return {blockNumber:number,blockHash:log.blockHash,transactionIndex,transactionHash:log.transactionHash,logIndex,
      sourceContract:log.address,deposit,nativeMessageId:deposit?log.transactionHash:null};
  });
  return locators.sort((a,b)=>a.blockNumber-b.blockNumber||a.transactionIndex-b.transactionIndex||a.logIndex-b.logIndex);
}
function retainedHeaders(headers){
  try{
  if(headers===undefined)return {};
  need(headers&&typeof headers==='object'&&!Array.isArray(headers),'response headers object required');
  const entries=headers instanceof Headers?[...headers.entries()]:Object.entries(headers),result={};
  for(const [name,value]of entries){if(!QUOTA_HEADERS.includes(name))continue;
    need(typeof value==='string'&&value.length<=128&&!/[\r\n]/.test(value),'quota header bound');result[name]=value;}
  return result;
  }catch(error){error.quotaObservationInvalid=true;throw error;}
}
function quotaFromHeaders(headers){
  try{
  const selected=retainedHeaders(headers),numbers={};
  for(const name of QUOTA_HEADERS.slice(0,6)){
    const value=selected[name];need(typeof value==='string'&&/^(?:0|[1-9][0-9]*)$/.test(value),'missing or malformed Tenderly quota header: '+name);
    numbers[name]=Number(value);need(Number.isSafeInteger(numbers[name]),'quota header integer bound');
  }
  need(numbers['x-tdly-egress-limit']>0&&numbers['x-tdly-limit']>0
    &&numbers['x-tdly-egress-remaining']<=numbers['x-tdly-egress-limit']&&numbers['x-tdly-remaining']<=numbers['x-tdly-limit'],'quota header limits differ');
  return {remainingBytes:numbers['x-tdly-egress-remaining'],remainingRequests:numbers['x-tdly-remaining'],
    egressResetTimestamp:numbers['x-tdly-egress-reset-timestamp'],rateResetTimestamp:numbers['x-tdly-reset-timestamp']};
  }catch(error){error.quotaObservationInvalid=true;throw error;}
}
function quotaAdmission(quota,finalizing){
  if(!quota)return;
  const reserve=finalizing?1:2;
  if(quota.remainingBytes<reserve*LIMIT.file||quota.remainingRequests<reserve){
    const error=Error('reported provider quota/rate reserve reached; stop without retries');error.quota=true;throw error;
  }
}
async function transport(request,timeout,endpoint){
  need(Object.values(PROVIDERS).some(provider=>provider.endpoint===endpoint),'transport endpoint is not in the fixed allowlist');
  const response=await fetch(endpoint,{method:'POST',redirect:'error',headers:{'content-type':'application/json'},body:JSON.stringify(request),signal:AbortSignal.timeout(timeout)});
  const headers=retainedHeaders(response.headers),chunks=[];let size=0;
  try{for await(const chunk of response.body){size+=chunk.length;need(size<=LIMIT.file,'RPC response body bound');chunks.push(chunk);}}
  catch(e){e.partial=Buffer.concat(chunks);e.httpStatus=response.status;e.responseHeaders=headers;throw e;}
  return {httpStatus:response.status,bytes:Buffer.concat(chunks),responseHeaders:headers};
}
function accessRestriction(error){
  return [401,403,429].includes(error?.httpStatus)
    ||/\b(?:rates?|quotas?|tokens?|authorization|authentication|credentials?)\b|unauthori[sz]ed|forbidden|access denied|api.?key|too many requests|compute.?units/i.test(String(error?.rpcError?.message??''));
}
function splitError(error){
  // Size/range adaptation stays within the same provider's published limits.
  // Access, account and rate restrictions always stop the attempt.
  if(accessRestriction(error))return false;
  const rpc=error.rpcError,message=String(rpc?.message??'');
  if(error.httpStatus===413)return true;
  if(error.httpStatus===400&&rpc?.code===35&&/^ranges over [1-9][0-9]* blocks are not supported on free plan$/.test(message))return true;
  return !!(rpc&&[-32000,-32005,-32602].includes(rpc.code)
    &&/block\s+range|range.*(?:limit|exceed|too (?:wide|large))|(?:query|response|result).*(?:too large|size|limit)|too many (?:logs|results)|more than [\d, ]+ (?:blocks|logs|results)/i.test(message));
}
function ref(root,file){const bytes=read(file);return {file:path.relative(root,file),sha256:sha(bytes),bytes:bytes.length};}
function refBytes(root,p){need(typeof p.file==='string'&&!path.isAbsolute(p.file)&&!p.file.split('/').includes('..'),'relative capture reference required');
  const bytes=read(path.join(root,p.file));need(bytes.length===p.bytes&&sha(bytes)===p.sha256,'retained RPC/page bytes changed');return bytes;}
function pageState(root,plan,planHash){
  const all=fs.readdirSync(path.join(root,'pages')).sort();
  const names=all.filter(name=>!/^\d{8}\.json\.writing-\d+-[0-9a-f-]{36}$/.test(name));need(names.length<=LIMIT.pages,'page count bound');
  let next=0,prior=null,deposits=0,logs=0;const pages=[];
  for(const [i,name]of names.entries()){
    need(name===String(i).padStart(8,'0')+'.json','page index gap');const file=path.join(root,'pages',name),bytes=read(file),page=JSON.parse(bytes);
    need(page.planSha256===planHash&&page.previousPageSha256===prior&&page.fromBlock===next&&page.throughBlock>=next
      &&page.throughBlock<=plan.frozenHead.number&&page.throughBlock-next<LIMIT.span,'range continuity/identity differs');
    const request=JSON.parse(refBytes(root,page.request)),raw=refBytes(root,page.response);
    need(request.method==='eth_getLogs'&&JSON.stringify(request.params)===JSON.stringify([{address:CONTRACTS.map(c=>c.address),fromBlock:tag(next),toBlock:tag(page.throughBlock)}]),'saved log query differs');
    const locators=validateLogs(envelope(raw,request.id),next,page.throughBlock,CONTRACTS,plan.responseLogCountExclusive??LIMIT.logs);
    need(page.logCount===locators.length&&page.locatorsSha256===sha(json(locators))&&page.depositCount===locators.filter(l=>l.deposit).length,'saved log summary differs');
    next=page.throughBlock+1;prior=sha(bytes);logs+=page.logCount;deposits+=page.depositCount;pages.push(ref(root,file));
  }
  return {next,prior,deposits,logs,pages};
}
function adaptiveSpan(root,next){
  // Failed attempts can only reduce the next request span. They never certify
  // absence or advance coverage; rederive the exact source range from raw input.
  let span=LIMIT.span;
  for(const name of fs.readdirSync(path.join(root,'rpc')).filter(n=>/\.(meta|rejected)\.json$/.test(n)).sort().reverse()){
    const meta=JSON.parse(read(path.join(root,'rpc',name))),request=JSON.parse(refBytes(root,meta.request));
    if(request.method!=='eth_getLogs'||quantity(request.params[0].fromBlock)!==next||!meta.error)continue;
    const width=quantity(request.params[0].toBlock)-next+1;
    if(splitError(meta)||/log count at provider truncation|RPC response body bound/.test(meta.error))span=Math.min(span,Math.max(1,Math.floor(width/2)));
  }
  return span;
}
async function capture({planFile,output,maxRequests=32,timeout=LIMIT.timeout,send=transport}){
  need(Number.isSafeInteger(maxRequests)&&maxRequests>=1&&maxRequests<=10000,'request budget bound');
  need(Number.isSafeInteger(timeout)&&timeout>=60000&&timeout<=3600000,'request timeout bound');
  const planBytes=read(planFile),plan=JSON.parse(planBytes),planHash=sha(planBytes);validatePlan(plan);const k=kernel();
  if(!fs.existsSync(output)){need(fs.realpathSync(path.dirname(output))===path.dirname(output),'canonical capture parent required');fs.mkdirSync(output);for(const d of ['rpc','pages','runs'])fs.mkdirSync(path.join(output,d));write(path.join(output,'plan.json'),planBytes);}
  need(fs.realpathSync(output)===output&&read(path.join(output,'plan.json')).equals(planBytes),'capture directory/plan differs');
  need(!fs.existsSync(path.join(output,'HEAD-DRIFT.json')),'retained frozen-head drift requires source review and a separate capture');
  const lock=path.join(output,'LOCK');
  if(fs.existsSync(lock)){
    const old=JSON.parse(read(lock,4096));need(Number.isSafeInteger(old.pid)&&old.pid>0,'retained controller PID differs');let alive=true;try{process.kill(old.pid,0);}catch(e){if(e.code==='ESRCH')alive=false;else throw e;}
    need(!alive,'another capture controller may be active');fs.renameSync(lock,path.join(output,'runs',`abandoned-lock-${old.pid}-${Date.now()}.json`));
  }
  write(lock,json({pid:process.pid,startedAt:new Date().toISOString(),planSha256:planHash}));
  const started=new Date().toISOString(),runId=crypto.randomUUID(),rpcRecords=[];let used=0,nextRpc=0,error=null,state,identityComplete=false,finalHeadCheck=null,quota=null,finalizing=false;
  async function rpc(method,params){
    if(plan.provider==='tenderly')quotaAdmission(quota,finalizing);
    if(used>=maxRequests){const e=Error('request budget reached; resume this capture');e.budget=true;throw e;}
    need(nextRpc<LIMIT.requests,'RPC evidence count bound');used++;const id=nextRpc++,base=path.join(output,'rpc',String(id).padStart(8,'0'));
    const request={jsonrpc:'2.0',id,method,params};write(base+'.request.json',json(request));
    let result,status=null,bytes=Buffer.alloc(0),headers={},failure;
    try{const response=await send(request,timeout,plan.endpoint);status=response.httpStatus;headers=retainedHeaders(response.responseHeaders);bytes=response.bytes;need(Buffer.isBuffer(bytes)&&bytes.length<=LIMIT.file,'RPC response bound');
      write(base+'.response.json',bytes);
      if(status!==200){
        const failure=Error('RPC HTTP status '+status);
        // Preserve only a valid, matching error envelope for classification.
        // A non-200 result is never a successful source response.
        try{envelope(bytes,id);}catch(e){if(e.rpcError)failure.rpcError=e.rpcError;}
        throw failure;
      }
      result=envelope(bytes,id);
      if(plan.provider==='tenderly'){quota=quotaFromHeaders(headers);quota.remainingBytes=Math.max(0,quota.remainingBytes-bytes.length);}
    }catch(e){failure=e;failure.httpStatus=status??e.httpStatus;if(e.responseHeaders)headers=retainedHeaders(e.responseHeaders);if(!fs.existsSync(base+'.response.json'))write(base+'.partial.bin',e.partial??bytes);}
    write(base+'.meta.json',json({endpoint:plan.endpoint,request:ref(output,base+'.request.json'),
      response:ref(output,base+(fs.existsSync(base+'.response.json')?'.response.json':'.partial.bin')),
      httpStatus:status??failure?.httpStatus??null,responseHeaders:headers,error:failure?String(failure.message):null,rpcError:failure?.rpcError??null,successfulResponse:!failure}));
    rpcRecords.push(ref(output,base+'.meta.json'));
    if(failure)throw failure;return {value:result,request:ref(output,base+'.request.json'),response:ref(output,base+'.response.json')};
  }
  try{
    const rpcNames=fs.readdirSync(path.join(output,'rpc'));need(rpcNames.length<=LIMIT.requests*5,'RPC evidence file bound');
    for(const name of rpcNames){const m=/^(\d{8})\./.exec(name);need(m,'unexpected RPC evidence file');nextRpc=Math.max(nextRpc,Number(m[1])+1);}
    state=pageState(output,plan,planHash);
    need((await rpc('eth_chainId',[])).value==='0x1','live source chain differs');
    const genesis=rpcHeader((await rpc('eth_getBlockByNumber',['0x0',false])).value,k);need(genesis.hash===GENESIS&&genesis.number===0,'live source genesis differs');
    const frozen=await checkFrozenHead(rpc,plan.frozenHead,k);
    for(const c of plan.contracts){const code=(await rpc('eth_getCode',[c.address,{blockHash:frozen.hash,requireCanonical:true}])).value;
      need(hex(code).length>0&&sha(hex(code))===c.frozenCodeSha256,'frozen configured contract code differs');}
    identityComplete=true;
    let span=adaptiveSpan(output,state.next);
    while(state.next<=plan.frozenHead.number){
      if(used>=maxRequests-1){const e=Error('request budget reached; final head check reserved');e.budget=true;throw e;}
      const from=state.next,through=Math.min(from+span-1,plan.frozenHead.number);let result,locators;
      try{result=await rpc('eth_getLogs',[{address:CONTRACTS.map(c=>c.address),fromBlock:tag(from),toBlock:tag(through)}]);locators=validateLogs(result.value,from,through,CONTRACTS,plan.responseLogCountExclusive);}
      catch(e){
        if(result)write(path.join(output,'rpc',path.basename(result.request.file).replace('.request.json','.rejected.json')),
          json({request:result.request,response:result.response,error:e.message}));
        if((splitError(e)||/log count at provider truncation|RPC response body bound/.test(e.message))&&span>1){span=Math.max(1,Math.floor(span/2));continue;}throw e;
      }
      const page={schema:'sora-nexus-hashi-provider-log-page-v1',planSha256:planHash,previousPageSha256:state.prior,fromBlock:from,throughBlock:through,
        request:result.request,response:result.response,logCount:locators.length,depositCount:locators.filter(l=>l.deposit).length,locatorsSha256:sha(json(locators))};
      need(state.pages.length<LIMIT.pages,'page count bound');const file=path.join(output,'pages',String(state.pages.length).padStart(8,'0')+'.json');write(file,json(page));
      state={next:through+1,prior:sha(json(page)),deposits:state.deposits+page.depositCount,logs:state.logs+page.logCount,pages:[...state.pages,ref(output,file)]};
    }
  }catch(e){error=e;}
  finally{
    finalizing=true;
    if(identityComplete&&!accessRestriction(error)&&!error?.quotaObservationInvalid){try{finalHeadCheck=await checkFrozenHead(rpc,plan.frozenHead,k);}catch(e){error=e;}}
    if(error?.headDrift&&!fs.existsSync(path.join(output,'HEAD-DRIFT.json')))write(path.join(output,'HEAD-DRIFT.json'),json(error.headDrift));
    let inputGuardError=null;
    try{need(read(planFile).equals(planBytes),'plan changed during capture');validatePlan(plan);}
    catch(e){inputGuardError=e.message;error=e;}
    const report={schema:'sora-nexus-hashi-provider-inventory-progress-v1',provider:plan.provider,endpoint:plan.endpoint,planSha256:planHash,startedAt:started,finishedAt:new Date().toISOString(),
      requestsThisRun:used,rpcRecords,nextBlock:state?.next??null,coveredThroughBlock:state?.next?state.next-1:null,pages:state?.pages??[],
      rpcLogCount:state?.logs??0,depositLogCount:state?.deposits??0,completeConfiguredAddressRange:!error&&!!finalHeadCheck&&state.next===plan.frozenHead.number+1,
      error:error?.message??null,budgetReached:error?.budget===true,quotaReached:error?.quota===true,remainingProviderQuota:quota,sourceFinality:'provider-observed finalized tag',
      inputGuardPassed:inputGuardError===null,inputGuardError,
      canonicalFrozenHeadCheckedAtBatchEnd:!!finalHeadCheck,finalHeadCheck,
      finalityCryptographicallyVerified:false,receiptsCryptographicallyVerified:false,historicalContractsExhaustivelyDiscovered:false,
      nativeAssociationsVerified:false,peerAuthenticationVerified:false,creditsAuthorized:0,productionReady:false};
    write(path.join(output,'runs',runId+'.json'),json(report));fs.unlinkSync(lock);
    if(error&&!error.budget)throw error;return report;
  }
}
async function main(argv){
  need(!process.env.NODE_OPTIONS,'NODE_OPTIONS is not admitted');const mode=argv.shift(),options={};
  for(let i=0;i<argv.length;i+=2){need(argv[i]?.startsWith('--')&&argv[i+1]&&!Object.hasOwn(options,argv[i]),'unique --name value required');options[argv[i]]=argv[i+1];}
  if(mode==='prepare'){
    const required=['--registry','--verification','--identity-directory','--output'];
    need(required.every(k=>options[k])&&Object.keys(options).every(k=>[...required,'--provider','--provider-probe-directory'].includes(k)),
      'prepare requires registry, verification, identity-directory, output; optional explicit provider and provider-probe-directory');
    prepare({registryFile:path.resolve(options['--registry']),verificationFile:path.resolve(options['--verification']),identityDirectory:path.resolve(options['--identity-directory']),
      provider:options['--provider']??'publicnode',...(options['--provider-probe-directory']?{providerProbeDirectory:path.resolve(options['--provider-probe-directory'])}:{}),output:path.resolve(options['--output'])});
    process.stdout.write(JSON.stringify({prepared:true,productionReady:false})+'\n');
  }else{
    need(mode==='capture'&&['--plan','--output','--max-requests','--request-timeout-ms'].every(k=>Object.hasOwn(options,k))&&Object.keys(options).length===4,'capture requires plan, output, max-requests, request-timeout-ms');
    const report=await capture({planFile:path.resolve(options['--plan']),output:path.resolve(options['--output']),maxRequests:Number(options['--max-requests']),timeout:Number(options['--request-timeout-ms'])});
    process.stdout.write(JSON.stringify({nextBlock:report.nextBlock,requests:report.requestsThisRun,completeConfiguredAddressRange:report.completeConfiguredAddressRange,productionReady:false})+'\n');
  }
}
if(require.main===module)main(process.argv.slice(2)).catch(error=>{console.error(error.message);process.exitCode=1;});
module.exports={ENDPOINT,PROVIDERS,providerAdmission,QUOTA_HEADERS,retainedHeaders,quotaFromHeaders,quotaAdmission,GENESIS,CHECKPOINT,TOPIC,CONTRACTS,LIMIT,KERNEL_PINS,quantity,rpcHeader,envelope,checkFrozenHead,derivePlan,prepare,validatePlan,validateLogs,pageState,adaptiveSpan,accessRestriction,splitError,capture,kernel};
