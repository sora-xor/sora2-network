// Synthetic accounting/input graphs; no claims of native or external evidence.
import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {compactLength,prefixReview,checkObservedPages,mergeCancellations,HASHI,SIGNING} from './prepare_checkpoint_bridge_reviews.mjs';
const hx=n=>'0x'+n.toString(16).padStart(2,'0').repeat(32);
const hash=b=>'0x'+createHash('sha256').update(b).digest('hex'); // Deliberately synthetic, not native Blake2.
test('complete bridge family lists follow native source inventories',async()=>{
  for(const [file,names]of [['bridge_hashi',HASHI],['bridge_signing',SIGNING]]) {
    const source=await readFile(new URL(`../../utils/incident-recovery/src/${file}.rs`,import.meta.url),'utf8');
    const list=source.match(/pub const ITEMS:[\s\S]*?= \[([\s\S]*?)\];/)?.[1];assert.ok(list);
    assert.deepEqual([...list.matchAll(/"([A-Za-z_.]+)"/g)].map(m=>m[1]),names);
  }
});
test('SCALE key vector lengths are canonical across compact boundaries',()=>{
  for(const [value,expected]of [[0,'00'],[63,'fc'],[64,'0101'],[16383,'fdff'],[16384,'02000100']])assert.equal(compactLength(value).toString('hex'),expected);
  for(const value of [-1,0.5,2**30,Number.NaN])assert.throws(()=>compactLength(value));
});
test('HASHI review commits exact name, sorted raw key vectors and raw value hashes',()=>{
  const storage=HASHI[0],rows=[{storage,key:'0x0102',valueHash:hx(3)},{storage,key:'0x0304',valueHash:hx(5)}];
  const input=[];const recording=bytes=>{input.push(Buffer.from(bytes));return hash(bytes);};
  const result=prefixReview(storage,rows,recording);
  assert.equal(input[0].toString(),'sora-nexus-hashi-prefix-v1:'+storage);
  assert.deepEqual(input[1],Buffer.concat([Buffer.from(hash(input[0]).slice(2),'hex'),Buffer.from('080102','hex'),Buffer.from(hx(3).slice(2),'hex')]));
  assert.equal(result.records,2);assert.equal(result.commitment,hash(input[2]));
  assert.notEqual(prefixReview(HASHI[1],[],hash).commitment,prefixReview(storage,[],hash).commitment);
  assert.throws(()=>prefixReview(storage,[rows[1],rows[0]],hash),/ordered/);
  assert.throws(()=>prefixReview(storage,[rows[0],rows[0]],hash),/ordered/);
  assert.throws(()=>prefixReview(storage,[{...rows[0],valueHash:'0x01'}],hash),/width/);
});
test('legacy selective helpers require their hints to match independently observed complete ranges',()=>{
  const page={file:'pages/top/00000000.jsonl.gz',sha256:'11'.repeat(32),keyCount:2,firstKey:hx(1),lastKey:hx(2)};
  const manifest={top:{keyCount:2,pages:[page]}},catalog=[{entries:2}],observed=[{...page}];
  checkObservedPages(manifest,observed,catalog);
  for(const patch of [{firstKey:hx(0)},{lastKey:hx(3)},{sha256:'22'.repeat(32)},{keyCount:1},{file:'other'}])
    assert.throws(()=>checkObservedPages(manifest,[{...page,...patch}],catalog));
  assert.throws(()=>checkObservedPages(manifest,observed,[{entries:1}]));
});
test('fresh cancellations preserve generated controls and do not import unrelated semantic authorizations',()=>{
  const semantic={controls:[{syntheticControl:true}],derivedAccounts:[{syntheticUtility:true}]};
  const plan={controls:[],derivedAccounts:[],bridgeCancellations:[{callHash:hx(1),preserve:[{key:hx(2)}]}]};
  const result=mergeCancellations(semantic,plan);assert.deepEqual(result.controls,semantic.controls);
  assert.deepEqual(result.derivedAccounts,semantic.derivedAccounts);assert.deepEqual(result.bridgeCancellations,plan.bridgeCancellations);
  assert.equal(semantic.bridgeCancellations,undefined);
  assert.throws(()=>mergeCancellations({...semantic,bridgeCancellations:[{}]},plan));
  assert.throws(()=>mergeCancellations(semantic,{...plan,controls:[{}]}));
  assert.throws(()=>mergeCancellations(semantic,{...plan,bridgeCancellations:[...plan.bridgeCancellations,...plan.bridgeCancellations]}));
});
