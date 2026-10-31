import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm, readFile, readdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { gzipSync } from 'node:zlib';
import { CHECKPOINT, ReadOnlyRpc, capture, captureTrie, decodePage, validateEndpoint,
  validateKeys, validateTopChanges, readTopEntries, writeAtomic, validateAnchor } from './capture_checkpoint.mjs';

async function temporary(fn) {
  const directory = await mkdtemp(join(tmpdir(), 'sora-recovery-capture-'));
  try { return await fn(directory); } finally { await rm(directory, { recursive: true, force: true }); }
}
function fakeRpc(entries, { failQuery = 0, child = false } = {}) {
  let queries = 0;
  const calls = [];
  return { calls, async call(method, params) {
    calls.push({ method, params });
    assert.equal(params.at(-1), CHECKPOINT.hash);
    if (method.endsWith('getKeysPaged')) {
      const [prefix, count, start] = child ? params.slice(1) : params;
      assert.equal(prefix, '0x');
      return entries.map(([key]) => key).filter((key) => start === null || key > start).slice(0, count);
    }
    queries += 1;
    if (queries === failQuery) throw new Error('simulated interrupted read');
    if (method === 'state_queryStorageAt') {
      return [{ block: CHECKPOINT.hash, changes: params[0].map((key) => entries.find((entry) => entry[0] === key)) }];
    }
    if (method === 'childstate_getStorageEntries') return params[1].map((key) => entries.find((entry) => entry[0] === key)?.[1]);
    if (method === 'childstate_getStorage') return entries.find((entry) => entry[0] === params[1])?.[1];
    throw new Error(`Unexpected method ${method}`);
  } };
}
test('endpoint admission rejects redirects, credentials, unapproved hosts and disguised URLs', () => {
  assert.equal(validateEndpoint('wss://mof2.sora.org'), 'wss://mof2.sora.org/');
  assert.equal(validateEndpoint('ws://127.0.0.1:9944'), 'ws://127.0.0.1:9944/');
  for (const endpoint of ['ws://mof2.sora.org', 'wss://example.invalid', 'wss://mof2.sora.org.evil.invalid',
    'wss://user:secret@mof2.sora.org', 'wss://mof2.sora.org/other', 'wss://mof2.sora.org/?redirect=1']) {
    assert.throws(() => validateEndpoint(endpoint));
  }
});
test('transport rejects mutation methods before opening any socket', async () => {
  const rpc = new ReadOnlyRpc('ws://localhost:9944');
  await assert.rejects(() => rpc.call('author_submitExtrinsic', ['0x']), /Non-read-only/);
  assert.equal(rpc.socket, null);
});
test('key pagination must be strictly ordered, complete and beyond the cursor', () => {
  assert.deepEqual(validateKeys(['0x01', '0x0100', '0x02'], null, 3), ['0x01', '0x0100', '0x02']);
  for (const keys of [['0x01', '0x01'], ['0x02', '0x01'], ['0xAA'], ['0x1']]) {
    assert.throws(() => validateKeys(keys, null, 3));
  }
  assert.throws(() => validateKeys(['0x01'], '0x01', 3));
  assert.throws(() => validateKeys(['0x01', '0x02'], null, 1));
});
test('queryStorageAt must cover exactly the requested keys at the pinned block', () => {
  const valid = [{ block: CHECKPOINT.hash, changes: [['0x02', '0x'], ['0x01', '0xaa']] }];
  assert.deepEqual(validateTopChanges(valid, ['0x01', '0x02']), [['0x01', '0xaa'], ['0x02', '0x']]);
  for (const response of [[], [{ ...valid[0], block: '0x' + '0'.repeat(64) }],
    [{ ...valid[0], changes: [['0x01', null], ['0x02', '0x']] }],
    [{ ...valid[0], changes: [['0x01', '0xaa'], ['0x01', '0xbb']] }],
    [{ ...valid[0], changes: [['0x01', '0xaa']] }]]) {
    assert.throws(() => validateTopChanges(response, ['0x01', '0x02']));
  }
});
test('top capture writes a terminal page and resumes only completed pages', () => temporary(async (directory) => {
  const entries = [['0x01', '0xaa'], ['0x02', '0xbb'], ['0x03', '0xcc']];
  await assert.rejects(() => captureTrie(fakeRpc(entries, { failQuery: 2 }), directory, 'top', 2), /interrupted/);
  assert.deepEqual(await readdir(join(directory, 'pages/top')), ['00000000.jsonl.gz']);
  await writeFile(join(directory, 'pages/top/00000001.jsonl.gz.partial-dead'), 'incomplete');
  const rpc = fakeRpc(entries);
  const result = await captureTrie(rpc, directory, 'top', 2);
  assert.equal(result.keyCount, 3);
  assert.equal(result.pageCount, 3);
  assert.equal(rpc.calls[0].params[2], '0x02');
  const finalRpc = fakeRpc(entries);
  const resumed = await captureTrie(finalRpc, directory, 'top', 2);
  assert.deepEqual(resumed, result);
  assert.equal(finalRpc.calls.length, 0);
}));
test('oversized top-value batches split without losing order or accepting missing values', async () => {
  const keys = ['0x01', '0x02', '0x03', '0x04'];
  const calls = [];
  const rpc = { async call(method, [group, block]) {
    assert.equal(method, 'state_queryStorageAt'); assert.equal(block, CHECKPOINT.hash);
    calls.push(group);
    if (group.length > 1) throw new Error('state_queryStorageAt: Response is too big');
    return [{ block, changes: [[group[0], '0xaa']] }];
  } };
  assert.deepEqual(await readTopEntries(rpc, keys), keys.map((key) => [key, '0xaa']));
  assert.equal(calls.length, 7);
  await assert.rejects(() => readTopEntries({ call: async () => { throw new Error('Response is too big'); } }, ['0x01']), /too big/);
  await assert.rejects(() => readTopEntries({ call: async () => [{ block: CHECKPOINT.hash, changes: [] }] }, keys), /coverage/);
});
test('saved page corruption, gaps and conflicting resume sizes fail closed', () => temporary(async (directory) => {
  await captureTrie(fakeRpc([['0x01', '0xaa']]), directory, 'top', 2);
  await assert.rejects(() => captureTrie(fakeRpc([]), directory, 'top', 3), /identity\/cursor/);
  const bytes = await readFile(join(directory, 'pages/top/00000000.jsonl.gz'));
  assert.throws(() => decodePage(bytes.subarray(0, 15), { trie: 'top', pageIndex: 0, startKey: null, pageSize: 2 }));
  await writeFile(join(directory, 'pages/top/00000003.jsonl.gz'), bytes);
  await assert.rejects(() => captureTrie(fakeRpc([]), directory, 'top', 2), /gap|terminal/);
}));
test('enumerates child trie pointers and captures child contents with either RPC method', () => temporary(async (directory) => {
  const child = '0x' + Buffer.from(':child_storage:default:example').toString('hex');
  const root = '0x' + '1'.repeat(64);
  const top = await captureTrie(fakeRpc([[child, root]]), directory, 'top', 1);
  assert.deepEqual(top.childTries, [{ key: child, root }]);
  const entries = [['0x01', '0xaa'], ['0x02', '0xbb']];
  const childResult = await captureTrie(fakeRpc(entries, { child: true }), directory, child, 1);
  assert.equal(childResult.keyCount, 2);
  await temporary(async (fallbackDirectory) => {
    const fallback = fakeRpc(entries, { child: true });
    assert.equal((await captureTrie(fallback, fallbackDirectory, child, 2, { readChildEntries: false })).keyCount, 2);
    assert.ok(fallback.calls.some(({ method }) => method === 'childstate_getStorage'));
  });
}));
test('unknown child trie kinds and missing child values cannot be silently skipped', () => temporary(async (directory) => {
  const child = '0x' + Buffer.from(':child_storage:unknown:example').toString('hex');
  await assert.rejects(() => captureTrie(fakeRpc([[child, '0x' + '1'.repeat(64)]]), directory, 'top', 1), /Unsupported child/);
  const proper = '0x' + Buffer.from(':child_storage:default:example').toString('hex');
  const rpc = { async call(method) { return method.endsWith('getKeysPaged') ? ['0x01'] : [null]; } };
  await assert.rejects(() => captureTrie(rpc, directory, proper, 1), /child storage-value/);
}));
test('atomic artifact writes refuse to overwrite previously completed evidence', () => temporary(async (directory) => {
  const path = join(directory, 'file');
  await writeAtomic(path, Buffer.from('original'));
  await assert.rejects(() => writeAtomic(path, Buffer.from('changed')), /overwrite/);
  assert.equal(await readFile(path, 'utf8'), 'original');
}));
test('checkpoint mismatch produces no completion claim and releases its lock', () => temporary(async (directory) => {
  const rpc = { async call(method) {
    if (method === 'chain_getBlockHash') return '0x' + '0'.repeat(64);
    return {};
  } };
  await assert.rejects(() => capture({ output: directory, endpoint: 'ws://127.0.0.1:1', rpc }), /header identity/);
  assert.ok(!(await readdir(directory)).includes('capture-complete.json'));
  assert.ok(!(await readdir(directory)).includes('.capture.lock'));
  await assert.rejects(() => capture({ output: directory, endpoint: 'ws://127.0.0.1:2', rpc }), /immutable artifact differs/);
}));
test('page decoding verifies the declared count, bound checkpoint and terminal marker', () => {
  const header = { schema: 'sora-nexus-checkpoint-page-v1', checkpointHash: CHECKPOINT.hash, trie: 'top',
    pageIndex: 0, startKey: null, pageSize: 2, keyCount: 1, lastKey: '0x01', terminal: false };
  const encode = (value) => gzipSync(Buffer.from(JSON.stringify(value) + '\n' + JSON.stringify(['0x01', '0xaa']) + '\n'));
  const expected = { trie: 'top', pageIndex: 0, startKey: null, pageSize: 2 };
  assert.equal(decodePage(encode(header), expected).entries.length, 1);
  for (const bad of [{ ...header, keyCount: 0 }, { ...header, terminal: true }, { ...header, checkpointHash: '0x00' }]) {
    assert.throws(() => decodePage(encode(bad), expected));
  }
});

test('historical anchors cannot replace checkpoint or capture abandoned state', () => {
  assert.equal(validateAnchor(CHECKPOINT), CHECKPOINT);
  assert.throws(() => validateAnchor({ ...CHECKPOINT, number: CHECKPOINT.number + 1 }));
  assert.throws(() => validateAnchor({ ...CHECKPOINT, hash: '0x' + '1'.repeat(64) }));
  assert.throws(() => validateAnchor({ ...CHECKPOINT, number: 0 }));
});
test('historical child fallback and page identity use the explicit ancestor', () => temporary(async (directory) => {
  const anchor = { ...CHECKPOINT, number: CHECKPOINT.number - 1, hash: CHECKPOINT.parentHash };
  const trie = '0x' + Buffer.from(':child_storage:default:history').toString('hex');
  const rpc = { async call(method, params) {
    assert.equal(params.at(-1), anchor.hash);
    if (method === 'childstate_getKeysPaged') return params[3] === null ? ['0x01'] : [];
    assert.equal(method, 'childstate_getStorage');
    return '0xaa';
  } };
  const result = await captureTrie(rpc, directory, trie, 2, { anchor, readChildEntries: false });
  assert.equal(result.keyCount, 1);
  const bytes = await readFile(join(directory, result.pages[0].file));
  assert.equal(decodePage(bytes, { trie, pageIndex: 0, startKey: null, pageSize: 2 }, anchor).entries.length, 1);
  assert.throws(() => decodePage(bytes, { trie, pageIndex: 0, startKey: null, pageSize: 2 }));
}));

test('read batches reject mixed mutation requests and oversized work before connecting', async () => {
  const rpc = new ReadOnlyRpc('ws://localhost:9944');
  await assert.rejects(() => rpc.batch([{method:'chain_getHeader',params:[]}, {method:'author_submitExtrinsic',params:['0x']}]), /Non-read-only/);
  await assert.rejects(() => rpc.batch(Array.from({length:129}, () => ({method:'chain_getHeader',params:[]}))), /1\.\.128/);
  assert.equal(rpc.socket, null);
});

test('RPC batches correlate out-of-order responses without changing requested order', async () => {
  const original = globalThis.WebSocket;
  class MockSocket extends EventTarget {
    static OPEN = 1;
    readyState = 1;
    constructor() { super(); queueMicrotask(() => this.dispatchEvent(new Event('open'))); }
    send(text) {
      const requests = JSON.parse(text);
      queueMicrotask(() => this.dispatchEvent(new MessageEvent('message', {data:JSON.stringify(requests.reverse().map(({id,params}) => ({jsonrpc:'2.0',id,result:params[0]})))})));
    }
    close() { this.dispatchEvent(new Event('close')); }
  }
  globalThis.WebSocket = MockSocket;
  const rpc = new ReadOnlyRpc('ws://localhost:9944');
  try {
    assert.deepEqual(await rpc.batch([{method:'chain_getHeader',params:['first']}, {method:'chain_getHeader',params:['second']}]), ['first','second']);
    assert.equal(rpc.pending.size, 0);
  } finally { rpc.close(); globalThis.WebSocket = original; }
});
