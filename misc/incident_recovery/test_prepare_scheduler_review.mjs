import test from 'node:test';
import assert from 'node:assert/strict';
import {Readable} from 'node:stream';
import {opaqueByteVector, selectedRows} from './prepare_scheduler_review.mjs';

test('selects whole matching opaque rows across chunk boundaries and skips others', async()=>{
  const input=Buffer.from('{"header":true}\n["0xaa01","0x0102"]\n["0xbb02","0x03"]\n');
  const pieces=Array.from({length:input.length},(_,i)=>input.subarray(i,i+1));
  const rows=[];for await(const row of selectedRows(Readable.from(pieces),['0xaa']))rows.push(row);
  assert.deepEqual(rows,[['0xaa01','0x0102']]);
});
test('rejects unterminated matching row', async()=>{
  await assert.rejects(async()=>{for await(const row of selectedRows(Readable.from([Buffer.from('["0xaa","0x00"]')]),['0xaa']))void row;},/unterminated/);
});
test('opaque vector requires exact canonical length, never parses historical call',()=>{
  assert.equal(opaqueByteVector(Buffer.from([12,255,255,255])),3);
  assert.equal(opaqueByteVector(Buffer.from([0])),0);
  assert.throws(()=>opaqueByteVector(Buffer.from([9,0,1,2])),/noncanonical/);
  assert.throws(()=>opaqueByteVector(Buffer.from([12,1,2])),/length/);
});
