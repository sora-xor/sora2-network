#!/usr/bin/env node
'use strict';

// Public RPC reads and unsigned call encoding only. This script never signs or submits.
// NODE_PATH=<directory containing @polkadot/api> node validation/prepare-collective-actions.cjs
//   [--collective council|technicalCommittee|all] [--endpoint wss://ws.mof.sora.org]
//   [--out /absolute/output/directory]
// Run again after each motion is finalized on chain, and immediately before closing it.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { blake2AsHex } = require('@polkadot/util-crypto');
const { u8aConcat, u8aToHex } = require('@polkadot/util');

const root = path.resolve(__dirname, '..');

function parseArgs(argv) {
  const args = { collective: 'all' };
  for (let i = 0; i < argv.length; i += 2) {
    const name = argv[i];
    assert(['--collective', '--endpoint', '--out'].includes(name), `Unknown argument: ${name}`);
    assert(argv[i + 1] && !argv[i + 1].startsWith('--'), `Missing value for ${name}`);
    args[name.slice(2)] = argv[i + 1];
  }
  assert(['all', 'council', 'technicalCommittee'].includes(args.collective), 'Invalid collective');
  return args;
}

function encodeActions(api, registry, collective, hash, index, weight, lengthBound) {
  const calls = {
    aye: api.tx[collective].vote(hash, index, true).method,
    close: api.tx[collective].close(hash, index, weight, lengthBound).method
  };
  return Object.fromEntries(Object.entries(calls).map(([name, call]) => {
    const decoded = registry.createType('Call', call.toHex());
    assert.equal(decoded.toHex(), call.toHex());
    assert.equal(decoded.section, collective);
    assert.equal(decoded.method, name === 'aye' ? 'vote' : 'close');
    return [name, { hex: call.toHex(), decoded: decoded.toHuman() }];
  }));
}

async function inspectMotion(api, at, blockHash, blockNumber, collective, expected, expectedMembers) {
  const hash = blake2AsHex(expected.toU8a(), 256);
  const [membersCodec, votingOption, proposalOption] = await Promise.all([
    at.query[collective].members(),
    at.query[collective].voting(hash),
    at.query[collective].proposalOf(hash)
  ]);
  const members = membersCodec.map((value) => value.toString());
  // Membership changes can invalidate a previously selected threshold and require review.
  assert.deepEqual(members, expectedMembers, `${collective} membership changed since packaging; review thresholds`);
  const minimumOriginAyes = collective === 'council'
    ? Math.ceil(members.length / 2)
    : Math.floor(members.length / 2) + 1;
  const unsigned = at.registry.createType('Extrinsic', { method: expected }, { version: api.extrinsicVersion });
  // SORA exposes TransactionPaymentApi.query_info, not TransactionPaymentCallApi.
  // The runtime returns dispatch_info.total_weight(), which safely bounds the
  // proposal's call_weight checked by pallet_collective::validate_and_get_proposal.
  assert(at.call.transactionPaymentApi?.queryInfo, 'TransactionPaymentApi.queryInfo is unavailable');
  // Portable metadata names the argument SpRuntimeUncheckedExtrinsic. Some API
  // versions treat that opaque type as Bytes and add a second length prefix.
  // Encode the runtime ABI directly: one encoded extrinsic followed by u32 len.
  const input = u8aToHex(u8aConcat(unsigned.toU8a(), at.registry.createType('u32', unsigned.encodedLength).toU8a()));
  const rawInfo = await api.rpc.state.call('TransactionPaymentApi_query_info', input, blockHash);
  const info = at.registry.createType(at.call.transactionPaymentApi.queryInfo.meta.type, rawInfo);
  const weight = {
    refTime: info.weight.refTime.toString(),
    proofSize: info.weight.proofSize.toString()
  };
  const report = {
    collective,
    proposalHash: hash,
    proposalCall: expected.toHuman(),
    proposalCallLength: expected.encodedLength,
    memberCount: members.length,
    minimumOriginAyes,
    weightSource: 'TransactionPaymentApi.query_info of unsigned inner proposal at finalized block',
    proposalWeightBound: weight
  };
  if (votingOption.isNone) {
    assert(proposalOption.isNone, `${collective} proposal storage exists without voting storage`);
    return { ...report, status: 'not-proposed-or-already-closed', unsignedCalls: null };
  }
  assert(proposalOption.isSome, `${collective} voting exists without a proposal`);
  const proposal = proposalOption.unwrap();
  assert.equal(proposal.toHex(), expected.toHex(), 'Stored proposal differs from packaged governance call');
  const raw = await api.rpc.state.getStorage(at.query[collective].proposalOf.key(hash), blockHash);
  assert(raw.isSome, 'Proposal raw storage is missing');
  const storedLength = (raw.unwrap().toHex().length - 2) / 2;
  assert(storedLength >= proposal.encodedLength, 'Stored proposal is shorter than its encoded call');
  // SDK collective close documents a possible four-byte storage length overhead.
  // Adding four to the actual raw storage length is a conservative small upper bound.
  const lengthBound = storedLength + 4;
  const votes = votingOption.unwrap();
  const index = votes.index.toNumber();
  const threshold = votes.threshold.toNumber();
  assert(threshold >= minimumOriginAyes && threshold <= members.length,
    `${collective} threshold ${threshold} cannot satisfy this governance origin; review proposal`);
  const ayes = votes.ayes.map((value) => value.toString());
  const nays = votes.nays.map((value) => value.toString());
  const unsignedCalls = encodeActions(api, at.registry, collective, hash, index, weight, lengthBound);
  return {
    ...report,
    status: 'proposed',
    proposalIndex: index,
    threshold,
    ayes,
    nays,
    membersNotYetVotingAye: members.filter((value) => !ayes.includes(value)),
    motionEndBlock: votes.end.toNumber(),
    motionHasEnded: blockNumber >= votes.end.toNumber(),
    enoughExplicitAyesToClose: ayes.length >= threshold,
    storedProposalLength: storedLength,
    closeLengthBound: lengthBound,
    unsignedCalls
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const live = JSON.parse(fs.readFileSync(path.join(__dirname, 'live-chain.json'), 'utf8'));
  const preimage = JSON.parse(fs.readFileSync(path.join(root, 'preimage.json'), 'utf8'));
  const bytes = fs.readFileSync(path.join(root, 'set-code-call.scale'));
  assert.equal(blake2AsHex(bytes, 256), preimage.proposal_hash, 'Packaged preimage hash mismatch');
  assert.equal(bytes.length, preimage.proposal_len, 'Packaged preimage length mismatch');
  const endpoint = args.endpoint || live.endpoint;
  const api = await ApiPromise.create({ provider: new WsProvider(endpoint) });
  try {
    assert.equal(api.genesisHash.toHex(), live.genesisHash, 'Wrong genesis hash');
    const blockHash = await api.rpc.chain.getFinalizedHead();
    const [at, header] = await Promise.all([api.at(blockHash), api.rpc.chain.getHeader(blockHash)]);
    assert.equal(at.runtimeVersion.specVersion.toNumber(), live.runtimeVersion.specVersion,
      'Runtime changed since packaging; regenerate and review governance encodings');
    assert.equal(api.runtimeVersion.specVersion.toNumber(), at.runtimeVersion.specVersion.toNumber(),
      'Best and finalized runtime versions differ; retry after finalization');
    assert.equal(at.consts.democracy.fastTrackVotingPeriod.toNumber(), live.fastTrackVotingPeriod,
      'Fast-track voting period changed');
    const lookup = { Lookup: { hash: preimage.proposal_hash, len: preimage.proposal_len } };
    const expected = {
      council: api.tx.democracy.externalProposeMajority(lookup).method,
      technicalCommittee: api.tx.democracy.fastTrack(preimage.proposal_hash, live.fastTrackVotingPeriod, 0).method
    };
    const filenames = {
      council: 'democracy-external-propose-majority-call.hex',
      technicalCommittee: 'democracy-fast-track-call.hex'
    };
    for (const [collective, call] of Object.entries(expected)) {
      assert.equal(call.toHex(), fs.readFileSync(path.join(root, filenames[collective]), 'utf8').trim(),
        `${collective} encoding differs from packaged call`);
      assert.equal(at.registry.createType('Call', call.toHex()).toHex(), call.toHex());
    }
    const selected = args.collective === 'all' ? Object.keys(expected) : [args.collective];
    const motions = await Promise.all(selected.map((collective) => inspectMotion(
      api, at, blockHash, header.number.toNumber(), collective, expected[collective],
      collective === 'council' ? live.councilMembers : live.technicalCommitteeMembers
    )));
    const nextExternal = await at.query.democracy.nextExternal();
    const report = {
      checkedAt: new Date().toISOString(), endpoint,
      genesisHash: api.genesisHash.toHex(), blockHash: blockHash.toHex(),
      blockNumber: header.number.toNumber(), specVersion: at.runtimeVersion.specVersion.toNumber(),
      setCodeProposalHash: preimage.proposal_hash,
      nextExternal: nextExternal.toJSON(), motions, submittedTransactions: 0,
      instructions: [
        'Hashes above identify the collective inner calls; they are distinct from the setCode preimage hash.',
        'A proposed motion starts with no aye votes. The proposer must vote explicitly too.',
        'Only collective members can submit aye votes; any signed account can close a motion.',
        'Submit close only after enough explicit ayes; otherwise it may fail TooEarly or reject after expiry.',
        'Check collective Executed.result is Ok; outer extrinsic success alone does not prove inner success.',
        'Close council successfully and verify Democracy.NextExternal before executing technical fastTrack.',
        'After successful technical close, use Democracy.Started to obtain the referendum index and vote in the referendum.',
        'Regenerate from fresh finalized state before use; this script neither signs nor submits.'
      ]
    };
    if (args.out) {
      const out = path.resolve(args.out);
      fs.mkdirSync(out, { recursive: true });
      // Every run gets a fresh subdirectory so old index-specific call files are never reused.
      const destination = fs.mkdtempSync(path.join(out, `collective-actions-${report.blockNumber}-`));
      for (const motion of motions) {
        for (const [action, call] of Object.entries(motion.unsignedCalls || {})) {
          fs.writeFileSync(path.join(destination, `${motion.collective}-${action}.hex`), `${call.hex}\n`);
        }
      }
      report.outputDirectory = destination;
      fs.writeFileSync(path.join(destination, 'collective-actions.json'), `${JSON.stringify(report, null, 2)}\n`);
    }
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  } finally {
    await api.disconnect();
  }
}

module.exports = { encodeActions, inspectMotion, parseArgs };
if (require.main === module) {
  if (process.argv.slice(2).includes('--help')) {
    console.log('Usage: node validation/prepare-collective-actions.cjs [--collective council|technicalCommittee|all] [--endpoint wss://ws.mof.sora.org] [--out DIRECTORY]\nReads finalized public state and generates unsigned aye/close call hex after the matching motion exists. Never signs or submits. Without --out, prints JSON only.');
    process.exit(0);
  }
  const timer = setTimeout(() => { console.error('Read-only governance inspection timed out'); process.exit(2); }, 60000);
  main().then(() => clearTimeout(timer)).catch((error) => {
    clearTimeout(timer);
    console.error(error);
    process.exitCode = 1;
  });
}
