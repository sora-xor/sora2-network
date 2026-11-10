#!/usr/bin/env node
// Public diagnostic evidence only; this never supplies recovery approval.
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { readFileSync, statSync, mkdirSync, writeFileSync } from 'node:fs';
import { resolve, relative } from 'node:path';
import { createHash } from 'node:crypto';
import { gunzipSync } from 'node:zlib';
const repo = fileURLToPath(new URL('../../', import.meta.url));
const require = createRequire(resolve(repo, 'runtime-rehearsal-sdk2609-2026-10-25/validation/package.json'));
const { xxhashAsU8a, blake2AsHex } = require('@polkadot/util-crypto');
const [captureArgument, outputArgument] = process.argv.slice(2);
if (!captureArgument || !outputArgument || process.argv.length !== 4) {
  throw new Error('Usage: prepare_checkpoint_elections_review.mjs CAPTURE NEW_OUTPUT_DIRECTORY');
}
const capture = resolve(captureArgument), output = resolve(outputArgument);
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
const items = ['ElectionsPhragmen.Members', 'ElectionsPhragmen.RunnersUp',
  'ElectionsPhragmen.Candidates', 'ElectionsPhragmen.ElectionRounds', 'ElectionsPhragmen.Voting'];
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
    if (records.length > 10004) throw new Error('Election inventory exceeds bound');
  }
}
for (const [prefix, storage] of prefixes) {
  if (counts[storage] === 0) records.push({ storage, key: prefix, valueHash: null });
}
records.sort((a, b) => a.key.localeCompare(b.key));
const review = { evidenceHash: blake2AsHex(Buffer.from(JSON.stringify({
  domain: 'sora-nexus/public-fixture-elections-review/v1', checkpoint, records,
}))), records };
mkdirSync(output); // Do not overwrite a prior diagnostic/review.
writeFileSync(resolve(output, 'review.json'), JSON.stringify(review, null, 2) + '\n', { flag: 'wx' });
writeFileSync(resolve(output, 'FIXTURE-PROVENANCE.json'), JSON.stringify({
  schema: 'sora-nexus-public-fixture-elections-review-v1', fixtureOnly: true,
  checkpoint, sourceCapture: capture,
  recoveryApprovalVerified: false, productionReady: false, counts,
  note: 'Structural empty-map diagnostic only. Native Rust capture verification and signed replacement/control review remain independent gates.',
}, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ review: resolve(output, 'review.json'),
  records: records.length, counts, fixtureOnly: true, recoveryApprovalVerified: false }));
