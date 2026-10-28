#!/usr/bin/env node
/** Read-only ancestor snapshots and exact replay inputs; only approved mof2/loopback. */
import { mkdir, readFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { CHECKPOINT, DEFAULT_ENDPOINT, ReadOnlyRpc, captureAt, sha256, writeAtomic } from './capture_checkpoint.mjs';

const json = (value) => Buffer.from(JSON.stringify(value, null, 2) + '\n');
const assert = (value, message) => { if (!value) throw new Error(message); };

async function immutable(path, bytes) {
  try {
    const old = await readFile(path);
    assert(old.equals(bytes), `Existing history evidence differs: ${path}`);
  } catch (error) { if (error.code !== 'ENOENT') throw error; await writeAtomic(path, bytes); }
}

export async function anchorAt(rpc, number) {
  assert(Number.isSafeInteger(number) && number > 0 && number < CHECKPOINT.number, 'Parent must precede retained checkpoint');
  const hash = await rpc.call('chain_getBlockHash', [number]);
  const [header, code] = await Promise.all([
    rpc.call('chain_getHeader', [hash]), rpc.call('state_getStorage', ['0x3a636f6465', hash]),
  ]);
  assert(Number(BigInt(header.number)) === number && /^0x(?:[0-9a-f]{2})+$/.test(code), 'Invalid historical identity/code');
  return { number, hash, parentHash: header.parentHash, stateRoot: header.stateRoot,
    extrinsicsRoot: header.extrinsicsRoot, runtimeCodeSha256: sha256(Buffer.from(code.slice(2), 'hex')) };
}

export async function captureParent({ number, output, endpoint = DEFAULT_ENDPOINT }) {
  const rpc = new ReadOnlyRpc(endpoint);
  try {
    const anchor = await anchorAt(rpc, number);
    let last = 0;
    return await captureAt(anchor, { output, endpoint, rpc, onProgress(progress) {
      if (progress.complete || Date.now() - last >= 15_000) {
        last = Date.now(); console.log(JSON.stringify({ phase: 'parent-capture', number, ...progress }));
      }
    } });
  } finally { rpc.close(); }
}

/** Download height-selected headers in bounded batches; Rust independently proves
 * every SCALE header hash links all the way to the fixed checkpoint. */
export async function captureAncestry(rpc, output, earliest) {
  assert(Number.isSafeInteger(earliest) && earliest > 0 && earliest < CHECKPOINT.number, 'Invalid ancestry start');
  const headers = [];
  let last = 0;
  for (let first = earliest; first <= CHECKPOINT.number; first += 128) {
    const numbers = Array.from({ length: Math.min(128, CHECKPOINT.number - first + 1) }, (_, i) => first + i);
    const batch = await Promise.all(numbers.map(async (number) => {
      try { return JSON.parse(await readFile(join(output, 'headers', `${number}.json`), 'utf8')); }
      catch (error) { if (error.code !== 'ENOENT') throw error; return null; }
    }));
    const missing = numbers.filter((_, i) => batch[i] === null);
    if (missing.length > 0) {
      const hashes = await rpc.call('chain_getBlockHash', [missing]);
      assert(Array.isArray(hashes) && hashes.length === missing.length && hashes.every((hash) => /^0x[0-9a-f]{64}$/.test(hash)),
        'Archive omitted ancestry hashes');
      const downloaded = await rpc.batch(hashes.map((hash) => ({ method: 'chain_getHeader', params: [hash] })));
      await Promise.all(missing.map(async (number, i) => {
        const header = downloaded[i];
        assert(header && Number(BigInt(header.number)) === number, 'Header ancestry number mismatch');
        if (number === CHECKPOINT.number) assert(hashes[i] === CHECKPOINT.hash && header.stateRoot === CHECKPOINT.stateRoot,
          'Wrong checkpoint identity');
        await immutable(join(output, 'headers', `${number}.json`), json(header));
        batch[number - first] = header;
      }));
    }
    for (let i = 0; i < batch.length; i += 1) assert(Number(BigInt(batch[i].number)) === numbers[i], 'Cached ancestry number mismatch');
    headers.push(...batch);
    if (Date.now() - last >= 15_000) {
      last = Date.now(); console.log(JSON.stringify({ phase: 'ancestry', through: numbers.at(-1), earliest, headers: headers.length }));
    }
  }
  assert(headers.at(-1).stateRoot === CHECKPOINT.stateRoot, 'Wrong cached checkpoint root');
  const bytes = json(headers);
  await immutable(join(output, 'ancestry.json'), bytes);
  return { file: 'ancestry.json', sha256: sha256(bytes) };
}

async function main() {
  const args = process.argv.slice(2);
  const options = { endpoint: DEFAULT_ENDPOINT };
  for (let index = 0; index < args.length; index += 2) {
    assert(['--endpoint', '--output', '--parent', '--plan', '--candidate', '--ancestry-start'].includes(args[index]) && args[index + 1],
      'Usage: capture_history.mjs --output DIR (--parent NUMBER | --plan PLAN.json --candidate WASM) [--endpoint URL]');
    options[args[index].slice(2)] = args[index + 1];
  }
  assert(options.output, 'Missing output directory');
  options.output = resolve(options.output);
  if (options.parent) {
    const result = await captureParent({ number: Number(options.parent), ...options });
    console.log(JSON.stringify({ phase: 'parent-capture-complete', output: options.output, topKeys: result.top.keyCount, stateRootVerified: false }));
    return;
  }
  if (options['ancestry-start']) {
    const rpc = new ReadOnlyRpc(options.endpoint);
    try { console.log(JSON.stringify(await captureAncestry(rpc, options.output, Number(options['ancestry-start'])))); }
    finally { rpc.close(); }
    return;
  }
  assert(options.plan && options.candidate, 'Replay preparation requires plan and exact candidate artifact');
  const plan = JSON.parse(await readFile(options.plan, 'utf8'));
  assert(plan.schema === 'sora-nexus-replay-range-plan-v1' && Array.isArray(plan.ranges) && plan.ranges.length > 0, 'Invalid range plan');
  const ranges = plan.ranges.map((range) => {
    assert(typeof range.name === 'string' && typeof range.rationale === 'string' && Array.isArray(range.boundaries) &&
      Number.isSafeInteger(range.first) && Number.isSafeInteger(range.last) && range.first > 1 && range.first <= range.last && range.last <= CHECKPOINT.number,
    'Invalid retained replay range');
    return { ...range, parent: range.first - 1 };
  });
  await mkdir(options.output, { recursive: true });
  await immutable(join(options.output, 'range-plan.json'), json(plan));
  const candidate = await readFile(resolve(options.candidate));
  assert(candidate.length > 0 && candidate.length <= 32 * 1024 * 1024, 'Invalid candidate size');
  await immutable(join(options.output, 'candidate.wasm'), candidate);
  const rpc = new ReadOnlyRpc(options.endpoint);
  try {
    const ancestry = await captureAncestry(rpc, options.output, Math.min(...ranges.map((r) => r.parent)));
    const completed = [];
    for (const range of ranges) {
      const parentCapture = `captures/${range.parent}`;
      await captureParent({ number: range.parent, output: join(options.output, parentCapture), endpoint: options.endpoint });
      const blocks = [];
      for (let number = range.first; number <= range.last; number += 1) {
        const hash = await rpc.call('chain_getBlockHash', [number]);
        const bytes = json(await rpc.call('chain_getBlock', [hash]));
        const file = `blocks/${number}.json`;
        await immutable(join(options.output, file), bytes);
        blocks.push({ file, sha256: sha256(bytes) });
      }
      completed.push({ name: range.name, rationale: range.rationale, boundaries: range.boundaries, parentCapture, blocks });
    }
    await immutable(join(options.output, 'replay-manifest.json'), json({ schema: 'sora-nexus-historical-replay-v1',
      checkpointHash: CHECKPOINT.hash, candidate: { file: 'candidate.wasm', sha256: sha256(candidate) }, ancestry, ranges: completed }));
    console.log(JSON.stringify({ complete: true, manifest: join(options.output, 'replay-manifest.json'), executionVerified: false }));
  } finally { rpc.close(); }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main().catch((error) => { console.error(`History capture stopped: ${error.message}`); process.exitCode = 1; });
}
