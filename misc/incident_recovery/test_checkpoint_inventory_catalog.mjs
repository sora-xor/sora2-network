// Synthetic capture envelopes only. These tests do not claim native root evidence.
import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,writeFile,symlink,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createHash} from 'node:crypto';
import {catalogCapture,sourceBase,inventoryFromCatalog,OPAQUE_REASONS,streamDigest} from './checkpoint_inventory_catalog.mjs';
const hx=n=>'0x'+n.toString(16).padStart(2,'0').repeat(32);
test('stream hashing preserves file bounds and rejects aliases with actionable paths',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'recovery-catalog-'));
  try {
    const file=join(dir,'page.gz'),alias=join(dir,'alias.gz'),data=Buffer.alloc(128,3);
    await writeFile(file,data);await symlink(file,alias);
    assert.equal(await streamDigest(file,128),createHash('sha256').update(data).digest('hex'));
    await assert.rejects(streamDigest(file,127),/page.gz.*128 bytes; maximum 127/);
    await assert.rejects(streamDigest(alias,128),/alias.gz/);
  } finally {await rm(dir,{recursive:true,force:true});}
});
function fixture() {
  const keys=[hx(1)+'01',hx(2)+'01',hx(3)+'01'];
  const manifest={complete:true,checkpoint:{hash:hx(9)},children:[],top:{trie:'top',pageCount:2,keyCount:3,pages:[
    {file:'pages/top/00000000.jsonl.gz',keyCount:3,firstKey:'incorrect hint',lastKey:'incorrect hint'},
    {file:'pages/top/00000001.jsonl.gz',keyCount:0}]}};
  const headers=[0,1].map(i=>({schema:'sora-nexus-checkpoint-page-v1',checkpointHash:hx(9),trie:'top',pageIndex:i,
    startKey:i?keys.at(-1):null,pageSize:512,keyCount:i?0:3,lastKey:keys.at(-1),terminal:!!i}));
  const pages=[keys,[]];
  const readPage=async function*(_page,index) {
    yield {line:JSON.stringify(headers[index]),prefix:''};
    for(const key of pages[index])yield {line:null,prefix:`["${key}","0x`,tail:'"]'};
  };
  const descriptors=new Map([1,2,3].map(n=>[hx(n),{name:`Pallet.Item${n}`,prefix:hx(n)}]));
  return {manifest,headers,pages,readPage,descriptors};
}
test('catalog scans every actual key, including interior prefixes, without trusting range hints',async()=>{
  const f=fixture(),observed=[],result=await catalogCapture(f.manifest,f.descriptors,f.readPage,row=>observed.push(row));
  assert.deepEqual(result,[1,2,3].map(n=>({trie:'top',storage:`Pallet.Item${n}`,prefix:hx(n),entries:1})));
  assert.equal(observed[0].firstKey,f.pages[0][0]);assert.equal(observed[0].lastKey,f.pages[0].at(-1));
  assert.equal(observed[1].firstKey,null);assert.equal(observed[1].lastKey,null);
});
test('catalog requires a complete ordered cursor chain and exact terminal coverage',async()=>{
  for(const mutate of [f=>{f.headers[1].startKey=hx(4);},f=>{f.headers[0].lastKey=hx(4);},
    f=>{f.headers[1].terminal=false;},f=>{f.manifest.top.keyCount++;},
    f=>{f.pages[0][1]=f.pages[0][0];},f=>{f.pages[0].reverse();},
    f=>{f.headers[0].keyCount=2;},f=>{f.manifest.top.pages[0].file='../escape';},
    f=>{f.manifest.children.push({});},f=>{f.descriptors.delete(hx(2));}]) {
    const f=fixture();mutate(f);await assert.rejects(catalogCapture(f.manifest,f.descriptors,f.readPage));
  }
});
test('exact legacy keys and deployed code are cataloged by their full source descriptors',async()=>{
  const f=fixture(),exact=hx(1)+'01';f.descriptors.set(exact,{name:'Legacy.Exact',prefix:exact});
  const result=await catalogCapture(f.manifest,f.descriptors,f.readPage);assert.equal(result[0].storage,'Legacy.Exact');
});
function base() {
  const legacy={storage:[{name:'Legacy.Root',disposition:'retain_historical',reason:'Exact immutable root.'}]};
  const dispositions=[{trie:'top',storage:'Legacy.Root',action:'retain_historical',reason:'Exact immutable root.',evidenceHash:hx(8)},
    ...Object.entries(OPAQUE_REASONS).map(([storage,reason])=>({trie:'top',storage,action:'retain_historical',reason,evidenceHash:hx(8)}))];
  const options={includePinnedCheckpointLegacy:true,dispositions,keyPreimages:[{trie:'top',key:hx(3),components:[hx(1)]}],
    watchedAccounts:[hx(4)],opaqueRetentions:[{trie:'top',key:'0x3a636f6465',valueHash:hx(5),evidenceHash:hx(6)}]};
  return {legacy,options};
}
test('source base retains the exact native-verified preimages/proofs and rejects fixture or arbitrary dispositions',()=>{
  const {legacy,options}=base(),result=sourceBase(options,legacy,hx(3));
  assert.deepEqual(result.keyPreimages,options.keyPreimages);assert.deepEqual(result.opaqueRetentions,options.opaqueRetentions);
  assert.equal(result.dispositions,undefined);
  for(const mutate of [o=>{o.dispositions[0].reason='PUBLIC FIXTURE ONLY';},o=>{o.dispositions[0].action='mutable_ownership';},
    o=>{o.additionalStorage=[{}];},o=>{o.dispositions.push({...o.dispositions[0]});},
    o=>{o.opaqueRetentions[0].key=hx(2);},o=>{o.opaqueRetentions[0].valueHash=hx(0);}]) {
    const changed=structuredClone(options);mutate(changed);assert.throws(()=>sourceBase(changed,legacy,hx(3)));
  }
});
test('fresh dispositions use explicit ownership policy; unhandled mapped state cannot get a generic waiver',()=>{
  const {legacy,options}=base(),clean=sourceBase(options,legacy,hx(3));
  const catalog=['System.Account','Session.Validators','Permissions.Owners'].map((storage,i)=>({trie:'top',storage,prefix:hx(i+1),entries:1}));
  const result=inventoryFromCatalog(clean,catalog,{'System.Account':'Exact financial row and refcounts preserved.'},hx(9));
  assert.equal(result.dispositions[0].action,'mutable_ownership');
  assert.ok(result.dispositions.slice(1).every(row=>row.action==='retain_historical'&&row.reason.includes('not waived')));
  assert.deepEqual(result.keyPreimages,options.keyPreimages);assert.equal(options.dispositions[0].reason,'Exact immutable root.');
  assert.throws(()=>inventoryFromCatalog(clean,[...catalog,catalog[0]],{},hx(9)));
});
