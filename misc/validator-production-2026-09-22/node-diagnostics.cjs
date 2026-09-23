#!/usr/bin/env node
'use strict';

// Node.js >= 18. Run on the affected node or through an existing SSH tunnel.
// Uses only the explicit, read-only JSON-RPC methods listed below.
const { readFileSync, writeFileSync } = require('node:fs');
const { resolve } = require('node:path');

const allowedMethods = new Set([
  'rpc_methods', 'chain_getBlockHash', 'chain_getHeader',
  'chain_getFinalizedHead', 'state_getRuntimeVersion', 'system_health',
  'system_syncState', 'system_nodeRoles', 'system_version',
  'author_hasKey',
]);

async function main() {
  const args = process.argv.slice(2);
  if (args.includes('--help')) {
    console.log('Usage: node node-diagnostics.cjs --stash <address> [--rpc http://127.0.0.1:9944] [--snapshot snapshot.json] [--out report.json]');
    console.log('The HTTP RPC must be loopback. No keys are exported, inserted, or rotated; no transactions are submitted.');
    return;
  }
  const options = {};
  for (let i = 0; i < args.length; i += 2) {
    if (!['--stash', '--rpc', '--snapshot', '--out'].includes(args[i]) || !args[i + 1] || args[i + 1].startsWith('--')) {
      throw new Error('Invalid argument; use --help.');
    }
    options[args[i].slice(2)] = args[i + 1];
  }
  if (!options.stash) throw new Error('--stash is required.');
  const rpcUrl = new URL(options.rpc || 'http://127.0.0.1:9944');
  if (!['http:', 'https:'].includes(rpcUrl.protocol) || !['127.0.0.1', '[::1]'].includes(rpcUrl.hostname) || rpcUrl.username || rpcUrl.password) {
    throw new Error('Use a literal loopback HTTP(S) RPC address, directly on the validator or through an existing SSH tunnel.');
  }
  const snapshotPath = resolve(options.snapshot || resolve(__dirname, 'snapshot.json'));
  const snapshot = JSON.parse(readFileSync(snapshotPath, 'utf8'));
  const stashIndex = snapshot.storage['session.validators'].indexOf(options.stash);
  if (stashIndex < 0) throw new Error('Stash is not in the supplied snapshot validator set. Capture an updated snapshot first.');
  const babeKey = snapshot.storage['babe.authorities'][stashIndex][0];
  if (!/^0x[0-9a-f]{64}$/i.test(babeKey)) throw new Error('Invalid BABE public key in snapshot.');
  let requestId = 0;
  async function rpc(method, params = []) {
    if (!allowedMethods.has(method)) throw new Error('RPC method is not in the read-only allowlist.');
    const id = ++requestId;
    const response = await fetch(rpcUrl, {
      method: 'POST', redirect: 'error',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id, method, params }),
      signal: AbortSignal.timeout(15000),
    });
    if (!response.ok) throw new Error(`${method}: HTTP ${response.status}`);
    const payload = await response.json();
    if (payload.id !== id) throw new Error(`${method}: response ID mismatch`);
    if (payload.error) throw new Error(`${method}: ${payload.error.code} ${payload.error.message}`);
    if (!Object.hasOwn(payload, 'result')) throw new Error(`${method}: no result`);
    return payload.result;
  }
  // Verify chain identity before looking for any public key in the local keystore.
  const genesis = await rpc('chain_getBlockHash', [0]);
  if (genesis !== snapshot.genesis) throw new Error('Node genesis does not match the SORA snapshot; stopped before key checks.');
  const report = {
    checkedAt: new Date().toISOString(),
    genesis, stash: options.stash,
    reference: {
      checkedAt: snapshot.checkedAt, block: snapshot.blockNumber, hash: snapshot.blockHash,
      session: snapshot.storage['session.currentIndex'], validatorIndex: stashIndex, babePublicKey: babeKey,
    },
    results: {}, errors: {}, submittedTransactions: 0,
    limitations: [
      'BABE key is the active key at the reference snapshot, not necessarily at collection time.',
      'author_hasKey=true proves key presence only; it does not prove usable VRF signing or the correct keystore password.',
      'checkedAt is the collector clock; clock drift requires an independent trusted time comparison.',
      'RPC errors or unavailable methods are inconclusive, not evidence of a missing key or an unhealthy node.',
      'Best and finalized headers are separate RPC reads, so their difference is not an atomic measurement.',
    ],
  };
  async function collect(label, method, params) {
    try { report.results[label] = await rpc(method, params); }
    catch (error) { report.errors[label] = error.message; }
  }
  await Promise.all([
    collect('rpcMethods', 'rpc_methods'),
    collect('health', 'system_health'),
    collect('syncState', 'system_syncState'),
    collect('roles', 'system_nodeRoles'),
    collect('nodeVersion', 'system_version'),
    collect('runtimeVersion', 'state_getRuntimeVersion'),
    collect('bestHeader', 'chain_getHeader'),
    collect('finalizedHash', 'chain_getFinalizedHead'),
    collect('referenceBlockHash', 'chain_getBlockHash', [snapshot.blockNumber]),
  ]);
  if (report.results.finalizedHash) await collect('finalizedHeader', 'chain_getHeader', [report.results.finalizedHash]);
  const methods = report.results.rpcMethods?.methods;
  if (Array.isArray(methods) && methods.includes('author_hasKey')) {
    await collect('hasReferenceBabeKey', 'author_hasKey', [babeKey, 'babe']);
  } else {
    report.errors.hasReferenceBabeKey = 'author_hasKey was not advertised; no key-presence request was sent.';
  }
  report.observations = [];
  if (report.results.referenceBlockHash && report.results.referenceBlockHash !== snapshot.blockHash) {
    report.observations.push('Reference block hash differs from the finalized public snapshot; investigate a fork or a wrong reference.');
  }
  if (report.results.health?.isSyncing === true) report.observations.push('Node reports syncing; BABE may skip slot claiming while major syncing.');
  if (Array.isArray(report.results.roles) && !report.results.roles.includes('Authority')) report.observations.push('Node does not report the Authority role.');
  if (report.results.hasReferenceBabeKey === false) report.observations.push('Node did not confirm presence of this snapshot BABE key; it may be absent or unreadable.');
  if (report.results.bestHeader && report.results.finalizedHeader) {
    report.bestMinusFinalized = (BigInt(report.results.bestHeader.number) - BigInt(report.results.finalizedHeader.number)).toString();
  }
  const output = JSON.stringify(report, null, 2) + '\n';
  if (options.out) writeFileSync(resolve(options.out), output, { flag: 'wx', mode: 0o600 });
  else process.stdout.write(output);
}

main().catch(error => { console.error(error.message); process.exitCode = 1; });
