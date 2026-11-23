'use strict';
// Local paired diagnostic timing; this is not a production throughput benchmark.
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync } = require('node:fs');
const { resolve } = require('node:path');
const { createHash } = require('node:crypto');
const { performance } = require('node:perf_hooks');
const { TypeRegistry } = require('@polkadot/types/create');
const api = require('@acala-network/chopsticks-executor');
const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
const oldPath = resolve(option('--old', resolve(__dirname, '../../runtime-rehearsal-dependency-migrations-2026-10-25/candidate.compact.compressed.wasm')));
const newPath = resolve(option('--new', '/tmp/sora2-stable2609-migration/diagnostic-sdk2609.compact.compressed.wasm'));
const output = resolve(option('--output', resolve(__dirname, 'paired-runtime-api-benchmark.json')));
const sha256 = value => createHash('sha256').update(value).digest('hex');
const values = new Map([['old135', readFileSync(oldPath)], ['new136', readFileSync(newPath)]]);
const report = { status: 'running', startedAt: new Date().toISOString(), localOnly: true,
  remoteRequests: 0, scriptSha256: sha256(readFileSync(__filename)),
  inputs: Object.fromEntries([...values].map(([name, bytes]) => [name, { bytes: bytes.length, sha256: sha256(bytes) }])),
  method: 'Six paired samples per API after warmups, alternating old/new and new/old order. Same installed executor, process, callbacks and parameters. Per-call timing includes executor task setup and compressed runtime loading.',
  limitations: ['Pure deterministic runtime APIs only; this does not measure block throughput, storage migrations, signed extensions, or staking payout execution.',
    'Concurrent machine workload is not controlled; alternation and process CPU timing reduce, but do not eliminate, scheduling bias.'],
  samples: [], warmups: [] };
const wasm = Object.fromEntries([...values].map(([name, bytes]) => [name, '0x' + bytes.toString('hex')]));
const callbacks = { getStorage: async key => { throw new Error('Unexpected storage read: ' + key); },
  getNextKey: async () => { throw new Error('Unexpected next-key request'); },
  offchainGetStorage: async () => { throw new Error('Unexpected offchain read'); },
  offchainTimestamp: async () => 0, offchainRandomSeed: async () => '0x' + '00'.repeat(32),
  offchainSubmitTransaction: async () => { throw new Error('Unexpected transaction submission'); } };
const registry = new TypeRegistry();
const methods = [['Core_version', []], ['TransactionPaymentApi_query_length_to_fee', [registry.createType('u32', 1000).toHex()]]];
const outputs = new Map();
let id = 0;
const save = () => writeFileSync(output, JSON.stringify(report, null, 2) + '\n');
async function measure(label, method, params, phase, pair) {
  const cpu = process.cpuUsage(); const start = performance.now();
  const result = await api.run_task({ id: ++id, wasm: wasm[label], calls: [[method, params]],
    mockSignatureHost: 0, allowUnresolvedImports: true, runtimeLogLevel: 0, storageProofSize: 0 }, callbacks);
  const elapsedMs = performance.now() - start; const used = process.cpuUsage(cpu);
  assert(result.Call, JSON.stringify(result));
  assert.deepEqual(result.Call.storageDiff, []); assert.deepEqual(result.Call.offchainStorageDiff, []);
  if (method === 'Core_version') {
    const version = registry.createType('RuntimeVersion', result.Call.result);
    assert.equal(version.specVersion.toNumber(), label === 'old135' ? 135 : 136);
    assert.equal(version.transactionVersion.toNumber(), label === 'old135' ? 131 : 132);
  } else {
    if (outputs.has(method)) assert.equal(result.Call.result, outputs.get(method), 'Identical length fee conversion');
    outputs.set(method, result.Call.result);
  }
  const row = { label, method, pair, elapsedMs, processCpuMs: (used.user + used.system) / 1000,
    outputSha256: sha256(result.Call.result), outputBytes: (result.Call.result.length - 2) / 2 };
  report[phase].push(row); save();
}
const median = values => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
const timeout = setTimeout(() => { report.status = 'timed_out'; save(); process.exit(2); }, 300000);
(async () => {
  for (const [method, params] of methods) for (const label of ['old135', 'new136']) await measure(label, method, params, 'warmups', -1);
  for (let pair = 0; pair < 6; pair++) for (const [method, params] of methods) {
    for (const label of pair % 2 ? ['new136', 'old135'] : ['old135', 'new136']) await measure(label, method, params, 'samples', pair);
  }
  report.results = methods.map(([method]) => {
    const rows = label => report.samples.filter(row => row.method === method && row.label === label);
    const oldMs = median(rows('old135').map(row => row.elapsedMs)), newMs = median(rows('new136').map(row => row.elapsedMs));
    return { method, samplesPerRuntime: 6, oldMedianElapsedMs: oldMs, newMedianElapsedMs: newMs,
      candidateToOldElapsedRatio: newMs / oldMs,
      oldMedianProcessCpuMs: median(rows('old135').map(row => row.processCpuMs)),
      newMedianProcessCpuMs: median(rows('new136').map(row => row.processCpuMs)) };
  });
  report.status = 'passed';
})().catch(error => { report.status = 'failed'; report.error = error.stack; process.exitCode = 1; }).finally(() => {
  clearTimeout(timeout); report.finishedAt = new Date().toISOString(); save();
  console.log(JSON.stringify({ status: report.status, results: report.results, error: report.error }));
});
