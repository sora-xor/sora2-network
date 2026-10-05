'use strict';
// Exact candidate Wasm, pinned public state, local dispatch only; no real keys.
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync, existsSync } = require('node:fs');
const { resolve } = require('node:path');
const { createHash } = require('node:crypto');
const { gunzipSync, gzipSync } = require('node:zlib');
const { StorageKey, GenericExtrinsic } = require('@polkadot/types');
const { hexToU8a, u8aToHex, compactAddLength } = require('@polkadot/util');
const { blake2AsHex, cryptoWaitReady, sr25519PairFromSeed, xxhashAsU8a } = require('@polkadot/util-crypto');
const { setup, BuildBlockMode, Block, destroyWorker } = require('@acala-network/chopsticks-core');
const { fullOverrideBundle } = require('@sora-substrate/type-definitions');
const { retiredLendingFixture } = require('./retired-lending-fixture.cjs');
const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
const opts = {
  endpoint: option('--endpoint', 'wss://mof2.sora.org'),
  wasm: resolve(option('--wasm', resolve(__dirname, '../framenode-runtime-4.8.12.compact.compressed.wasm'))),
  snapshot: resolve(option('--snapshot', resolve(__dirname, 'snapshot.json'))),
  cache: resolve(option('--cache', resolve(__dirname, '.public-read-cache.json.gz'))),
  syntheticFixtures: args.includes('--synthetic-fixtures'),
  output: resolve(option('--output', resolve(__dirname, 'fee-policy-wasm-rehearsal.json'))),
};
const snapshot = JSON.parse(readFileSync(opts.snapshot));
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const fakeSignature = new Uint8Array(64).fill(0xcd); fakeSignature.set([0xde, 0xad, 0xbe, 0xef]);
const report = { status: 'running', startedAt: new Date().toISOString(), localOnly: true,
  syntheticFixturesEnabled: opts.syntheticFixtures,
  endpoint: opts.endpoint, pinnedBlockHash: snapshot.blockHash, pinnedBlockNumber: snapshot.blockNumber,
  executor: '@acala-network/chopsticks-core@1.5.1', mockSignatureHost: true, allowUnresolvedImports: true,
  offchainWorker: false, submittedTransactions: 0, privateKeysRead: 0, realSignatures: 0, remoteRpcMethods: {},
  unsignedChecks: [], signedChecks: [], fixtureOverrides: [], fixtureLimitations: [], localOnlySyntheticEvidence: true,
  constraints: [
    'Only enumerated read-only public RPC supplies block-hash-pinned state.',
    'Main branches replace only :code locally. Explicit --synthetic-fixtures branches additionally record every fixture key/value override; these do not claim mainnet-state coverage.',
    'Synthetic headers and extrinsics use the mocked signature host. Real cryptography is covered by native tests; consensus validity and governance enactment are outside this rehearsal.',
    'Independent fee cases branch from initialized local state; replay checks retain prior local execution. No transactions are submitted to the public network.',
    'Migration ownership proofs use deterministic synthetic ed25519/SHA3 keys and real Wasm verification; only transaction/header signatures use the mocked host.',
  ] };
let chain, cache;
const save = () => writeFileSync(opts.output, JSON.stringify(report, null, 2) + '\n');
const progress = value => { report.stage = value; save(); console.log(new Date().toISOString(), value); };
function storageKey(meta, pallet, name, params = []) {
  return new StorageKey(meta.registry, [meta.query[pallet][name], params]);
}
async function rawQuery(block, pallet, name, params = []) {
  const meta = await block.meta;
  const key = storageKey(meta, pallet, name, params);
  const raw = await block.get(key.toHex());
  if (meta.query[pallet][name].meta.modifier.isOptional) return meta.registry.createType('Option<' + key.outputType + '>', hexToU8a(raw ? '0x01' + raw.slice(2) : '0x00'));
  return meta.registry.createType(key.outputType, raw ? hexToU8a(raw) : undefined);
}
function installReadCache() {
  cache = existsSync(opts.cache) ? JSON.parse(gunzipSync(readFileSync(opts.cache)).toString()) : { format: 1, values: {}, keyRanges: {} };
  assert.equal(cache.format, 1);
  report.readCache = { path: opts.cache, content: 'Immutable public storage scoped by block hash', existingValues: Object.keys(cache.values).length, storageHits: 0, keyPageHits: 0, fetchedValues: 0, fetchedKeyPages: 0 };
  const scoped = (hash, key) => hash + ':' + key;
  const getStorage = chain.api.getStorage.bind(chain.api);
  const getKeys = chain.api.getKeysPaged.bind(chain.api);
  const batch = async (keys, hash) => {
    if (!keys.length) return;
    try {
      const result = await chain.api.send('state_queryStorageAt', [keys, hash], true);
      const changes = new Map(result[0]?.changes || []);
      for (const key of keys) { assert(changes.has(key)); cache.values[scoped(hash, key)] = changes.get(key); report.readCache.fetchedValues++; }
    } catch (error) {
      if (keys.length < 2 || !(error.code === -32008 || /Response is too big|Exceeded max limit/.test(error.message))) throw error;
      const mid = Math.floor(keys.length / 2); await batch(keys.slice(0, mid), hash); await batch(keys.slice(mid), hash);
    }
  };
  chain.api.getStorage = async (key, hash) => {
    if (!hash) return getStorage(key, hash);
    const lookup = scoped(hash, key);
    if (Object.hasOwn(cache.values, lookup)) { report.readCache.storageHits++; return cache.values[lookup]; }
    const value = await getStorage(key, hash); cache.values[lookup] = value ?? null; report.readCache.fetchedValues++; return value;
  };
  chain.api.getKeysPaged = async (prefix, count, start, hash) => {
    if (!hash || prefix.startsWith('0x3a6368696c645f73746f726167653a')) return getKeys(prefix, count, start, hash);
    const ranges = cache.keyRanges[scoped(hash, prefix)] ||= [];
    const normalized = start || prefix;
    for (const range of ranges) {
      if (normalized < range.start || (!range.complete && normalized > range.end)) continue;
      const remaining = range.keys.filter(key => key > normalized);
      if (remaining.length >= count || range.complete) { report.readCache.keyPageHits++; return remaining.slice(0, count); }
    }
    const fetchCount = prefix.length < 66 ? count : Math.max(1000, count);
    const keys = await getKeys(prefix, fetchCount, normalized, hash);
    const missing = keys.filter(key => !Object.hasOwn(cache.values, scoped(hash, key)));
    for (let i = 0; i < missing.length; i += 256) await batch(missing.slice(i, i + 256), hash);
    ranges.push({ start: normalized, end: keys.at(-1) || normalized, complete: keys.length < fetchCount, keys }); report.readCache.fetchedKeyPages++;
    return keys.slice(0, count);
  };
}
function secondaryIndex(epoch, slot) {
  const encoded = Buffer.alloc(8); encoded.writeBigUInt64LE(BigInt(slot));
  return Number(BigInt(blake2AsHex(Buffer.concat([Buffer.from(epoch.randomness.slice(2), 'hex'), encoded]), 256)) % BigInt(epoch.authorities.length));
}
async function initializeUpgrade(parent) {
  const registry = await parent.registry;
  registry.register({ RehearsalEpoch: { epochIndex: 'u64', startSlot: 'u64', duration: 'u64', authorities: 'Vec<(AuthorityId,u64)>', randomness: 'H256', config: 'BabeEpochConfiguration' } });
  const epochRaw = await parent.call('BabeApi_current_epoch', []);
  const epoch = registry.createType('RehearsalEpoch', epochRaw.result).toJSON();
  const slot = (await rawQuery(parent, 'babe', 'currentSlot')).toNumber() + 1;
  assert(slot < epoch.startSlot + epoch.duration, 'Select a non-boundary snapshot to isolate migration');
  const predigest = registry.createType('RawBabePreDigest', { SecondaryPlain: { authorityIndex: secondaryIndex(epoch, slot), slotNumber: slot } });
  const digest = registry.createType('DigestItem', { PreRuntime: ['BABE', compactAddLength(predigest.toU8a())] });
  const header = registry.createType('Header', { parentHash: parent.hash, number: parent.number + 1, stateRoot: '0x' + '00'.repeat(32), extrinsicsRoot: '0x' + '00'.repeat(32), digest: { logs: [digest] } });
  const block = new Block(chain, parent.number + 1, header.hash.toHex(), parent, { header, extrinsics: [], storage: parent.storage });
  progress('Executing exact candidate Core_initialize_block and on-runtime-upgrade hooks');
  const response = await block.call('Core_initialize_block', [header.toHex()]);
  block.pushStorageLayer().setAll(response.storageDiff);
  report.initialize = { header: header.toJSON(), slot, result: response.result, storageDiffEntries: response.storageDiff.length, runtimeLogs: response.runtimeLogs };
  assert.equal((await rawQuery(block, 'system', 'lastRuntimeUpgrade')).unwrap().specVersion.toNumber(), 134);
  const meta = await block.meta;
  const protectedPrefixes = ['claimedRewards', 'ledger'].map(name => meta.query.staking[name].keyPrefix().toHex());
  const protectedWrites = response.storageDiff.filter(([key]) => protectedPrefixes.some(prefix => key.startsWith(prefix)));
  assert.deepEqual(protectedWrites, [], 'Upgrade hooks must preserve all staking claim and ledger entries');
  assert.equal((await rawQuery(block, 'balances', 'totalIssuance')).toString(),
    (await rawQuery(parent, 'balances', 'totalIssuance')).toString(), 'Upgrade hooks must preserve native XOR issuance');
  report.initialize.stakingClaimAndLedgerWrites = protectedWrites.length;
  report.initialize.nativeXorIssuancePreserved = true;
  const timestamp = new GenericExtrinsic(registry, meta.tx.timestamp.set(slot * 6000)).toHex();
  const applied = await block.call('BlockBuilder_apply_extrinsic', [timestamp]);
  block.pushStorageLayer().setAll(applied.storageDiff);
  const outcome = registry.createType('ApplyExtrinsicResult', applied.result);
  assert(outcome.isOk && outcome.asOk.isOk);
  report.timestamp = { value: slot * 6000, outcome: outcome.toJSON() };
  return block;
}
function signedExtrinsic(registry, call, block, account) {
  const tx = new GenericExtrinsic(registry, call);
  tx.signFake(report.signer.address, { blockHash: block.hash, genesisHash: snapshot.genesis,
    runtimeVersion: report.candidate.runtimeVersion, nonce: account.nonce });
  tx.signature.set(fakeSignature);
  return tx;
}
function spendable(account) {
  const data = account.data;
  const frozen = data.frozen?.toBigInt() ?? data.feeFrozen?.toBigInt() ?? 0n;
  return data.free.toBigInt() > frozen ? data.free.toBigInt() - frozen : 0n;
}
async function feeQuote(block, call, account) {
  const registry = await block.registry;
  const tx = signedExtrinsic(registry, call, block, account);
  const response = await block.call('TransactionPaymentApi_query_info',
    [tx.toHex(), u8aToHex(registry.createType('u32', tx.encodedLength).toU8a())]);
  return registry.createType('RuntimeDispatchInfo', response.result).partialFee.toBigInt();
}
async function chooseSigner(block, fixtures) {
  const registry = await block.registry;
  report.signer = { address: snapshot.feePayerCandidates[0] };
  const account = await rawQuery(block, 'system', 'account', [report.signer.address]);
  let initialFee = 0n;
  for (const fixture of fixtures) {
    const estimate = await feeQuote(block, fixture, account);
    assert(estimate > 0n);
    if (estimate > initialFee) initialFee = estimate;
  }
  const minimum = initialFee * 2n + 10n ** 18n;
  report.signerSelection = { largestFixtureFee: initialFee.toString(), minimumSpendable: minimum.toString(), fixtureQuotes: fixtures.length }; save();
  for (const address of snapshot.feePayerCandidates) {
    const candidate = await rawQuery(block, 'system', 'account', [address]);
    if (spendable(candidate) > minimum) {
      report.signer = { address, free: candidate.data.free.toString(), minimumSpendable: minimum.toString(),
        selectedFromPublicAccounts: true, storageOverrides: [] };
      return;
    }
  }
  throw new Error('Public fee payer candidates lack spendable funding for the largest fixture fee');
}

async function signedCheck(block, label, call, expectSuccess, expectFree, expectedError = null) {
  const registry = await block.registry;
  const before = await rawQuery(block, 'system', 'account', [report.signer.address]);
  const tx = signedExtrinsic(registry, call, block, before);
  const feeResponse = await block.call('TransactionPaymentApi_query_info',
    [tx.toHex(), u8aToHex(registry.createType('u32', tx.encodedLength).toU8a())]);
  const quote = registry.createType('RuntimeDispatchInfo', feeResponse.result);
  assert(quote.partialFee.toBigInt() > 0n, label + ' must quote a positive fee before dispatch');
  const response = await block.call('BlockBuilder_apply_extrinsic', [tx.toHex()]);
  const applied = registry.createType('ApplyExtrinsicResult', response.result);
  const shadow = new Block(chain, block.number, block.hash, block, {
    header: await block.header, extrinsics: [], storage: block.storage,
    storageDiff: Object.fromEntries(response.storageDiff) });
  const after = await rawQuery(shadow, 'system', 'account', [report.signer.address]);
  const previousEvents = await rawQuery(block, 'system', 'events');
  const events = (await rawQuery(shadow, 'system', 'events')).slice(previousEvents.length)
    .map(({ event }) => ({ section: event.section, method: event.method, data: event.data.toJSON() }));
  const charged = before.data.free.toBigInt() - after.data.free.toBigInt();
  const row = { method: label, expectSuccess, expectFree, expectedError,
    payer: report.signer.address, callHex: call.toHex(), signed: tx.isSigned,
    outcome: applied.toJSON(), quotedFee: quote.partialFee.toString(), xorCharged: charged.toString(),
    before: before.toJSON(), after: after.toJSON(), events,
    storageDiffEntries: response.storageDiff.length };
  report.signedChecks.push(row); save();
  assert(applied.isOk, label + ' funded signed call must be admitted');
  assert.equal(applied.asOk.isOk, expectSuccess, label + ' dispatch result');
  if (expectedError && applied.asOk.isErr) {
    const error = applied.asOk.asErr;
    row.decodedError = error.isModule ? registry.findMetaError(error.asModule).name : error.type;
    assert.equal(row.decodedError, expectedError, label + ' expected dispatch error');
  }
  assert.equal(after.nonce.toBigInt(), before.nonce.toBigInt() + 1n);
  if (expectFree) assert.equal(charged, 0n, label + ' successful useful execution must refund');
  else assert(charged > 0n, label + ' execution must keep a positive fee');
  const paid = events.filter(event => event.section === 'transactionPayment' && event.method === 'TransactionFeePaid');
  assert.equal(paid.length, 1, label + ' must emit one actual fee event');
  assert(registry.createType('AccountId', paid[0].data[0]).eq(registry.createType('AccountId', report.signer.address)));
  assert.equal(BigInt(paid[0].data[1]), charged, label + ' event and balance must agree');
  assert.equal(BigInt(paid[0].data[2]), 0n);
  row.actualFeeEventVerified = true; row.passed = true; save();
  return shadow;
}
async function unsignedCheck(block, label, call) {
  const registry = await block.registry; const tx = new GenericExtrinsic(registry, call);
  const row = { method: label, callHex: call.toHex(), signed: tx.isSigned, sources: {} };
  report.unsignedChecks.push(row); save();
  for (const source of ['External', 'Local', 'InBlock']) {
    const response = await block.call('TaggedTransactionQueue_validate_transaction',
      [registry.createType('TransactionSource', source).toHex(), tx.toHex(), block.hash]);
    const validity = registry.createType('TransactionValidity', response.result);
    row.sources[source] = validity.toJSON();
    assert(validity.isErr, label + ' bare ' + source + ' must reject');
  }
  const response = await block.call('BlockBuilder_apply_extrinsic', [tx.toHex()]);
  const applied = registry.createType('ApplyExtrinsicResult', response.result);
  row.apply = applied.toJSON(); row.storageDiffEntries = response.storageDiff.length;
  assert(applied.isErr, label + ' bare block application must reject before dispatch');
  row.passed = true; save();
}
function defaultCall(meta, method, overrides = {}) {
  const values = method.meta.args.map(arg => {
    // Decorated transaction metadata stores the resolved type as Text (usually LookupN),
    // unlike raw portable metadata's numeric lookup ID.
    const encoded = arg.type.toString();
    const type = /^\d+$/.test(encoded) ? meta.registry.createLookupType(Number(encoded)) : encoded;
    return meta.registry.createType(type, overrides[arg.name.toString()]);
  });
  return method(...values);
}
async function requestedPreimageFixture(block) {
  if (!opts.syntheticFixtures) {
    report.fixtureLimitations.push('First requested-preimage provision requires --synthetic-fixtures; public-only state cannot request fresh arbitrary bytes as Root. Native Executive coverage verifies this route.');
    return false;
  }
  const meta = await block.meta;
  const bytes = Buffer.alloc(65536, 0x61); const hash = blake2AsHex(bytes);
  const key = storageKey(meta, 'preimage', 'requestStatusFor', [hash]);
  assert(!(await block.get(key.toHex())), 'synthetic request hash must be absent');
  const lookup = meta.registry.createLookupType(meta.query.preimage.requestStatusFor.meta.type.asMap.value);
  const value = meta.registry.createType(lookup, { Requested: { maybeTicket: null, count: 1, maybeLen: null } });
  const fixture = new Block(chain, block.number, block.hash, block, { header: await block.header,
    extrinsics: [], storage: block.storage });
  fixture.pushStorageLayer().setAll([[key.toHex(), value.toHex()]]);
  report.fixtureOverrides.push({ kind: 'requestedPreimage', fixtureOnly: true,
    purpose: 'Root-equivalent request for known synthetic bytes; transaction execution uses exact candidate Wasm',
    key: key.toHex(), value: value.toHex(), hash, bytes: bytes.length, sha256: sha256(bytes),
    publicStateBefore: null });
  const call = meta.tx.preimage.notePreimage(u8aToHex(bytes));
  progress('Exact Wasm: first requested preimage provision refunds');
  const supplied = await signedCheck(fixture, 'preimage.notePreimage first synthetic requested provision', call, true, true);
  const status = (await rawQuery(supplied, 'preimage', 'requestStatusFor', [hash])).toHex();
  progress('Exact Wasm: requested preimage replay retains fees');
  const replayed = await signedCheck(supplied, 'preimage.notePreimage synthetic requested replay', call, false, false, 'AlreadyNoted');
  assert.equal((await rawQuery(replayed, 'preimage', 'requestStatusFor', [hash])).toHex(), status);
  return true;
}
async function paidMigrationFixture(block) {
  assert(opts.syntheticFixtures, 'Paid migration coverage requires --synthetic-fixtures');
  const proofPath = resolve(__dirname, 'migration-proof-fixture.json');
  const proof = JSON.parse(readFileSync(proofPath));
  assert(proof.synthetic);
  const meta = await block.meta;
  const signer = meta.registry.createType('AccountId', proof.account);
  const previousSigner = report.signer;
  report.signer = { address: signer.toString(), synthetic: true };
  const val = '0x0200040000000000000000000000000000000000000000000000000000000000';
  const claim = 300n * 10n ** 18n;
  // The legacy claim pallet transfers from Generic("bridge", "main"), which
  // need not equal the currently configured bridge peer multisig account.
  const technicalType = meta.registry.createLookupType(meta.query.technical.techAccounts.meta.type.asMap.value);
  const technical = meta.registry.createType(technicalType, { Generic: [u8aToHex(Buffer.from('bridge')), u8aToHex(Buffer.from('main'))] });
  const escrow = meta.registry.createType('AccountId', Buffer.concat([
    Buffer.from([84, 115, 79, 144, 249, 113, 160, 44, 96, 155, 45, 104, 78, 97, 181, 87]),
    Buffer.from(xxhashAsU8a(technical.toU8a(), 128)),
  ]));
  assert.equal((await rawQuery(block, 'technical', 'techAccounts', [escrow])).unwrap().toHex(), technical.toHex());
  const escrowBefore = (await rawQuery(block, 'tokens', 'accounts', [escrow, val])).free.toBigInt();
  assert(escrowBefore >= claim, 'Pinned bridge escrow must back the synthetic VAL claim');
  assert.equal((await rawQuery(block, 'tokens', 'accounts', [signer, val])).free.toBigInt(), 0n);
  for (const name of ['balances', 'publicKeys', 'migratedAccounts', 'referrers']) {
    assert(!(await block.get(storageKey(meta, 'irohaMigration', name, [proof.address]).toHex())),
      'Synthetic migration address must not collide with public state');
  }
  assert(!meta.tx.irohaMigration.sponsorMigration && !meta.tx.irohaMigration.revokeSponsorship);
  assert(!meta.query.irohaMigration.feeSponsorships);
  const override = async (parent, label, rows) => {
    const values = [];
    for (const [pallet, name, params, json] of rows) {
      const key = storageKey(meta, pallet, name, params);
      const value = meta.registry.createType(key.outputType, json);
      const encoded = u8aToHex(value.toU8a());
      const before = await parent.get(key.toHex());
      values.push([key.toHex(), encoded]);
      report.fixtureOverrides.push({ kind: 'paidMigration', fixtureOnly: true, label,
        pallet, name, key: key.toHex(), value: encoded, parentStateBefore: before ?? null });
    }
    const branch = new Block(chain, parent.number, parent.hash, parent, {
      header: await parent.header, extrinsics: [], storage: parent.storage });
    branch.pushStorageLayer().setAll(values); return branch;
  };
  const account = (await rawQuery(block, 'system', 'account', [signer])).toJSON();
  assert.equal(BigInt(account.data.free), 0n);
  account.providers = 1; account.nonce = 0;
  const zero = await override(block, 'backed claim and existing zero-XOR recipient', [
    ['system', 'account', [signer], account],
    ['irohaMigration', 'balances', [proof.address], claim.toString()],
    ['irohaMigration', 'publicKeys', [proof.address], [[false, proof.publicKey]]],
  ]);
  const call = meta.tx.irohaMigration.migrate(proof.address, proof.publicKey, proof.signature);
  const tx = signedExtrinsic(meta.registry, call, zero, await rawQuery(zero, 'system', 'account', [signer]));
  const rejectedRow = { method: 'irohaMigration.migrate zero-XOR valid VAL claim', sources: {},
    account: signer.toString(), proofIsSynthetic: true };
  report.zeroXorMigration = rejectedRow;
  progress('Exact Wasm: migration requires XOR before a valid VAL claim can execute');
  for (const source of ['External', 'Local', 'InBlock']) {
    const response = await zero.call('TaggedTransactionQueue_validate_transaction',
      [meta.registry.createType('TransactionSource', source).toHex(), tx.toHex(), zero.hash]);
    const validity = meta.registry.createType('TransactionValidity', response.result);
    rejectedRow.sources[source] = validity.toJSON();
    assert(validity.isErr && validity.asErr.isInvalid && validity.asErr.asInvalid.isPayment);
  }
  const rejected = await zero.call('BlockBuilder_apply_extrinsic', [tx.toHex()]);
  const rejectedResult = meta.registry.createType('ApplyExtrinsicResult', rejected.result);
  rejectedRow.apply = rejectedResult.toJSON();
  assert(rejectedResult.isErr && rejectedResult.asErr.isInvalid && rejectedResult.asErr.asInvalid.isPayment);
  const rejectedBlock = new Block(chain, zero.number, zero.hash, zero, {
    header: await zero.header, extrinsics: [], storage: zero.storage,
    storageDiff: Object.fromEntries(rejected.storageDiff) });
  for (const [pallet, name, params] of [['system', 'account', [signer]],
    ['tokens', 'accounts', [signer, val]], ['tokens', 'accounts', [escrow, val]],
    ['irohaMigration', 'balances', [proof.address]], ['irohaMigration', 'publicKeys', [proof.address]],
    ['irohaMigration', 'migratedAccounts', [proof.address]]]) {
    assert.equal((await rawQuery(rejectedBlock, pallet, name, params)).toHex(),
      (await rawQuery(zero, pallet, name, params)).toHex());
  }
  rejectedRow.claimBalancesAndNoncePreserved = true; rejectedRow.passed = true;
  const funding = 10n ** 30n;
  const fundedAccount = structuredClone(account); fundedAccount.data.free = funding.toString();
  const funded = await override(zero, 'XOR-funded claimant and matching native issuance', [
    ['system', 'account', [signer], fundedAccount],
    ['balances', 'totalIssuance', [], ((await rawQuery(zero, 'balances', 'totalIssuance')).toBigInt() + funding).toString()],
  ]);
  progress('Exact Wasm: successful migration delivers VAL and retains the XOR fee');
  const success = await signedCheck(funded, 'irohaMigration.migrate paid success', call, true, false);
  assert.equal((await rawQuery(success, 'tokens', 'accounts', [signer, val])).free.toBigInt(), claim);
  assert.equal((await rawQuery(success, 'tokens', 'accounts', [escrow, val])).free.toBigInt(), escrowBefore - claim);
  assert((await rawQuery(success, 'irohaMigration', 'migratedAccounts', [proof.address])).unwrap().eq(signer));
  assert((await rawQuery(success, 'irohaMigration', 'balances', [proof.address])).isNone);
  const replay = await signedCheck(success, 'irohaMigration.migrate paid replay', call, false, false, 'AccountAlreadyMigrated');
  assert.equal((await rawQuery(replay, 'tokens', 'accounts', [signer, val])).free.toBigInt(), claim);
  const referrer = '0x' + '50'.repeat(32);
  const failure = await override(funded, 'referral conflict after valid ownership and VAL transfer', [
    ['irohaMigration', 'referrers', [proof.address], 'paid-wasm-referrer@sora'],
    ['irohaMigration', 'migratedAccounts', ['paid-wasm-referrer@sora'], referrer],
    ['referrals', 'referrers', [signer], referrer],
  ]);
  progress('Exact Wasm: failed migration keeps its fee and rolls back the VAL claim');
  const failed = await signedCheck(failure, 'irohaMigration.migrate paid settlement failure', call, false, false, 'ReferralMigrationFailed');
  for (const [pallet, name, params] of [['tokens', 'accounts', [signer, val]],
    ['tokens', 'accounts', [escrow, val]], ['irohaMigration', 'balances', [proof.address]],
    ['irohaMigration', 'publicKeys', [proof.address]], ['irohaMigration', 'migratedAccounts', [proof.address]]]) {
    assert.equal((await rawQuery(failed, pallet, name, params)).toHex(),
      (await rawQuery(failure, pallet, name, params)).toHex());
  }
  report.paidMigration = { passed: true, claimVal: '300', ownershipProofVerifiedByWasm: true,
    syntheticProofSha256: sha256(readFileSync(proofPath)),
    proofGeneratorSha256: sha256(readFileSync(resolve(__dirname, 'migration-proof-fixture.rs'))),
    successRetainsXorFee: true, failureRetainsXorFee: true, failureRollsBackValAndClaim: true,
    replayPaidWithoutDuplicateVal: true, sponsorshipAbiAbsent: true };
  report.signer = previousSigner; save(); return true;
}
async function publicCancellation(block) {
  const meta = await block.meta;
  const prefix = meta.query.orderBook.limitOrders.keyPrefix().toHex();
  const keys = await chain.api.getKeysPaged(prefix, 64, prefix, snapshot.blockHash);
  report.cancellationSearch = { boundedPublicKeys: 64, inspectedKeys: keys.length, injectedState: false };
  const savedSigner = report.signer;
  const candidates = [];
  for (const hex of keys) {
    const key = new StorageKey(meta.registry, hex); key.setMeta(meta.query.orderBook.limitOrders.meta);
    const [id, orderId] = key.args;
    const stored = await rawQuery(block, 'orderBook', 'limitOrders', [id, orderId]);
    if (stored.isNone) continue; // The initialized local block may already expire a public order.
    const order = stored.unwrap();
    // Avoid conflating native-XOR order unlocks with transaction fee balance deltas.
    if (order.side.toString().toLowerCase() !== 'sell') continue;
    const native = '0x0200000000000000000000000000000000000000000000000000000000000000';
    if (id.base.toHex() === native) continue;
    const account = await rawQuery(block, 'system', 'account', [order.owner]);
    const storedBook = await rawQuery(block, 'orderBook', 'orderBooks', [id]);
    if (storedBook.isNone) continue;
    const book = storedBook.unwrap();
    if (!['Trade', 'PlaceAndCancel', 'OnlyCancel'].includes(book.status.toString())) continue;
    candidates.push({ id, orderId, order, account });
  }
  // Prefer real public funding before creating a labeled fixture supplement.
  candidates.sort((a, b) => spendable(a.account) > spendable(b.account) ? -1 :
    spendable(a.account) < spendable(b.account) ? 1 : 0);
  for (const { id, orderId, order, account } of candidates) {
    report.signer = { address: order.owner.toString(), selectedFromPublicOrderOwner: true,
      storageOverrides: [], free: account.data.free.toString() };
    const calls = [
      ['cancelLimitOrder', meta.tx.orderBook.cancelLimitOrder(id, orderId)],
      ['cancelLimitOrdersBatch', meta.tx.orderBook.cancelLimitOrdersBatch([[id, [orderId]]])],
    ];
    let maximum = 0n;
    for (const [, call] of calls) {
      const quote = await feeQuote(block, call, account);
      if (quote > maximum) maximum = quote;
    }
    let execution = block;
    let syntheticFunding = false;
    const minimum = maximum * 2n + 10n ** 18n;
    if (spendable(account) <= minimum) {
      if (!opts.syntheticFixtures) { report.signer = savedSigner; continue; }
      const frozen = account.data.frozen?.toBigInt() ?? account.data.feeFrozen?.toBigInt() ?? 0n;
      const funded = frozen + minimum + 1n;
      const json = account.toJSON(); json.data.free = funded.toString();
      // Existing sell orders retain their real public storage. Only the local
      // account/issuance funding is supplemented for upfront-fee admission.
      if (account.providers.toNumber() === 0 && account.sufficients.toNumber() === 0) json.providers = 1;
      const accountKey = storageKey(meta, 'system', 'account', [order.owner]);
      const accountValue = meta.registry.createType(accountKey.outputType, json);
      const issuance = await rawQuery(block, 'balances', 'totalIssuance');
      const issuanceKey = storageKey(meta, 'balances', 'totalIssuance');
      const issuanceValue = meta.registry.createType(issuanceKey.outputType,
        (issuance.toBigInt() + funded - account.data.free.toBigInt()).toString());
      execution = new Block(chain, block.number, block.hash, block, { header: await block.header,
        extrinsics: [], storage: block.storage });
      execution.pushStorageLayer().setAll([[accountKey.toHex(), accountValue.toHex()],
        [issuanceKey.toHex(), u8aToHex(issuanceValue.toU8a())]]);
      const overrides = [
        { key: accountKey.toHex(), before: account.toHex(), value: accountValue.toHex() },
        { key: issuanceKey.toHex(), before: await block.get(issuanceKey.toHex()) ?? null,
          value: u8aToHex(issuanceValue.toU8a()) },
      ];
      report.fixtureOverrides.push({ kind: 'existingPublicOrderOwnerFunding', fixtureOnly: true,
        purpose: 'Keep actual public sell-order storage while funding exact-Wasm fee admission locally',
        owner: order.owner.toString(), fundedFree: funded.toString(), overrides });
      report.signer.storageOverrides = overrides.map(row => row.key); syntheticFunding = true;
    }
    for (const [method, call] of calls) {
      progress('Exact Wasm: useful public order cancellation refunds ' + method);
      const cancelled = await signedCheck(execution, 'orderBook.' + method + ' existing public sell order', call, true, true);
      assert((await rawQuery(cancelled, 'orderBook', 'limitOrders', [id, orderId])).isNone);
      progress('Exact Wasm: cancelled order replay retains fees ' + method);
      await signedCheck(cancelled, 'orderBook.' + method + ' public cancelled-order replay', call, false, false, 'UnknownLimitOrder');
    }
    report.publicCancellation = { book: id.toJSON(), orderId: orderId.toJSON(), owner: order.owner.toString(),
      methods: calls.map(([method]) => method), freeSuccess: true, paidReplay: true,
      selectedFromPublicState: true, syntheticFunding, storageOverrides: report.signer.storageOverrides };
    report.signer = savedSigner; return true;
  }
  report.signer = savedSigner;
  report.fixtureLimitations.push('The bounded 64-key search had no cancellable non-XOR-base public sell order; useful cancellation uses an explicitly recorded local order placed through candidate Wasm on an existing public book.');
  if (!opts.syntheticFixtures) return false;
  return syntheticCancellation(block);
}
async function syntheticCancellation(block) {
  const meta = await block.meta;
  const prefix = meta.query.orderBook.orderBooks.keyPrefix().toHex();
  const keys = await chain.api.getKeysPaged(prefix, 32, prefix, snapshot.blockHash);
  report.cancellationSearch.boundedPublicBookKeys = 32;
  report.cancellationSearch.inspectedBookKeys = keys.length;
  const native = '0x0200000000000000000000000000000000000000000000000000000000000000';
  let selected;
  for (const hex of keys) {
    const key = new StorageKey(meta.registry, hex); key.setMeta(meta.query.orderBook.orderBooks.meta);
    const [id] = key.args;
    const stored = await rawQuery(block, 'orderBook', 'orderBooks', [id]);
    if (stored.isNone || id.base.toHex() === native) continue;
    const book = stored.unwrap();
    if (!['Trade', 'PlaceAndCancel'].includes(book.status.toString()) || book.techStatus.toString() !== 'Ready') continue;
    if (!book.minLotSize.isDivisible.isTrue) continue;
    selected = { id, book }; break;
  }
  assert(selected, 'The bounded search must find an existing divisible public book for synthetic placement');
  const { id, book } = selected;
  const owner = meta.registry.createType('AccountId', report.signer.address);
  const amount = book.minLotSize.inner.toBigInt();
  const price = book.tickSize.inner.toBigInt() * 100n;
  assert(amount > 0n && price > 0n);
  const tokenKey = storageKey(meta, 'tokens', 'accounts', [owner, id.base]);
  const tokenRaw = await block.get(tokenKey.toHex());
  const token = await rawQuery(block, 'tokens', 'accounts', [owner, id.base]);
  const tokenJson = token.toJSON();
  tokenJson.free = (token.free.toBigInt() + token.frozen.toBigInt() + amount).toString();
  const tokenValue = meta.registry.createType(tokenKey.outputType, tokenJson);
  const added = tokenValue.free.toBigInt() - token.free.toBigInt();
  const issuanceKey = storageKey(meta, 'tokens', 'totalIssuance', [id.base]);
  const issuance = await rawQuery(block, 'tokens', 'totalIssuance', [id.base]);
  const issuanceValue = meta.registry.createType(issuanceKey.outputType, (issuance.toBigInt() + added).toString());
  const overrides = [
    { pallet: 'tokens', name: 'accounts', key: tokenKey.toHex(), before: tokenRaw ?? null, value: tokenValue.toHex() },
    { pallet: 'tokens', name: 'totalIssuance', key: issuanceKey.toHex(), before: await block.get(issuanceKey.toHex()) ?? null,
      value: u8aToHex(issuanceValue.toU8a()) },
  ];
  // ORML Tokens adds one System provider for each newly created token account.
  // Match that invariant when the synthetic base-asset account did not exist.
  if (!tokenRaw) {
    const accountKey = storageKey(meta, 'system', 'account', [owner]);
    const account = await rawQuery(block, 'system', 'account', [owner]);
    const json = account.toJSON(); json.providers = account.providers.toNumber() + 1;
    const value = meta.registry.createType(accountKey.outputType, json);
    overrides.push({ pallet: 'system', name: 'account', key: accountKey.toHex(), before: account.toHex(), value: value.toHex() });
  }
  const fixture = new Block(chain, block.number, block.hash, block, { header: await block.header,
    extrinsics: [], storage: block.storage });
  fixture.pushStorageLayer().setAll(overrides.map(({ key, value }) => [key, value]));
  report.fixtureOverrides.push({ kind: 'syntheticSellOrderFunding', fixtureOnly: true,
    purpose: 'Fund non-XOR base asset locally, preserve issuance/provider invariants, then place a real order through exact candidate Wasm on an existing public book',
    owner: owner.toString(), book: id.toJSON(), publicBook: book.toJSON(), addedBaseAsset: added.toString(), overrides });
  report.cancellationSearch.injectedState = true;
  progress('Exact Wasm: place useful synthetic sell order on existing public book');
  const placed = await signedCheck(fixture, 'orderBook.placeLimitOrder synthetic sell fixture',
    meta.tx.orderBook.placeLimitOrder(id, price.toString(), amount.toString(), 'Sell', 100000), true, false);
  const orderId = (await rawQuery(placed, 'orderBook', 'orderBooks', [id])).unwrap().lastOrderId;
  const order = await rawQuery(placed, 'orderBook', 'limitOrders', [id, orderId]);
  assert(order.isSome && order.unwrap().owner.eq(owner) && order.unwrap().side.isSell,
    'Candidate placement must create a cancellable sell order for the funded owner');
  const locked = await rawQuery(placed, 'tokens', 'accounts', [owner, id.base]);
  assert.equal(tokenValue.free.toBigInt() - locked.free.toBigInt(), amount, 'Placement must actually lock the base asset');
  const calls = [
    ['cancelLimitOrder', meta.tx.orderBook.cancelLimitOrder(id, orderId)],
    ['cancelLimitOrdersBatch', meta.tx.orderBook.cancelLimitOrdersBatch([[id, [orderId]]])],
  ];
  for (const [method, call] of calls) {
    progress('Exact Wasm: useful synthetic order cancellation refunds ' + method);
    const cancelled = await signedCheck(placed, 'orderBook.' + method + ' synthetic sell order', call, true, true);
    assert((await rawQuery(cancelled, 'orderBook', 'limitOrders', [id, orderId])).isNone);
    const unlocked = await rawQuery(cancelled, 'tokens', 'accounts', [owner, id.base]);
    assert.equal(unlocked.free.toBigInt() - locked.free.toBigInt(), amount, 'Cancellation must return the locked base asset');
    assert.equal((await rawQuery(cancelled, 'tokens', 'totalIssuance', [id.base])).toBigInt(), issuanceValue.toBigInt());
    progress('Exact Wasm: cancelled synthetic order replay retains fees ' + method);
    await signedCheck(cancelled, 'orderBook.' + method + ' synthetic cancelled-order replay', call, false, false, 'UnknownLimitOrder');
  }
  report.syntheticCancellation = { fixtureOnly: true, book: id.toJSON(), orderId: orderId.toJSON(), owner: owner.toString(),
    methods: calls.map(([method]) => method), placementExecutedByCandidate: true, realBaseAssetLockAndUnlock: true,
    freeSuccess: true, paidReplay: true, selectedBookFromPublicState: true, overrides };
  save(); return true;
}
async function zeroXorLegacyBridgeFixture(block) {
  assert(opts.syntheticFixtures, 'Required zero-XOR legacy peer Wasm coverage requires --synthetic-fixtures');
  const meta = await block.meta;
  const bridge = await rawQuery(block, 'ethBridge', 'bridgeAccount', [0]);
  assert(bridge.isSome, 'Public legacy network zero must have a bridge account');
  const id = bridge.unwrap();
  const publicKey = sr25519PairFromSeed(new Uint8Array(32).fill(91)).publicKey;
  const signer = meta.registry.createType('AccountId', publicKey);
  const remoteHash = '0x' + '71'.repeat(32);
  for (const name of ['requests', 'loadToIncomingRequestHash']) {
    const key = storageKey(meta, 'ethBridge', name, [0, remoteHash]);
    assert(!(await block.get(key.toHex())), 'Synthetic bridge request hash must be absent: ' + name);
  }
  const overrides = [];
  const add = async (pallet, name, params, json) => {
    const key = storageKey(meta, pallet, name, params);
    const before = await block.get(key.toHex());
    const value = meta.registry.createType(key.outputType, json);
    overrides.push({ key: key.toHex(), before: before ?? null, value: value.toHex(), pallet, name });
  };
  await add('bridgeMultisig', 'accounts', [id], { signatories: [signer.toHex()], threshold: 67 });
  await add('ethBridge', 'peers', [0], [signer.toHex()]);
  const account = await rawQuery(block, 'system', 'account', [signer]);
  assert.equal(account.data.free.toBigInt(), 0n, 'Known synthetic signer must have no public XOR');
  const zero = account.toJSON(); zero.nonce = 0; zero.providers = 1; zero.consumers = 0; zero.sufficients = 0;
  zero.data.free = '0'; zero.data.reserved = '0'; zero.data.frozen = '0';
  await add('system', 'account', [signer], zero);
  const fixture = new Block(chain, block.number, block.hash, block, {
    header: await block.header, extrinsics: [], storage: block.storage });
  fixture.pushStorageLayer().setAll(overrides.map(row => [row.key, row.value]));
  report.fixtureOverrides.push({ kind: 'zeroXorLegacyBridgePeer', fixtureOnly: true,
    purpose: 'Synthetic single-peer quorum accepts an authenticated protocol report of a remote failure; real public network/bridge account retained',
    network: 0, bridgeAccount: id.toString(), signer: signer.toString(), remoteHash,
    signerSource: 'Deterministic public test seed [91;32], fake transaction signature', overrides });
  const timepoint = { height: { Sidechain: 10 }, index: 17 };
  const load = { Transaction: { author: signer.toHex(), hash: remoteHash, timepoint,
    kind: 'Transfer', networkId: 0 } };
  const inner = meta.tx.ethBridge.importIncomingRequest(load, { Err: 'CannotLookup' });
  const call = meta.tx.bridgeMultisig.asMultiThreshold1(id, inner, timepoint);
  const previousSigner = report.signer;
  report.signer = { address: signer.toString(), free: '0', syntheticZeroXorPeer: true,
    storageOverrides: overrides.map(row => row.key) };
  progress('Exact Wasm: authenticated legacy peer reports remote failure with zero XOR');
  const accepted = await signedCheck(fixture, 'bridgeMultisig.asMultiThreshold1 accepted synthetic remote failure',
    call, true, true);
  const after = await rawQuery(accepted, 'system', 'account', [signer]);
  assert.equal(after.data.free.toBigInt(), 0n); assert.equal(after.nonce.toBigInt(), 1n);
  const status = await rawQuery(accepted, 'ethBridge', 'requestStatuses', [0, remoteHash]);
  assert(status.isSome && status.unwrap().isFailed, 'Accepted remote failure must consume the load request');
  const acceptedRow = report.signedChecks.at(-1);
  assert(acceptedRow.events.some(event => event.section === 'ethBridge' && event.method === 'RequestAborted'));
  progress('Exact Wasm: authenticated legacy protocol replay rejects before execution');
  const replay = meta.tx.bridgeMultisig.asMultiThreshold1(id, inner, { height: { Sidechain: 10 }, index: 18 });
  const tx = signedExtrinsic(meta.registry, replay, accepted, after);
  const row = { method: 'bridgeMultisig.asMultiThreshold1 consumed load replay with changed outer timepoint',
    signed: tx.isSigned, payer: signer.toString(), sources: {}, callHex: replay.toHex() };
  report.bridgeProtocolReplay = row; save();
  for (const source of ['External', 'Local', 'InBlock']) {
    const response = await accepted.call('TaggedTransactionQueue_validate_transaction',
      [meta.registry.createType('TransactionSource', source).toHex(), tx.toHex(), accepted.hash]);
    const validity = meta.registry.createType('TransactionValidity', response.result);
    row.sources[source] = validity.toJSON();
    assert(validity.isErr && validity.asErr.isInvalid && validity.asErr.asInvalid.isCall);
  }
  const response = await accepted.call('BlockBuilder_apply_extrinsic', [tx.toHex()]);
  const result = meta.registry.createType('ApplyExtrinsicResult', response.result);
  row.apply = result.toJSON(); row.storageDiffEntries = response.storageDiff.length;
  assert(result.isErr && result.asErr.isInvalid && result.asErr.asInvalid.isCall);
  const rejected = new Block(chain, accepted.number, accepted.hash, accepted, {
    header: await accepted.header, extrinsics: [], storage: accepted.storage,
    storageDiff: Object.fromEntries(response.storageDiff) });
  assert.equal((await rawQuery(rejected, 'system', 'account', [signer])).toHex(), after.toHex(),
    'Rejected protocol replay must neither charge nor consume the nonce');
  assert.equal((await rawQuery(rejected, 'ethBridge', 'requestStatuses', [0, remoteHash])).toHex(), status.toHex());
  row.passed = true; report.signer = previousSigner; save(); return true;
}
async function capacityAccountingBranch(block, label) {
  const meta = await block.meta;
  const overrides = [];
  for (const name of ['blockWeight', 'blockSize']) {
    const key = storageKey(meta, 'system', name);
    overrides.push({ pallet: 'system', name, key: key.toHex(), before: await block.get(key.toHex()) ?? null, value: null });
  }
  const next = new Block(chain, block.number, block.hash, block, { header: await block.header,
    extrinsics: [], storage: block.storage });
  next.pushStorageLayer().setAll(overrides.map(({ key, value }) => [key, value]));
  report.fixtureOverrides.push({ kind: 'capacityTransactionAccounting', fixtureOnly: true, label,
    purpose: 'Isolate each exact-Wasm transaction admission from aggregate block assembly while retaining protocol state, balances and nonces; this does not claim all fixture transactions fit one block', overrides });
  return next;
}
async function capacitySignedReject(block, label, call, reason = 'Call') {
  const registry = await block.registry;
  const before = await rawQuery(block, 'system', 'account', [report.signer.address]);
  const tx = signedExtrinsic(registry, call, block, before);
  const beforeEvents = (await rawQuery(block, 'system', 'events')).toHex();
  const row = { method: label, signed: tx.isSigned, signer: report.signer.address,
    callHex: call.toHex(), expectedInvalidReason: reason, sources: {} };
  (report.capacityRejections ||= []).push(row);
  for (const source of ['External', 'Local', 'InBlock']) {
    const response = await block.call('TaggedTransactionQueue_validate_transaction',
      [registry.createType('TransactionSource', source).toHex(), tx.toHex(), block.hash]);
    const result = registry.createType('TransactionValidity', response.result);
    row.sources[source] = result.toJSON();
    assert(result.isErr && result.asErr.isInvalid && result.asErr.asInvalid.type === reason, label);
  }
  const response = await block.call('BlockBuilder_apply_extrinsic', [tx.toHex()]);
  const result = registry.createType('ApplyExtrinsicResult', response.result);
  row.apply = result.toJSON();
  assert(result.isErr && result.asErr.isInvalid && result.asErr.asInvalid.type === reason, label);
  const rejected = new Block(chain, block.number, block.hash, block, { header: await block.header,
    extrinsics: [], storage: block.storage, storageDiff: Object.fromEntries(response.storageDiff) });
  assert.equal((await rawQuery(rejected, 'system', 'account', [report.signer.address])).toHex(), before.toHex());
  assert.equal((await rawQuery(rejected, 'system', 'events')).toHex(), beforeEvents,
    'Rejected capacity transaction must emit no fee or protocol events');
  row.nonceAndZeroXorPreserved = true; row.passed = true; save();
}
async function bridgeCapacityFixture(block) {
  assert(opts.syntheticFixtures, 'Capacity rehearsal requires explicitly recorded synthetic peers/state');
  const meta = await block.meta;
  const bridge = await rawQuery(block, 'ethBridge', 'bridgeAccount', [0]); assert(bridge.isSome);
  const id = bridge.unwrap();
  const peers = [101, 102, 103, 104].map(seed => meta.registry.createType('AccountId',
    sr25519PairFromSeed(new Uint8Array(32).fill(seed)).publicKey));
  const sorted = values => values.map(value => value.toHex()).sort();
  const overrides = [];
  const add = async (target, pallet, name, params, json) => {
    const key = storageKey(meta, pallet, name, params);
    const value = meta.registry.createType(key.outputType, json);
    const row = { pallet, name, key: key.toHex(), before: await target.get(key.toHex()) ?? null,
      value: u8aToHex(value.toU8a()) };
    return row;
  };
  overrides.push(await add(block, 'bridgeMultisig', 'accounts', [id], { signatories: sorted(peers), threshold: 67 }));
  overrides.push(await add(block, 'ethBridge', 'peers', [0], sorted(peers)));
  assert.equal((await rawQuery(block, 'bridgeMultisig', 'newPendingOperations', [id])).toNumber(), 0);
  for (const peer of peers) {
    const account = await rawQuery(block, 'system', 'account', [peer]);
    assert.equal(account.data.free.toBigInt(), 0n); assert.equal(account.data.reserved.toBigInt(), 0n);
    const zero = account.toJSON(); zero.nonce = 0; zero.providers = 1; zero.consumers = 0; zero.sufficients = 0;
    zero.data.free = '0'; zero.data.reserved = '0'; zero.data.frozen = '0';
    overrides.push(await add(block, 'system', 'account', [peer], zero));
  }
  const fixture = new Block(chain, block.number, block.hash, block, { header: await block.header,
    extrinsics: [], storage: block.storage });
  fixture.pushStorageLayer().setAll(overrides.map(({ key, value }) => [key, value]));
  report.fixtureOverrides.push({ kind: 'capacityFourZeroXorPeers', fixtureOnly: true,
    purpose: 'Retain the actual initialized legacy network/bridge account; model four current synthetic peers with a three-peer quorum and no XOR',
    network: 0, bridgeAccount: id.toString(), peers: peers.map(String), quorum: 3, overrides });
  const previousSigner = report.signer;
  const usePeer = peer => { report.signer = { address: peer.toString(), syntheticZeroXorPeer: true, free: '0' }; };
  const protocol = byte => {
    const remoteHash = '0x' + byte.toString(16).padStart(2, '0').repeat(32);
    const timepoint = { height: { Sidechain: 10 }, index: byte };
    const load = { Transaction: { author: peers[0].toHex(), hash: remoteHash, timepoint, kind: 'Transfer', networkId: 0 } };
    const inner = meta.tx.ethBridge.importIncomingRequest(load, { Err: 'CannotLookup' });
    const hash = blake2AsHex(inner.toU8a());
    const outer = meta.tx.bridgeMultisig.asMulti(id, timepoint, inner.toHex(), false, { refTime: 0, proofSize: 0 });
    return { inner, outer, hash, timepoint, remoteHash };
  };
  usePeer(peers[0]); let fair = fixture;
  progress('Exact Wasm: one zero-XOR peer opens its 32-operation fair quota');
  for (let byte = 100; byte < 132; byte++) {
    fair = await capacityAccountingBranch(fair, 'quota opening ' + byte);
    fair = await signedCheck(fair, 'capacity.asMulti opening ' + byte, protocol(byte).outer, true, true);
    assert.equal((await rawQuery(fair, 'bridgeMultisig', 'newPendingOperations', [id])).toNumber(), byte - 99);
    assert.equal((await rawQuery(fair, 'bridgeMultisig', 'pendingOperationsByProposer', [id, peers[0]])).toNumber(), byte - 99);
  }
  assert.equal((await rawQuery(fair, 'system', 'account', [peers[0]])).nonce.toNumber(), 32);
  fair = await capacityAccountingBranch(fair, '33rd proposal rejection');
  await capacitySignedReject(fair, 'capacity.asMulti same-peer 33rd opening rejects', protocol(132).outer);
  usePeer(peers[1]); fair = await capacityAccountingBranch(fair, 'honest peer admission');
  fair = await signedCheck(fair, 'capacity.asMulti another zero-XOR peer retains capacity', protocol(200).outer, true, true);
  assert.equal((await rawQuery(fair, 'bridgeMultisig', 'newPendingOperations', [id])).toNumber(), 33);
  assert.equal((await rawQuery(fair, 'bridgeMultisig', 'pendingOperationsByProposer', [id, peers[1]])).toNumber(), 1);
  report.capacityFairQuota = { peerQuota: 32, actualCandidateOpenings: 32, rejectedSamePeerOpening: 33,
    honestPeerAdmitted: true, globalCountAfterHonestAdmission: 33, passed: true };
  // Independent full-queue branch: create the target through Wasm, then model a
  // pre-fix counted backlog, stale protocol state and a removed proposer.
  const target = protocol(211); usePeer(peers[0]);
  let full = await capacityAccountingBranch(fixture, 'orphan target creation');
  full = await signedCheck(full, 'capacity.asMulti orphan target creation', target.outer, true, true);
  assert((await rawQuery(full, 'bridgeMultisig', 'calls', [target.hash])).isNone);
  const backlog = [];
  for (let byte = 0; byte < 127; byte++) {
    const hash = '0x' + byte.toString(16).padStart(2, '0').repeat(32); assert.notEqual(hash, target.hash);
    backlog.push(await add(full, 'bridgeMultisig', 'multisigs', [id, hash], {
      when: target.timepoint, deposit: 999, depositor: peers[0].toHex(), approvals: [peers[0].toHex()] }));
    backlog.push(await add(full, 'bridgeMultisig', 'countedNewOperations', [id, hash], null));
  }
  backlog.push(await add(full, 'bridgeMultisig', 'newPendingOperations', [id], 128));
  backlog.push(await add(full, 'bridgeMultisig', 'dispatchedCalls', [target.hash, target.timepoint], null));
  const canonicalHash = '0x' + 'ab'.repeat(32);
  backlog.push(await add(full, 'ethBridge', 'loadToIncomingRequestHash', [0, target.remoteHash], canonicalHash));
  backlog.push(await add(full, 'ethBridge', 'requestStatuses', [0, canonicalHash], { Failed: 'CannotLookup' }));
  backlog.push(await add(full, 'bridgeMultisig', 'accounts', [id], { signatories: sorted(peers.slice(1)), threshold: 67 }));
  backlog.push(await add(full, 'ethBridge', 'peers', [0], sorted(peers.slice(1))));
  const filled = new Block(chain, full.number, full.hash, full, { header: await full.header,
    extrinsics: [], storage: full.storage });
  filled.pushStorageLayer().setAll(backlog.map(({ key, value }) => [key, value])); full = filled;
  report.fixtureOverrides.push({ kind: 'capacityFullOrphanedQueue', fixtureOnly: true,
    purpose: 'Model 127 legacy counted operations plus the candidate-created target, absent stored call bytes, stale canonical protocol state, a global tombstone from an unrelated multisig, and removal of the proposer; remaining three peers form the current quorum',
    targetCallHash: target.hash, targetTimepoint: target.timepoint, canonicalHash, overrides: backlog });
  usePeer(peers[1]); full = await capacityAccountingBranch(full, 'full-queue new-opening rejection');
  await capacitySignedReject(full, 'capacity.asMulti full shared queue rejects a new proposal', protocol(212).outer);
  const cancel = meta.tx.ethBridge.cancelPendingMultisig(0, target.hash, target.timepoint);
  usePeer(peers[0]); full = await capacityAccountingBranch(full, 'removed proposer rejection');
  await capacitySignedReject(full, 'ethBridge.cancelPendingMultisig removed proposer rejects', cancel, 'Payment');
  usePeer(peers[1]); full = await capacityAccountingBranch(full, 'wrong timepoint rejection');
  await capacitySignedReject(full, 'ethBridge.cancelPendingMultisig wrong timepoint rejects',
    meta.tx.ethBridge.cancelPendingMultisig(0, target.hash, { height: { Sidechain: 10 }, index: 212 }));
  const protocolBefore = (await rawQuery(full, 'ethBridge', 'requestStatuses', [0, canonicalHash])).toHex();
  progress('Exact Wasm: current zero-XOR quorum releases a full orphaned queue without proposer or payload');
  for (let index = 1; index < 4; index++) {
    usePeer(peers[index]); full = await capacityAccountingBranch(full, 'quorum cancellation vote ' + index);
    full = await signedCheck(full, 'ethBridge.cancelPendingMultisig current zero-XOR vote ' + index, cancel, true, true);
    assert(!report.signedChecks.at(-1).events.some(event => event.section === 'bridgeMultisig' && event.method === 'MultisigExecuted'));
    assert.equal((await rawQuery(full, 'bridgeMultisig', 'newPendingOperations', [id])).toNumber(), index < 3 ? 128 : 127);
    assert.equal((await rawQuery(full, 'bridgeMultisig', 'pendingOperationsByProposer', [id, peers[0]])).toNumber(), index < 3 ? 1 : 0);
    assert.equal((await rawQuery(full, 'bridgeMultisig', 'multisigs', [id, target.hash])).isSome, index < 3);
    if (index === 1) await capacitySignedReject(full, 'ethBridge.cancelPendingMultisig duplicate partial vote rejects', cancel);
  }
  for (const name of ['calls', 'countedNewOperations', 'proposerCountedOperations', 'cancellationApprovals']) {
    const params = name === 'calls' ? [target.hash] : [id, target.hash];
    assert((await rawQuery(full, 'bridgeMultisig', name, params)).isNone, 'Quorum must clear ' + name);
  }
  assert.equal((await rawQuery(full, 'ethBridge', 'requestStatuses', [0, canonicalHash])).toHex(), protocolBefore);
  assert.equal((await rawQuery(full, 'ethBridge', 'loadToIncomingRequestHash', [0, target.remoteHash])).toHex(), canonicalHash);
  const tombstoneKey = storageKey(meta, 'bridgeMultisig', 'dispatchedCalls', [target.hash, target.timepoint]);
  assert.equal(await full.get(tombstoneKey.toHex()), '0x',
    'Quorum cleanup must preserve the unrelated global execution tombstone, including its empty Unit bytes');
  await capacitySignedReject(full, 'ethBridge.cancelPendingMultisig completed cleanup replay rejects', cancel);
  usePeer(peers[1]); full = await capacityAccountingBranch(full, 'released capacity reuse');
  full = await signedCheck(full, 'capacity.asMulti released slot admits honest zero-XOR peer', protocol(212).outer, true, true);
  assert.equal((await rawQuery(full, 'bridgeMultisig', 'newPendingOperations', [id])).toNumber(), 128);
  for (const peer of peers) assert.equal((await rawQuery(full, 'system', 'account', [peer])).data.free.toBigInt(), 0n);
  report.capacityQuorumCleanup = { initialGlobalCount: 128, finalCountAfterCleanup: 127,
    countAfterHonestReuse: 128, quorum: 3, proposerExcluded: true, noStoredCallBytes: true,
    targetCallNotExecuted: true, protocolStatusAndCanonicalHashPreserved: true,
    unrelatedGlobalTombstonePreserved: true,
    replayRejected: true, zeroXorAllPeersPreserved: true, passed: true };
  report.signer = previousSigner; save(); return true;
}
async function main() {
  assert.equal(snapshot.status, 'passed'); assert(existsSync(opts.wasm));
  const manifest = JSON.parse(readFileSync(resolve(__dirname, 'package.json')));
  report.dependencies = { manifest, installed: {}, lockSha256: sha256(readFileSync(resolve(__dirname, 'package-lock.json'))) };
  for (const [name, version] of Object.entries(manifest.dependencies)) {
    const installed = JSON.parse(readFileSync(resolve(__dirname, 'node_modules', name, 'package.json'))).version;
    assert.equal(installed, version); report.dependencies.installed[name] = installed;
  }
  await cryptoWaitReady(); progress('Loading pinned deployed public state');
  chain = await setup({ endpoint: opts.endpoint, block: snapshot.blockHash,
    registeredTypes: { typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } },
    buildBlockMode: BuildBlockMode.Manual, mockSignatureHost: true, allowUnresolvedImports: true,
    offchainWorker: false, processQueuedMessages: false, saveBlocks: false, runtimeLogLevel: 3, rpcTimeout: 60000 });
  const readonly = new Set(['chain_getBlock', 'chain_getBlockHash', 'chain_getHeader', 'chain_getFinalizedHead',
    'state_getStorage', 'state_getKeysPaged', 'state_queryStorageAt', 'state_getMetadata', 'state_getRuntimeVersion',
    'state_call', 'system_properties', 'system_chain', 'system_name', 'system_version', 'rpc_methods']);
  const send = chain.api.send.bind(chain.api);
  chain.api.send = (method, ...params) => { assert(readonly.has(method), 'Forbidden public RPC: ' + method);
    report.remoteRpcMethods[method] = (report.remoteRpcMethods[method] || 0) + 1; return send(method, ...params); };
  installReadCache();
  const deployed = chain.head;
  assert.equal(deployed.number, snapshot.blockNumber);
  assert.equal((await rawQuery(deployed, 'system', 'blockHash', [0])).toHex(), snapshot.genesis);
  report.deployedVersion = await deployed.runtimeVersion;
  assert.equal(report.deployedVersion.specVersion, snapshot.runtimeVersion.specVersion);
  report.deployedWasmSha256 = sha256(hexToU8a(await deployed.get('0x3a636f6465')));
  assert.equal(report.deployedWasmSha256, snapshot.code.sha256);
  const wasm = readFileSync(opts.wasm); report.candidate = { path: opts.wasm, bytes: wasm.length, sha256: sha256(wasm) };
  deployed.setWasm(u8aToHex(wasm)); report.candidate.runtimeVersion = await deployed.runtimeVersion;
  assert.equal(report.candidate.runtimeVersion.specVersion, 134);
  assert.equal(report.candidate.runtimeVersion.transactionVersion, 131);
  const initialized = await initializeUpgrade(deployed);
  const meta = await initialized.meta;
  const initial = meta.tx.rewards.addUmiNftReceivers([]);
  await chooseSigner(initialized, [initial, meta.tx.orderBook.cancelLimitOrdersBatch([]),
    meta.tx.preimage.notePreimage(u8aToHex(Buffer.alloc(65536, 0x61))),
    meta.tx.kensetsu.accrue('18446744073709551615'),
    meta.tx.kensetsu.liquidate('18446744073709551615'),
    meta.tx.apolloPlatform.liquidate('0x' + 'ff'.repeat(32),
      '0x0200040000000000000000000000000000000000000000000000000000000000')]);
  const id = { dexId: 0, base: '0x0200040000000000000000000000000000000000000000000000000000000000',
    quote: '0x0200000000000000000000000000000000000000000000000000000000000000' };
  for (const [label, groups] of [
    ['empty outer batch', []], ['empty inner group', [[id, []]]],
    ['all empty inner groups', Array.from({ length: 16 }, () => [id, []])],
    ['mixed real-ID/empty groups', [[id, [18446744073709551615n.toString()]], [id, []]]],
  ]) {
    progress('Exact Wasm: paid cancellation failure ' + label);
    await signedCheck(initialized, 'orderBook.cancelLimitOrdersBatch ' + label,
      meta.tx.orderBook.cancelLimitOrdersBatch(groups), false, false, 'EmptyCancellationBatch');
  }
  progress('Exact Wasm: unauthorized Rewards failure retains fees');
  await signedCheck(initialized, 'rewards.addUmiNftReceivers BadOrigin', initial, false, false, 'BadOrigin');
  const invalid = [
    ['kensetsu.accrue missing CDP', meta.tx.kensetsu.accrue('18446744073709551615')],
    ['kensetsu.liquidate missing CDP', meta.tx.kensetsu.liquidate('18446744073709551615')],
    ['apolloPlatform.liquidate missing position', meta.tx.apolloPlatform.liquidate('0x' + 'ff'.repeat(32), id.base)],
  ];
  for (const [label, call] of invalid) {
    progress('Exact Wasm: signed invalid maintenance pays and bare rejects ' + label);
    await signedCheck(initialized, label, call, false, false);
    await unsignedCheck(initialized, label, call);
  }
  for (const section of ['bridgeInboundChannel', 'substrateBridgeInboundChannel']) {
    const call = defaultCall(meta, meta.tx[section].submit, { networkId: section === 'bridgeInboundChannel' ? { Sub: 'Mainnet' } : 'Mainnet',
      commitment: { Sub: { nonce: 1, messages: [] } },
      // MultiProof intentionally has no variant at index zero. An explicit
      // real variant keeps the fixture SCALE-valid while its empty signatures
      // and digest fail authenticated admission.
      proof: { Multisig: { digest: { logs: [] }, proof: [] } },
    });
    progress('Exact Wasm: bare inbound relay rejected ' + section);
    await unsignedCheck(initialized, section + '.submit invalid empty proof', call);
  }
  const retiredLendingExitsAndGuards = await retiredLendingFixture(initialized, {
    chain, Block, report, rawQuery, storageKey, signedCheck, unsignedCheck, progress, save,
    syntheticFixtures: opts.syntheticFixtures,
  });
  const preimageFirstAndReplay = await requestedPreimageFixture(initialized);
  const paidMigrationSuccessAndFailure = await paidMigrationFixture(initialized);
  const zeroXorLegacyPeerAndReplayRejection = await zeroXorLegacyBridgeFixture(initialized);
  const bridgeCapacityQuotaAndQuorumCleanup = await bridgeCapacityFixture(initialized);
  const successfulCancellationAndPaidReplay = await publicCancellation(initialized);
  assert(successfulCancellationAndPaidReplay, 'Required useful cancellation Wasm coverage needs a real public order or explicit synthetic fixture');
  report.inputs = { candidateSha256: report.candidate.sha256,
    scriptSha256: sha256(readFileSync(__filename)),
    retirementHelperSha256: sha256(readFileSync(resolve(__dirname, 'retired-lending-fixture.cjs'))),
    snapshotSha256: sha256(readFileSync(opts.snapshot)),
    packageLockSha256: sha256(readFileSync(resolve(__dirname, 'package-lock.json'))) };
  report.checks = { exactCandidateWasmUpgradeExecuted: true,
    allEmptyCancellationShapesRetainFees: true, unauthorizedRewardsRetainsFees: true,
    signedInvalidKensetsuAndApolloRetainFees: true, bareKensetsuAndApolloReject: true,
    retiredLendingExitsAndGuards,
    invalidInboundProofsReject: true, allCheckedFeeEventsMatchBalances: true,
    requestedPreimageFirstAndReplay: preimageFirstAndReplay,
    paidMigrationSuccessAndFailure,
    zeroXorMigrationRejectsBeforeExecution: paidMigrationSuccessAndFailure,
    successfulCancellationAndPaidReplay, zeroXorLegacyPeerAndReplayRejection,
    bridgeCapacityFairQuotaProtectsHonestPeers: bridgeCapacityQuotaAndQuorumCleanup,
    bridgeCapacityFullQueueQuorumCleanup: bridgeCapacityQuotaAndQuorumCleanup,
    bridgeCapacityCleanupPreservesProtocolAndRejectsReplay: bridgeCapacityQuotaAndQuorumCleanup,
    protectedStakingStateAndXorIssuancePreservedByUpgrade: true,
    noPublicTransactionSubmission: true };
  report.status = 'passed';
}
const timeout = setTimeout(() => { report.status = 'timed_out'; save(); process.exit(2); }, 900000);
main().catch(error => { report.status = 'failed'; report.error = { message: error.message, stack: error.stack };
  console.error(error); process.exitCode = 1; }).finally(async () => { clearTimeout(timeout);
  if (cache) { const bytes = gzipSync(JSON.stringify(cache)); writeFileSync(opts.cache, bytes);
    report.readCache.savedBytes = bytes.length; report.readCache.sha256 = sha256(bytes); }
  report.finishedAt = new Date().toISOString(); save();
  if (chain) await chain.close(); await destroyWorker(); });
