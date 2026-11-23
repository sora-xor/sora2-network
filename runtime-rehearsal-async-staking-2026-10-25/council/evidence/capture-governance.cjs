'use strict';
// Read-only, pinned-finalized-block inspection. No keys, signing, or submission.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');
const repo = path.resolve(__dirname, '../../..');
const deps = path.join(repo, 'runtime-rehearsal-async-staking-2026-10-25/validation/node_modules');
const { ApiPromise, WsProvider } = require(path.join(deps, '@polkadot/api'));
const { Metadata, TypeRegistry } = require(path.join(deps, '@polkadot/types'));
const { decorateExtrinsics } = require(path.join(deps, '@polkadot/types/cjs/metadata/index.js'));
const { blake2AsHex } = require(path.join(deps, '@polkadot/util-crypto'));
const packageRoot = path.resolve(__dirname, '..');
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const write = (name, data) => fs.writeFileSync(path.join(__dirname, name), JSON.stringify(data, null, 2) + '\n');
const endpoint = 'wss://mof2.sora.org';
const timeout = setTimeout(() => { console.error('Read-only governance capture timed out'); process.exit(1); }, 55000);
let provider, api;
(async () => {
  provider = new WsProvider(endpoint, false);
  await provider.connect();
  api = await ApiPromise.create({ provider, noInitWarn: true });
  const blockHash = (await api.rpc.chain.getFinalizedHead()).toHex();
  const [header, version, codeHex, genesis, metadataHex] = await Promise.all([
    provider.send('chain_getHeader', [blockHash]),
    provider.send('state_getRuntimeVersion', [blockHash]),
    provider.send('state_getStorage', ['0x3a636f6465', blockHash]),
    provider.send('chain_getBlockHash', [0]),
    provider.send('state_getMetadata', [blockHash]),
  ]);
  assert.equal(genesis, '0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5');
  assert.equal(sha(Buffer.from(codeHex.slice(2), 'hex')), 'de7173a9e0137a32265b341354383958d85beef81103571e6b0aa09622ef2221');
  assert.equal(version.specVersion, 134);
  assert.equal(version.transactionVersion, 131);
  const metadata = Buffer.from(metadataHex.slice(2), 'hex');
  fs.writeFileSync(path.join(__dirname, 'live-metadata.scale'), metadata);
  const registry = new TypeRegistry();
  const decodedMetadata = new Metadata(registry, metadataHex);
  registry.setMetadata(decodedMetadata);
  const tx = decorateExtrinsics(registry, decodedMetadata.asLatest, decodedMetadata.version);
  const at = await api.at(blockHash);
  const proposalHash = JSON.parse(fs.readFileSync(path.join(packageRoot, 'runtime.preimage.json'))).proposal_hash;
  const [council, technical, nextExternal, preimageStatus, requestStatus] = await Promise.all([
    at.query.council.members(), at.query.technicalCommittee.members(),
    at.query.democracy.nextExternal(),
    at.query.preimage.statusFor(proposalHash), at.query.preimage.requestStatusFor(proposalHash),
  ]);
  assert(council.length > 0 && technical.length > 0);
  const councilThreshold = Math.floor(council.length / 2) + 1;
  const technicalThreshold = Math.floor(technical.length / 2) + 1;
  const votingPeriod = at.consts.democracy.fastTrackVotingPeriod.toNumber();
  const summary = JSON.parse(fs.readFileSync(path.join(packageRoot, 'runtime.preimage.json')));
  const wasm = fs.readFileSync(path.join(packageRoot, 'sora-nexus-runtime-137.compact.compressed.wasm'));
  const call = tx.system.setCode('0x' + wasm.toString('hex'));
  const raw = fs.readFileSync(path.join(packageRoot, 'runtime.preimage.call'));
  assert.equal(Buffer.from(call.toU8a()).compare(raw), 0);
  assert.equal(blake2AsHex(raw, 256), summary.proposal_hash);
  assert.equal(raw.length, summary.proposal_len);
  const external = tx.democracy.externalProposeMajority({ Lookup: { hash: summary.proposal_hash, len: raw.length } });
  assert.equal(external.encodedLength, 39);
  const councilCall = tx.council.propose(councilThreshold, external, external.encodedLength);
  const fastTrack = tx.democracy.fastTrack(summary.proposal_hash, votingPeriod, 0);
  assert.equal(fastTrack.encodedLength, 42);
  const technicalCall = tx.technicalCommittee.propose(technicalThreshold, fastTrack, fastTrack.encodedLength);
  const notePreimage = tx.preimage.notePreimage('0x' + raw.toString('hex'));
  for (const [name, value] of [['council-propose.hex', councilCall], ['technical-fast-track.hex', technicalCall], ['note-preimage.hex', notePreimage]]) {
    fs.writeFileSync(path.join(packageRoot, name), value.toHex() + '\n');
    const decoded = registry.createType('Call', value.toU8a());
    assert.equal(decoded.toHex(), value.toHex());
  }
  const params = {
    checkedAt: new Date().toISOString(), endpoint, blockHash, blockNumber: parseInt(header.number, 16),
    genesisHash: genesis, liveRuntime: version, liveCodeSha256: sha(Buffer.from(codeHex.slice(2), 'hex')),
    liveMetadataSha256: sha(metadata), candidateSpecVersion: 137, candidateTransactionVersion: 133,
    candidateSha256: sha(wasm), candidateBlake2_256: blake2AsHex(wasm, 256),
    upgradeCall: { section: 'system', method: 'setCode', codeFile: 'sora-nexus-runtime-137.compact.compressed.wasm', callIndex: call.callIndex.toString(), wasmBytes: wasm.length },
    preimage: { section: 'preimage', method: 'notePreimage', bytesFile: 'runtime.preimage.hex', binaryBytesFile: 'runtime.preimage.call', callHexFile: 'note-preimage.hex', proposalHash: summary.proposal_hash, proposalLen: raw.length, existingStatus: preimageStatus.toJSON(), existingRequestStatus: requestStatus.toJSON() },
    council: { members: council.toJSON(), memberCount: council.length, threshold: councilThreshold,
      thresholdPolicy: 'strict majority, matching the repository helper', length_bound: external.encodedLength,
      proposal: { section: 'democracy', method: 'externalProposeMajority', args: { proposal: { Lookup: { hash: summary.proposal_hash, len: raw.length } } } },
      proposalHash: blake2AsHex(external.toU8a(), 256), callHexFile: 'council-propose.hex' },
    optionalFastTrack: { members: technical.toJSON(), memberCount: technical.length, threshold: technicalThreshold,
      voting_period: votingPeriod, delay: 0, length_bound: fastTrack.encodedLength,
      proposal: { section: 'democracy', method: 'fastTrack', args: { proposal_hash: summary.proposal_hash, voting_period: votingPeriod, delay: 0 } },
      proposalHash: blake2AsHex(fastTrack.toU8a(), 256), callHexFile: 'technical-fast-track.hex',
      timingPolicy: 'Current helper defaults for council review; timing is not a submission or activation instruction.' },
    currentExternalProposal: nextExternal.toJSON(),
    encodingVerifiedAgainstLiveMetadata: true, rpcPolicy: 'Read-only public RPC; provider assertions trusted.',
    signaturesCreated: 0, transactionsSubmitted: 0,
  };
  fs.writeFileSync(path.join(packageRoot, 'governance-params.json'), JSON.stringify(params, null, 2) + '\n');
  write('live-governance.json', params);
  console.log(JSON.stringify({ status: 'passed', blockNumber: params.blockNumber, liveSpec: version.specVersion,
    councilMembers: council.length, councilThreshold, technicalMembers: technical.length, technicalThreshold,
    votingPeriod, proposalHash: summary.proposal_hash, proposalLen: raw.length, nextExternal: params.currentExternalProposal,
    existingPreimage: params.preimage.existingStatus }, null, 2));
})().catch(error => { console.error(error); process.exitCode = 1; }).finally(async () => {
  clearTimeout(timeout);
  if (api) await api.disconnect(); else if (provider) await provider.disconnect();
});
