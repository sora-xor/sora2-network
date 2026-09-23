'use strict';
// Encode unsigned calls only. No keyring, transaction submission, or signing.
const assert = require('node:assert/strict');
const { createRequire } = require('node:module');
const { readFileSync, writeFileSync } = require('node:fs');
const { resolve } = require('node:path');
const { createHash } = require('node:crypto');
const deps = createRequire(resolve(process.env.SORA_REPAIR_DEPENDENCIES || __dirname + '/package.json'));
const { ApiPromise, WsProvider } = deps('@polkadot/api');
const { fullOverrideBundle } = deps('@sora-substrate/type-definitions');
const { blake2AsHex, cryptoWaitReady, xxhashAsU8a } = deps('@polkadot/util-crypto');
const dir = resolve(__dirname, '..');
const sha256 = data => createHash('sha256').update(data).digest('hex');
const genesis = '0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5';
const expectedDeployedSha = 'db948406c5f22d4923b2760019de53bcd0ef756ed05accaf5156ca3041988447';
const timer = setTimeout(() => { console.error('Read-only package preflight timed out'); process.exit(2); }, 120000);
(async () => {
  await cryptoWaitReady();
  const wasmName = 'framenode-runtime-4.8.10.compact.compressed.wasm';
  const wasm = readFileSync(resolve(dir, wasmName));
  const rehearsal = JSON.parse(readFileSync(resolve(dir, 'validation/babe-repair-wasm-rehearsal.json')));
  assert.equal(rehearsal.status, 'passed');
  assert.equal(rehearsal.candidate.sha256, sha256(wasm), 'Rehearsal must cover this exact Wasm');
  assert.equal(rehearsal.candidate.runtimeVersion.specVersion, 132);
  const api = await ApiPromise.create({
    provider: new WsProvider('wss://mof2.sora.org'),
    typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } },
  });
  try {
    assert.equal(api.genesisHash.toHex(), genesis, 'Wrong chain');
    const hash = await api.rpc.chain.getFinalizedHead();
    const at = await api.at(hash);
    const header = await api.rpc.chain.getHeader(hash);
    const version = (await api.rpc.state.getRuntimeVersion(hash)).toJSON();
    assert.equal(version.specVersion, 131, 'Deployment preflight changed; review before encoding');
    const current = (await at.query.babe.epochConfig()).toJSON();
    const next = (await at.query.babe.nextEpochConfig()).toJSON();
    const pending = (await at.query.babe.pendingEpochConfigChange()).toJSON();
    const legacy = { c: [1, 4], allowedSlots: 'PrimaryAndSecondaryVRFSlots' };
    assert.deepEqual(current, legacy);
    assert.deepEqual(next, legacy);
    assert.equal(pending, null, 'An existing governance configuration plan must be reviewed');
    const deployed = (await api.rpc.state.getStorage('0x3a636f6465', hash)).unwrap().toU8a(true);
    assert.equal(sha256(deployed), expectedDeployedSha, 'Deployed runtime changed');
    assert.equal((await at.query.system.blockHash(0)).toHex(), genesis, 'Runtime migration genesis guard must match');
    const marker = '0x' + Buffer.concat([
      Buffer.from(xxhashAsU8a('Babe', 128)), Buffer.from(xxhashAsU8a('MainnetPlainConfigRepairV1', 128)),
    ]).toString('hex');
    assert((await api.rpc.state.getStorage(marker, hash)).isNone, 'One-time repair was already considered; review chain state');
    const canonicalHeaders = [];
    let ancestor = header;
    for (let i = 0; i < 20; i++) {
      const pre = ancestor.digest.logs.filter(log => log.isPreRuntime && log.asPreRuntime[0].toHex() === '0x42414245');
      assert.equal(pre.length, 1, 'Canonical header must have one BABE pre-digest');
      const variant = pre[0].asPreRuntime[1].toU8a(true)[0];
      assert([1, 2].includes(variant), 'Canonical slot mode changed; review before encoding');
      canonicalHeaders.push({ hash: ancestor.hash.toHex(), number: ancestor.number.toNumber(), preDigestVariant: variant });
      if (i < 19) ancestor = await api.rpc.chain.getHeader(ancestor.parentHash);
    }
    assert(canonicalHeaders.some(h => h.preDigestVariant === 2), 'No canonical Plain proof in the preflight sample');
    const runtimeCode = '0x' + wasm.toString('hex');
    const call = api.tx.system.setCode(runtimeCode).method;
    const encoded = call.toU8a();
    const proposalHash = blake2AsHex(encoded);
    writeFileSync(resolve(dir, 'set-code-call.scale'), encoded);
    writeFileSync(resolve(dir, 'set-code-call.hex'), call.toHex() + '\n');
    writeFileSync(resolve(dir, 'preimage-note-call.hex'), api.tx.preimage.notePreimage(call.toHex()).method.toHex() + '\n');
    const report = {
      preparedAt: new Date().toISOString(), release: '4.8.10',
      candidate: { file: wasmName, bytes: wasm.length, sha256: sha256(wasm), specVersion: 132, transactionVersion: 131 },
      proposal: { method: 'system.setCode', hash: proposalHash, bytes: encoded.length, signed: false },
      preflight: { endpoint: 'wss://mof2.sora.org', genesis, hash: hash.toHex(), block: header.number.toNumber(),
        runtimeVersion: version, deployedSha256: sha256(deployed), current, next, pending,
        repairMarkerAbsent: true, canonicalHeaders },
      activation: 'Migration schedules Plain; first epoch boundary announces it; second activates it. Wait for current and next Plain before fresh state-sync recovery.',
      submittedTransactions: 0, privateKeysRead: 0,
      limitations: ['Re-run live preflight before enactment; preparation is not deployment.', 'Existing incompatible node caches require the reviewed offline repair or fresh post-fix state synchronization.'],
    };
    writeFileSync(resolve(dir, 'runtime-upgrade-4.8.10-info.json'), JSON.stringify(report, null, 2) + '\n');
    console.log(JSON.stringify({ candidateSha256: report.candidate.sha256, proposalHash, proposalBytes: encoded.length, preflightBlock: report.preflight.block, submittedTransactions: 0 }));
  } finally { await api.disconnect(); }
})().catch(error => { console.error(error); process.exitCode = 1; }).finally(() => clearTimeout(timer));
