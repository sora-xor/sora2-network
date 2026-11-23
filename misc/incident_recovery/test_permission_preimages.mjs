import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {recoverComponents} from './recover_permission_preimages.mjs';
const hash=b=>createHash('sha256').update(b).digest();
const hex=b=>'0x'+b.toString('hex');
test('only exact two-component preimages are admitted, never missing or conflicting candidates',()=>{
  const prefix='0x'+'11'.repeat(32),first=Buffer.from([1,0,0,0]),second=Buffer.from([1]);
  const descriptor={prefix,keyTypes:[4,341]};
  const key=prefix+hash(first).toString('hex')+hash(second).toString('hex');
  const maps=[new Map([[hex(hash(first)),[hex(first)]]]),new Map([[hex(hash(second)),[hex(second)]]])];
  assert.deepEqual(recoverComponents(key,descriptor,maps,hash),[hex(first),hex(second)]);
  assert.equal(recoverComponents(key,descriptor,[maps[0],new Map()],hash),null);
  assert.throws(()=>recoverComponents(key,descriptor,[new Map([[hex(hash(first)),[hex(first),'0x02']]]),maps[1]],hash),/collision/);
  assert.throws(()=>recoverComponents(key,descriptor,[new Map([[hex(hash(first)),['0x02']]]),maps[1]],hash),/rehash/);
  assert.throws(()=>recoverComponents(key+'00',descriptor,maps,hash),/shape/);
});
