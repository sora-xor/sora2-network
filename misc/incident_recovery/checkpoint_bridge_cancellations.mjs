#!/usr/bin/env node
/** Offline, exact-checkpoint cancellation evidence. Opaque calls are hashed,
 * never decoded, dispatched, re-encoded or used to manufacture refunds. */
import {createReadStream} from 'node:fs';
import {readFile, writeFile} from 'node:fs/promises';
import {createGunzip} from 'node:zlib';
import {createHash} from 'node:crypto';
import {createRequire} from 'node:module';
import {resolve, join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {lines} from './independent_attacker_inventory.mjs';
import {CHECKPOINT} from './capture_checkpoint.mjs';

const assert = (ok, message) => {if (!ok) throw new Error(message);};
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const hex = bytes => `0x${Buffer.from(bytes).toString('hex')}`;
const bytes = text => {
  assert(typeof text === 'string' && /^0x(?:[0-9a-f]{2})*$/.test(text), 'noncanonical hex');
  return Buffer.from(text.slice(2), 'hex');
};

export function uniqueBy(rows, key, name) {
  const result = new Map();
  for (const row of rows) {
    const id = key(row);
    assert(!result.has(id), `duplicate ${name}: ${id}`);
    result.set(id, row);
  }
  return result;
}

/** Classification is historical evidence under exactly the same metadata.
 * Changing a pending approval/deposit record does not change its opaque call
 * hash; the new record must be separately bound by its checkpoint value hash. */
export function bindPayload(pending, payload, proof, hash) {
  assert(payload && proof, 'missing recovered preimage or verification row');
  assert(pending.account === payload.account && payload.account === proof.account &&
    pending.hash === payload.hash && payload.hash === proof.hash, 'pending payload identity mismatch');
  const call = bytes(payload.callHex);
  assert(call.length > 0 && call.length <= 1024 * 1024 &&
    call.length === payload.callBytes && call.length === proof.callBytes &&
    hash(call) === pending.hash, 'opaque call preimage mismatch');
  assert(payload.source.kind === proof.sourceKind, 'source kind mismatch');
  const incompatible = proof.deployedDecode === 'rejected' ||
    proof.deployedDecode === 'rejected-absent-DEXAPI-call-11';
  assert(incompatible || proof.deployedDecode === 'decoded', 'unknown historical classification');
  if (proof.sourceKind === 'retained-historical-request-reconstruction') {
    assert(proof.rawRequestsVerified === true, 'request reconstruction was not verified');
  } else {
    assert(proof.sourceKind === 'archived-pending-creation-extrinsic' &&
      proof.originalExtrinsicRoundtrip && proof.headerHashVerified &&
      proof.extrinsicsRootVerified && proof.witnessedAdjacentStorageTransition,
    'archived preimage lacks adjacent authenticated transition evidence');
  }
  return {call, incompatible};
}

export function storedCallMatches(encoded, call) {
  if (encoded === undefined) return null;
  const raw = bytes(encoded);
  assert(raw.length >= 49, 'truncated opaque call envelope');
  const mode = raw[0] & 3;
  assert(mode < 3, 'oversized compact call length');
  const width = 1 << mode;
  let size = 0;
  for (let i = width - 1; i >= 0; --i) size = size * 256 + raw[i];
  size = Math.floor(size / 4);
  assert((mode === 0 || size >= (mode === 1 ? 64 : 16384)) &&
    size === call.length && raw.length === width + size + 48 &&
    raw.subarray(width, width + size).equals(call), 'stored opaque call preimage mismatch');
  return raw;
}

export async function run({capture, evidence, dependencies, output}) {
  assert(capture && evidence && dependencies && output, '--capture DIR --evidence DIR --dependencies PACKAGE_JSON --output FILE required');
  const require = createRequire(resolve(dependencies));
  const {Metadata, TypeRegistry} = require('@polkadot/types');
  const {blake2AsU8a, xxhashAsU8a, decodeAddress} = require('@polkadot/util-crypto');
  const hash = value => hex(blake2AsU8a(value, 256));
  const load = async (base, file) => {
    const raw = await readFile(join(base, file));
    return {raw, value: JSON.parse(raw), sha256: sha(raw)};
  };
  const manifest = await load(capture, 'capture-complete.json');
  const root = await load(capture, '../checkpoint-verification.json');
  const metadataWitness = await load(capture, '../checkpoint-metadata-witness.json');
  assert(manifest.value.complete && manifest.value.checkpoint.hash === CHECKPOINT.hash &&
    manifest.value.children.length === 0 && root.value.stateRootVerified &&
    root.value.headerHashVerified && root.value.stateRoot === CHECKPOINT.stateRoot &&
    root.value.checkpointHash === CHECKPOINT.hash && root.value.captureManifestSha256 === manifest.sha256,
  'independently verified checkpoint capture required');
  const metadataBytes = await readFile(join(capture, 'runtime-metadata.scale'));
  assert(metadataWitness.value.wasmExecutionVerified &&
    metadataWitness.value.checkpointHash === CHECKPOINT.hash &&
    metadataWitness.value.stateRoot === CHECKPOINT.stateRoot &&
    metadataWitness.value.metadataSha256 === sha(metadataBytes) &&
    manifest.value.artifacts['runtime-metadata.scale'].sha256 === sha(metadataBytes), 'executed metadata mismatch');
  const payloads = await load(evidence, 'bridge-payload-evidence.json');
  const proof = await load(evidence, 'bridge-payload-recovery-verification.json');
  const requests = await load(evidence, 'bridge-request-recovery-source.json');
  const archive = await load(evidence, 'bridge-archived-payload-source.json');
  const earlier = await load(evidence, 'bridge-readiness.json');
  const verifier = await readFile(join(evidence, 'verify-bridge-payload-recovery.cjs'));
  assert(proof.value.status === 'passed' && proof.value.localOnly &&
    proof.value.submittedTransactions === 0 && proof.value.remoteRequests === 0,
  'historical offline payload verification required');
  for (const [name, value] of Object.entries({payloadEvidenceSha256: payloads.sha256,
    sourceRequestsSha256: requests.sha256, archivedSourceSha256: archive.sha256,
    verifierSha256: sha(verifier), baselineMetadataSha256: sha(metadataBytes)})) {
    assert(proof.value.inputs[name] === value, `historical proof input mismatch: ${name}`);
  }
  const registry = new TypeRegistry();
  registry.register(require('@sora-substrate/type-definitions').types);
  const metadata = new Metadata(registry, metadataBytes); registry.setMetadata(metadata);
  const json = metadata.asLatest.toJSON();
  const types = new Map(json.lookup.types.map(row => [row.id, row.type]));
  const descriptors = new Map();
  for (const pallet of json.pallets) if (pallet.storage) for (const entry of pallet.storage.items) {
    const name = `${pallet.name}.${entry.name}`;
    if (pallet.name === 'BridgeMultisig' || ['EthBridge.Requests', 'EthBridge.RequestStatuses'].includes(name)) {
      const map = entry.type.map;
      descriptors.set(name, {name, entry, prefix: hex(Buffer.concat([
        xxhashAsU8a(pallet.storage.prefix, 128), xxhashAsU8a(entry.name, 128)])),
      keyTypes: map ? (map.hashers.length === 1 ? [map.key] : types.get(map.key)?.def.tuple) : []});
    }
  }
  const descriptor = name => {const found = descriptors.get(name); assert(found, `missing ${name}`); return found;};
  const canonicalCodec = (type, raw) => {
    assert(raw.length <= 1024 * 1024, 'typed record exceeds bound');
    const codec = registry.createTypeUnsafe(`Lookup${type}`, [raw]);
    assert(Buffer.from(codec.toU8a()).equals(raw), 'typed storage roundtrip failed');
    return codec;
  };
  const hashKey = (kind, raw) => {
    if (kind === 'Identity') return raw;
    if (kind === 'Twox64Concat') return Buffer.concat([xxhashAsU8a(raw, 64), raw]);
    if (kind === 'Blake2_128Concat') return Buffer.concat([blake2AsU8a(raw, 128), raw]);
    throw new Error('unsupported storage key hasher');
  };
  const key = (name, components) => {
    const d = descriptor(name), map = d.entry.type.map;
    assert(map && components.length === map.hashers.length && components.length === d.keyTypes?.length,
      'map key shape mismatch');
    return hex(Buffer.concat([bytes(d.prefix), ...components.map((raw, i) => {
      canonicalCodec(d.keyTypes[i], raw); return hashKey(map.hashers[i], raw);
    })]));
  };
  const decodeKey = (name, storageKey) => {
    const d = descriptor(name), map = d.entry.type.map;
    assert(map && storageKey.startsWith(d.prefix), 'map key prefix mismatch');
    let rest = bytes(`0x${storageKey.slice(d.prefix.length)}`); const parts = [];
    for (let i = 0; i < map.hashers.length; ++i) {
      const width = {Identity: 0, Twox64Concat: 8, Blake2_128Concat: 16}[map.hashers[i]];
      assert(width !== undefined, 'unreviewed opaque key hasher');
      const decoded = registry.createTypeUnsafe(`Lookup${d.keyTypes[i]}`, [rest.subarray(width)]);
      const component = Buffer.from(decoded.toU8a());
      const encoded = hashKey(map.hashers[i], component);
      assert(component.length > 0 && rest.subarray(0, encoded.length).equals(encoded), 'key component hash mismatch');
      rest = rest.subarray(encoded.length); parts.push(component);
    }
    assert(rest.length === 0 && key(name, parts) === storageKey, 'trailing/noncanonical map key');
    return parts;
  };
  const identity = row => `${hex(decodeAddress(row.account))}:${row.hash}`;
  const payloadById = uniqueBy(payloads.value.payloads, identity, 'payload');
  const proofById = uniqueBy(proof.value.rows, identity, 'proof');
  const sourceRequests = uniqueBy(requests.value, row => row.key, 'request source');
  const oldPending = uniqueBy(earlier.value.accounts.flatMap(a => a.operations.map(o => ({...o, account: a.account}))),
    row => row.storageKey, 'earlier pending key');
  const wantedKeys = new Set();
  for (const payload of payloads.value.payloads) if (payload.source.kind === 'retained-historical-request-reconstruction') {
    for (const sourceKey of [payload.source.requestKey, payload.source.loadRequestKey]) {
      assert(sourceRequests.has(sourceKey), 'request preimage source missing');
      const components = decodeKey('EthBridge.Requests', sourceKey);
      wantedKeys.add(sourceKey); wantedKeys.add(key('EthBridge.RequestStatuses', components));
    }
  }
  const bridgePrefix = descriptor('BridgeMultisig.Multisigs').prefix.slice(0, 34);
  const selectedRanges = [bridgePrefix, ...new Set([...wantedKeys].map(k => k.slice(0, 66)))];
  const records = new Map(), counts = {}, pageEvidence = [];
  for (const page of manifest.value.top.pages) {
    if (!page.keyCount || !selectedRanges.some(prefix => page.firstKey <= prefix + 'f'.repeat(512) && page.lastKey >= prefix)) continue;
    const path = resolve(capture, page.file);
    assert(path.startsWith(`${resolve(capture)}/`), 'capture page escapes directory');
    const digest = createHash('sha256'); for await (const chunk of createReadStream(path)) digest.update(chunk);
    assert(digest.digest('hex') === page.sha256, 'capture page checksum mismatch');
    pageEvidence.push({file: page.file, sha256: page.sha256});
    let header = true;
    for await (const row of lines(createReadStream(path).pipe(createGunzip()))) {
      if (header) {assert(row.line && JSON.parse(row.line).checkpointHash === CHECKPOINT.hash, 'page identity mismatch'); header = false; continue;}
      if (!row.line) {assert(!row.prefix.startsWith(`["${bridgePrefix}`), 'oversized bridge bookkeeping'); continue;}
      const [storageKey, value] = JSON.parse(row.line);
      if (storageKey.startsWith(bridgePrefix)) {
        const d = [...descriptors.values()].find(d => storageKey.startsWith(d.prefix));
        if (d) counts[d.name] = (counts[d.name] ?? 0) + 1;
        else assert(storageKey.slice(34) === hex(xxhashAsU8a(':__STORAGE_VERSION__:', 128)).slice(2), 'unknown bridge storage');
        records.set(storageKey, value);
      } else if (wantedKeys.has(storageKey)) records.set(storageKey, value);
    }
  }
  const multisigs = descriptor('BridgeMultisig.Multisigs');
  const pending = [...records.entries()].filter(([k]) => k.startsWith(multisigs.prefix));
  const seen = new Set(), held = [], incompatible = [], blockers = [], changedSinceEarlier = [];
  const provenance = {schema: 'sora-nexus-checkpoint-bridge-evidence-v1', checkpointHash: CHECKPOINT.hash,
    captureManifestSha256: manifest.sha256, stateVerificationSha256: root.sha256,
    metadataWitnessSha256: metadataWitness.sha256, metadataSha256: sha(metadataBytes),
    historicalPayloadProofSha256: proof.sha256, payloadsSha256: payloads.sha256,
    historicalRequestSourceSha256: requests.sha256, archivedPayloadSourceSha256: archive.sha256,
    currentEarlierInventorySha256: earlier.sha256, originalHistoricalInventorySha256: proof.value.inputs.inventorySha256,
    originalInventoryDigestMatchesCurrentFile: earlier.sha256 === proof.value.inputs.inventorySha256,
    originalInventorySupersededByVerifiedCheckpoint: true, pages: pageEvidence};
  const reviewEvidenceHash = hash(Buffer.from(JSON.stringify(provenance)));
  const candidates = new Map();
  for (const [storageKey, rawValue] of pending) {
    try {
      const [owner, callHash] = decodeKey('BridgeMultisig.Multisigs', storageKey);
      assert(owner.length === 32 && callHash.length === 32, 'pending identity width');
      const id = `${hex(owner)}:${hex(callHash)}`;
      assert(!seen.has(id), 'duplicate pending identity'); seen.add(id);
      const payload = payloadById.get(id), rowProof = proofById.get(id);
      const bound = bindPayload({account: payload?.account, hash: hex(callHash)}, payload, rowProof, hash);
      assert(hex(decodeAddress(payload.account)) === hex(owner), 'recovered owner mismatch');
      const value = canonicalCodec(multisigs.entry.type.map.value, bytes(rawValue));
      const old = oldPending.get(storageKey);
      if (!old || old.rawValue !== rawValue) changedSinceEarlier.push({key: storageKey, oldValueHash: old ? hash(bytes(old.rawValue)) : null, valueHash: hash(bytes(rawValue))});
      if (!bound.incompatible) {held.push({key: storageKey, hash: hex(callHash), reason: 'compatible opaque operation retained under persistent bridge halt'}); continue;}
      const callsKey = key('BridgeMultisig.Calls', [callHash]);
      const stored = storedCallMatches(records.get(callsKey), bound.call);
      const preservation = new Map();
      const preserve = k => preservation.set(k, {key: k, valueHash: records.has(k) ? hash(bytes(records.get(k))) : null});
      const timepoint = Buffer.from(value.get('when').toU8a());
      preserve(key('BridgeMultisig.DispatchedCalls', [callHash, timepoint]));
      for (const name of ['BridgeMultisig.CountedNewOperations', 'BridgeMultisig.ProposerCountedOperations']) {
        if (!descriptors.has(name)) continue;
        const marker = key(name, [owner, callHash]);
        assert(!records.has(marker), 'counted operation needs separate counter adapter'); preserve(marker);
      }
      if (payload.source.kind === 'retained-historical-request-reconstruction') {
        for (const sourceKey of [payload.source.requestKey, payload.source.loadRequestKey]) {
          assert(records.get(sourceKey) === sourceRequests.get(sourceKey).rawValue, 'historical request bytes changed at checkpoint');
          preserve(sourceKey); preserve(key('EthBridge.RequestStatuses', decodeKey('EthBridge.Requests', sourceKey)));
        }
      }
      const operations = [{storage: 'BridgeMultisig.Multisigs', key: storageKey, valueHash: hash(bytes(rawValue))}];
      if (descriptors.has('BridgeMultisig.CancellationApprovals')) {
        const approvalKey = key('BridgeMultisig.CancellationApprovals', [owner, callHash]);
        if (records.has(approvalKey)) {
          canonicalCodec(descriptor('BridgeMultisig.CancellationApprovals').entry.type.map.value, bytes(records.get(approvalKey)));
          operations.push({storage: 'BridgeMultisig.CancellationApprovals', key: approvalKey, valueHash: hash(bytes(records.get(approvalKey)))});
        } else preserve(approvalKey);
      }
      const existing = candidates.get(hex(callHash));
      if (existing) {
        assert(existing.recoveredCallPreimage === payload.callHex, 'shared preimage conflict');
        existing.operations.push(...operations);
        for (const [k, v] of preservation) existing.preservation.set(k, v);
      } else candidates.set(hex(callHash), {callHash: hex(callHash), recoveredCallPreimage: payload.callHex,
        callsValueHash: stored ? hash(stored) : null, operations, preservation, reviewEvidenceHash});
      incompatible.push({key: storageKey, valueHash: hash(bytes(rawValue)), callHash: hex(callHash),
        sourceKind: payload.source.kind, callsPresent: Boolean(stored), refundGenerated: false});
    } catch (error) {blockers.push({key: storageKey, reason: error.message});}
  }
  const absentHistorical = [...payloadById.keys()].filter(id => !seen.has(id));
  // An error involving a shared call must not leave a partial cancellation.
  const blockedCalls = new Set(blockers.map(row => row.key.slice(-64)));
  for (const call of candidates.keys()) if (blockedCalls.has(call.slice(2))) candidates.delete(call);
  const bridgeCancellations = [...candidates.values()].sort((a, b) => a.callHash.localeCompare(b.callHash)).map(({preservation, ...row}) =>
    ({...row, operations: row.operations.sort((a, b) => a.key.localeCompare(b.key)), preserve: [...preservation.values()].sort((a, b) => a.key.localeCompare(b.key))}));
  const plan = {controls: [], bridgeCancellations, derivedAccounts: []};
  const planBytes = Buffer.from(JSON.stringify(plan, null, 2) + '\n');
  const report = {schema: 'sora-nexus-checkpoint-bridge-cancellation-v1', provenance, reviewEvidenceHash,
    scriptSha256: sha(await readFile(new URL(import.meta.url))), planSha256: sha(planBytes), counts,
    pendingOperations: pending.length, incompatibleOperations: incompatible.length,
    cancellationCallGroups: bridgeCancellations.length, compatibleOperationsHeld: held.length,
    changedSinceEarlier, absentHistoricalPayloads: absentHistorical, blockers, incompatible, held,
    submittedTransactions: 0, remoteRequests: 0, callExecution: false, callDecoding: false,
    nominalDepositRefunds: false, productionReady: false,
    limitations: 'Exact bookkeeping candidates only. Request/status/replay rows remain unchanged. Compatible operations and unrelated stored calls stay halted. Consensus, signed release inputs, incoming/outgoing reconciliation and exact-artifact rehearsals remain separate gates.'};
  await writeFile(output, planBytes, {flag: 'wx'});
  await writeFile(`${output}.report.json`, JSON.stringify(report, null, 2) + '\n', {flag: 'wx'});
  console.log(JSON.stringify({output, pending: pending.length, incompatible: incompatible.length,
    callGroups: bridgeCancellations.length, held: held.length, changedSinceEarlier: changedSinceEarlier.length,
    absentHistorical: absentHistorical.length, blockers: blockers.length}));
  return report;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const args = process.argv.slice(2), options = {};
  for (let i = 0; i < args.length; i += 2) {assert(args[i].startsWith('--') && args[i + 1], 'expected named arguments'); options[args[i].slice(2)] = args[i + 1];}
  await run(options);
}
