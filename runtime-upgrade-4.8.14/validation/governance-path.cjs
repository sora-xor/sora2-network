// Live-mainnet fork check of governance ON the candidate runtime: a council motion,
// a technical committee fast-track, a referendum and the Root dispatch that removes
// Root's bridge latch. This is the path every later upgrade and the bridge reopening
// take once the candidate is enacted, so its admission rules must not lock it out.
// Public RPC is read-only; every transaction runs in a local Chopsticks fork with
// mocked signatures. Nothing is submitted to the network.
// Usage: node governance-path.cjs [--block <hash> | --latest-finalized] <candidate.wasm> [report.json]
const { createRequire } = require('node:module');
const req = createRequire(process.env.SORA_VALIDATION_PACKAGE || require('node:path').resolve(__dirname, '../../runtime-upgrade-4.8.12/validation/package.json'));
const { setup, BuildBlockMode, ChopsticksProvider, setStorage, destroyWorker } = req('@acala-network/chopsticks-core');
const { ApiPromise, WsProvider } = req('@polkadot/api');
const { fullOverrideBundle } = req('@sora-substrate/type-definitions');
const { u8aToHex, stringToU8a } = req('@polkadot/util');
const { cryptoWaitReady, encodeAddress, randomAsU8a, blake2AsHex } = req('@polkadot/util-crypto');
const fs = require('node:fs');
const assert = require('node:assert/strict');
const { parseArgs } = require('node:util');
const { MAINNET_GENESIS, RECORDED_BLOCK, POLICY_KEY, candidateSpec, mainnetBaseline, sha256 } = require('./release-options.cjs');

const ENDPOINT = process.env.ENDPOINT || 'wss://mof2.sora.org';
const LATCH = u8aToHex(stringToU8a(':sora-nexus:recovery:bridge-halt:v1'));
const types = { typesBundle: { spec: { 'sora-substrate': fullOverrideBundle.spec.sora } } };
const report = { endpoint: ENDPOINT, checks: [] };

const check = (name, ok, detail) => {
  report.checks.push({ name, ok, detail });
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}${detail === undefined ? '' : ` — ${typeof detail === 'string' ? detail : JSON.stringify(detail)}`}`);
};

function mockSign(api, tx, address, nonce) {
  tx.signFake(address, { nonce, blockHash: api.genesisHash, genesisHash: api.genesisHash, runtimeVersion: api.runtimeVersion, signedExtensions: api.registry.signedExtensions });
  const signature = new Uint8Array(64).fill(0xcd);
  signature.set([0xde, 0xad, 0xbe, 0xef]);
  tx.signature.set(signature);
  return tx;
}

async function fund(chain, api, address) {
  const info = (await api.query.system.account(address)).toJSON();
  info.providers = Math.max(1, info.providers || 0);
  info.data.free = '1000000000000000000000';
  await setStorage(chain, { System: { Account: [[[address], info]] } });
}

async function eventsAt(api, hash) {
  const records = await (await api.at(hash)).query.system.events();
  return records.map(({ event }) => {
    const entry = { section: event.section, method: event.method, data: event.data.toHuman() };
    if (event.section === 'system' && event.method === 'ExtrinsicFailed') {
      const error = event.data[0];
      entry.error = error.isModule ? (({ section, name }) => `${section}.${name}`)(api.registry.findMetaError(error.asModule)) : error.toString();
    }
    return entry;
  });
}
const governance = (events) => events.filter((e) => ['council', 'technicalCommittee', 'democracy', 'scheduler', 'utility', 'preimage'].includes(e.section) || e.error)
  .map((e) => `${e.section}.${e.method}${e.error ? `(${e.error})` : ''}`);
const has = (events, section, method) => events.some((e) => e.section === section && e.method === method);
const clean = (events) => !has(events, 'system', 'ExtrinsicFailed');

function parseOptions(args) {
  const { values, positionals } = parseArgs({ args, allowPositionals: true, options: { block: { type: 'string' }, 'latest-finalized': { type: 'boolean' } } });
  assert(positionals.length === 1 || positionals.length === 2, 'Expected <candidate.wasm> [report.json]');
  assert(!(values.block !== undefined && values['latest-finalized']), 'Choose either --block or --latest-finalized');
  const block = values['latest-finalized'] ? null : (values.block ?? RECORDED_BLOCK);
  assert(block === null || /^0x[0-9a-f]{64}$/i.test(block), '--block must be a 32-byte block hash');
  return { wasmPath: positionals[0], reportPath: positionals[1], block };
}

async function run({ wasmPath, reportPath, block }) {
  await cryptoWaitReady();
  const spec = candidateSpec();
  const wasm = fs.readFileSync(wasmPath);
  report.candidate = { path: wasmPath, bytes: wasm.length, sha256: sha256(wasm) };
  const probe = await ApiPromise.create({ provider: new WsProvider(ENDPOINT), noInitWarn: true, ...types });
  try {
    const baseline = await mainnetBaseline(probe, block);
    report.genesisHash = MAINNET_GENESIS;
    report.base = baseline.base;
    report.deployed = baseline.deployed;
  } finally {
    await probe.disconnect();
  }
  console.log(`Forking ${ENDPOINT} at finalized #${report.base.number} ${report.base.hash}`);
  const chain = await setup({ endpoint: ENDPOINT, block: report.base.hash, registeredTypes: types, buildBlockMode: BuildBlockMode.Manual,
    mockSignatureHost: true, allowUnresolvedImports: true, offchainWorker: false, runtimeLogLevel: 3, rpcTimeout: 120000 });
  assert.equal(await chain.head.get(POLICY_KEY), report.deployed.policyCommitment, 'Fork recovery policy differs from mainnet');
  const latchBefore = await chain.head.get(LATCH);
  chain.head.setWasm(u8aToHex(wasm));
  const upgrade = await chain.newBlock();
  const api = await ApiPromise.create({ provider: new ChopsticksProvider(chain), noInitWarn: true, ...types });
  const installed = (await upgrade.runtimeVersion).specVersion;
  report.candidate.specVersion = installed;
  check(`fork runs the candidate runtime ${spec}`, installed === spec, { block: upgrade.number });
  check("Root's recovery latch is set before the governance run", latchBefore !== undefined && latchBefore !== null && (await chain.head.get(LATCH)) === latchBefore, latchBefore);
  await chain.newBlock();

  const council = (await api.query.council.members()).map((member) => member.toString());
  const committee = (await api.query.technicalCommittee.members()).map((member) => member.toString());
  const councilThreshold = Math.ceil(council.length / 2);
  const committeeThreshold = Math.floor(committee.length / 2) + 1;
  report.council = { members: council.length, threshold: councilThreshold };
  report.technicalCommittee = { members: committee.length, threshold: committeeThreshold };
  assert(council.length > councilThreshold && committee.length >= committeeThreshold, 'Unexpected collective sizes');
  const stranger = encodeAddress(randomAsU8a(32), 69);
  const voter = encodeAddress(randomAsU8a(32), 69);
  for (const account of [...council, ...committee, stranger, voter]) await fund(chain, api, account);
  const submit = async (tx, who) => {
    const nonce = (await api.query.system.account(who)).nonce.toNumber();
    try { await api.rpc.author.submitExtrinsic(mockSign(api, tx, who, nonce)); return null; } catch (error) { return String(error.message || error); }
  };
  const build = async () => eventsAt(api, (await chain.newBlock()).hash);
  const bound = { refTime: 500_000_000_000, proofSize: 3_000_000 };

  // The Root call that reopens the chain side of the bridges, as a guarded majority proposal.
  const removeLatch = api.tx.system.killStorage([LATCH]);
  const proposal = { Inline: removeLatch.method.toHex() };
  const proposalHash = blake2AsHex(removeLatch.method.toU8a());
  const motion = api.tx.utility.batchAll([api.tx.democracy.externalPropose(proposal), api.tx.democracy.externalProposeMajority(proposal)]);
  const motionHash = blake2AsHex(motion.method.toU8a());
  const motionLength = motion.method.encodedLength;
  const motionIndex = (await api.query.council.proposalCount()).toNumber();

  let error = await submit(api.tx.council.propose(councilThreshold, motion.method, motionLength), stranger);
  check('a non-member council.propose is refused before inclusion', /BadSigner|Invalid signing address/i.test(error || ''), error);
  error = await submit(api.tx.council.propose(councilThreshold, motion.method, motionLength), council[0]);
  let events = await build();
  check('a council member proposes the latch removal', !error && has(events, 'council', 'Proposed') && clean(events), error || governance(events));
  error = await submit(api.tx.council.vote(motionHash, motionIndex, true), stranger);
  check('a non-member council.vote is refused before inclusion', /BadSigner|Invalid signing address/i.test(error || ''), error);
  // The proposer's aye is not implied: all but the deciding aye are sent one by one.
  for (const member of council.slice(1, councilThreshold)) {
    error = await submit(api.tx.council.vote(motionHash, motionIndex, true), member);
    events = await build();
    check(`council aye from ${member} is included`, !error && has(events, 'council', 'Voted') && clean(events), error || governance(events));
  }
  error = await submit(api.tx.council.close(motionHash, motionIndex, bound, motionLength), stranger);
  check('a council.close that cannot succeed yet is refused before inclusion', Boolean(error), error);
  const decide = api.tx.utility.batchAll([api.tx.council.vote(motionHash, motionIndex, true), api.tx.council.close(motionHash, motionIndex, bound, motionLength)]);
  error = await submit(decide, council[councilThreshold]);
  events = await build();
  const executed = events.find((e) => e.section === 'council' && e.method === 'Executed');
  check('the deciding council aye and the close in one batchAll execute the motion',
    !error && has(events, 'council', 'Closed') && has(events, 'council', 'Approved') && Boolean(executed) && executed.data.result === 'Ok' && clean(events), error || governance(events));
  const nextExternal = (await api.query.democracy.nextExternal()).toJSON();
  check('democracy.nextExternal holds the latch removal as a simple-majority proposal',
    Array.isArray(nextExternal) && nextExternal[0].inline === removeLatch.method.toHex() && /simplemajority/i.test(JSON.stringify(nextExternal[1])), nextExternal);

  const fastTrack = api.tx.democracy.fastTrack(proposalHash, api.consts.democracy.fastTrackVotingPeriod.toNumber(), 0);
  const fastTrackHash = blake2AsHex(fastTrack.method.toU8a());
  const fastTrackLength = fastTrack.method.encodedLength;
  const fastTrackIndex = (await api.query.technicalCommittee.proposalCount()).toNumber();
  error = await submit(api.tx.technicalCommittee.propose(committeeThreshold, fastTrack.method, fastTrackLength), stranger);
  check('a non-member technicalCommittee.propose is refused before inclusion', /BadSigner|Invalid signing address/i.test(error || ''), error);
  error = await submit(api.tx.technicalCommittee.propose(committeeThreshold, fastTrack.method, fastTrackLength), committee[0]);
  events = await build();
  check('a technical committee member proposes the fast-track', !error && has(events, 'technicalCommittee', 'Proposed') && clean(events), error || governance(events));
  for (const member of committee.slice(1, committeeThreshold + 1)) {
    error = await submit(api.tx.technicalCommittee.vote(fastTrackHash, fastTrackIndex, true), member);
    events = await build();
    check(`technical committee aye from ${member} is included`, !error && has(events, 'technicalCommittee', 'Voted') && clean(events), error || governance(events));
  }
  const referendum = (await api.query.democracy.referendumCount()).toNumber();
  error = await submit(api.tx.technicalCommittee.close(fastTrackHash, fastTrackIndex, bound, fastTrackLength), stranger);
  events = await build();
  check('a non-member closes the decided committee motion and the referendum starts',
    !error && has(events, 'technicalCommittee', 'Executed') && has(events, 'democracy', 'Started') && clean(events), error || governance(events));

  error = await submit(api.tx.democracy.vote(referendum, { Standard: { vote: { aye: true, conviction: 'None' }, balance: '100000000000000000000' } }), voter);
  events = await build();
  check('an aye vote on the referendum is included', !error && has(events, 'democracy', 'Voted') && clean(events), error || governance(events));
  const info = (await api.query.democracy.referendumInfoOf(referendum)).toJSON();
  const ongoing = info && info.ongoing;
  check('the referendum runs the fast-track period with no enactment delay',
    Boolean(ongoing) && ongoing.delay === 0 && ongoing.end > chain.head.number, ongoing && { end: ongoing.end, delay: ongoing.delay, threshold: ongoing.threshold });
  assert(ongoing, 'Referendum did not start');
  // Local shortcut, recorded in the report: end the vote in the next block instead of
  // building the whole voting period. Tallying and enactment then run unchanged.
  report.localShortcut = { referendum, recordedEnd: ongoing.end, movedTo: chain.head.number + 1 };
  ongoing.end = chain.head.number + 1;
  await setStorage(chain, { Democracy: { ReferendumInfoOf: [[[referendum], info]] } });
  let passed = false;
  let dispatched = null;
  const seen = [];
  for (let built = 0; built < 4 && (await chain.head.get(LATCH)) != null; built++) {
    events = await build();
    seen.push(...governance(events));
    passed = passed || has(events, 'democracy', 'Passed');
    dispatched = events.find((e) => e.section === 'scheduler' && e.method === 'Dispatched') || dispatched;
  }
  check('the referendum passes', passed, seen);
  check('the scheduler dispatches the Root call with Ok', Boolean(dispatched) && dispatched.data.result === 'Ok', dispatched ? dispatched.data : seen);
  const latchAfter = await chain.head.get(LATCH);
  check(`Root's recovery latch is removed on runtime ${spec}`, latchAfter == null, String(latchAfter));
  check('the recovery policy commitment is untouched', (await chain.head.get(POLICY_KEY)) === report.deployed.policyCommitment);

  // A later upgrade notes its runtime preimage under the candidate's admission rules.
  const preimage = api.tx.system.setCode(u8aToHex(wasm)).method.toU8a();
  error = await submit(api.tx.preimage.notePreimage(u8aToHex(preimage)), council[council.length - 1]);
  events = await build();
  check('a runtime-sized preimage is noted on the candidate runtime', !error && has(events, 'preimage', 'Noted') && clean(events), error || { bytes: preimage.length, events: governance(events) });

  report.ok = report.checks.every((c) => c.ok);
  if (reportPath) fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
  console.log(report.ok ? 'ALL CHECKS PASSED' : 'SOME CHECKS FAILED');
  await api.disconnect();
  await chain.close();
  await destroyWorker();
  return report.ok ? 0 : 1;
}

if (require.main === module) {
  const options = parseOptions(process.argv.slice(2));
  run(options).then((code) => process.exit(code)).catch((error) => {
    console.error('SCRIPT ERROR', error);
    if (options.reportPath) fs.writeFileSync(options.reportPath, JSON.stringify({ ...report, ok: false, error: String(error && error.stack || error) }, null, 2));
    process.exit(2);
  });
}

module.exports = { parseOptions };
