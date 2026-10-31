'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),crypto=require('node:crypto');
const c=require('./capture_hashi_deposits.cjs');
const sha=b=>crypto.createHash('sha256').update(b).digest('hex'),json=v=>Buffer.from(JSON.stringify(v)+'\n'),hx=n=>'0x'+n.toString(16).padStart(2,'0').repeat(32);
const root=path.resolve(__dirname,'../..'),fixtures=path.join(root,'artifacts/recovery-28191817/fixtures');
function temp(t){const dir=fs.mkdtempSync(path.join(fs.realpathSync(os.tmpdir()),'sora-nexus-hashi-'));t.after(()=>fs.rmSync(dir,{recursive:true}));return dir;}
function log(extra={}){return {address:c.CONTRACTS[0].address,removed:false,blockNumber:'0x1',transactionIndex:'0x0',logIndex:'0x0',blockHash:hx(1),transactionHash:hx(2),topics:[c.TOPIC],data:'0x'+'00'.repeat(128),...extra};}
function saveRef(dir,file,value){const bytes=Buffer.isBuffer(value)?value:json(value);fs.writeFileSync(path.join(dir,file),bytes);return {file,sha256:sha(bytes),bytes:bytes.length};}
function page(dir,index,from,through,logs,prior=null){
  const request={jsonrpc:'2.0',id:index,method:'eth_getLogs',params:[{address:c.CONTRACTS.map(x=>x.address),fromBlock:'0x'+from.toString(16),toBlock:'0x'+through.toString(16)}]};
  const requestRef=saveRef(dir,`rpc/${index}.request.json`,request),response=saveRef(dir,`rpc/${index}.response.json`,{jsonrpc:'2.0',id:index,result:logs});
  const locators=c.validateLogs(logs,from,through),value={schema:'sora-nexus-hashi-provider-log-page-v1',planSha256:hx(8),previousPageSha256:prior,fromBlock:from,throughBlock:through,request:requestRef,response,logCount:logs.length,depositCount:locators.filter(l=>l.deposit).length,locatorsSha256:sha(json(locators))};
  saveRef(dir,`pages/${String(index).padStart(8,'0')}.json`,value);return sha(json(value));
}
test('canonical quantities and RPC envelopes reject null, RPC errors, ambiguity and HTTP-shaped impostors',()=>{
  assert.equal(c.quantity('0x0'),0);assert.equal(c.quantity('0xff'),255);
  for(const value of ['0x00','0X1','0x','0x20000000000000',1])assert.throws(()=>c.quantity(value));
  assert.deepEqual(c.envelope(json({jsonrpc:'2.0',id:1,result:[]}),1),[]);
  for(const value of [{jsonrpc:'2.0',id:1,result:null},{jsonrpc:'2.0',id:2,result:[]},
    {jsonrpc:'2.0',id:1,result:[],error:{code:-1}},{jsonrpc:'2.0',id:1,error:{code:-32000,message:'missing history'}}])assert.throws(()=>c.envelope(json(value),1));
  assert.throws(()=>c.envelope(Buffer.from('{"jsonrpc":"2.0","id":1,"result":[],"result":[]}'),1),/unambiguous/);
});
test('all three configured contract logs survive inventory; deposits retain transaction identity without invented sender',()=>{
  const rows=[log(),log({address:c.CONTRACTS[1].address,logIndex:'0x1'}),log({address:c.CONTRACTS[2].address,logIndex:'0x2',topics:[hx(9)],data:'0x'})];
  const out=c.validateLogs(rows,0,10);assert.equal(out.length,3);assert.equal(out.filter(r=>r.deposit).length,2);
  assert.equal(out[0].nativeMessageId,out[1].nativeMessageId);assert.equal(out[2].nativeMessageId,null);
  assert.equal(Object.hasOwn(out[0],'sender'),false);assert.equal(Object.hasOwn(out[0],'recipient'),false);
});
test('malformed, duplicate, removed and contradictory logs fail instead of disappearing',()=>{
  for(const row of [log({removed:true}),log({removed:undefined}),log({blockNumber:'0xb'}),log({address:'0x'+'99'.repeat(20)}),
    log({topics:[c.TOPIC,hx(3)]}),log({data:'0x00'}),log({data:'0x'+'00'.repeat(64)+'01'+'00'.repeat(63)})])assert.throws(()=>c.validateLogs([row],0,10));
  assert.throws(()=>c.validateLogs([log(),log()],0,10),/duplicate/);
  assert.throws(()=>c.validateLogs([log(),log({blockHash:hx(4),logIndex:'0x1'})],0,10),/conflicting block/);
  assert.throws(()=>c.validateLogs([log(),log({transactionIndex:'0x1',logIndex:'0x1'})],0,10),/conflicting transaction/);
  assert.throws(()=>c.validateLogs(Array(c.LIMIT.logs).fill(log()),0,10),/truncation/);
});
test('resumption revalidates a consecutive page chain and the exact raw response bytes',t=>{
  const dir=temp(t);fs.mkdirSync(path.join(dir,'rpc'));fs.mkdirSync(path.join(dir,'pages'));
  const first=page(dir,0,0,3,[log()]);page(dir,1,4,9,[],first);
  const plan={frozenHead:{number:20}},state=c.pageState(dir,plan,hx(8));assert.equal(state.next,10);assert.equal(state.deposits,1);assert.equal(state.pages.length,2);
  fs.writeFileSync(path.join(dir,'rpc/1.response.json'),json({jsonrpc:'2.0',id:1,error:{code:-1,message:'not empty'}}));
  assert.throws(()=>c.pageState(dir,plan,hx(8)),/bytes changed/);
});
test('range gaps, overlapping pages and an unbound replacement plan cannot resume',t=>{
  const dir=temp(t);fs.mkdirSync(path.join(dir,'rpc'));fs.mkdirSync(path.join(dir,'pages'));
  const first=page(dir,0,0,3,[]);page(dir,1,5,9,[],first);
  assert.throws(()=>c.pageState(dir,{frozenHead:{number:20}},hx(8)),/continuity/);
  assert.throws(()=>c.pageState(dir,{frozenHead:{number:20}},hx(7)),/continuity/);
});
test('an interrupted atomic page write never advances coverage or hides a committed page gap',t=>{
  const dir=temp(t);fs.mkdirSync(path.join(dir,'rpc'));fs.mkdirSync(path.join(dir,'pages'));
  page(dir,0,0,3,[]);
  fs.writeFileSync(path.join(dir,'pages/00000001.json.writing-123-00000000-0000-0000-0000-000000000001'),'{partial');
  assert.equal(c.pageState(dir,{frozenHead:{number:20}},hx(8)).next,4);
  fs.writeFileSync(path.join(dir,'pages/00000002.json'),'{partial');
  assert.throws(()=>c.pageState(dir,{frozenHead:{number:20}},hx(8)),/page index gap/);
});
test('provider range limits reduce request span but rate limits and missing history remain explicit failures',t=>{
  assert.equal(c.splitError({rpcError:{code:-32005,message:'block range limit exceeded'}}),true);
  assert.equal(c.splitError({httpStatus:413}),true);assert.equal(c.splitError({httpStatus:429}),false);
  assert.equal(c.splitError({rpcError:{code:-32000,message:'historical state is unavailable'}}),false);
  assert.equal(c.splitError({httpStatus:400,rpcError:{code:35,message:'ranges over 10000 blocks are not supported on free plan'}}),true);
  for(const message of ['rate limit exceeded','too many requests','account quota exceeded','API key required for block range','archive requests require a token',
    'block range unauthorized','block range access denied','block range forbidden','block range credentials required'])
    assert.equal(c.splitError({rpcError:{code:-32005,message}}),false);
  assert.equal(c.splitError({httpStatus:413,rpcError:{code:-32005,message:'block range requires authentication'}}),false);
  for(const httpStatus of [401,403,429])assert.equal(c.splitError({httpStatus,rpcError:{code:-32005,message:'block range limit exceeded'}}),false);
  const dir=temp(t);fs.mkdirSync(path.join(dir,'rpc'));
  const request=saveRef(dir,'rpc/00000000.request.json',{jsonrpc:'2.0',id:0,method:'eth_getLogs',params:[{fromBlock:'0x0',toBlock:'0xc34f'}]});
  saveRef(dir,'rpc/00000000.meta.json',{request,error:'RPC range limit',rpcError:{code:-32005,message:'range limit'}});
  assert.equal(c.adaptiveSpan(dir,0),25000);assert.equal(c.adaptiveSpan(dir,1),c.LIMIT.span);
});
const actualAvailable=fs.existsSync(path.join(fixtures,'public-counterpart-identities-v1/eth-2.json'))
  &&fs.existsSync(path.join(fixtures,'trusted-csv-bridge-source-registry-v1/generated/report.json'))
  &&fs.existsSync(path.join(fixtures,'trusted-csv-native-admission-v2/capture-verification.json'));
function actualPlan(dir){return c.prepare({registryFile:path.join(fixtures,'trusted-csv-bridge-source-registry-v1/generated/report.json'),
  verificationFile:path.join(fixtures,'trusted-csv-native-admission-v2/capture-verification.json'),identityDirectory:path.join(fixtures,'public-counterpart-identities-v1'),output:path.join(dir,'plan.json')});}
test('retained public identity and checkpoint registry produce a fixed plan without RPC',{skip:!actualAvailable},t=>{
  const dir=temp(t),plan=actualPlan(dir);c.validatePlan(plan);assert.equal(plan.frozenHead.number,26298768);assert.equal(plan.fromBlock,0);assert.equal(plan.contracts.length,3);
  for(const update of [{fromBlock:1},{productionReady:true},{historicalContractsExhaustivelyDiscovered:true},{frozenHead:{...plan.frozenHead,number:1}},
    {inputs:plan.inputs.slice(1)}])assert.throws(()=>c.validatePlan({...plan,...update}));
});
test('batch-end head check uses the frozen height and detects a valid alternative header',{skip:!actualAvailable},async()=>{
  const retained=JSON.parse(fs.readFileSync(path.join(fixtures,'public-counterpart-identities-v1/eth-2.json'))),header=JSON.parse(retained.responseText).result,k=c.kernel();
  const frozen=c.rpcHeader(header,k),calls=[];
  const check=await c.checkFrozenHead(async(method,params)=>{calls.push({method,params});return {value:header,request:{synthetic:true},response:{synthetic:true}};},frozen,k);
  assert.deepEqual(calls,[{method:'eth_getBlockByNumber',params:[header.number,false]}]);assert.equal(check.hash,frozen.hash);
  const changed={...header,extraData:'0x01'},fields=k.decode(k.hex(frozen.header));fields[12]=Buffer.from([1]);changed.hash=k.hx(k.keccak(k.encode(fields)));
  await assert.rejects(c.checkFrozenHead(async()=>({value:changed,request:{synthetic:true},response:{synthetic:true}}),frozen,k),e=>e.headDrift?.observed.hash===changed.hash);
});
test('budget interruption preserves raw request/response and never certifies an empty history',{skip:!actualAvailable},async t=>{
  const dir=temp(t);actualPlan(dir);const output=path.join(dir,'capture');let calls=0;
  const send=async(request,timeout)=>{calls++;assert.equal(timeout,900000);assert.equal(request.method,'eth_chainId');return {httpStatus:200,bytes:json({jsonrpc:'2.0',id:request.id,result:'0x1'})};};
  for(let i=0;i<2;i++){const report=await c.capture({planFile:path.join(dir,'plan.json'),output,maxRequests:1,send});
    assert.equal(report.budgetReached,true);assert.equal(report.nextBlock,0);assert.equal(report.pages.length,0);assert.equal(report.completeConfiguredAddressRange,false);assert.equal(report.inputGuardPassed,true);}
  assert.equal(calls,2);assert.equal(fs.readdirSync(path.join(output,'rpc')).length,6);assert.equal(fs.existsSync(path.join(output,'LOCK')),false);
});
test('HTTP failure is retained and propagates, with no page or completion claim',{skip:!actualAvailable},async t=>{
  const dir=temp(t);actualPlan(dir);const output=path.join(dir,'capture');
  await assert.rejects(c.capture({planFile:path.join(dir,'plan.json'),output,send:async()=>({httpStatus:429,bytes:Buffer.from('busy')})}),/HTTP status 429/);
  const reports=fs.readdirSync(path.join(output,'runs')).map(n=>JSON.parse(fs.readFileSync(path.join(output,'runs',n))));
  assert.equal(reports.length,1);assert.equal(reports[0].completeConfiguredAddressRange,false);assert.match(reports[0].error,/429/);
  assert.equal(fs.readdirSync(path.join(output,'pages')).length,0);assert.equal(fs.readFileSync(path.join(output,'rpc/00000000.response.json'),'utf8'),'busy');
});
test('a non-200 canonical RPC error is retained for range classification but never creates a page',{skip:!actualAvailable},async t=>{
  const dir=temp(t);actualPlan(dir);const output=path.join(dir,'capture');
  const error={code:35,message:'ranges over 10000 blocks are not supported on free plan'};
  await assert.rejects(c.capture({planFile:path.join(dir,'plan.json'),output,
    send:async request=>({httpStatus:400,bytes:json({jsonrpc:'2.0',id:request.id,error})})}),/HTTP status 400/);
  const meta=JSON.parse(fs.readFileSync(path.join(output,'rpc/00000000.meta.json')));
  assert.deepEqual(meta.rpcError,error);assert.equal(meta.successfulResponse,false);assert.equal(c.splitError(meta),true);
  assert.equal(fs.readdirSync(path.join(output,'pages')).length,0);
});
test('provider choice is explicit and never admits arbitrary endpoints or a silent fallback',()=>{
  for(const provider of ['unknown','https://eth.drpc.org/',undefined])assert.throws(()=>c.providerAdmission(provider,undefined,{},null),/allowlist/);
  assert.throws(()=>c.providerAdmission('drpc',undefined,{},null),/separately retained/);
  assert.throws(()=>c.providerAdmission('publicnode','/unexpected',{},null),/alternate-provider/);
  assert.deepEqual(c.providerAdmission('publicnode',undefined,{},null),{provider:'publicnode',endpoint:c.ENDPOINT,source:c.PROVIDERS.publicnode.source,probe:null,inputs:[]});
});
const drpcAvailable=actualAvailable&&fs.existsSync(path.join(fixtures,'public-ethereum-archive-capability-v1/rpc-3.json'));
function drpcPlan(dir,providerProbeDirectory=path.join(fixtures,'public-ethereum-archive-capability-v1')){
  return c.prepare({registryFile:path.join(fixtures,'trusted-csv-bridge-source-registry-v1/generated/report.json'),
    verificationFile:path.join(fixtures,'trusted-csv-native-admission-v2/capture-verification.json'),identityDirectory:path.join(fixtures,'public-counterpart-identities-v1'),
    provider:'drpc',providerProbeDirectory,output:path.join(dir,'plan.json')});
}
test('separate dRPC plan binds chain genesis identical frozen header and only the limited archive probe',{skip:!drpcAvailable},async t=>{
  const dir=temp(t),plan=drpcPlan(dir);c.validatePlan(plan);
  assert.equal(plan.endpoint,'https://eth.drpc.org/');assert.equal(plan.provider,'drpc');
  assert.equal(plan.providerSource,'https://drpc.org/docs/ethereum-api');
  assert.deepEqual(plan.capabilityProbe,{fromBlock:0,throughBlock:99,logCount:0,fullArchiveCapabilityEstablished:false});
  assert.equal(plan.inputs.filter(p=>p.file.includes('public-ethereum-archive-capability-v1/rpc-')).length,4);
  for(const update of [{endpoint:c.ENDPOINT},{provider:'publicnode'},{capabilityProbe:{...plan.capabilityProbe,fullArchiveCapabilityEstablished:true}}])assert.throws(()=>c.validatePlan({...plan,...update}));
  let calls=0;
  const report=await c.capture({planFile:path.join(dir,'plan.json'),output:path.join(dir,'capture'),maxRequests:1,
    send:async(request,timeout,endpoint)=>{calls++;assert.equal(endpoint,plan.endpoint);return {httpStatus:200,bytes:json({jsonrpc:'2.0',id:request.id,result:'0x1'})};}});
  assert.equal(calls,1);assert.equal(report.endpoint,plan.endpoint);assert.equal(report.provider,'drpc');assert.equal(report.pages.length,0);
  assert.equal(report.completeConfiguredAddressRange,false);
  const meta=JSON.parse(fs.readFileSync(path.join(dir,'capture/rpc/00000000.meta.json')));assert.equal(meta.endpoint,plan.endpoint);
});
test('dRPC probe cannot substitute a provider source, chain, block range or frozen header',{skip:!drpcAvailable},t=>{
  for(const [index,mutate]of [[0,row=>row.endpoint=c.ENDPOINT],[0,row=>row.source='https://example.invalid'],
    [0,row=>row.responseText=JSON.stringify({jsonrpc:'2.0',id:0,result:'0x2'})],[2,row=>row.request.params[0]='0x1'],
    [3,row=>row.request.params[0].toBlock='0x64']]){
    const dir=temp(t),probe=path.join(dir,'probe');fs.mkdirSync(probe);
    for(let i=0;i<4;i++){const row=JSON.parse(fs.readFileSync(path.join(fixtures,`public-ethereum-archive-capability-v1/rpc-${i}.json`)));if(i===index)mutate(row);fs.writeFileSync(path.join(probe,`rpc-${i}.json`),json(row));}
    assert.throws(()=>drpcPlan(dir,probe),/differs/);
  }
});
test('provider switch cannot reuse an existing capture directory or its pages',{skip:!drpcAvailable},async t=>{
  const dir=temp(t),old=path.join(dir,'old'),next=path.join(dir,'new');fs.mkdirSync(old);fs.mkdirSync(next);actualPlan(old);drpcPlan(next);
  let calls=0;const send=async request=>{calls++;return {httpStatus:200,bytes:json({jsonrpc:'2.0',id:request.id,result:'0x1'})};};
  await c.capture({planFile:path.join(old,'plan.json'),output:path.join(dir,'capture'),maxRequests:1,send});
  await assert.rejects(c.capture({planFile:path.join(next,'plan.json'),output:path.join(dir,'capture'),maxRequests:1,send}),/directory\/plan differs/);
  assert.equal(calls,1);
});
function quotaHeaders(changes={}){return {'x-tdly-egress-limit':'1000000000','x-tdly-egress-remaining':'999999000','x-tdly-egress-reset-timestamp':'1793373937594',
  'x-tdly-limit':'20','x-tdly-remaining':'19','x-tdly-reset-timestamp':'1793287538594',...changes};}
test('Tenderly quota headers are bounded and missing malformed or contradictory budgets fail closed',()=>{
  assert.equal(c.quotaFromHeaders(quotaHeaders()).remainingBytes,999999000);
  for(const update of [{'x-tdly-egress-remaining':undefined},{'x-tdly-egress-limit':'0'},
    {'x-tdly-remaining':'21'},{'x-tdly-egress-remaining':'1000000001'},
    {'x-tdly-egress-reset-timestamp':'01'},{'x-tdly-limit':'9007199254740992'}])assert.throws(()=>c.quotaFromHeaders(quotaHeaders(update)));
  assert.throws(()=>c.retainedHeaders({'retry-after':'1\n2'}));
  assert.deepEqual(c.retainedHeaders({'server':'example','retry-after':'1'}),{'retry-after':'1'});
});
test('quota admission reserves worst-case bytes and a final-head request; it never automatically waits',()=>{
  const good={remainingBytes:2*c.LIMIT.file,remainingRequests:2};assert.doesNotThrow(()=>c.quotaAdmission(good,false));
  for(const quota of [{...good,remainingBytes:good.remainingBytes-1},{...good,remainingRequests:1}])assert.throws(()=>c.quotaAdmission(quota,false),e=>e.quota===true);
  assert.doesNotThrow(()=>c.quotaAdmission({remainingBytes:c.LIMIT.file,remainingRequests:1},true));
  assert.throws(()=>c.quotaAdmission({remainingBytes:c.LIMIT.file-1,remainingRequests:1},true),e=>e.quota===true);
  assert.throws(()=>c.quotaAdmission({remainingBytes:c.LIMIT.file,remainingRequests:0},true),e=>e.quota===true);
});
test('the provider-specific 3000-log truncation limit is stricter than the generic bound',()=>{
  assert.throws(()=>c.validateLogs(Array(3000).fill(log()),0,10,c.CONTRACTS,3000),/truncation/);
  assert.equal(c.validateLogs([log()],0,10,c.CONTRACTS,3000).length,1);
  assert.throws(()=>c.validateLogs([],0,10,c.CONTRACTS,0),/provider log bound/);
});
const tenderlyAvailable=actualAvailable&&fs.existsSync(path.join(fixtures,'public-ethereum-tenderly-capability-v1/rpc-3.json'));
function tenderlyPlan(dir){return c.prepare({registryFile:path.join(fixtures,'trusted-csv-bridge-source-registry-v1/generated/report.json'),
  verificationFile:path.join(fixtures,'trusted-csv-native-admission-v2/capture-verification.json'),identityDirectory:path.join(fixtures,'public-counterpart-identities-v1'),
  provider:'tenderly',providerProbeDirectory:path.join(fixtures,'public-ethereum-tenderly-capability-v1'),output:path.join(dir,'plan.json')});}
test('Tenderly plan binds its distinct50000-block probe and retains live quota headers',{skip:!tenderlyAvailable},async t=>{
  const dir=temp(t),plan=tenderlyPlan(dir);c.validatePlan(plan);
  assert.equal(plan.endpoint,'https://mainnet.gateway.tenderly.co');assert.equal(plan.responseLogCountExclusive,3000);
  assert.deepEqual(plan.capabilityProbe,{fromBlock:0,throughBlock:49999,logCount:0,fullArchiveCapabilityEstablished:false});
  assert.equal(plan.quotaPolicy.minimumEgressReserveBytes,2*c.LIMIT.file);
  for(const update of [{responseLogCountExclusive:10000},{quotaPolicy:null}])assert.throws(()=>c.validatePlan({...plan,...update}));
  const report=await c.capture({planFile:path.join(dir,'plan.json'),output:path.join(dir,'capture'),maxRequests:1,
    send:async(request,timeout,endpoint)=>{assert.equal(endpoint,plan.endpoint);return {httpStatus:200,responseHeaders:quotaHeaders(),bytes:json({jsonrpc:'2.0',id:request.id,result:'0x1'})};}});
  assert.equal(report.budgetReached,true);assert.equal(report.pages.length,0);
  const meta=JSON.parse(fs.readFileSync(path.join(dir,'capture/rpc/00000000.meta.json')));assert.deepEqual(meta.responseHeaders,quotaHeaders());
});
test('low or absent live Tenderly quota stops after one request and cannot certify any range',{skip:!tenderlyAvailable},async t=>{
  for(const responseHeaders of [quotaHeaders({'x-tdly-egress-remaining':'100000000'}),undefined]){
    const dir=temp(t);tenderlyPlan(dir);let requests=0;
    await assert.rejects(c.capture({planFile:path.join(dir,'plan.json'),output:path.join(dir,'capture'),maxRequests:8,
      send:async request=>{requests++;return {httpStatus:200,responseHeaders,bytes:json({jsonrpc:'2.0',id:request.id,result:'0x1'})};}}),/quota/);
    assert.equal(requests,1);assert.equal(fs.readdirSync(path.join(dir,'capture/pages')).length,0);
    const reports=fs.readdirSync(path.join(dir,'capture/runs')).map(n=>JSON.parse(fs.readFileSync(path.join(dir,'capture/runs',n))));assert.equal(reports[0].completeConfiguredAddressRange,false);
  }
});
test('rate access and invalid-quota stops issue no final-head request after the rejected log call',{skip:!tenderlyAvailable},async t=>{
  const probe=path.join(fixtures,'public-ethereum-tenderly-capability-v1');
  const values=[0,1,2].map(i=>JSON.parse(JSON.parse(fs.readFileSync(path.join(probe,`rpc-${i}.json`))).responseText).result);
  const codes=[3,4,5].map(i=>JSON.parse(JSON.parse(fs.readFileSync(path.join(fixtures,`public-counterpart-identities-v1/eth-${i}.json`))).responseText).result);
  for(const failure of [{status:429},{status:401},{status:403},{status:200,missingQuota:true},
    {status:200,invalidQuota:true},{status:200,rpcError:{code:-32000,message:'rate limit exceeded'}}]){
    const dir=temp(t);tenderlyPlan(dir);let calls=0;
    const send=async request=>{
      calls++;assert.ok(calls<=7,'a restricted provider received another request');
      const value=calls<=3?values[calls-1]:calls<=6?codes[calls-4]:[];
      const responseHeaders=calls===7&&failure.missingQuota?undefined:calls===7&&failure.invalidQuota?quotaHeaders({'x-tdly-remaining':'not-a-number'}):quotaHeaders();
      return {httpStatus:calls===7?failure.status:200,responseHeaders,
        bytes:json({jsonrpc:'2.0',id:request.id,...(calls===7&&failure.rpcError?{error:failure.rpcError}:{result:value})})};
    };
    await assert.rejects(c.capture({planFile:path.join(dir,'plan.json'),output:path.join(dir,'capture'),maxRequests:8,send}));
    assert.equal(calls,7);assert.equal(fs.readdirSync(path.join(dir,'capture/pages')).length,0);
    const reports=fs.readdirSync(path.join(dir,'capture/runs')).map(n=>JSON.parse(fs.readFileSync(path.join(dir,'capture/runs',n))));
    assert.equal(reports[0].canonicalFrozenHeadCheckedAtBatchEnd,false);assert.equal(reports[0].requestsThisRun,7);
    assert.equal(reports[0].completeConfiguredAddressRange,false);
  }
});
