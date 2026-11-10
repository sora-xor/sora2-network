#!/usr/bin/env node
/** Choose replay boundaries from observed chain transitions, never a fixed short window. */
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { CHECKPOINT, DEFAULT_ENDPOINT, ReadOnlyRpc, writeAtomic, sha256 } from './capture_checkpoint.mjs';

const args = process.argv.slice(2), options = {};
for (let i = 0; i < args.length; i += 2) options[args[i].slice(2)] = args[i + 1];
if (!options.dependencies || !options.output) throw new Error('--dependencies PACKAGE_JSON --output PLAN.json required');
const require = createRequire(resolve(options.dependencies));
const { xxhashAsU8a } = require('@polkadot/util-crypto');
const rpc = new ReadOnlyRpc(options.endpoint ?? DEFAULT_ENDPOINT);
const observations = [];
const storageKey = (pallet, name) => '0x' + Buffer.concat([xxhashAsU8a(pallet, 128), xxhashAsU8a(name, 128)]).toString('hex');
const cache = new Map();
async function observe(kind, number) {
  const id = `${kind}:${number}`;
  if (cache.has(id)) return cache.get(id);
  const hash = await rpc.call('chain_getBlockHash', [number]);
  if (!hash) throw new Error('Archive omitted a retained block');
  let value;
  if (kind === 'runtime') value = (await rpc.call('state_getRuntimeVersion', [hash])).specVersion;
  else {
    const key = kind === 'session' ? storageKey('Session', 'CurrentIndex') : storageKey('Babe', 'EpochIndex');
    const encoded = await rpc.call('state_getStorage', [key, hash]);
    const size = kind === 'session' ? 4 : 8;
    if (typeof encoded !== 'string' || encoded.length !== 2 + size * 2) throw new Error(`Unavailable ${kind} counter`);
    const bytes = Buffer.from(encoded.slice(2), 'hex');
    value = kind === 'session' ? bytes.readUInt32LE() : Number(bytes.readBigUInt64LE());
  }
  if (!Number.isSafeInteger(value)) throw new Error('Invalid historical counter');
  cache.set(id, value); observations.push({ kind, number, hash, value });
  return value;
}
async function latestTransition(kind) {
  const value = await observe(kind, CHECKPOINT.number);
  let high = CHECKPOINT.number, step = 1, low;
  for (;;) {
    low = Math.max(1, CHECKPOINT.number - step);
    const prior = await observe(kind, low);
    if (prior !== value) break;
    high = low;
    if (low === 1) throw new Error(`No ${kind} transition found in retained history`);
    step *= 2;
  }
  while (high - low > 1) {
    const middle = Math.floor((high + low) / 2);
    if (await observe(kind, middle) === value) high = middle;
    else low = middle;
  }
  const before = await observe(kind, high - 1), after = await observe(kind, high);
  if (before === after || after !== value) throw new Error('Transition bracket invalid');
  return { first: high, before, after };
}
try {
  if (await rpc.call('chain_getBlockHash', [CHECKPOINT.number]) !== CHECKPOINT.hash) throw new Error('Wrong retained checkpoint');
  const session = await latestTransition('session');
  const epoch = await latestTransition('epoch');
  const upgrade = await latestTransition('runtime');
  if (upgrade.after !== 134 || upgrade.first >= CHECKPOINT.number) throw new Error('Cannot exercise deployed spec134 after its activation');
  const ranges = [{ name: 'checkpoint', first: CHECKPOINT.number, last: CHECKPOINT.number, boundaries: ['checkpoint'],
    rationale: 'Exact retained checkpoint block exercises the substitution cutoff and matches its pinned post-state root.' }];
  const add = (name, transition, boundary) => {
    const existing = ranges.find((r) => r.first === transition.first);
    if (existing) { existing.boundaries.push(boundary); existing.rationale += ` Observed ${boundary} transition ${transition.before} to ${transition.after}.`; }
    else ranges.push({ name, first: transition.first, last: Math.min(CHECKPOINT.number, transition.first + 1), boundaries: [boundary],
      rationale: `Latest retained ${boundary} transition observed in archive state: ${transition.before} to ${transition.after}; includes the first following block.` });
  };
  add('latest-session', session, 'session'); add('latest-epoch', epoch, 'epoch'); add('spec-134-upgrade', upgrade, 'upgrade');
  const observationBytes = Buffer.from(JSON.stringify(observations, null, 2) + '\n');
  await writeAtomic(resolve(options.output) + '.observations.json', observationBytes);
  const plan = { schema: 'sora-nexus-replay-range-plan-v1', ranges, discovery: { checkpointHash: CHECKPOINT.hash,
    observationsSha256: sha256(observationBytes), session, epoch, upgrade,
    note: 'RPC observations select ranges only. Parent trie reconstruction and exact Wasm execution independently verify transitions.' } };
  await writeAtomic(resolve(options.output), Buffer.from(JSON.stringify(plan, null, 2) + '\n'));
  console.log(JSON.stringify({ output: resolve(options.output), session, epoch, upgrade, ranges }));
} finally { rpc.close(); }
