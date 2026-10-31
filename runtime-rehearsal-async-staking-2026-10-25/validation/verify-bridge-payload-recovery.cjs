'use strict';
// Offline verification of public historical call preimages. No RPC or execution.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { Metadata, TypeRegistry } = require('@polkadot/types');
const { blake2AsHex, xxhashAsHex } = require('@polkadot/util-crypto');
const { types: historicalTypes } = require('@sora-substrate/type-definitions');
const { compactToU8a, u8aToHex } = require('@polkadot/util');
const { calculate_state_root } = require('@acala-network/chopsticks-executor');

const sha256 = value => createHash('sha256').update(value).digest('hex');
function registryFrom(file) {
  const bytes = fs.readFileSync(file), registry = new TypeRegistry();
  const metadata = new Metadata(registry, new Uint8Array(bytes));
  registry.setMetadata(metadata);
  const bridge = metadata.asLatest.pallets.find(pallet => pallet.name.toString() === 'EthBridge');
  const retiredIndex = metadata.asLatest.pallets.find(pallet => pallet.index.toNumber() === 30);
  assert.equal(bridge.index.toNumber(), 31);
  assert.equal(retiredIndex.name.toString(), 'DEXAPI');
  if (retiredIndex.calls.isSome) {
    const calls = registry.lookup.getSiType(retiredIndex.calls.unwrap().type).def;
    assert(calls.isVariant && !calls.asVariant.variants.some(variant => variant.index.toNumber() === 11),
      'Historical import index 30:11 must remain non-callable');
  }
  const requestStorage = bridge.storage.unwrap().items.find(item => item.name.toString() === 'Requests');
  assert(requestStorage && requestStorage.type.isMap);
  const requestType = registry.createLookupType(requestStorage.type.asMap.value);
  const multisig = metadata.asLatest.pallets.find(pallet => pallet.name.toString() === 'BridgeMultisig');
  const pendingStorage = multisig.storage.unwrap().items.find(item => item.name.toString() === 'Multisigs');
  const pendingType = registry.createLookupType(pendingStorage.type.asMap.value);
  return { registry, requestType, pendingType, sha256: sha256(bytes) };
}
async function verifyRecovery({ evidencePath, requestsPath, archivePath, inventoryPath, baselineMetadataPath, candidateMetadataPath }) {
  const evidenceBytes = fs.readFileSync(evidencePath), requestsBytes = fs.readFileSync(requestsPath);
  const evidence = JSON.parse(evidenceBytes), sourceRows = JSON.parse(requestsBytes);
  const inventoryBytes = fs.readFileSync(inventoryPath), inventory = JSON.parse(inventoryBytes);
  assert(inventory.pendingScanComplete && inventory.requestScanComplete, 'Complete captured inventory is required');
  const expectedIdentities = inventory.pending.map(row => inventory.account + ':' + row.hash).sort();
  assert.equal(new Set(expectedIdentities).size, expectedIdentities.length, 'Duplicate captured pending identity');
  assert.deepEqual(evidence.payloads.map(row => row.account + ':' + row.hash).sort(), expectedIdentities,
    'Recovered payloads must cover every captured pending account/hash identity exactly once');
  const baseline = registryFrom(baselineMetadataPath), candidate = registryFrom(candidateMetadataPath);
  const requests = new Map(sourceRows.map(row => [row.key, row]));
  const archiveBytes = archivePath ? fs.readFileSync(archivePath) : null;
  const archive = archiveBytes ? JSON.parse(archiveBytes) : null;
  const historical = new Map();
  assert.equal(requests.size, sourceRows.length, 'Duplicate source request key');
  const network = '0x00000000';
  const requestPrefix = xxhashAsHex('EthBridge', 128) + xxhashAsHex('Requests', 128).slice(2)
    + xxhashAsHex(network, 64).slice(2) + network.slice(2);
  for (const row of sourceRows) {
    assert.equal(row.key, requestPrefix + row.hash.slice(2), 'Incorrect raw request storage key');
    assert(/^0x(?:[0-9a-f]{2})+$/.test(row.rawValue));
  }
  const unique = new Set(), rows = [];
  for (const payload of evidence.payloads) {
    assert(!unique.has(payload.account + ':' + payload.hash), 'Duplicate pending payload identity');
    unique.add(payload.account + ':' + payload.hash);
    assert.equal(blake2AsHex(payload.callHex, 256), payload.hash, 'Call preimage hash mismatch');
    assert.equal(payload.callBytes, (payload.callHex.length - 2) / 2);
    if (payload.source.kind === 'archived-pending-creation-extrinsic') {
      assert(archive && archive.status === 'passed', 'Archived source capture is required');
      const source = payload.source;
      assert.equal(source.reference, 'bridge-archived-payload-source.json');
      const block = archive.blocks.find(block => block.blockHash === source.blockHash);
      const transition = archive.transitions.find(item => item.hash === payload.hash);
      assert(block && transition, 'Archived block and matching storage transition are required');
      assert.equal(block.height, source.height);
      assert.equal(transition.before.height + 1, transition.after.height);
      assert.equal(transition.before.value, null);
      assert(transition.after.value);
      assert.equal(transition.after.height, source.height);
      assert.equal(transition.after.blockHash, source.blockHash);
      for (const boundary of [transition.before, transition.after]) {
        const observed = archive.observations.find(observation => observation.height === boundary.height
          && observation.blockHash === boundary.blockHash
          && observation.values.some(([key, value]) => key === transition.key && value === boundary.value));
        assert(observed, 'Storage boundary must match an original raw RPC observation');
      }
      const accountHex = baseline.registry.createType('AccountId32', payload.account).toHex();
      const expectedKey = xxhashAsHex('BridgeMultisig', 128) + xxhashAsHex('Multisigs', 128).slice(2)
        + xxhashAsHex(accountHex, 64).slice(2) + accountHex.slice(2)
        + blake2AsHex(payload.hash, 128).slice(2) + payload.hash.slice(2);
      assert.equal(transition.key, expectedKey, 'Storage transition account/hash identity mismatch');
      if (!historical.has(source.blockHash)) {
        const registry = new TypeRegistry();
        registry.register({ ...historicalTypes, Timepoint: 'BridgeTimepoint' });
        registry.setMetadata(new Metadata(registry, block.metadataHex));
        assert.equal(registry.createType('Header', block.header).hash.toHex(), source.blockHash, 'Archived header hash mismatch');
        const extrinsicsRoot = await calculate_state_root(block.extrinsics.map((value, index) =>
          [u8aToHex(compactToU8a(index)), value]), 0);
        assert.equal(extrinsicsRoot, block.header.extrinsicsRoot, 'Archived extrinsics root mismatch');
        historical.set(source.blockHash, registry);
      }
      const registry = historical.get(source.blockHash);
      assert(Number.isInteger(source.extrinsicIndex) && source.extrinsicIndex >= 0);
      assert.equal(block.extrinsics[source.extrinsicIndex], source.extrinsicHex, 'Source extrinsic bytes mismatch');
      const extrinsic = registry.createType('Extrinsic', source.extrinsicHex);
      assert.equal(extrinsic.toHex(), source.extrinsicHex, 'Historical extrinsic must roundtrip every byte');
      const outer = extrinsic.method;
      assert.equal(outer.section, 'bridgeMultisig');
      assert.equal(outer.method, 'asMulti');
      assert.equal(outer.args[0].toHex(), accountHex);
      assert.equal(outer.args[2].toHex(), payload.callHex, 'Exact historical inner payload mismatch');
      const pending = baseline.registry.createTypeUnsafe(baseline.pendingType, [transition.after.value], { isPedantic: true });
      assert.equal(pending.toHex(), transition.after.value, 'Pending state must roundtrip every byte');
      assert(outer.args[1].isSome, 'Archived bridge timepoint is required');
      assert.equal(pending.when.toHex(), outer.args[1].unwrap().toHex(), 'Pending timepoint must match the original call');
      const decode = context => {
        try {
          const call = context.registry.createTypeUnsafe('Call', [payload.callHex], { isPedantic: true });
          assert.equal(call.toHex(), payload.callHex);
          assert.equal(call.encodedLength, payload.callBytes);
          return { status: 'decoded', section: call.section, method: call.method, json: call.toJSON() };
        } catch (error) { return { status: 'rejected' }; }
      };
      const before = decode(baseline), after = decode(candidate);
      assert.deepEqual(after, before, 'Archived payload decode changed between runtimes');
      rows.push({ account: payload.account, hash: payload.hash, callBytes: payload.callBytes,
        sourceKind: source.kind, sourceBlockHash: source.blockHash, sourceHeight: source.height,
        originalExtrinsicRoundtrip: true, headerHashVerified: true, extrinsicsRootVerified: true,
        witnessedAdjacentStorageTransition: true,
        deployedDecode: before.status, candidateDecode: after.status,
        ...(before.status === 'decoded' ? { currentSection: before.section, currentMethod: before.method } : {}) });
      continue;
    }
    assert.equal(payload.source.kind, 'retained-historical-request-reconstruction');
    assert.equal(payload.source.reference, 'bridge-request-recovery-source.json');
    assert.equal(payload.source.palletIndex, 30);
    assert.equal(payload.callHex.slice(0, 4), '0x1e');
    const currentCallHex = '0x1f' + payload.callHex.slice(4);
    assert.equal(payload.currentCallHex, currentCallHex, 'Only the historical pallet index byte may change');
    const incomingRow = requests.get(payload.source.requestKey);
    const loadRow = requests.get(payload.source.loadRequestKey);
    assert(incomingRow && loadRow, 'Both exact stored request sources are required');
    const decodes = [];
    for (const context of [baseline, candidate]) {
      const incoming = context.registry.createTypeUnsafe(context.requestType, [incomingRow.rawValue], { isPedantic: true });
      const load = context.registry.createTypeUnsafe(context.requestType, [loadRow.rawValue], { isPedantic: true });
      assert(incoming.isIncoming && load.isLoadIncoming);
      assert.equal(incoming.asIncoming[1].toHex(), incomingRow.hash, 'Stored incoming hash must match its storage key');
      assert.equal(incoming.toHex(), incomingRow.rawValue);
      assert.equal(load.toHex(), loadRow.rawValue);
      const call = context.registry.createTypeUnsafe('Call', [currentCallHex], { isPedantic: true });
      assert.equal(call.section, 'ethBridge');
      assert.equal(call.method, 'importIncomingRequest');
      assert.equal(call.toHex(), currentCallHex);
      assert.equal(call.args[0].toHex(), load.asLoadIncoming.toHex());
      assert(call.args[1].isOk);
      assert.equal(call.args[1].asOk.toHex(), incoming.asIncoming[0].toHex());
      assert.throws(() => context.registry.createTypeUnsafe('Call', [payload.callHex], { isPedantic: true }),
        'Historical payload must already fail the deployed call decoder');
      decodes.push(call.toJSON());
    }
    assert.deepEqual(decodes[0], decodes[1], 'Request argument semantics changed between runtimes');
    rows.push({ account: payload.account, hash: payload.hash, callBytes: payload.callBytes,
      sourceKind: payload.source.kind,
      historicalPalletIndex: 30, currentPalletIndex: 31, rawRequestsVerified: true,
      deployedDecode: 'rejected-absent-DEXAPI-call-11', candidateDecode: 'rejected-absent-DEXAPI-call-11',
      currentPrefixArgumentsIdentical: true });
  }
  return { status: 'passed', localOnly: true, remoteRequests: 0, submittedTransactions: 0,
    payloadCount: rows.length, maxCallBytes: rows.length ? Math.max(...rows.map(row => row.callBytes)) : null,
    inputs: { payloadEvidenceSha256: sha256(evidenceBytes), sourceRequestsSha256: sha256(requestsBytes),
      inventorySha256: sha256(inventoryBytes),
      archivedSourceSha256: archiveBytes ? sha256(archiveBytes) : null,
      baselineMetadataSha256: baseline.sha256, candidateMetadataSha256: candidate.sha256,
      trieExecutorSha256: sha256(fs.readFileSync(require.resolve('@acala-network/chopsticks-executor'))),
      verifierSha256: sha256(fs.readFileSync(__filename)) }, rows,
    classifications: rows.reduce((counts, row) => { const key = row.deployedDecode === 'decoded' ? 'decodedIdentically' : 'rejectedByBoth'; counts[key] = (counts[key] || 0) + 1; return counts; }, {}),
    scope: 'Exact preimages verified either from stored request bytes with only the historic pallet index byte restored, or from archived extrinsics at witnessed adjacent absent-to-present storage boundaries. Deployed and candidate decoding is compared for every payload. Historical storage, block and metadata bytes were read from the approved public endpoint; no first-ever creation, execution success or operator readiness is claimed.' };
}
module.exports = { verifyRecovery };
if (require.main === module) {
  const [evidencePath, requestsPath, outputPath, archivePath, inventoryPath] = process.argv.slice(2);
  assert(evidencePath && requestsPath && outputPath && archivePath && inventoryPath,
    'Usage: node verify-bridge-payload-recovery.cjs EVIDENCE REQUESTS OUTPUT ARCHIVE INVENTORY');
  verifyRecovery({ evidencePath, requestsPath, archivePath, inventoryPath,
    baselineMetadataPath: path.join(__dirname, 'baseline-metadata.scale'),
    candidateMetadataPath: path.join(__dirname, 'candidate-metadata.scale') }).then(proof => {
    fs.writeFileSync(outputPath, JSON.stringify(proof, null, 2) + '\n');
    console.log(JSON.stringify({ status: proof.status, payloadCount: proof.payloadCount, maxCallBytes: proof.maxCallBytes }));
  }).catch(error => { console.error(error); process.exitCode = 1; });
}
