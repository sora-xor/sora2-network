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
  const descriptors = new Map();
  for (const pallet of registry.metadata.toJSON().pallets) {
    if (!pallet.storage) continue;
    for (const entry of pallet.storage.items) {
      const name = `${pallet.name}.${entry.name}`;
      if (!['Democracy.VotingOf', 'Democracy.ReferendumInfoOf'].includes(name)) continue;
      assert(entry.type.map?.hashers.length === 1 && entry.type.map.hashers[0] === 'Twox64Concat', 'unreviewed Democracy map hasher');
      const prefix = hx(Buffer.concat([xxhashAsU8a(pallet.storage.prefix, 128), xxhashAsU8a(entry.name, 128)]));
      descriptors.set(name, {name, prefix, entry, width: name.endsWith('VotingOf') ? 32 : 4});
    }
  }
  assert(descriptors.size === 2, 'missing Democracy metadata');
  const records = [], values = new Map(), pages = [];
  let cursor = null, count = 0, terminal = false;
  const key = (descriptor, suffix) => hx(Buffer.concat([bytes(descriptor.prefix), xxhashAsU8a(suffix, 64), suffix]));
  // Read every capture page, even those outside Democracy. Page-range hints are
  // not used as evidence of prefix completeness or of a missing referendum.
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
        const suffix = bytes(k.slice(0, 2) + k.slice(-descriptor.width * 2));
        assert(key(descriptor, suffix) === k, 'noncanonical Democracy map key');
        const codec = registry.createTypeUnsafe(`Lookup${descriptor.entry.type.map.value}`, [bytes(v)]);
        assert(hx(codec.toU8a()) === v, 'noncanonical Democracy value');
        const decoded = codec.toJSON();
        if (descriptor.name.endsWith('ReferendumInfoOf')) assert(decoded.finished && !decoded.ongoing, 'ongoing/unreviewed referendum');
        records.push({storage: descriptor.name, key: k, valueHash: blake(bytes(v))});
        values.set(k, decoded);
      }
    }
    assert(lines.length === page.keyCount && header.lastKey === cursor && header.terminal === (lines.length === 0), 'incomplete page coverage');
    if (page.keyCount > 0) assert(page.firstKey === first && page.lastKey === cursor, 'page range hints contradict actual rows');
    terminal = header.terminal;
    pages.push({file: page.file, sha256: page.sha256, keyCount: page.keyCount});
  }
  assert(terminal && count === manifest.value.top.keyCount && count === verification.value.topKeys, 'incomplete top trie capture');
  const voting = descriptors.get('Democracy.VotingOf'), referenda = descriptors.get('Democracy.ReferendumInfoOf');
  const missing = new Map();
  let direct = 0, delegating = 0, directVotes = 0, finished = 0;
  for (const [k, value] of values) {
    if (k.startsWith(referenda.prefix)) { finished++; continue; }
    assert(k.startsWith(voting.prefix), 'unknown diagnostic row');
    if (value.direct) {
      direct++; directVotes += value.direct.votes.length;
      for (const [index] of value.direct.votes) {
        const suffix = Buffer.alloc(4); suffix.writeUInt32LE(index);
        const k = key(referenda, suffix);
        if (!values.has(k)) missing.set(index, (missing.get(index) ?? 0) + 1);
      }
    } else { assert(value.delegating, 'unknown voting variant'); delegating++; }
  }
  for (const index of missing.keys()) {
    const suffix = Buffer.alloc(4); suffix.writeUInt32LE(index);
    records.push({storage: referenda.name, key: key(referenda, suffix), valueHash: null});
  }
  records.sort((a, b) => a.key < b.key ? -1 : a.key > b.key ? 1 : 0);
  assert(new Set(records.map(record => record.key)).size === records.length, 'duplicate diagnostic review row');
  const diagnosticPolicy = {
    schema: 'sora-nexus-empty-mapping-control-diagnostic-v1', checkpointHash: CHECKPOINT.hash,
    replacements: [], purpose: 'Read-only native checkpoint validation; no authorized state mutations',
    productionReady: false, releaseApproval: false,
  };
  const evidence = {
    schema: 'sora-nexus-checkpoint-democracy-diagnostic-evidence-v1',
    diagnosticPolicy, diagnosticPolicyCommitment: blake(Buffer.from(JSON.stringify(diagnosticPolicy))),
    checkpointHash: CHECKPOINT.hash, stateRoot: CHECKPOINT.stateRoot,
    captureManifestSha256: manifest.sha256, stateVerificationSha256: verification.sha256,
    metadataWitnessSha256: witness.sha256, metadataSha256: sha(metadataBytes), metadataBlake2: blake(metadataBytes),
    executingCodeBlake2: witness.value.executingCodeBlake2,
    generatorSha256: sha(await readFile(new URL(import.meta.url))),
    compilerSourceSha256: sha(await readFile(new URL('../../utils/incident-recovery/src/democracy.rs', import.meta.url))),
    recordsBlake2: blake(Buffer.from(JSON.stringify(records))),
    topKeysChecked: count, allPageChecksumsAndCursorCoverageVerified: true, pages,
    votingRows: direct + delegating, directRows: direct, delegatingRows: delegating, directVotes,
    finishedReferenda: finished, absentReferencedReferenda: missing.size,
    missingVoteCount: [...missing.values()].reduce((a, b) => a + b, 0),
    missingVoteIndices: Object.fromEntries([...missing.entries()].sort((a, b) => a[0] - b[0])),
    productionReady: false, releaseApproval: false,
  };
  const evidenceBytes = Buffer.from(JSON.stringify(evidence, null, 2) + '\n');
  const evidenceFile = output.replace(/-review\.json$/, '-evidence.json');
  assert(evidenceFile !== output, '--output must end in -review.json');
  const review = {evidenceHash: blake(evidenceBytes), records};
  await writeFile(evidenceFile, evidenceBytes, {flag: 'wx'});
  await writeFile(output, JSON.stringify(review, null, 2) + '\n', {flag: 'wx'});
  return {review: output, evidence: evidenceFile, evidenceHash: review.evidenceHash,
    recordCount: records.length, votingRows: direct + delegating, finishedReferenda: finished,
    absentReferencedReferenda: missing.size, missingVoteCount: evidence.missingVoteCount,
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
