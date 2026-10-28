import test from 'node:test';
import assert from 'node:assert/strict';
import {validateGraph} from './checkpoint_referrals.mjs';

test('referral graph keeps genesis/runtime provider provenance unknown',()=>{
  for(const providers of [1,2]) assert.equal(validateGraph(new Map([['owner','referrer']]),new Map([['referrer',['owner']]]),new Map([['owner',{providers,consumers:1}],['referrer',{providers,consumers:1}]])).get('owner'),1);
});
test('duplicate, orphan, missing and under-backed references fail',()=>{
  const forward=new Map([['owner','referrer']]), accounts=new Map([['owner',{providers:1,consumers:1}],['referrer',{providers:1,consumers:1}]]);
  assert.throws(()=>validateGraph(forward,new Map([['referrer',['owner','owner']]]),accounts));
  assert.throws(()=>validateGraph(forward,new Map([['referrer',['owner','other']]]),accounts));
  assert.throws(()=>validateGraph(forward,new Map(),accounts));
  accounts.get('owner').consumers=0;
  assert.throws(()=>validateGraph(forward,new Map([['referrer',['owner']]]),accounts));
});
test('self referral retains two consumer references',()=>{
  assert.throws(()=>validateGraph(new Map([['owner','owner']]),new Map([['owner',['owner']]]),new Map([['owner',{providers:1,consumers:1}]])));
  assert.equal(validateGraph(new Map([['owner','owner']]),new Map([['owner',['owner']]]),new Map([['owner',{providers:2,consumers:2}]])).get('owner'),2);
});
