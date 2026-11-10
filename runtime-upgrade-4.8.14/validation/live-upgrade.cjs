// Live-mainnet fork check of a SORA Nexus runtime upgrade and the guardian halt.
// Public RPC is read-only; the upgrade and every transaction run in a local
// Chopsticks fork with mocked signatures. Nothing is submitted to the network.
// Usage: node live-upgrade.cjs [--candidate-spec 140|141] [--block <hash> | --latest-finalized] [--via-140] <candidate.wasm> [report.json]
const { createRequire } = require('node:module');
const req = createRequire(process.env.SORA_VALIDATION_PACKAGE || require('node:path').resolve(__dirname, '../../runtime-upgrade-4.8.12/validation/package.json'));
const { setup, BuildBlockMode, ChopsticksProvider, setStorage, destroyWorker } = req('@acala-network/chopsticks-core');
const { ApiPromise } = req('@polkadot/api');
const { fullOverrideBundle } = req('@sora-substrate/type-definitions');
const { u8aToHex, hexToU8a, stringToU8a } = req('@polkadot/util');
const { cryptoWaitReady, encodeAddress, randomAsU8a, xxhashAsHex } = req('@polkadot/util-crypto');
const fs = require('node:fs');
const path = require('node:path');
const { StorageKey } = req('@polkadot/types');
const assert = require('node:assert/strict');

const ENDPOINT = process.env.ENDPOINT || 'wss://mof2.sora.org';
const { MAINNET_GENESIS, RECORDED_BLOCK, CODE_KEY, POLICY_KEY, candidateSpec, mainnetBaseline, sha256 } = require('./release-options.cjs');
const { parseArgs } = require('node:util');
// The accounts `BridgeHaltInitialGuardians` lists; the upgrade from spec 138 seeds exactly these.
const GUARDIANS = ['cnSECvLF9MwFojPXk94PVKS4N34WsgZDPNDYMjxcbJWWTPWkY', 'cnU8UgdM4fs9iiCxkRDW8vp2bx4e68LbePkcuQPv1LERKH5oH'];
const LATCH = u8aToHex(stringToU8a(':sora-nexus:recovery:bridge-halt:v1'));
const SIGNAL = u8aToHex(stringToU8a(':sora-nexus:bridge-halt:latest:v1'));
const report = { endpoint: ENDPOINT, checks: [] };

const check = (name, ok, detail) => {
  report.checks.push({ name, ok, detail });
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}${detail === undefined ? '' : ` — ${typeof detail === 'string' ? detail : JSON.stringify(detail)}`}`);
};

function mockSign(api, tx, address, nonce) {
  tx.signFake(address, {
    nonce,
    blockHash: api.genesisHash,
    genesisHash: api.genesisHash,
    runtimeVersion: api.runtimeVersion,
    signedExtensions: api.registry.signedExtensions,
  });
  const signature = new Uint8Array(64).fill(0xcd);
  signature.set([0xde, 0xad, 0xbe, 0xef]);
  tx.signature.set(signature);
  return tx;
}

async function events(api, hash) {
  const records = await (await api.at(hash)).query.system.events();
  return records.map(({ event }) => ({ section: event.section, method: event.method, data: event.data.toHuman() }));
}

async function fund(chain, api, address) {
  const info = (await api.query.system.account(address)).toJSON();
  info.providers = Math.max(1, info.providers || 0);
  info.data.free = '1000000000000000000000';
  await setStorage(chain, { System: { Account: [[[address], info]] } });
}

function parseOptions(args) {
  const { values, positionals } = parseArgs({
    args, allowPositionals: true,
    options: { 'candidate-spec': { type: 'string' }, block: { type: 'string' }, 'latest-finalized': { type: 'boolean' }, 'via-140': { type: 'boolean' } },
  });
  assert(positionals.length === 1 || positionals.length === 2, 'Expected <candidate.wasm> [report.json]');
  assert(!(values.block !== undefined && values['latest-finalized']), 'Choose either --block or --latest-finalized');
  const block = values['latest-finalized'] ? null : (values.block ?? RECORDED_BLOCK);
  assert(block === null || /^0x[0-9a-f]{64}$/i.test(block), '--block must be a 32-byte block hash');
  return { wasmPath: positionals[0], reportPath: positionals[1], block, spec: candidateSpec(values['candidate-spec']), via140: values['via-140'] ?? false };
}

const guardianStorage = (name) => xxhashAsHex('BridgeHalt', 128) + xxhashAsHex(name, 128).slice(2);

async function guardianState(block) {
  const [guardians, lastHalt, signal] = await Promise.all([
    block.get(guardianStorage('Guardians')), block.get(guardianStorage('LastHalt')), block.get(SIGNAL),
  ]);
  return { guardians: guardians ?? null, lastHalt: lastHalt ?? null, signal: signal ?? null };
}

function guardianTransition(baselineSpec, before, after, addresses, seeded, expected = GUARDIANS) {
  if (baselineSpec === 138) {
    return seeded && addresses.length === expected.length && expected.every((guardian) => addresses.includes(guardian)) &&
      before.guardians === null && before.lastHalt === null && before.signal === null &&
      after.lastHalt === null && after.signal === null;
  }
  return !seeded && before.guardians === after.guardians &&
    before.lastHalt === after.lastHalt && before.signal === after.signal;
}

const localApi = (chain) => ApiPromise.create({ provider: new ChopsticksProvider(chain), noInitWarn: true,
  typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } });

// Exercise the already-seeded path even before mainnet has enacted spec 140.
// These local fixture changes never qualify as a governance rehearsal.
async function installIntermediate(chain) {
  const file = path.resolve(__dirname, '../../runtime-upgrade-4.8.14-spec-140/framenode-runtime-4.8.14.compact.compressed.wasm');
  const wasm = fs.readFileSync(file);
  const digest = sha256(wasm);
  assert.equal(digest, '0d9c61bc3142318f595404cc7809128a11beaf80369660688d9b740d08517b7e');
  chain.head.setWasm(u8aToHex(wasm));
  const installed = await chain.newBlock();
  assert.equal((await installed.runtimeVersion).specVersion, 140);
  const api = await localApi(chain);
  try {
    const guardian = encodeAddress(Buffer.alloc(32, 0x73), 69);
    await setStorage(chain, { BridgeHalt: { Guardians: [guardian] } });
    await fund(chain, api, guardian);
    const nonce = (await api.query.system.account(guardian)).nonce.toNumber();
    await api.rpc.author.submitExtrinsic(mockSign(api, api.tx.bridgeHalt.halt('0x'), guardian, nonce));
    const halted = await chain.newBlock();
    const previous = (await (await api.at(halted.hash)).query.bridgeHalt.lastHalt()).toJSON();
    assert(previous && previous.number === 1 && encodeAddress(previous.guardian, 69) === guardian);
    report.intermediate = { specVersion: 140, codeSha256: digest, fixtureGuardian: guardian, previousHalt: previous };
  } finally {
    await api.disconnect();
  }
}

async function run({ wasmPath, reportPath, block, spec, via140 }) {
  await cryptoWaitReady();
  const wasm = fs.readFileSync(wasmPath);
  report.candidate = { path: wasmPath, bytes: wasm.length, sha256: sha256(wasm) };

  const probe = await ApiPromise.create({ provider: new (req('@polkadot/api').WsProvider)(ENDPOINT), noInitWarn: true,
    typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } });
  try {
    const baseline = await mainnetBaseline(probe, block);
    report.genesisHash = MAINNET_GENESIS;
    report.base = baseline.base;
    report.deployed = baseline.deployed;
  } finally {
    await probe.disconnect();
  }
  console.log(`Forking ${ENDPOINT} at finalized #${report.base.number} ${report.base.hash}`);

  const chain = await setup({
    endpoint: ENDPOINT,
    block: report.base.hash,
    registeredTypes: { typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } },
    buildBlockMode: BuildBlockMode.Manual,
    mockSignatureHost: true,
    allowUnresolvedImports: true,
    offchainWorker: false,
    runtimeLogLevel: 3,
    rpcTimeout: 120000,
  });
  const base = chain.head;
  assert.equal(base.hash, report.base.hash);
  assert.equal(base.number, report.base.number);
  const meta = await base.meta;
  const genesisKey = new StorageKey(meta.registry, [meta.query.system.blockHash, [0]]);
  assert.equal(await base.get(genesisKey.toHex()), MAINNET_GENESIS, 'Fork storage genesis differs from mainnet');
  assert.equal(await base.get(POLICY_KEY), report.deployed.policyCommitment, 'Fork recovery policy differs from mainnet');
  assert.equal((await base.runtimeVersion).specVersion, report.deployed.specVersion);
  assert.equal(sha256(hexToU8a(await base.get(CODE_KEY))), report.deployed.codeSha256, 'Fork Wasm differs from mainnet');
  assert([138, 139, 140].includes(report.deployed.specVersion) && report.deployed.specVersion < spec, 'Unsupported upgrade path');
  check('fork uses finalized mainnet state and the expected prior runtime', true, report.deployed);

  if (via140) {
    assert(report.deployed.specVersion === 138 && spec > 140, '--via-140 requires a spec-138 base and a later candidate');
    await installIntermediate(chain);
  }
  const deployed = chain.head;
  const priorSpec = (await deployed.runtimeVersion).specVersion;
  const before = await guardianState(deployed);
  const latchBefore = await deployed.get(LATCH);
  deployed.setWasm(u8aToHex(wasm));
  const candidateVersion = await deployed.runtimeVersion;
  report.candidate.specVersion = candidateVersion.specVersion;
  report.candidate.transactionVersion = candidateVersion.transactionVersion;
  check(`candidate is spec ${spec}, transaction version 134`, candidateVersion.specVersion === spec && candidateVersion.transactionVersion === 134,
    { spec: candidateVersion.specVersion, tx: candidateVersion.transactionVersion });

  const api = await localApi(chain);
  const issuanceBefore = (await api.query.balances.totalIssuance()).toString();

  // The upgrade block: Core_initialize_block runs every on_runtime_upgrade hook.
  const started = Date.now();
  const upgrade = await chain.newBlock();
  report.upgradeBlock = { number: upgrade.number, hash: upgrade.hash, seconds: (Date.now() - started) / 1000 };
  const upgradeApi = await api.at(upgrade.hash);
  const lastUpgrade = (await upgradeApi.query.system.lastRuntimeUpgrade()).toJSON();
  check(`upgrade block applied runtime ${spec}`, (await upgrade.runtimeVersion).specVersion === spec && lastUpgrade.specVersion === spec, lastUpgrade);
  const upgradeEvents = await events(api, upgrade.hash);
  report.upgradeEvents = upgradeEvents;
  const failed = upgradeEvents.filter((e) => e.method === 'ExtrinsicFailed');
  check('no inherent failed in the upgrade block', failed.length === 0, failed);
  const seeded = upgradeEvents.some((e) => e.section === 'bridgeHalt' && e.method === 'GuardianAdded');
  const guardians = (await upgradeApi.query.bridgeHalt.guardians()).toJSON().map((key) => encodeAddress(key, 69));
  const after = await guardianState(upgrade);
  report.guardianTransition = { priorSpec, before, after, guardians };
  // The superseded spec-140 cut seeded only the first account.
  const transitionOk = guardianTransition(priorSpec, before, after, guardians, seeded, spec === 140 ? GUARDIANS.slice(0, 1) : GUARDIANS);
  check(priorSpec === 138 ? 'migration seeded the requested guardian' : 'existing guardian state preserved', transitionOk, report.guardianTransition);
  // Never install a test signer over failed migration evidence.
  assert(transitionOk, 'Unexpected guardian migration');
  const weight = (await upgradeApi.query.system.blockWeight()).toJSON();
  const maxBlock = api.consts.system.blockWeights.maxBlock.toJSON();
  report.upgradeWeight = { used: weight, maxBlock };
  const mandatory = BigInt(weight.mandatory.refTime);
  check('upgrade block weight fits in a block', mandatory <= BigInt(maxBlock.refTime), { mandatoryRefTime: mandatory.toString(), maxRefTime: String(maxBlock.refTime) });
  check('native XOR issuance unchanged by the upgrade', (await upgradeApi.query.balances.totalIssuance()).toString() === issuanceBefore, issuanceBefore);
  check('mainnet recovery policy preserved by the upgrade', (await upgrade.get(POLICY_KEY)) === report.deployed.policyCommitment);
  check("Root's recovery latch left as it was", (await upgrade.get(LATCH)) === latchBefore, latchBefore);
  check('last halt and worker signal preserved by the upgrade', before.lastHalt === after.lastHalt && before.signal === after.signal);

  // A normal block after the upgrade.
  const next = await chain.newBlock();
  check(`the next block builds on runtime ${spec}`, (await next.runtimeVersion).specVersion === spec, { number: next.number });

  // An empty guardian set is a legitimate governance choice. Only after
  // checking exact preservation, introduce a local smoke-test signer if needed.
  const guardian = GUARDIANS.find((seededGuardian) => guardians.includes(seededGuardian)) ?? guardians[0] ?? GUARDIANS[0];
  if (!guardians.length) {
    report.localGuardianFixture = guardian;
    await setStorage(chain, { BridgeHalt: { Guardians: [guardian] } });
  }
  const previous = (await upgradeApi.query.bridgeHalt.lastHalt()).toJSON();
  const number = Math.min((previous?.number ?? 0) + 1, 0xffffffff);
  if (previous) {
    assert(previous.block <= deployed.number, 'Previous halt lies beyond the baseline');
    const cooldown = api.consts.bridgeHalt.haltCooldown.toNumber();
    while (chain.head.number + 1 < previous.block + cooldown) await chain.newBlock();
  }
  await fund(chain, api, guardian);
  const stranger = encodeAddress(randomAsU8a(32), 69);
  await fund(chain, api, stranger);
  const guardianBefore = (await api.query.system.account(guardian)).data.free.toString();
  const message = 'live-fork rehearsal: guardian halt';
  const nonce = (await api.query.system.account(guardian)).nonce.toNumber();
  const haltTx = mockSign(api, api.tx.bridgeHalt.halt(u8aToHex(stringToU8a(message))), guardian, nonce);
  await api.rpc.author.submitExtrinsic(haltTx);
  const strangerTx = mockSign(api, api.tx.bridgeHalt.halt('0x'), stranger, 0);
  let strangerError = null;
  try { await api.rpc.author.submitExtrinsic(strangerTx); } catch (error) { strangerError = String(error.message || error); }
  check('a non-guardian halt is rejected before inclusion', /Invalid signing address|BadSigner/i.test(strangerError || ''), strangerError);
  const haltBlock = await chain.newBlock();
  const haltEvents = await events(api, haltBlock.hash);
  report.haltEvents = haltEvents;
  const halted = haltEvents.find((e) => e.section === 'bridgeHalt' && e.method === 'BridgeHalted');
  check('guardian halt advances the previous number', Boolean(halted) && Number(String(halted.data.number).replaceAll(',', '')) === number && halted.data.guardian === guardian, halted);
  const fee = haltEvents.find((e) => e.section === 'transactionPayment' && e.method === 'TransactionFeePaid' && e.data.who === guardian);
  check('guardian paid no fee', Boolean(fee) && fee.data.actualFee === '0' && fee.data.tip === '0', fee);
  const haltApi = await api.at(haltBlock.hash);
  check('guardian balance unchanged', (await haltApi.query.system.account(guardian)).data.free.toString() === guardianBefore);
  const signal = await haltBlock.get(SIGNAL);
  const bytes = Buffer.alloc(8);
  bytes.writeUInt32LE(number, 0);
  bytes.writeUInt32LE(haltBlock.number, 4);
  const expected = u8aToHex(bytes);
  check('workers see the next halt at its block', signal === expected, { signal, expected });
  const last = (await haltApi.query.bridgeHalt.lastHalt()).toJSON();
  check('LastHalt records number, guardian and block', last && last.number === number && encodeAddress(last.guardian, 69) === guardian && last.block === haltBlock.number, last);
  check("Root's recovery latch still untouched", (await haltBlock.get(LATCH)) === latchBefore);

  // Within the cooldown, another free halt is refused.
  let staleError = null;
  try { await api.rpc.author.submitExtrinsic(mockSign(api, api.tx.bridgeHalt.halt('0x'), guardian, nonce + 1)); } catch (error) { staleError = String(error.message || error); }
  check('a second halt inside the cooldown is refused', /outdated|Stale/i.test(staleError || ''), staleError);

  // Every other seeded guardian halts too, fee-free, once the cooldown has passed.
  let lastHaltBlock = haltBlock.number;
  let lastNumber = number;
  for (const other of GUARDIANS.filter((seededGuardian) => seededGuardian !== guardian && guardians.includes(seededGuardian))) {
    const cooldown = api.consts.bridgeHalt.haltCooldown.toNumber();
    while (chain.head.number + 1 < lastHaltBlock + cooldown) await chain.newBlock();
    await fund(chain, api, other);
    const otherBefore = (await api.query.system.account(other)).data.free.toString();
    const otherNonce = (await api.query.system.account(other)).nonce.toNumber();
    await api.rpc.author.submitExtrinsic(mockSign(api, api.tx.bridgeHalt.halt(u8aToHex(stringToU8a(message))), other, otherNonce));
    const otherBlock = await chain.newBlock();
    const otherEvents = await events(api, otherBlock.hash);
    const otherHalt = otherEvents.find((e) => e.section === 'bridgeHalt' && e.method === 'BridgeHalted');
    lastNumber += 1;
    check(`seeded guardian ${other} halts after the cooldown`, Boolean(otherHalt) && Number(String(otherHalt.data.number).replaceAll(',', '')) === lastNumber && otherHalt.data.guardian === other, otherHalt);
    const otherFee = otherEvents.find((e) => e.section === 'transactionPayment' && e.method === 'TransactionFeePaid' && e.data.who === other);
    const otherAfter = (await (await api.at(otherBlock.hash)).query.system.account(other)).data.free.toString();
    check(`seeded guardian ${other} paid no fee`, Boolean(otherFee) && otherFee.data.actualFee === '0' && otherFee.data.tip === '0' && otherAfter === otherBefore, otherFee);
    lastHaltBlock = otherBlock.number;
  }

  report.ok = report.checks.every((c) => c.ok);
  if (reportPath) fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
  console.log(report.ok ? 'ALL CHECKS PASSED' : 'SOME CHECKS FAILED');
  await api.disconnect();
  await chain.close();
  await destroyWorker();
  return report.ok ? 0 : 1;
}

function runCli(args) {
  const options = parseOptions(args);
  run(options).then((code) => process.exit(code)).catch((error) => {
    console.error('SCRIPT ERROR', error);
    if (options.reportPath) fs.writeFileSync(options.reportPath, JSON.stringify({
      ...report, ok: false, error: String(error && error.stack || error),
    }, null, 2));
    process.exit(2);
  });
}

if (require.main === module) runCli(process.argv.slice(2));

module.exports = { parseOptions, runCli, guardianTransition };
