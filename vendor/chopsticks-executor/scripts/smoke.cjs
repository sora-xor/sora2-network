// Offline ABI, trie/proof, decompression, callback, and actual Wasm execution checks.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const fixtures = require('../tests/compatibility-fixtures.json');
const cjsPath = path.resolve(process.argv[2] || path.join(__dirname, '../dist/cjs/index.js'));

async function exercise(api) {
  for (const { entries, version, root } of fixtures.stateRoots) {
    assert.equal(await api.calculate_state_root(entries, version), root);
  }
  for (const { nodes, updates, result, decoded } of fixtures.proofs) {
    assert.deepEqual(await api.create_proof(nodes, updates), result);
    assert.deepEqual(await api.decode_proof(result[0], result[1]), decoded);
  }
  assert.equal(await api.testing({ getStorage: async key => `stored:${key}` }, '0x01'), 'stored:0x01');
  await assert.rejects(api.calculate_state_root([], 2));
  await assert.rejects(api.get_runtime_version('0x00000000'));
  await assert.rejects(api.decode_proof(`0x${'00'.repeat(32)}`, ['0xff']));
  // This tiny compressed frame advertises a huge window. Never run it against
  // the vulnerable historical reference; the fixed decoder must reject it.
  await assert.rejects(api.get_runtime_version('0x52bc537646db8e0528b52ffd00ff010000'));

  const wasm = `0x${fs.readFileSync(path.join(__dirname, '../tests/runtime-fixture.wasm')).toString('hex')}`;
  assert.deepEqual(await api.get_runtime_version(wasm), fixtures.runtimeVersion);
  const callbacks = {
    getStorage: async () => undefined, getNextKey: async () => undefined,
    offchainGetStorage: async () => undefined, offchainTimestamp: async () => 0,
    offchainRandomSeed: async () => `0x${'00'.repeat(32)}`,
    offchainSubmitTransaction: async () => false,
  };
  const task = { id: 1, wasm, calls: [['Core_version', []]], mockSignatureHost: 0,
    allowUnresolvedImports: true, runtimeLogLevel: 0, storageProofSize: 0 };
  assert.deepEqual(await api.run_task(task, callbacks), fixtures.runtimeCall);
  assert.deepEqual(await api.run_task({ ...task, calls: [['Fixture_add', []]] }, callbacks), fixtures.arithmeticCall);
}

(async () => {
  await exercise(require(cjsPath));
  const esmPath = path.resolve(path.dirname(cjsPath), '../esm/index.js');
  await exercise(await import(pathToFileURL(esmPath)));
  console.log('PASS CJS/ESM: 12 fixed state roots, proof round trips, malformed input, decompression bomb, callbacks, and actual Wasm execution');
})().catch(error => { console.error(error); process.exitCode = 1; });
