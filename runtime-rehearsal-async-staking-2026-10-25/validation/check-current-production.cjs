'use strict';
// Read current finalized runtime identity without modifying the pinned rehearsal snapshot.
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync } = require('node:fs');
const { resolve } = require('node:path');
const { createHash } = require('node:crypto');
const { WsProvider } = require('@polkadot/rpc-provider');
const { hexToU8a } = require('@polkadot/util');
const sha256 = value => createHash('sha256').update(value).digest('hex');
const snapshotPath = resolve(__dirname, 'snapshot.json');
const snapshotBytes = readFileSync(snapshotPath);
const snapshot = JSON.parse(snapshotBytes);
const endpoint = 'wss://mof2.sora.org';
const output = resolve(__dirname, 'current-production-identity.json');
const report = { status: 'running', startedAt: new Date().toISOString(), endpoint,
  readOnly: true, submittedTransactions: 0, remoteRpcMethods: {},
  inputs: { scriptSha256: sha256(readFileSync(__filename)), snapshotSha256: sha256(snapshotBytes),
    rpcGuardSha256: sha256(readFileSync(resolve(__dirname, 'readonly-rpc.cjs'))) } };
const save = () => writeFileSync(output, JSON.stringify(report, null, 2) + '\n');
let provider;
const restore = require('./readonly-rpc.cjs').installReadonlyRpc(report);
const timeout = setTimeout(() => { report.status = 'timed_out'; report.finishedAt = new Date().toISOString(); save(); process.exit(2); }, 90000);
async function main() {
  assert.equal(snapshot.status, 'passed');
  assert.equal(snapshot.endpoint, endpoint);
  assert.equal(sha256(readFileSync(resolve(__dirname, 'live-runtime.compact.compressed.wasm'))), snapshot.code.sha256);
  provider = new WsProvider(endpoint, 1000, {}, 60000);
  await provider.isReady;
  const finalizedHash = await provider.send('chain_getFinalizedHead', []);
  const [genesis, header, codeHex, runtimeVersion, pinnedCanonicalHash] = await Promise.all([
    provider.send('chain_getBlockHash', [0]),
    provider.send('chain_getHeader', [finalizedHash]),
    provider.send('state_getStorage', ['0x3a636f6465', finalizedHash]),
    provider.send('state_getRuntimeVersion', [finalizedHash]),
    provider.send('chain_getBlockHash', [snapshot.blockNumber]),
  ]);
  assert.equal(genesis, snapshot.genesis);
  assert.equal(pinnedCanonicalHash, snapshot.blockHash);
  const blockNumber = Number(BigInt(header.number));
  assert(blockNumber >= snapshot.blockNumber);
  assert.equal(await provider.send('chain_getBlockHash', [blockNumber]), finalizedHash);
  assert.match(codeHex, /^0x(?:[a-f\d]{2})+$/i);
  const code = hexToU8a(codeHex);
  report.pinnedBaseline = { blockNumber: snapshot.blockNumber, blockHash: snapshot.blockHash,
    runtimeVersion: snapshot.runtimeVersion, codeSha256: snapshot.code.sha256 };
  report.currentFinalized = { blockNumber, blockHash: finalizedHash, header, runtimeVersion,
    codeBytes: code.length, codeSha256: sha256(code) };
  report.sameRuntimeVersion = Object.keys(snapshot.runtimeVersion).every(key => {
    try { assert.deepEqual(runtimeVersion[key], snapshot.runtimeVersion[key]); return true; } catch { return false; }
  });
  report.sameRuntimeCode = sha256(code) === snapshot.code.sha256;
  report.pinnedBlockRemainsCanonical = true;
  report.snapshotPreserved = sha256(readFileSync(snapshotPath)) === sha256(snapshotBytes);
  assert(report.snapshotPreserved);
  report.pinnedRuntimeStillRepresentative = report.sameRuntimeVersion && report.sameRuntimeCode;
  report.scope = 'Runtime identity only: this checks code bytes and runtime version at the current finalized head. It does not claim all current storage equals the pinned snapshot.';
  report.status = 'passed';
}
main().catch(error => { report.status = 'failed'; report.error = error.message; process.exitCode = 1; }).finally(async () => {
  clearTimeout(timeout);
  if (provider) await provider.disconnect().catch(() => {});
  restore(); report.finishedAt = new Date().toISOString(); save();
  console.log(JSON.stringify({ status: report.status, finalizedBlock: report.currentFinalized?.blockNumber,
    runtimeStillRepresentative: report.pinnedRuntimeStillRepresentative, output, error: report.error }));
});
