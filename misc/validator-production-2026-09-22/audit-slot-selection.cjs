/* Public-data-only BABE arithmetic and canonical-header audit. No RPC, keys,
 * signatures created, node database access, or transaction submissions.
 * Dependencies: --dependencies /path/to/package.json (existing installation).
 */
'use strict';
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync } = require('node:fs');
const { resolve } = require('node:path');
const { createRequire } = require('node:module');
const { createHash } = require('node:crypto');
const args = process.argv.slice(2);
const option = (name, fallback) => {
  const at = args.indexOf(name);
  return at < 0 ? fallback : args[at + 1];
};
const deps = createRequire(resolve(option('--dependencies', '/tmp/sora-author-replay-20260922/package.json')));
const { blake2AsU8a, sr25519Verify } = deps('@polkadot/util-crypto');
const { TypeRegistry } = deps('@polkadot/types');
const { ristretto255 } = deps('@noble/curves/ed25519');
const snapshot = JSON.parse(readFileSync(resolve(__dirname, 'snapshot.json')));
const trace = JSON.parse(readFileSync(resolve(__dirname, 'author-stop-trace.json')));
const completed = JSON.parse(readFileSync(resolve(__dirname, 'completed-sessions.json')));
const canonical = JSON.parse(readFileSync(resolve(__dirname, 'canonical-header.json')));
const configurationEvidence = JSON.parse(readFileSync(resolve(__dirname, 'babe-config-evidence.json')));
const sdkRoot = option('--sdk-root', '/Users/takemiyamakoto/.cargo/git/checkouts/polkadot-sdk-dee0edd6eefa0594/e373717');
const sourcePaths = [
  'substrate/client/consensus/babe/src/authorship.rs',
  'substrate/client/consensus/babe/src/verification.rs',
  'substrate/primitives/consensus/babe/src/lib.rs',
  'substrate/primitives/consensus/babe/src/digests.rs',
];
const sourceHashes = Object.fromEntries(sourcePaths.map(path => [path,
  createHash('sha256').update(readFileSync(resolve(sdkRoot, path))).digest('hex')]));
function secondaryIndex(randomness, slot, count) {
  // Exact SDK input: SCALE ([u8;32], Slot(u64)), Blake2b-256, U256 BE % len.
  const bytes = Buffer.alloc(40);
  const random = Buffer.from(randomness.replace(/^0x/, ''), 'hex');
  assert.equal(random.length, 32);
  random.copy(bytes);
  bytes.writeBigUInt64LE(BigInt(slot), 32);
  const digest = Buffer.from(blake2AsU8a(bytes, 256)).toString('hex');
  return Number(BigInt('0x' + digest) % BigInt(count));
}
const testVector = {
  source: 'authorship.rs::tests::secondary_slot_author_selection_works',
  randomness: '0x' + '03'.repeat(32), slot: 100, authorities: 1000, expectedIndex: 167,
};
testVector.actualIndex = secondaryIndex(testVector.randomness, testVector.slot, testVector.authorities);
assert.equal(testVector.actualIndex, testVector.expectedIndex);
const report = {
  checkedAt: new Date().toISOString(), localOnly: true, networkRequests: 0,
  privateKeysRead: 0, signaturesCreated: 0, submittedTransactions: 0,
  sdkRevision: 'e3737178ec726cffe506c907263aaaa417893fd0', sourceHashes,
  referenceHash: snapshot.blockHash, testVector,
  limits: [
    'Secondary assignments are opportunities; a competing primary block may win the same slot.',
    'Arithmetic is independently implemented in JavaScript and checked against the pinned SDK test vector; this is not execution of the Rust node verifier.',
    'Raw epoch configurations are independently decoded as two LE-u64 values followed by AllowedSlots: 0=Primary, 1=Plain, 2=VRF, matching the exact pinned Rust enum declaration.',
    'Public-key decoding and canonical seal verification use existing JavaScript cryptographic libraries; no validator private key is used.',
    'Historical seals use that epoch authority list where captured; otherwise the current snapshot public key is identified explicitly.',
  ],
};
report.rawEpochConfigurations = configurationEvidence.states.map(state => {
  const decoded = {};
  for (const name of ['epochConfig', 'nextEpochConfig']) {
    const raw = Buffer.from(state.rawStorage[name].slice(2), 'hex');
    assert.equal(raw.length, 17, 'BabeEpochConfiguration fixed SCALE length');
    const allowedSlots = ['PrimarySlots', 'PrimaryAndSecondaryPlainSlots', 'PrimaryAndSecondaryVRFSlots'][raw[16]];
    assert(allowedSlots, 'Unknown AllowedSlots SCALE discriminant');
    const config = { c: [Number(raw.readBigUInt64LE(0)), Number(raw.readBigUInt64LE(8))], allowedSlots };
    assert.deepEqual(config, state.storage[name]);
    decoded[name] = { raw: state.rawStorage[name], discriminant: raw[16], decoded: config };
  }
  return { label: state.label, block: state.block, hash: state.hash, ...decoded };
});
report.publicKeys = snapshot.storage['babe.authorities'].map(([publicKey, weight], index) => {
  const point = ristretto255.Point.fromHex(publicKey.slice(2));
  assert(!point.equals(ristretto255.Point.ZERO));
  return { index, publicKey, weight, validNonidentityRistrettoPoint: true };
});
assert.equal(new Set(report.publicKeys.map(item => item.publicKey)).size, report.publicKeys.length);
report.epochs = trace.slotInputs.map(input => {
  const startSlot = BigInt(input.genesisSlot) + BigInt(input.epochIndex) * BigInt(input.epochDuration);
  const counts = Array(input.authorities.length).fill(0);
  for (let offset = 0; offset < input.epochDuration; offset++) {
    counts[secondaryIndex(input.randomness, startSlot + BigInt(offset), counts.length)]++;
  }
  assert.equal(counts.reduce((sum, n) => sum + n, 0), input.epochDuration);
  const observed = completed.sessions.find(session => session.session === input.session);
  const totalWeight = input.authorities.reduce((sum, [, weight]) => sum + weight, 0);
  const [cNumerator, cDenominator] = input.epochConfig.c;
  return {
    session: input.session, hash: input.hash, randomness: input.randomness,
    startSlot: startSlot.toString(), endSlotExclusive: (startSlot + BigInt(input.epochDuration)).toString(),
    epochConfig: input.epochConfig,
    validators: input.authorities.map(([publicKey, weight], index) => ({
      index, publicKey, weight, secondaryAssignments: counts[index],
      observedCanonicalBlocks: observed?.validators.find(item => item.index === index)?.blocks ?? null,
      primarySelectionProbability: 1 - (1 - cNumerator / cDenominator) ** (weight / totalWeight),
    })),
  };
});
const registry = new TypeRegistry();
assert.equal(canonical.hash, canonical.epochStateReference);
const sameStateEpoch = {
  session: canonical.epoch.epochIndex, authorities: canonical.epoch.authorities,
  randomness: canonical.epoch.randomness, epochConfig: canonical.epoch.config,
};
const verificationInputs = [...trace.slotInputs, sameStateEpoch];
const canonicalHeader = registry.createType('Header', canonical.header).toJSON();
const canonicalPredigest = Buffer.from(canonicalHeader.digest.logs.find(log => log.preRuntime).preRuntime[1].slice(2), 'hex');
const canonicalIndex = canonicalPredigest.readUInt32LE(1);
const records = [...trace.lastAuthors, {
  lastAuthoredBlock: canonical.block, hash: canonical.hash, session: canonical.epoch.epochIndex,
  stash: snapshot.storage['session.validators'][canonicalIndex], header: canonicalHeader,
  epochSource: 'same-block BabeApi_current_epoch',
}];
report.canonicalHeaders = records.map(record => {
  const header = registry.createType('Header', record.header);
  assert.equal(header.hash.toHex(), record.hash, 'Canonical header hash must match captured block');
  const preDigestHex = record.header.digest.logs.find(log => log.preRuntime?.[0] === '0x42414245').preRuntime[1];
  const preDigest = Buffer.from(preDigestHex.slice(2), 'hex');
  const variant = ({ 1: 'Primary', 2: 'SecondaryPlain', 3: 'SecondaryVRF' })[preDigest[0]];
  assert(variant, 'Unknown BABE pre-digest variant');
  const index = preDigest.readUInt32LE(1);
  const slot = preDigest.readBigUInt64LE(5);
  const epoch = verificationInputs.find(input => input.session === record.session);
  const snapshotIndex = snapshot.storage['session.validators'].indexOf(record.stash);
  const publicKey = epoch?.authorities[index][0] ?? snapshot.storage['babe.authorities'][snapshotIndex][0];
  const unsealed = JSON.parse(JSON.stringify(record.header));
  const seal = unsealed.digest.logs.pop();
  assert.equal(seal.seal[0], '0x42414245');
  const preHeader = registry.createType('Header', unsealed);
  const sealValid = sr25519Verify(preHeader.hash.toU8a(), seal.seal[1], publicKey);
  const expectedSecondaryIndex = epoch && variant !== 'Primary'
    ? secondaryIndex(epoch.randomness, slot, epoch.authorities.length) : null;
  if (epoch) {
    assert(sealValid, 'Canonical header seal must verify against captured epoch authority');
    if (expectedSecondaryIndex !== null) assert.equal(index, expectedSecondaryIndex);
  }
  const allowed = epoch?.epochConfig.allowedSlots;
  const permittedByCapturedRuntimeEpoch = allowed === undefined ? null
    : variant === 'Primary' ||
      (variant === 'SecondaryPlain' && allowed === 'PrimaryAndSecondaryPlainSlots') ||
      (variant === 'SecondaryVRF' && allowed === 'PrimaryAndSecondaryVRFSlots');
  return {
    block: record.lastAuthoredBlock, hash: record.hash, session: record.session,
    epochSource: record.epochSource ?? 'completed-session storage snapshot',
    stash: record.stash, index, slot: slot.toString(), variant, preDigestHex,
    preDigestBytes: preDigest.length, headerHashMatches: true, publicKey,
    publicKeySource: epoch ? 'captured epoch authorities' : 'current snapshot authority',
    sealValid, expectedSecondaryIndex, capturedRuntimeAllowedSlots: allowed ?? null,
    permittedByCapturedRuntimeEpoch,
    sdkRejectionWhenDisallowed: permittedByCapturedRuntimeEpoch === false ? 'SecondarySlotAssignmentsDisabled' : null,
  };
});
report.status = 'passed';
writeFileSync(resolve(option('--out', resolve(__dirname, 'slot-selection-audit.json'))), JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify({ status: report.status, epochs: report.epochs.length,
  configConflicts: report.canonicalHeaders.filter(item => item.permittedByCapturedRuntimeEpoch === false).map(item => item.block) }));
