import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { CHECKPOINT, sha256 } from './capture_checkpoint.mjs';
import { captureAncestry } from './capture_history.mjs';

async function temporary(run) {
  const folder = await mkdtemp(join(tmpdir(), 'sora-nexus-history-'));
  try { await run(folder); } finally { await rm(folder, {recursive:true,force:true}); }
}
function fakeRpc() {
  const byHash = new Map();
  const calls = [];
  return { calls, async call(method, [numbers]) {
    calls.push({method,numbers}); assert.equal(method,'chain_getBlockHash');
    return numbers.map(number => {
      const hash = number === CHECKPOINT.number ? CHECKPOINT.hash : '0x' + number.toString(16).padStart(64,'0');
      byHash.set(hash,{number:'0x'+number.toString(16),stateRoot:CHECKPOINT.stateRoot,parentHash:CHECKPOINT.parentHash});
      return hash;
    });
  }, async batch(requests) {
    calls.push({batch:requests.length});
    return requests.map(({method,params}) => {assert.equal(method,'chain_getHeader');return byHash.get(params[0]);});
  } };
}
test('history downloader resumes immutable headers and binds ancestry artifact checksum', () => temporary(async folder => {
  const rpc=fakeRpc();
  const result=await captureAncestry(rpc,folder,CHECKPOINT.number-2);
  const bytes=await readFile(join(folder,result.file));
  assert.equal(result.sha256,sha256(bytes));
  assert.deepEqual(JSON.parse(bytes).map(h=>Number(BigInt(h.number))),[CHECKPOINT.number-2,CHECKPOINT.number-1,CHECKPOINT.number]);
  assert.equal(rpc.calls.length,2);
  const idle=fakeRpc();
  assert.deepEqual(await captureAncestry(idle,folder,CHECKPOINT.number-2),result);
  assert.equal(idle.calls.length,0);
  const changed=JSON.parse(await readFile(join(folder,'headers',`${CHECKPOINT.number-1}.json`)));
  changed.number='0x1';
  await writeFile(join(folder,'headers',`${CHECKPOINT.number-1}.json`),JSON.stringify(changed));
  await assert.rejects(()=>captureAncestry(idle,folder,CHECKPOINT.number-2),/Cached ancestry number mismatch/);
}));
test('history downloader refuses a conflicting checkpoint response before completion', () => temporary(async folder => {
  const rpc=fakeRpc(),batch=rpc.batch;
  rpc.batch=async requests => (await batch(requests)).map(header=>({...header,stateRoot:'0x'+'00'.repeat(32)}));
  await assert.rejects(()=>captureAncestry(rpc,folder,CHECKPOINT.number-1),/Wrong checkpoint identity/);
  await assert.rejects(()=>readFile(join(folder,'ancestry.json')),error=>error.code==='ENOENT');
}));
