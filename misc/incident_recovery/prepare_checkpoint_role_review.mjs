#!/usr/bin/env node
// Public diagnostic evidence only; this never supplies operator approval.
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { readFileSync, statSync, mkdirSync, writeFileSync } from 'node:fs';
import { resolve, relative } from 'node:path';
import { createHash } from 'node:crypto';
import { gunzipSync } from 'node:zlib';
const repo = fileURLToPath(new URL('../../', import.meta.url));
const require = createRequire(resolve(repo, 'runtime-rehearsal-sdk2609-2026-10-25/validation/package.json'));
const { xxhashAsU8a, blake2AsHex } = require('@polkadot/util-crypto');
const [captureArgument, operatorsArgument, outputArgument] = process.argv.slice(2);
if (!captureArgument || !operatorsArgument || !outputArgument || process.argv.length !== 5) {
  throw new Error('Usage: prepare_checkpoint_role_review.mjs CAPTURE OPERATORS_SCALE NEW_OUTPUT_DIRECTORY');
}
const capture = resolve(captureArgument), operators = resolve(operatorsArgument), output = resolve(outputArgument);
const bounded = (path, maximum) => {
  if (statSync(path).size > maximum) throw new Error(`File exceeds bound: ${path}`);
  const bytes = readFileSync(path);
  if (bytes.length > maximum) throw new Error(`File grew beyond bound: ${path}`);
  return bytes;
};
const manifest = JSON.parse(bounded(resolve(capture, 'capture-complete.json'), 64 * 1024 ** 2));
const checkpoint = '0x7f7c253fa72f32e7b03f32bdc9ae456fd7174fb131b689be6d876006124129f0';
if (manifest.checkpoint?.hash !== checkpoint || manifest.checkpoint?.number !== 28191817 || !manifest.complete) {
  throw new Error('Wrong/incomplete checkpoint capture');
}
const items = ['Staking.Nominators', 'Staking.Validators', 'Staking.CounterForNominators',
  'Staking.CounterForValidators', 'BagsList.ListNodes', 'BagsList.ListBags',
  'BagsList.CounterForListNodes', 'BagsList.__STORAGE_VERSION__'];
const hx = value => Buffer.from(value).toString('hex');
const prefixes = new Map(items.map(name => {
  const [pallet, item] = name.split('.');
  return ['0x' + hx(xxhashAsU8a(pallet, 128)) + hx(xxhashAsU8a(item === '__STORAGE_VERSION__' ? ':__STORAGE_VERSION__:' : item, 128)), name];
}));
const records = [], seen = new Set(), counts = Object.fromEntries(items.map(item => [item, 0]));
for (const page of manifest.top.pages) {
  if (![...prefixes.keys()].some(prefix => page.lastKey >= prefix && page.firstKey <= prefix + 'f'.repeat(512))) continue;
  const path = resolve(capture, page.file);
  const rel = relative(capture, path);
  if (rel.startsWith('..') || rel.startsWith('/')) throw new Error('Capture path escapes its directory');
  const compressed = bounded(path, 64 * 1024 ** 2);
  if (createHash('sha256').update(compressed).digest('hex') !== page.sha256) throw new Error('Capture page digest mismatch');
  const lines = gunzipSync(compressed, { maxOutputLength: 64 * 1024 ** 2 }).toString().trim().split('\n');
  for (const line of lines.slice(1)) {
    const [key, value] = JSON.parse(line);
    const storage = prefixes.get(key.slice(0, 66));
    if (!storage) continue;
    if (!/^0x(?:[0-9a-f]{2})+$/.test(key) || !/^0x(?:[0-9a-f]{2})*$/.test(value) || seen.has(key)) throw new Error('Malformed/duplicate captured record');
    seen.add(key); counts[storage]++;
    records.push({ storage, key, valueHash: blake2AsHex(Buffer.from(value.slice(2), 'hex')) });
    if (records.length > 100000) throw new Error('Role inventory exceeds bound');
  }
}
if (Object.values(counts).some(count => count === 0)) throw new Error('Expected actual checkpoint role prefix missing');
records.sort((a, b) => a.key.localeCompare(b.key));
const operatorBytes = bounded(operators, 1024 ** 2), operatorsHash = blake2AsHex(operatorBytes);
const review = { evidenceHash: blake2AsHex(Buffer.from(JSON.stringify({
  domain: 'sora-nexus/public-fixture-role-review/v1', checkpoint, operatorsHash, records,
}))), operatorsHash, records, retainedNominations: [] };
mkdirSync(output); // Do not overwrite a prior diagnostic/review.
writeFileSync(resolve(output, 'review.json'), JSON.stringify(review, null, 2) + '\n', { flag: 'wx' });
writeFileSync(resolve(output, 'FIXTURE-PROVENANCE.json'), JSON.stringify({
  schema: 'sora-nexus-public-fixture-role-review-v1', fixtureOnly: true,
  checkpoint, sourceCapture: capture, operatorsFile: operators, operatorsHash,
  operatorApprovalVerified: false, productionReady: false, counts,
  note: 'Structural empty-map diagnostic only. Native Rust capture verification and operator approval remain independent gates.',
}, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ review: resolve(output, 'review.json'), operatorsFile: operators,
  records: records.length, counts, fixtureOnly: true, operatorApprovalVerified: false }));
