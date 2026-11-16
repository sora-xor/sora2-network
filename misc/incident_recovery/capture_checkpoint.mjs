#!/usr/bin/env node
/** Read-only, resumable capture. Completeness is not a claim of independent trie-root verification. */
import { mkdir, readFile, readdir, rename, open, unlink, stat } from 'node:fs/promises';
import { resolve, join, dirname, relative } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
import { gzipSync, gunzipSync } from 'node:zlib';

export const CHECKPOINT = Object.freeze({
  number: 28_191_817,
  hash: '0x7f7c253fa72f32e7b03f32bdc9ae456fd7174fb131b689be6d876006124129f0',
  parentHash: '0x60b63e40ce4de9b9c1408bf7455a6f5ced796825f2437a0ee8496476d80c56d6',
  stateRoot: '0x79bda41a1db53470463653f358890f2d1fca8ad622c33a1d7095294b0b140452',
  extrinsicsRoot: '0x4a24fdf92274ca78ef10c57956d642b664287780b241a06fe45b68d2d11faa7d',
  runtimeCodeSha256: 'de7173a9e0137a32265b341354383958d85beef81103571e6b0aa09622ef2221',
});
export const DEFAULT_ENDPOINT = 'wss://mof2.sora.org';
const CHILD_PREFIX = '0x' + Buffer.from(':child_storage:').toString('hex');
const DEFAULT_CHILD_PREFIX = '0x' + Buffer.from(':child_storage:default:').toString('hex');
const CODE_KEY = '0x3a636f6465';
const SCHEMA = 'sora-nexus-checkpoint-capture-v1';
const PAGE_SCHEMA = 'sora-nexus-checkpoint-page-v1';
export const MAX_PAGE_BYTES = 256 * 1024 * 1024;
const READ_METHODS = new Set(['chain_getBlockHash', 'chain_getHeader', 'chain_getBlock',
  'state_getRuntimeVersion', 'state_getMetadata', 'state_getStorage', 'state_getKeysPaged',
  'state_queryStorageAt', 'state_getReadProof', 'childstate_getKeysPaged', 'childstate_getStorageEntries', 'childstate_getStorage', 'rpc_methods']);
export const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
const jsonBytes = (value) => Buffer.from(JSON.stringify(value, null, 2) + '\n');
const isHex = (value) => typeof value === 'string' && /^0x(?:[0-9a-f]{2})*$/.test(value);
function assert(condition, message) { if (!condition) throw new Error(message); }

/** No redirects, credentials, alternate hosts or provider fallbacks. */
export function validateEndpoint(endpoint) {
  const url = new URL(endpoint);
  const loopback = ['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname);
  assert(!url.username && !url.password && !url.search && !url.hash && url.pathname === '/', 'Endpoint must be a bare approved WebSocket origin');
  assert((url.protocol === 'wss:' && url.hostname === 'mof2.sora.org' && (!url.port || url.port === '443')) ||
    (loopback && ['ws:', 'wss:'].includes(url.protocol)), 'Only the approved mof2 endpoint or loopback is permitted');
  return url.href;
}
export class ReadOnlyRpc {
  constructor(endpoint, { timeoutMs = 30_000 } = {}) {
    this.endpoint = validateEndpoint(endpoint);
    this.timeoutMs = timeoutMs;
    this.pending = new Map();
    this.nextId = 1;
    this.socket = null;
    this.connecting = null;
  }
  async connect() {
    if (this.socket?.readyState === WebSocket.OPEN) return;
    if (this.connecting) return this.connecting;
    this.connecting = new Promise((resolveConnect, rejectConnect) => {
      const socket = new WebSocket(this.endpoint);
      this.socket = socket;
      const timer = setTimeout(() => { socket.close(); rejectConnect(new Error('RPC connection timeout')); }, this.timeoutMs);
      socket.addEventListener('open', () => { clearTimeout(timer); resolveConnect(); }, { once: true });
      socket.addEventListener('error', () => { clearTimeout(timer); rejectConnect(new Error('RPC connection failed')); }, { once: true });
      socket.addEventListener('message', (event) => {
        try {
          assert(typeof event.data === 'string' && event.data.length <= 64 * 1024 * 1024, 'RPC response exceeds safety bound');
          const decoded = JSON.parse(event.data);
          const responses = Array.isArray(decoded) ? decoded : [decoded];
          assert(responses.length > 0 && responses.length <= 128, 'Invalid RPC response batch');
          for (const response of responses) {
            const request = this.pending.get(response.id);
            if (!request) continue;
            this.pending.delete(response.id);
            clearTimeout(request.timer);
            if (response.error) {
              const error = new Error(`${request.method}: ${response.error.message ?? 'RPC error'}`);
              error.code = response.error.code;
              request.reject(error);
            } else { request.resolve(response.result); }
          }
        } catch (error) { this.failPending(error); socket.close(); }
      });
      socket.addEventListener('close', () => {
        clearTimeout(timer);
        this.failPending(new Error('RPC disconnected'));
        if (this.socket === socket) this.socket = null;
      });
    }).finally(() => { this.connecting = null; });
    return this.connecting;
  }
  failPending(error) {
    for (const request of this.pending.values()) { clearTimeout(request.timer); request.reject(error); }
    this.pending.clear();
  }
  async call(method, params) {
    assert(READ_METHODS.has(method), `Non-read-only method rejected: ${method}`);
    await this.connect();
    return new Promise((resolveRequest, rejectRequest) => {
      const id = this.nextId++;
      const timer = setTimeout(() => {
        this.pending.delete(id);
        rejectRequest(new Error(`${method}: RPC timeout`));
      }, this.timeoutMs);
      this.pending.set(id, { resolve: resolveRequest, reject: rejectRequest, timer, method });
      this.socket.send(JSON.stringify({ jsonrpc: '2.0', id, method, params }));
    });
  }
  /** At most 128 lightweight reads per wire batch; callers await each batch. */
  async batch(requests) {
    assert(Array.isArray(requests) && requests.length > 0 && requests.length <= 128, 'RPC batch must contain 1..128 reads');
    for (const request of requests) assert(READ_METHODS.has(request.method), `Non-read-only method rejected: ${request.method}`);
    await this.connect();
    const messages = [];
    const promises = requests.map(({ method, params }) => new Promise((resolveRequest, rejectRequest) => {
      const id = this.nextId++;
      const timer = setTimeout(() => {
        this.pending.delete(id); rejectRequest(new Error(`${method}: RPC timeout`));
      }, this.timeoutMs);
      this.pending.set(id, { resolve: resolveRequest, reject: rejectRequest, timer, method });
      messages.push({ jsonrpc: '2.0', id, method, params });
    }));
    const result = Promise.all(promises);
    try { this.socket.send(JSON.stringify(messages)); } catch (error) { this.failPending(error); }
    return result;
  }
  close() { this.failPending(new Error('RPC closed')); this.socket?.close(); }
}
async function exists(path) { try { await stat(path); return true; } catch (error) { if (error.code === 'ENOENT') return false; throw error; } }
/** A completed file is immutable. Interrupted temporary files are never resumed as pages. */
export async function writeAtomic(path, bytes) {
  await mkdir(dirname(path), { recursive: true });
  assert(!await exists(path), `Refusing to overwrite completed artifact: ${path}`);
  const temporary = `${path}.partial-${process.pid}-${Date.now()}`;
  const handle = await open(temporary, 'wx', 0o600);
  try { await handle.writeFile(bytes); await handle.sync(); } finally { await handle.close(); }
  await rename(temporary, path);
  const directory = await open(dirname(path), 'r');
  try { await directory.sync(); } finally { await directory.close(); }
}
async function writeOrVerify(path, bytes) {
  if (await exists(path)) assert((await readFile(path)).equals(bytes), `Existing immutable artifact differs: ${path}`);
  else await writeAtomic(path, bytes);
}
export function validateKeys(keys, startKey, pageSize) {
  assert(Array.isArray(keys) && keys.length <= pageSize, 'Invalid storage-key page length');
  let previous = startKey;
  for (const key of keys) {
    assert(isHex(key), 'Noncanonical storage key');
    assert(previous === null || key > previous, 'Storage keys must be strictly increasing and beyond cursor');
    previous = key;
  }
  return keys;
}
export function validateTopChanges(response, keys, anchor = CHECKPOINT) {
  assert(Array.isArray(response) && response.length === 1 && response[0].block === anchor.hash,
    'Storage changes are not bound to the checkpoint');
  const changes = response[0].changes;
  assert(Array.isArray(changes) && changes.length === keys.length, 'Incomplete storage-value coverage');
  const values = new Map();
  const expected = new Set(keys);
  for (const pair of changes) {
    assert(Array.isArray(pair) && pair.length === 2 && expected.has(pair[0]) && !values.has(pair[0]) && isHex(pair[1]),
      'Unexpected, missing, duplicate or absent storage value');
    values.set(pair[0], pair[1]);
  }
  return keys.map((key) => [key, values.get(key)]);
}
/** Large historical preimages can exceed the archive's response cap in a batch. */
export async function readTopEntries(rpc, keys, anchor = CHECKPOINT) {
  let response;
  try { response = await rpc.call('state_queryStorageAt', [keys, anchor.hash]); }
  catch (error) {
    if (keys.length < 2 || !/response is too (big|large)/i.test(error.message)) throw error;
    const split = Math.floor(keys.length / 2);
    // Sequential subdivision keeps the original four-group concurrency bound.
    const left = await readTopEntries(rpc, keys.slice(0, split), anchor);
    return left.concat(await readTopEntries(rpc, keys.slice(split), anchor));
  }
  return validateTopChanges(response, keys, anchor);
}
async function readChildValues(rpc, trie, keys, anchor = CHECKPOINT) {
  const values = [];
  for (let offset = 0; offset < keys.length; offset += 8) {
    values.push(...await Promise.all(keys.slice(offset, offset + 8).map((key) =>
      rpc.call('childstate_getStorage', [trie, key, anchor.hash]))));
  }
  return values;
}
export function decodePage(bytes, { trie, pageIndex, startKey, pageSize }, anchor = CHECKPOINT) {
  const lines = gunzipSync(bytes, { maxOutputLength: MAX_PAGE_BYTES }).toString('utf8').trimEnd().split('\n');
  const header = JSON.parse(lines.shift());
  assert(header.schema === PAGE_SCHEMA && header.checkpointHash === anchor.hash && header.trie === trie &&
    header.pageIndex === pageIndex && header.startKey === startKey && header.pageSize === pageSize, 'Page identity/cursor mismatch');
  const entries = lines.map((line) => JSON.parse(line));
  assert(entries.length === header.keyCount, 'Page count mismatch');
  for (const pair of entries) assert(Array.isArray(pair) && pair.length === 2 && isHex(pair[1]), 'Invalid saved storage entry');
  const keys = entries.map(([key]) => key);
  validateKeys(keys, startKey, pageSize);
  assert(header.lastKey === (keys.at(-1) ?? startKey) && header.terminal === (keys.length === 0), 'Page cursor/terminal mismatch');
  return { header, entries };
}
export async function captureTrie(rpc, output, trie, pageSize, { onProgress = () => {}, readChildEntries = true, anchor = CHECKPOINT } = {}) {
  const directoryName = trie === 'top' ? 'top' : `child-${sha256(Buffer.from(trie))}`;
  const directory = join(output, 'pages', directoryName);
  await mkdir(directory, { recursive: true });
  const files = (await readdir(directory)).filter((name) => /^\d{8}\.jsonl\.gz$/.test(name)).sort();
  const summary = { trie, directory: relative(output, directory), keyCount: 0, pageCount: 0, pages: [], childTries: [], codeSha256: null };
  let startKey = null;
  let terminal = false;
  function addPage(bytes, page, filename) {
    for (const [key, value] of page.entries) {
      if (trie === 'top' && key.startsWith(CHILD_PREFIX)) {
        assert(key.startsWith(DEFAULT_CHILD_PREFIX), 'Unsupported child-trie kind: capture cannot claim completeness');
        assert(value.length === 66, 'Child trie root is not a 32-byte hash');
        summary.childTries.push({ key, root: value });
      }
      if (trie === 'top' && key === CODE_KEY) summary.codeSha256 = sha256(Buffer.from(value.slice(2), 'hex'));
    }
    summary.keyCount += page.entries.length;
    summary.pageCount += 1;
    summary.pages.push({ file: relative(output, join(directory, filename)), sha256: sha256(bytes),
      keyCount: page.entries.length, firstKey: page.entries[0]?.[0] ?? null, lastKey: page.header.lastKey });
    startKey = page.header.lastKey;
    terminal = page.header.terminal;
  }
  for (let index = 0; index < files.length; index += 1) {
    assert(files[index] === `${String(index).padStart(8, '0')}.jsonl.gz`, 'Saved page sequence has a gap');
    assert(!terminal, 'Saved pages exist after terminal page');
    const bytes = await readFile(join(directory, files[index]));
    addPage(bytes, decodePage(bytes, { trie, pageIndex: index, startKey, pageSize }, anchor), files[index]);
  }
  while (!terminal) {
    const pageIndex = summary.pageCount;
    const keys = validateKeys(await rpc.call(trie === 'top' ? 'state_getKeysPaged' : 'childstate_getKeysPaged',
      trie === 'top' ? ['0x', pageSize, startKey, anchor.hash] : [trie, '0x', pageSize, startKey, anchor.hash]), startKey, pageSize);
    let entries = [];
    // Keep each value response bounded while allowing independent reads concurrently.
    for (let offset = 0; offset < keys.length; offset += 512) {
      const groups = [];
      for (let part = offset; part < Math.min(offset + 512, keys.length); part += 128) groups.push(keys.slice(part, Math.min(part + 128, offset + 512)));
      const responses = await Promise.all(groups.map(async (group) => {
        if (trie === 'top') return readTopEntries(rpc, group, anchor);
        const values = readChildEntries
          ? await rpc.call('childstate_getStorageEntries', [trie, group, anchor.hash])
          : await readChildValues(rpc, trie, group, anchor);
        assert(Array.isArray(values) && values.length === group.length && values.every(isHex), 'Incomplete child storage-value coverage');
        return group.map((key, index) => [key, values[index]]);
      }));
      entries.push(...responses.flat());
    }
    const header = { schema: PAGE_SCHEMA, checkpointHash: anchor.hash, trie, pageIndex, startKey,
      pageSize, keyCount: keys.length, lastKey: keys.at(-1) ?? startKey, terminal: keys.length === 0 };
    const uncompressed = Buffer.from([JSON.stringify(header), ...entries.map((entry) => JSON.stringify(entry))].join('\n') + '\n');
    assert(uncompressed.length <= MAX_PAGE_BYTES, 'Storage page exceeds 256 MiB uncompressed bound; capture is incomplete');
    const bytes = gzipSync(uncompressed);
    const filename = `${String(pageIndex).padStart(8, '0')}.jsonl.gz`;
    await writeAtomic(join(directory, filename), bytes);
    addPage(bytes, { header, entries }, filename);
    onProgress({ trie, pageCount: summary.pageCount, keyCount: summary.keyCount, lastKey: startKey, complete: terminal });
  }
  return summary;
}
async function acquireLock(output, anchor = CHECKPOINT) {
  const path = join(output, '.capture.lock');
  if (await exists(path)) {
    const lock = JSON.parse(await readFile(path, 'utf8'));
    assert(Number.isSafeInteger(lock.pid) && lock.pid > 0, 'Malformed capture lock; inspect manually');
    let alive = true;
    try { process.kill(lock.pid, 0); } catch (error) { if (error.code === 'ESRCH') alive = false; else throw error; }
    assert(!alive, 'Another capture process owns this directory');
    await unlink(path);
  }
  const handle = await open(path, 'wx', 0o600);
  try { await handle.writeFile(jsonBytes({ pid: process.pid, checkpointHash: anchor.hash })); await handle.sync(); }
  finally { await handle.close(); }
  return async () => { await unlink(path); };
}
export async function captureAt(anchor, { endpoint = DEFAULT_ENDPOINT, output, pageSize = 512, rpc, onProgress = () => {} }) {
  validateAnchor(anchor);
  endpoint = validateEndpoint(endpoint);
  assert(Number.isSafeInteger(pageSize) && pageSize >= 1 && pageSize <= 1000, 'Page size must be 1..1000');
  output = resolve(output);
  await mkdir(output, { recursive: true });
  const releaseLock = await acquireLock(output, anchor);
  const client = rpc ?? new ReadOnlyRpc(endpoint);
  const identity = { schema: SCHEMA, checkpoint: anchor, endpoint, pageSize };
  try {
    await writeOrVerify(join(output, 'capture-identity.json'), jsonBytes(identity));
    const [hash, header, parentHeader, block, version, metadata, code, methods] = await Promise.all([
      client.call('chain_getBlockHash', [anchor.number]),
      client.call('chain_getHeader', [anchor.hash]),
      client.call('chain_getHeader', [anchor.parentHash]),
      client.call('chain_getBlock', [anchor.hash]),
      client.call('state_getRuntimeVersion', [anchor.hash]),
      client.call('state_getMetadata', [anchor.hash]),
      client.call('state_getStorage', [CODE_KEY, anchor.hash]),
      client.call('rpc_methods', []),
    ]);
    assert(hash === anchor.hash && header?.parentHash === anchor.parentHash &&
      header.stateRoot === anchor.stateRoot && header.extrinsicsRoot === anchor.extrinsicsRoot &&
      BigInt(header.number) === BigInt(anchor.number), 'Checkpoint header identity mismatch');
    assert(BigInt(parentHeader.number) === BigInt(anchor.number - 1), 'Checkpoint parent number mismatch');
    assert(JSON.stringify(block?.block?.header) === JSON.stringify(header), 'Checkpoint block/header mismatch');
    assert(Number.isSafeInteger(version?.specVersion) && version.specVersion <= 134 &&
      (anchor.number !== CHECKPOINT.number || (version.specVersion === 134 && version.transactionVersion === 131)), 'Unexpected checkpoint runtime version');
    assert(isHex(metadata) && isHex(code), 'Missing runtime metadata/code');
    assert(sha256(Buffer.from(code.slice(2), 'hex')) === anchor.runtimeCodeSha256, 'Checkpoint runtime code hash mismatch');
    const artifacts = {
      'checkpoint-header.json': jsonBytes(header), 'parent-header.json': jsonBytes(parentHeader),
      'checkpoint-block.json': jsonBytes(block), 'runtime-version.json': jsonBytes(version),
      'runtime-metadata.scale': Buffer.from(metadata.slice(2), 'hex'),
      'runtime-code.wasm': Buffer.from(code.slice(2), 'hex'),
    };
    for (const [name, bytes] of Object.entries(artifacts)) await writeOrVerify(join(output, name), bytes);
    const top = await captureTrie(client, output, 'top', pageSize, { onProgress, anchor });
    assert(top.codeSha256 === anchor.runtimeCodeSha256, 'Enumerated top trie omits or changes runtime code');
    const children = [];
    if (top.childTries.length) assert(methods.methods.includes('childstate_getKeysPaged'), 'Paged child capture is unavailable; capture is incomplete');
    for (const child of top.childTries) children.push({ root: child.root, ...await captureTrie(client, output, child.key, pageSize,
      { onProgress, anchor, readChildEntries: methods.methods.includes('childstate_getStorageEntries') }) });
    const completion = {
      ...identity, complete: true, stateRootVerified: false, headerHashVerified: false,
      consensusProofVerified: false, historyComplete: false,
      verificationNote: 'All RPC pages captured; independent trie/header/consensus verification remains required.',
      artifacts: Object.fromEntries(Object.entries(artifacts).map(([name, bytes]) => [name, { sha256: sha256(bytes), bytes: bytes.length }])),
      top, children,
    };
    await writeOrVerify(join(output, 'capture-complete.json'), jsonBytes(completion));
    return completion;
  } finally {
    if (!rpc) client.close();
    await releaseLock();
  }
}
/** Generic ancestor capture; ancestry must be authenticated separately by the replay verifier. */
export function validateAnchor(anchor) {
  assert(anchor && Number.isSafeInteger(anchor.number) && anchor.number > 0 && anchor.number <= CHECKPOINT.number,
    'Historical capture must be a retained non-genesis block');
  for (const key of ['hash', 'parentHash', 'stateRoot', 'extrinsicsRoot']) {
    assert(typeof anchor[key] === 'string' && /^0x[0-9a-f]{64}$/.test(anchor[key]), `Invalid capture anchor ${key}`);
  }
  assert(typeof anchor.runtimeCodeSha256 === 'string' && /^[0-9a-f]{64}$/.test(anchor.runtimeCodeSha256), 'Invalid captured code hash');
  if (anchor.number === CHECKPOINT.number) {
    for (const key of Object.keys(CHECKPOINT)) assert(anchor[key] === CHECKPOINT[key], 'Checkpoint anchor cannot be replaced');
  }
  return anchor;
}
/** Existing callers remain pinned to the approved checkpoint. */
export async function capture(options) { return captureAt(CHECKPOINT, options); }
async function main() {
  const args = process.argv.slice(2);
  const options = { endpoint: DEFAULT_ENDPOINT, output: resolve(dirname(fileURLToPath(import.meta.url)), '../../artifacts/recovery-28191817/checkpoint'), pageSize: 512 };
  for (let index = 0; index < args.length; index += 2) {
    assert(['--endpoint', '--output', '--page-size'].includes(args[index]) && args[index + 1], 'Usage: capture_checkpoint.mjs [--endpoint URL] [--output DIR] [--page-size 1..1000]');
    if (args[index] === '--endpoint') options.endpoint = args[index + 1];
    if (args[index] === '--output') options.output = args[index + 1];
    if (args[index] === '--page-size') options.pageSize = Number(args[index + 1]);
  }
  let lastProgress = 0;
  const result = await capture({ ...options, onProgress(progress) {
    if (progress.complete || Date.now() - lastProgress >= 15_000) {
      lastProgress = Date.now(); console.log(JSON.stringify({ phase: 'capture', ...progress }));
    }
  } });
  console.log(JSON.stringify({ complete: true, output: options.output, topKeys: result.top.keyCount,
    children: result.children.length, stateRootVerified: result.stateRootVerified }));
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main().catch((error) => { console.error(`Capture stopped: ${error.message}`); process.exitCode = 1; });
}
