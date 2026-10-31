import test from 'node:test';
import assert from 'node:assert/strict';
import {parseTyped,inspectOversizedPreimage} from './independent_attacker_inventory.mjs';

const types=new Map([
  [0,{path:['sp_core','crypto','AccountId32'],def:{composite:{fields:[{name:null,type:1}]}}}],
  [1,{path:[],def:{array:{len:32,type:2}}}],
  [2,{path:[],def:{primitive:'U8'}}],
  [3,{path:['primitive_types','H256'],def:{composite:{fields:[{name:null,type:1}]}}}],
  [4,{path:[],def:{sequence:{type:2}}}],
  [5,{path:[],def:{tuple:[]}}],
  [6,{path:[],def:{sequence:{type:5}}}],
  [7,{path:[],def:{primitive:'U128'}}],
  [8,{path:['primitive_types','H160'],def:{composite:{fields:[{name:null,type:9}]}}}],
  [9,{path:[],def:{array:{len:20,type:2}}}],
]);
test('only typed AccountId32/H160 identity locations are reported',()=>{
  const watch=new Set(['0x'+'01'.repeat(32),'0x'+'02'.repeat(20)]);
  assert.equal(parseTyped(Buffer.alloc(32,1),0,types,{watched:watch}).references.length,1);
  assert.equal(parseTyped(Buffer.alloc(32,1),3,types,{watched:watch}).references.length,0);
  assert.equal(parseTyped(Buffer.concat([Buffer.from([128]),Buffer.alloc(32,1)]),4,types,{watched:watch}).references.length,0);
  assert.equal(parseTyped(Buffer.alloc(20,2),8,types,{watched:watch}).references[0].kind,'h160_address_reference');
});
test('allocation bombs, oversize records and noncanonical compact lengths fail before allocation',()=>{
  assert.throws(()=>parseTyped(Buffer.from([3,255,255,255,255]),6,types),/allocation budget/);
  assert.throws(()=>parseTyped(Buffer.alloc(1024*1024+1),4,types),/1 MiB/);
  assert.throws(()=>parseTyped(Buffer.from([1,0]),4,types),/noncanonical compact/);
  assert.throws(()=>parseTyped(Buffer.from([8,1]),4,types),/truncated/);
});
test('u128 summaries are exact decimal strings and component consumption is explicit',()=>{
  assert.equal(parseTyped(Buffer.alloc(16,255),7,types,{render:true}).value,((1n<<128n)-1n).toString());
  assert.equal(parseTyped(Buffer.alloc(17,0),7,types).consumed,16);
});
test('large legacy preimage exposes its provider but never treats opaque call bytes as owners',()=>{
  const legacyTypes=new Map(types);legacyTypes.set(6,{def:{primitive:'U128'}});legacyTypes.set(50,{path:['sora_nexus_checkpoint_legacy','LegacyDemocracyPreimageStatus'],def:{}});
  const key='0x'+'12'.repeat(64),length=1024*1024+16;
  const compactLength=Buffer.alloc(4);compactLength.writeUInt32LE(length*4+2);
  const provider=Buffer.alloc(32,1),suffix=Buffer.alloc(53);provider.copy(suffix);suffix.writeBigUInt64LE(17n,32);suffix.writeUInt32LE(123,48);
  const value=Buffer.concat([Buffer.from([1]),compactLength,Buffer.alloc(length,2),suffix]);
  const line=JSON.stringify([key,'0x'+value.toString('hex')]);const row={prefix:line.slice(0,4096),tail:line.slice(-4096),bytes:line.length};
  const descriptor={name:'Democracy.Preimages',valueType:50};
  const result=inspectOversizedPreimage(row,key,descriptor,legacyTypes,new Set(['0x'+provider.toString('hex')]));
  assert.equal(result.references.length,1);assert.equal(result.value.deposit,'17');assert.equal(result.opaqueBytes,length);
  assert.throws(()=>inspectOversizedPreimage({...row,bytes:row.bytes+2},key,descriptor,legacyTypes),/length mismatch/);
  assert.throws(()=>inspectOversizedPreimage(row,key,{...descriptor,name:'Balances.Accounts'},legacyTypes),/unreviewed/);
});
test('large byte preimages retain their exact length without allocating typed byte nodes',()=>{
  const key='0x'+'12'.repeat(64),length=1024*1024+1;const count=Buffer.alloc(4);count.writeUInt32LE(length*4+2);
  const line=JSON.stringify([key,'0x'+Buffer.concat([count,Buffer.alloc(length)]).toString('hex')]);
  const row={prefix:line.slice(0,4096),tail:line.slice(-4096),bytes:line.length};
  assert.equal(inspectOversizedPreimage(row,key,{name:'Preimage.PreimageFor',valueType:4},types).opaqueBytes,length);
  assert.throws(()=>inspectOversizedPreimage({...row,bytes:row.bytes-2},key,{name:'Preimage.PreimageFor',valueType:4},types),/length mismatch/);
});
