#!/usr/bin/env node
/** Offline, complete-capture diagnostic input. Never grants replacement authority. */
import {readFile, writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {gunzipSync} from 'node:zlib';
import {createRequire} from 'node:module';
import {resolve, join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {CHECKPOINT} from './capture_checkpoint.mjs';

const assert = (ok, message) => { if (!ok) throw new Error(message); };
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const hx = bytes => `0x${Buffer.from(bytes).toString('hex')}`;
const bytes = text => {
  assert(/^0x(?:[0-9a-f]{2})*$/.test(text), 'noncanonical hexadecimal');
  return Buffer.from(text.slice(2), 'hex');
};

export async function run({capture, dependencies, output}) {
  assert(capture && dependencies && output, '--capture --dependencies PACKAGE_JSON --output required');
  const require = createRequire(resolve(dependencies));
  const {Metadata, TypeRegistry} = require('@polkadot/types');
  const {blake2AsU8a, xxhashAsU8a} = require('@polkadot/util-crypto');
  const blake = value => hx(blake2AsU8a(value, 256));
  const load = async file => { const raw = await readFile(join(capture, file)); return {value: JSON.parse(raw), sha256: sha(raw)}; };
  const manifest = await load('capture-complete.json');
  const verification = await load('../checkpoint-verification.json');
  const witness = await load('../checkpoint-metadata-witness.json');
  assert(manifest.value.complete && manifest.value.checkpoint.hash === CHECKPOINT.hash &&
    verification.value.checkpointHash === CHECKPOINT.hash && verification.value.stateRootVerified &&
    verification.value.headerHashVerified && verification.value.stateRoot === CHECKPOINT.stateRoot &&
    verification.value.captureManifestSha256 === manifest.sha256, 'verified exact checkpoint required');
  const metadataBytes = await readFile(join(capture, 'runtime-metadata.scale'));
  assert(witness.value.wasmExecutionVerified && witness.value.checkpointHash === CHECKPOINT.hash &&
    witness.value.stateRoot === CHECKPOINT.stateRoot && witness.value.metadataSha256 === sha(metadataBytes) &&
    manifest.value.artifacts['runtime-metadata.scale'].sha256 === sha(metadataBytes), 'executed metadata mismatch');
  const registry = new TypeRegistry();
  registry.register(require('@sora-substrate/type-definitions').types);
  registry.setMetadata(new Metadata(registry, metadataBytes));
  const metadata = registry.metadata.toJSON();
  const descriptors = new Map();
  for (const pallet of metadata.pallets) {
    if (!pallet.storage) continue;
    for (const entry of pallet.storage.items) {
      const name = `${pallet.name}.${entry.name}`;
      if (!['BridgeProxy.Senders', 'BridgeProxy.Transactions'].includes(name)) continue;
      assert(entry.type.map?.hashers.length === 2 && entry.type.map.hashers[1] === 'Blake2_128Concat' && entry.type.map.hashers[0] === (name.endsWith('Senders') ? 'Twox64Concat' : 'Blake2_128Concat'), 'unreviewed bridge map hashers');
      const prefix = hx(Buffer.concat([xxhashAsU8a(pallet.storage.prefix, 128), xxhashAsU8a(entry.name, 128)]));
      const keyTypes = metadata.lookup.types.find(t => t.id === entry.type.map.key).type.def.tuple;
      descriptors.set(name, {name, prefix, entry, keyTypes});
    }
  }
  assert(descriptors.size === 2, 'missing BridgeProxy metadata');
  const records = [], counts = {}, pages = [];
  let cursor = null, count = 0, terminal = false;
  // Read all pages; index range hints cannot establish inventory completeness.
  assert(manifest.value.top.pages.length === manifest.value.top.pageCount, 'page count mismatch');
  for (const [index, page] of manifest.value.top.pages.entries()) {
    assert(!terminal, 'page after terminal cursor');
    const path = resolve(capture, page.file);
    assert(path.startsWith(resolve(capture) + '/'), 'capture page path escape');
    const compressed = await readFile(path);
    assert(sha(compressed) === page.sha256, 'capture page checksum mismatch');
    const text = gunzipSync(compressed, {maxOutputLength: 256 * 1024 * 1024}).toString('utf8');
    const lines = text.trimEnd().split('\n');
    const header = JSON.parse(lines.shift());
    assert(header.schema === 'sora-nexus-checkpoint-page-v1' && header.checkpointHash === CHECKPOINT.hash &&
      header.trie === 'top' && header.pageIndex === index && header.startKey === cursor &&
      header.keyCount === page.keyCount && header.pageSize > 0 && header.pageSize <= 1000 &&
      header.keyCount <= header.pageSize, 'capture page identity/count/cursor mismatch');
    let first = null;
    for (const line of lines) {
      const pair = JSON.parse(line);
      assert(Array.isArray(pair) && pair.length === 2, 'malformed capture row');
      const [k, v] = pair;
      bytes(k); bytes(v);
      assert(cursor === null || cursor < k, 'unordered/duplicate capture key');
      first ??= k; cursor = k; count++;
      for (const descriptor of descriptors.values()) {
        if (!k.startsWith(descriptor.prefix)) continue;
        const types = descriptor.keyTypes;
        let tail = bytes('0x' + k.slice(descriptor.prefix.length));
        for (const [position, hasher] of descriptor.entry.type.map.hashers.entries()) {
          const size = hasher === 'Twox64Concat' ? 8 : 16;
          const codec = registry.createTypeUnsafe(`Lookup${types[position]}`, [tail.subarray(size)]);
          const encoded = codec.toU8a();
          const digest = hasher === 'Twox64Concat' ? xxhashAsU8a(encoded, 64) : blake2AsU8a(encoded, 128);
          assert(tail.subarray(0, size).equals(Buffer.from(digest)) && tail.subarray(size, size + encoded.length).equals(Buffer.from(encoded)), 'noncanonical bridge key');
          tail = tail.subarray(size + encoded.length);
        }
        assert(tail.length === 0, 'trailing bridge key bytes');
        const codec = registry.createTypeUnsafe(`Lookup${descriptor.entry.type.map.value}`, [bytes(v)]);
        assert(hx(codec.toU8a()) === v, 'noncanonical bridge value');
        if (descriptor.name.endsWith('Transactions')) {
          const value = codec.toJSON(), label = value.direction + ':' + value.status;
          counts[label] = (counts[label] ?? 0) + 1;
        }
        records.push({storage: descriptor.name, key: k, valueHash: blake(bytes(v))});
      }
    }
    assert(lines.length === page.keyCount && header.lastKey === cursor && header.terminal === (lines.length === 0), 'incomplete page coverage');
    if (page.keyCount > 0) assert(page.firstKey === first && page.lastKey === cursor, 'page range hints contradict actual rows');
    terminal = header.terminal;
    pages.push({file: page.file, sha256: page.sha256, keyCount: page.keyCount});
  }
  assert(terminal && count === manifest.value.top.keyCount && count === verification.value.topKeys, 'incomplete top trie capture');
  records.sort((a, b) => a.key < b.key ? -1 : a.key > b.key ? 1 : 0);
  assert(new Set(records.map(record => record.key)).size === records.length, 'duplicate diagnostic review row');
  const evidence = {
    schema: 'sora-nexus-PUBLIC-FIXTURE-ONLY-bridge-history-evidence-v2',
    checkpointHash: CHECKPOINT.hash, stateRoot: CHECKPOINT.stateRoot,
    captureManifestSha256: manifest.sha256, stateVerificationSha256: verification.sha256,
    metadataWitnessSha256: witness.sha256, metadataSha256: sha(metadataBytes), metadataBlake2: blake(metadataBytes),
    executingCodeBlake2: witness.value.executingCodeBlake2,
    generatorSha256: sha(await readFile(new URL(import.meta.url))),
    compilerSourceSha256: sha(await readFile(new URL('../../utils/incident-recovery/src/bridge_history.rs', import.meta.url))),
    recordsBlake2: blake(Buffer.from(JSON.stringify(records))),
    topKeysChecked: count, allPageChecksumsAndCursorCoverageVerified: true, pages, counts,
    cancelAllTerminalOutgoing: true,
    purpose: 'Diagnostic review input for native compilation; no victim mapping or release approval',
    fixtureOnly: true, productionReady: false, releaseApproval: false,
  };
  const evidenceBytes = Buffer.from(JSON.stringify(evidence, null, 2) + '\n');
  const evidenceFile = output.replace(/-review\.json$/, '-evidence.json');
  assert(evidenceFile !== output, '--output must end in -review.json');
  const review = {evidenceHash: blake(evidenceBytes), cancelAllTerminalOutgoing: true, records};
  await writeFile(evidenceFile, evidenceBytes, {flag: 'wx'});
  await writeFile(output, JSON.stringify(review, null, 2) + '\n', {flag: 'wx'});
  return {review: output, evidence: evidenceFile, evidenceHash: review.evidenceHash,
    recordCount: records.length, counts,
    topKeysChecked: count, productionReady: false};
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const args = {};
  for (let i = 2; i < process.argv.length; i += 2) {
    assert(process.argv[i].startsWith('--') && process.argv[i + 1], 'invalid arguments');
    args[process.argv[i].slice(2)] = process.argv[i + 1];
  }
  console.log(JSON.stringify(await run(args)));
}
