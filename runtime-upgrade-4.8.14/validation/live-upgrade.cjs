// Live-mainnet fork check of a SORA runtime upgrade and the guardian halt.
// Public RPC is read-only; the upgrade and every transaction run in a local
// Chopsticks fork with mocked signatures. Nothing is submitted to the network.
// Usage: node live-upgrade.cjs <candidate.compact.compressed.wasm> [report.json]
const { createRequire } = require('node:module');
const req = createRequire(process.env.SORA_VALIDATION_PACKAGE || require('node:path').resolve(__dirname, '../../runtime-upgrade-4.8.12/validation/package.json'));
const { setup, BuildBlockMode, ChopsticksProvider, setStorage, destroyWorker } = req('@acala-network/chopsticks-core');
const { ApiPromise } = req('@polkadot/api');
const { fullOverrideBundle } = req('@sora-substrate/type-definitions');
const { u8aToHex, hexToU8a, stringToU8a } = req('@polkadot/util');
const { cryptoWaitReady, encodeAddress, randomAsU8a } = req('@polkadot/util-crypto');
const fs = require('node:fs');
const { createHash } = require('node:crypto');
const assert = require('node:assert/strict');

const ENDPOINT = process.env.ENDPOINT || 'wss://mof2.sora.org';
const WASM = process.argv[2];
const OUT = process.argv[3];
const GUARDIAN = 'cnSECvLF9MwFojPXk94PVKS4N34WsgZDPNDYMjxcbJWWTPWkY';
const LATCH = u8aToHex(stringToU8a(':sora-nexus:recovery:bridge-halt:v1'));
const SIGNAL = u8aToHex(stringToU8a(':sora-nexus:bridge-halt:latest:v1'));
const report = { endpoint: ENDPOINT, checks: [] };
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
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

(async () => {
  await cryptoWaitReady();
  const wasm = fs.readFileSync(WASM);
  report.candidate = { path: WASM, bytes: wasm.length, sha256: sha256(wasm) };

  // Pin the newest finalized block.
  const probe = await ApiPromise.create({ provider: new (req('@polkadot/api').WsProvider)(ENDPOINT), noInitWarn: true,
    typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } });
  const finalized = await probe.rpc.chain.getFinalizedHead();
  const finalizedHeader = await probe.rpc.chain.getHeader(finalized);
  report.base = { hash: finalized.toHex(), number: finalizedHeader.number.toNumber() };
  await probe.disconnect();
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
  const deployed = chain.head;
  report.deployed = { specVersion: (await deployed.runtimeVersion).specVersion, codeSha256: sha256(hexToU8a(await deployed.get('0x3a636f6465'))) };
  const latchBefore = await deployed.get(LATCH);
  check('live chain runs the expected runtime', report.deployed.specVersion === 138, report.deployed);

  const issuanceKey = null;
  deployed.setWasm(u8aToHex(wasm));
  const candidateVersion = await deployed.runtimeVersion;
  report.candidate.specVersion = candidateVersion.specVersion;
  report.candidate.transactionVersion = candidateVersion.transactionVersion;
  check('candidate is spec 140, transaction version 134', candidateVersion.specVersion === 140 && candidateVersion.transactionVersion === 134,
    { spec: candidateVersion.specVersion, tx: candidateVersion.transactionVersion });

  const api = await ApiPromise.create({ provider: new ChopsticksProvider(chain), noInitWarn: true,
    typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } });
  const issuanceBefore = (await api.query.balances.totalIssuance()).toString();

  // The upgrade block: Core_initialize_block runs every on_runtime_upgrade hook.
  const started = Date.now();
  const upgrade = await chain.newBlock();
  report.upgradeBlock = { number: upgrade.number, hash: upgrade.hash, seconds: (Date.now() - started) / 1000 };
  const upgradeApi = await api.at(upgrade.hash);
  const lastUpgrade = (await upgradeApi.query.system.lastRuntimeUpgrade()).toJSON();
  check('upgrade block applied runtime 140', (await upgrade.runtimeVersion).specVersion === 140 && lastUpgrade.specVersion === 140, lastUpgrade);
  const upgradeEvents = await events(api, upgrade.hash);
  report.upgradeEvents = upgradeEvents;
  const failed = upgradeEvents.filter((e) => e.method === 'ExtrinsicFailed');
  check('no inherent failed in the upgrade block', failed.length === 0, failed);
  check('migration seeded the requested guardian',
    upgradeEvents.some((e) => e.section === 'bridgeHalt' && e.method === 'GuardianAdded' && e.data.account === GUARDIAN));
  const guardians = (await upgradeApi.query.bridgeHalt.guardians()).toJSON().map((key) => encodeAddress(key, 69));
  check('guardian list is exactly the seeded account', JSON.stringify(guardians) === JSON.stringify([GUARDIAN]), guardians);
  const weight = (await upgradeApi.query.system.blockWeight()).toJSON();
  const maxBlock = api.consts.system.blockWeights.maxBlock.toJSON();
  report.upgradeWeight = { used: weight, maxBlock };
  const mandatory = BigInt(weight.mandatory.refTime);
  check('upgrade block weight fits in a block', mandatory <= BigInt(maxBlock.refTime), { mandatoryRefTime: mandatory.toString(), maxRefTime: String(maxBlock.refTime) });
  check('native XOR issuance unchanged by the upgrade', (await upgradeApi.query.balances.totalIssuance()).toString() === issuanceBefore, issuanceBefore);
  check("Root's recovery latch left as it was", (await upgrade.get(LATCH)) === latchBefore, latchBefore);
  check('no guardian halt before anyone halts', (await upgrade.get(SIGNAL)) === undefined || (await upgrade.get(SIGNAL)) === null);

  // A normal block after the upgrade.
  const next = await chain.newBlock();
  check('the next block builds on runtime 140', (await next.runtimeVersion).specVersion === 140, { number: next.number });

  // Guardian halt from the real seeded account, with a mocked signature.
  await fund(chain, api, GUARDIAN);
  const stranger = encodeAddress(randomAsU8a(32), 69);
  await fund(chain, api, stranger);
  const guardianBefore = (await api.query.system.account(GUARDIAN)).data.free.toString();
  const message = 'live-fork rehearsal: guardian halt';
  const haltTx = mockSign(api, api.tx.bridgeHalt.halt(u8aToHex(stringToU8a(message))), GUARDIAN, 0);
  await api.rpc.author.submitExtrinsic(haltTx);
  const strangerTx = mockSign(api, api.tx.bridgeHalt.halt('0x'), stranger, 0);
  let strangerError = null;
  try { await api.rpc.author.submitExtrinsic(strangerTx); } catch (error) { strangerError = String(error.message || error); }
  check('a non-guardian halt is rejected before inclusion', /Invalid signing address|BadSigner/i.test(strangerError || ''), strangerError);
  const haltBlock = await chain.newBlock();
  const haltEvents = await events(api, haltBlock.hash);
  report.haltEvents = haltEvents;
  const halted = haltEvents.find((e) => e.section === 'bridgeHalt' && e.method === 'BridgeHalted');
  check('guardian halt included as halt #1', Boolean(halted) && halted.data.number === '1' && halted.data.guardian === GUARDIAN, halted);
  const fee = haltEvents.find((e) => e.section === 'transactionPayment' && e.method === 'TransactionFeePaid' && e.data.who === GUARDIAN);
  check('guardian paid no fee', Boolean(fee) && fee.data.actualFee === '0' && fee.data.tip === '0', fee);
  const haltApi = await api.at(haltBlock.hash);
  check('guardian balance unchanged', (await haltApi.query.system.account(GUARDIAN)).data.free.toString() === guardianBefore);
  const signal = await haltBlock.get(SIGNAL);
  const expected = u8aToHex(new Uint8Array([1, 0, 0, 0, ...new Uint8Array(new Uint32Array([haltBlock.number]).buffer)]));
  check('workers see halt #1 at its block', signal === expected, { signal, expected });
  const last = (await haltApi.query.bridgeHalt.lastHalt()).toJSON();
  check('LastHalt records number, guardian and block', last && last.number === 1 && encodeAddress(last.guardian, 69) === GUARDIAN && last.block === haltBlock.number, last);
  check("Root's recovery latch still untouched", (await haltBlock.get(LATCH)) === latchBefore);

  // Within the cooldown, another free halt is refused.
  let staleError = null;
  try { await api.rpc.author.submitExtrinsic(mockSign(api, api.tx.bridgeHalt.halt('0x'), GUARDIAN, 1)); } catch (error) { staleError = String(error.message || error); }
  check('a second halt inside the cooldown is refused', /outdated|Stale/i.test(staleError || ''), staleError);

  report.ok = report.checks.every((c) => c.ok);
  if (OUT) fs.writeFileSync(OUT, JSON.stringify(report, null, 2));
  console.log(report.ok ? 'ALL CHECKS PASSED' : 'SOME CHECKS FAILED');
  await api.disconnect();
  await chain.close();
  await destroyWorker();
  process.exit(report.ok ? 0 : 1);
})().catch((error) => {
  console.error('SCRIPT ERROR', error);
  if (OUT) fs.writeFileSync(OUT, JSON.stringify({ ...report, error: String(error && error.stack || error) }, null, 2));
  process.exit(2);
});
