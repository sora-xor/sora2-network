'use strict';
// Public, read-only RPC only. No signer, keyring, or transaction submission.
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync } = require('node:fs');
const { resolve } = require('node:path');
const { createHash } = require('node:crypto');
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { fullOverrideBundle } = require('@sora-substrate/type-definitions');
const { stringToHex, hexToU8a } = require('@polkadot/util');
const args = process.argv.slice(2);
const option = (key, fallback) => args.includes(key) ? args[args.indexOf(key) + 1] : fallback;
const endpoint = option('--endpoint', 'wss://ws.mof.sora.org');
const output = resolve(option('--output', resolve(__dirname, 'snapshot.json')));
const baselinePath = resolve(option('--baseline', resolve(__dirname, 'live-runtime-132.compact.compressed.wasm')));
const GENESIS = '0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5';
const sha256 = value => createHash('sha256').update(value).digest('hex');
const report = { status: 'running', capturedAt: new Date().toISOString(), endpoint, submittedTransactions: 0, privateKeysRead: 0, remoteRpcMethods: {} };
const readonly = new Set(['chain_getBlock', 'chain_getBlockHash', 'chain_getHeader', 'chain_getFinalizedHead', 'state_getStorage', 'state_getKeysPaged', 'state_queryStorageAt', 'state_getMetadata', 'state_getRuntimeVersion', 'state_call', 'system_properties', 'system_chain', 'system_name', 'system_version', 'rpc_methods', 'state_subscribeRuntimeVersion', 'state_unsubscribeRuntimeVersion']);
let api;
const save = () => writeFileSync(output, JSON.stringify(report, null, 2) + '\n');
async function main() {
  const provider = new WsProvider(endpoint);
  const send = provider.send.bind(provider);
  provider.send = (method, ...params) => {
    assert(readonly.has(method), 'Forbidden public RPC: ' + method);
    report.remoteRpcMethods[method] = (report.remoteRpcMethods[method] || 0) + 1;
    return send(method, ...params);
  };
  api = await ApiPromise.create({ provider, noInitWarn: true, typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } });
  assert.equal(api.genesisHash.toHex(), GENESIS);
  const blockNumber = option('--block-number', null);
  const hash = option('--block', blockNumber !== null
    ? (await api.rpc.chain.getBlockHash(Number(blockNumber))).toHex()
    : (await api.rpc.chain.getFinalizedHead()).toHex());
  const at = await api.at(hash);
  const header = await api.rpc.chain.getHeader(hash);
  const version = (await api.rpc.state.getRuntimeVersion(hash)).toJSON();
  assert.equal(version.specVersion, 132, 'Expected deployed spec-132 runtime; review a changed baseline');
  report.genesis = GENESIS;
  report.blockHash = hash;
  report.blockNumber = header.number.toNumber();
  report.header = header.toJSON();
  report.runtimeVersion = version;
  const metadata = (await api.rpc.state.getMetadata(hash)).toU8a();
  const metadataPath = resolve(__dirname, 'deployed-spec-132-metadata.scale');
  writeFileSync(metadataPath, metadata);
  report.metadata = { path: metadataPath, bytes: metadata.length, sha256: sha256(metadata) };
  const code = hexToU8a((await api.rpc.state.getStorage('0x3a636f6465', hash)).unwrap().toHex());
  writeFileSync(baselinePath, code);
  report.code = { path: baselinePath, bytes: code.length, sha256: sha256(code) };
  report.publicationMarker = (await api.rpc.state.getStorage(stringToHex('runtime:migrations:val_staking_rewards_published'), hash)).toJSON();
  assert.equal(report.publicationMarker, null, 'Reward publication migration already ran; review baseline');
  const [active, current, xorIssuance] = await Promise.all([at.query.staking.activeEra(), at.query.staking.currentEra(), at.query.balances.totalIssuance()]);
  report.activeEra = active.unwrap().toJSON();
  report.currentEra = current.unwrap().toNumber();
  report.historyDepth = at.consts.staking.historyDepth.toNumber();
  report.xorIssuance = xorIssuance.toString();
  assert.equal(report.historyDepth, 84);
  report.eras = [];
  for (let era = report.currentEra - report.historyDepth; era < Math.min(report.activeEra.index, report.currentEra); era++) {
    const [standard, val] = await Promise.all([at.query.staking.erasValidatorReward(era), at.query.xorFee.valStakingEraReward(era)]);
    const valRaw = await api.rpc.state.getStorage(at.query.xorFee.valStakingEraReward.key(era), hash);
    report.eras.push({ era, standard: standard.isSome ? standard.unwrap().toString() : null, val: val.toString(), valPresent: valRaw.isSome });
  }
  for (let era = report.activeEra.index - 1; era >= report.activeEra.index - 5 && !report.payoutSample; era--) {
    const budget = report.eras.find(row => row.era === era);
    if (!budget || budget.standard === null || !budget.valPresent || BigInt(budget.val) === 0n) continue;
    const points = await at.query.staking.erasRewardPoints(era);
    for (const [validator, rewardPoints] of points.individual) {
      if (rewardPoints.isZero()) continue;
      const [overview, claimed, bonded, payee, clipped] = await Promise.all([
        at.query.staking.erasStakersOverview(era, validator), at.query.staking.claimedRewards(era, validator),
        at.query.staking.bonded(validator), at.query.staking.payee(validator), at.query.staking.erasStakersClipped(era, validator),
      ]);
      if (bonded.isNone || payee.isNone || payee.unwrap().type === 'None') continue;
      const ledger = await at.query.staking.ledger(bonded.unwrap());
      if (ledger.isNone || ledger.unwrap().legacyClaimedRewards.some(item => item.toNumber() === era)) continue;
      const pageCount = overview.isSome ? Math.max(1, overview.unwrap().pageCount.toNumber()) : (!clipped.total.isZero() ? 1 : 0);
      const claimedPages = claimed.map(item => item.toNumber());
      const page = Array.from({ length: pageCount }, (_, index) => index).find(index => !claimedPages.includes(index));
      if (page === undefined) continue;
      const exposure = overview.isSome ? (await at.query.staking.erasStakersPaged(era, validator, page)).unwrap() : clipped;
      const prefs = await at.query.staking.erasValidatorPrefs(era, validator);
      report.payoutSample = { era, validator: validator.toString(), controller: bonded.unwrap().toString(), page, pageCount, budget,
        claimedPages, ledger: ledger.unwrap().toJSON(), payee: payee.unwrap().toJSON(), rewardPoints: rewardPoints.toString(),
        totalRewardPoints: points.total.toString(), overview: overview.toJSON(), exposure: exposure.toJSON(), prefs: prefs.toJSON() };
      break;
    }
  }
  assert(report.payoutSample, 'No funded unclaimed page in the latest five eras');
  report.summary = { boundedCompletedEras: report.eras.length, nonzeroValBudgets: report.eras.filter(row => BigInt(row.val) > 0n).length,
    standardZeroValPositive: report.eras.filter(row => row.standard === '0' && BigInt(row.val) > 0n).length };
  report.inputs = { scriptSha256: sha256(readFileSync(__filename)), packageLockSha256: sha256(readFileSync(resolve(__dirname, 'package-lock.json'))) };
  report.status = 'passed';
}
const timeout = setTimeout(() => { report.status = 'timed_out'; save(); process.exit(2); }, 300000);
main().catch(error => { report.status = 'failed'; report.error = { message: error.message, stack: error.stack }; console.error(error); process.exitCode = 1; }).finally(async () => {
  clearTimeout(timeout); report.finishedAt = new Date().toISOString(); save();
  if (api) await api.disconnect().catch(() => {});
  console.log(JSON.stringify({ status: report.status, output, block: report.blockNumber, summary: report.summary, payoutSample: report.payoutSample && { era: report.payoutSample.era, validator: report.payoutSample.validator, page: report.payoutSample.page }, error: report.error?.message }));
});
