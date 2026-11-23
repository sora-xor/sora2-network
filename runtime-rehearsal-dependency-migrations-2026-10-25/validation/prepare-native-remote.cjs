'use strict';
// Prepare a current, pinned, read-only native migration scope. Never submits transactions.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { WsProvider } = require('@polkadot/api');
const { Metadata, TypeRegistry } = require('@polkadot/types');
const { cryptoWaitReady, xxhashAsHex } = require('@polkadot/util-crypto');
const { validateDependencies } = require('./validate-dependencies.cjs');

const sha256 = value => createHash('sha256').update(value).digest('hex');
const read = file => fs.readFileSync(path.resolve(__dirname, file));
const snapshotBytes = read('snapshot.json');
const snapshot = JSON.parse(snapshotBytes);
const previous = JSON.parse(read('../../runtime-upgrade-4.8.11/validation/native-remote-scope.json'));
const report = { status: 'running', createdAt: new Date().toISOString(),
  endpoint: snapshot.endpoint, sourceBlockNumber: snapshot.blockNumber,
  sourceBlockHash: snapshot.blockHash, sourceRuntimeSpecVersion: snapshot.runtimeVersion.specVersion,
  sourceTransactionVersion: snapshot.runtimeVersion.transactionVersion,
  preflight: { readOnly: true, submittedTransactions: 0, privateKeysRead: 0, remoteRpcMethods: {} } };
const save = () => fs.writeFileSync(path.resolve(__dirname, 'native-remote-scope.json'), JSON.stringify(report, null, 2) + '\n');
let provider;

async function main() {
  assert.equal(snapshot.status, 'passed');
  assert.equal(snapshot.endpoint, 'wss://mof2.sora.org');
  assert.equal(snapshot.genesis, '0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5');
  assert.equal(snapshot.finality.selectedCanonicalHash, snapshot.blockHash);
  assert(snapshot.blockNumber <= snapshot.finality.finalizedNumber);
  const metadataBytes = read('deployed-metadata.scale');
  assert.equal(sha256(metadataBytes), snapshot.metadata.sha256);
  report.dependencies = validateDependencies(__dirname);
  delete report.dependencies.manifest;
  await cryptoWaitReady();
  const metadata = new Metadata(new TypeRegistry(), metadataBytes);
  const storageKey = (pallet, item) => xxhashAsHex(pallet, 128) + xxhashAsHex(item, 128).slice(2);
  const frameVersionKeys = metadata.asLatest.pallets.filter(pallet => pallet.storage.isSome).map(pallet => {
    const prefix = pallet.storage.unwrap().prefix.toString();
    return { pallet: pallet.name.toString(), prefix, key: storageKey(prefix, ':__STORAGE_VERSION__:') };
  });
  const allowed = new Set(['chain_getBlockHash', 'chain_getHeader', 'chain_getFinalizedHead',
    'state_getStorage', 'state_queryStorageAt', 'state_getRuntimeVersion']);
  provider = new WsProvider(snapshot.endpoint);
  const send = provider.send.bind(provider);
  provider.send = (method, params) => {
    assert(allowed.has(method), `Forbidden public RPC: ${method}`);
    report.preflight.remoteRpcMethods[method] = (report.preflight.remoteRpcMethods[method] || 0) + 1;
    return send(method, params);
  };
  await provider.isReady;
  assert.equal(await provider.send('chain_getBlockHash', [0]), snapshot.genesis);
  assert.equal(await provider.send('chain_getBlockHash', [snapshot.blockNumber]), snapshot.blockHash);
  const header = await provider.send('chain_getHeader', [snapshot.blockHash]);
  assert.equal(parseInt(header.number, 16), snapshot.blockNumber);
  assert.equal(header.stateRoot, snapshot.header.stateRoot);
  const currentFinalized = await provider.send('chain_getFinalizedHead', []);
  const currentFinalizedHeader = await provider.send('chain_getHeader', [currentFinalized]);
  assert(snapshot.blockNumber <= parseInt(currentFinalizedHeader.number, 16));
  const version = await provider.send('state_getRuntimeVersion', [snapshot.blockHash]);
  assert.equal(version.specVersion, snapshot.runtimeVersion.specVersion);
  assert.equal(version.transactionVersion, snapshot.runtimeVersion.transactionVersion);
  assert.equal(version.stateVersion, snapshot.runtimeVersion.stateVersion);
  assert([0, 1].includes(version.stateVersion), 'Unsupported captured trie state version');
  const code = await provider.send('state_getStorage', ['0x3a636f6465', snapshot.blockHash]);
  assert.equal(sha256(Buffer.from(code.slice(2), 'hex')), snapshot.code.sha256);
  const keys = [...new Set([...frameVersionKeys.map(row => row.key),
    ...previous.preflight.customVersionGuards.map(row => row.key), ...previous.preflight.markers.map(row => row.key)])];
  const changes = await provider.send('state_queryStorageAt', [keys, snapshot.blockHash]);
  assert.equal(changes.length, 1); assert.equal(changes[0].block, snapshot.blockHash);
  const values = new Map(changes[0].changes);
  assert.equal(values.size, keys.length);
  const expectedVersions = new Map(previous.preflight.auditedStorageVersions.map(row => [row.pallet, row.expectedVersion]));
  report.preflight.frameStorageVersions = frameVersionKeys.map(row => {
    const raw = values.get(row.key);
    assert(raw === null || /^0x[0-9a-f]{4}$/i.test(raw), `Malformed FRAME version: ${row.pallet}`);
    const decodedVersion = raw === null ? 0 : Buffer.from(raw.slice(2), 'hex').readUInt16LE();
    const expected = expectedVersions.get(row.pallet);
    if (expected !== undefined) assert.equal(decodedVersion, expected, `Expand/review migration scope for ${row.pallet}`);
    return { ...row, raw, decodedVersion, ...(expected === undefined ? {} : { expectedVersion: expected }) };
  });
  for (const pallet of expectedVersions.keys()) assert(frameVersionKeys.some(row => row.pallet === pallet), `Missing audited pallet ${pallet}`);
  report.preflight.customVersionGuards = previous.preflight.customVersionGuards.map(row => {
    const raw = values.get(row.key);
    assert.equal(raw, row.raw, `Legacy ${row.pallet} migration became active: expand scope first`);
    return { pallet: row.pallet, key: row.key, raw, decodedVersion: row.decodedVersion, expectedHook: 'No-op on the freshly checked live guard.' };
  });
  report.preflight.markers = previous.preflight.markers.map(row => ({ label: row.label, key: row.key,
    raw: values.get(row.key), interpretation: values.get(row.key) === '0x01' ? 'Already applied; guarded migration is a no-op.' : 'Not marked applied; configured migration must execute its normal path.' }));
  report.preflight.canonicality = { selectedHash: snapshot.blockHash, selectedNumber: snapshot.blockNumber,
    latestFinalizedHash: currentFinalized, latestFinalizedNumber: parseInt(currentFinalizedHeader.number, 16), checkedAt: new Date().toISOString() };
  const pallets = [...previous.scope.pallets, 'EthBridge', 'Timestamp'];
  const hashedPrefixes = previous.scope.hashedPrefixes.filter(row => row.label === 'Raw runtime: prefix');
  const hashedKeys = previous.scope.hashedKeys.filter(row => !row.label.startsWith('EthBridge '));
  for (const row of frameVersionKeys) if (!hashedKeys.some(existing => existing.key === row.key)) hashedKeys.push({ key: row.key, label: `${row.pallet} FRAME storage version`, reason: 'Retain every metadata-visible version guard, including pallets outside the full-prefix scope.' });
  for (const name of ['Number', 'ParentHash', 'Digest', 'ExecutionPhase']) hashedKeys.push({ key: storageKey('System', name), label: `System.${name}`, reason: 'Preserve execution context without importing all System.Account entries.' });
  hashedKeys.push({ key: storageKey('Balances', 'TotalIssuance'), label: 'Balances.TotalIssuance', reason: 'Retain native issuance; this does not import account balances or enable global balance invariants.' });
  report.scope = { pallets, hashedPrefixes, hashedKeys, childTrie: false,
    explanation: 'Retains full configured migration-state pallets, complete EthBridge state for bridge audits, all FRAME/custom version guards and raw migration markers. It is a scoped snapshot; every exclusion is explicit below.' };
  report.environment = { REQUIRE_REMOTE: '1', REMOTE_RPC_URL: snapshot.endpoint, REMOTE_BLOCK_HASH: snapshot.blockHash,
    REMOTE_UPGRADE_CHECKS: 'pre-and-post', REMOTE_STATE_VERSION: String(snapshot.runtimeVersion.stateVersion), REMOTE_CHILD_TRIE: '0', REMOTE_PALLETS: pallets.join(','),
    REMOTE_HASHED_PREFIXES: hashedPrefixes.map(row => row.key).join(','), REMOTE_HASHED_KEYS: hashedKeys.map(row => row.key).join(',') };
  report.execution = { status: 'not-run', package: 'framenode-runtime', defaultFeatures: false, features: ['std', 'try-runtime'],
    featureClassification: 'Production chain feature selection plus common/test instrumentation from the chain-spec development dependency; not byte-identical production dependency behavior.',
    test: 'remote_try_runtime_upgrade_rehearsal', checks: 'Configured pre/post upgrade hooks, retained-state decoding, audited storage versions, complete bridge replay/index/bounds checks, and multi-block migration completion.',
    forbiddenFeatures: ['private-net', 'wip', 'stage', 'runtime-benchmarks', 'reduced-pswap-reward-periods'],
    candidateBinding: 'Native source execution; pair with the separately hashed exact-Wasm fork rehearsal. This is not a deployment eligibility decision.' };
  report.inputs = { snapshotSha256: sha256(snapshotBytes), metadataSha256: snapshot.metadata.sha256,
    deployedCodeSha256: snapshot.code.sha256, scriptSha256: sha256(read('prepare-native-remote.cjs')),
    priorScopeTemplateSha256: sha256(read('../../runtime-upgrade-4.8.11/validation/native-remote-scope.json')),
    providerRdapSha256: sha256(read('../rpc-provider-rdap.json')) };
  report.limitations = [
    'Native unit-test dependency unification enables common/test: FixedWrapper balance conversion uses stricter unwrap/checked behavior, and common include macros use test fallbacks. This is production chain feature selection with test instrumentation. Exact default-production Wasm fork execution establishes production runtime semantics.',
    'No global try_state claim: System.Account, account balances/tokens, assets and unrelated pallets are omitted. Full Staking/XorFee retains real reward budgets, exposures and claims for migration hooks; it does not execute real user payouts.',
    'Child tries are omitted because the selected migration paths use ordinary pallet/raw storage. This is not a complete chain-state snapshot or cryptographic state proof.',
    'Legacy custom pallet data is omitted only after current pinned preflight verifies its no-op guard. Any guard/version mismatch aborts preparation and requires source/scope review.',
    'Already-applied migration markers mean current-state replay exercises their idempotent no-op branch. Synthetic/native fixture tests separately cover active historical migrations.',
    'The native runner uses a fresh scope-specific snapshot and rejects pre-existing cache files; a matching block hash alone does not prove filter completeness.',
    'Public RPC responses are trusted for this diagnostic rehearsal. Finality/canonicality are provider assertions, not independent consensus verification.',
    'No signed transactions, governance enactment, offchain-worker execution, full block consensus execution or validator deployment occurs.'
  ];
  report.status = 'preflight-passed-scope-prepared';
}
const timer = setTimeout(() => { report.status = 'timed_out'; save(); process.exit(2); }, 180000);
main().catch(error => { report.status = 'failed'; report.error = { message: error.message, stack: error.stack }; process.exitCode = 1; })
  .finally(async () => { clearTimeout(timer); report.finishedAt = new Date().toISOString(); save(); if (provider) await provider.disconnect(); console.log(JSON.stringify({ status: report.status, block: report.sourceBlockNumber, error: report.error?.message })); });
