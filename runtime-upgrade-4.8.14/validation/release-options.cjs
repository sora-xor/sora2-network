'use strict';

const assert = require('node:assert/strict');
const { createHash } = require('node:crypto');
const path = require('node:path');

const MAINNET_GENESIS = '0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5';
const RECORDED_BLOCK = '0x21ca4145bf9ee732a8508f0a99f8c877e5111b130401a2aa5f2d81600b6a1c3d';
const CODE_KEY = '0x3a636f6465';
const POLICY_KEY = '0x' + Buffer.from(':sora-nexus:recovery:policy:v1').toString('hex');
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');

function candidateSpec(input) {
  const packageName = path.basename(path.dirname(__dirname));
  const spec = input ?? { 'runtime-upgrade-4.8.14': '141' }[packageName];
  assert(spec === '140' || spec === '141', '--candidate-spec must be 140 or 141');
  return Number(spec);
}

async function finalizedBlock(chainRpc, block) {
  const finalized = await chainRpc.getFinalizedHead();
  const finalizedHeader = await chainRpc.getHeader(finalized);
  if (block === null) return { hash: finalized.toHex(), number: finalizedHeader.number.toNumber() };
  assert(/^0x[0-9a-f]{64}$/i.test(block), '--block must be a 32-byte block hash');
  const header = await chainRpc.getHeader(block);
  const number = header.number.toNumber();
  assert(number <= finalizedHeader.number.toNumber(), 'Requested block has not finalized');
  const canonical = (await chainRpc.getBlockHash(number)).toHex();
  assert.equal(canonical, block.toLowerCase(), 'Requested block is outside the finalized chain');
  return { hash: canonical, number };
}

// The runtime uses System.BlockHash(0) and the compiled-policy commitment in
// state to admit the production chain. Inspect both at the canonical base.
async function mainnetBaseline(api, block) {
  assert.equal((await api.rpc.chain.getBlockHash(0)).toHex(), MAINNET_GENESIS, 'RPC genesis differs from mainnet');
  const base = await finalizedBlock(api.rpc.chain, block);
  const [at, version, code, policy] = await Promise.all([
    api.at(base.hash), api.rpc.state.getRuntimeVersion(base.hash),
    api.rpc.state.getStorage(CODE_KEY, base.hash), api.rpc.state.getStorage(POLICY_KEY, base.hash),
  ]);
  assert.equal((await at.query.system.blockHash(0)).toHex(), MAINNET_GENESIS, 'Base storage genesis differs from mainnet');
  assert.equal(at.runtimeVersion.specVersion.toNumber(), version.specVersion.toNumber(),
    'Finalized block crosses a runtime upgrade; choose the next finalized block');
  assert(code.isSome && policy.isSome, 'Missing deployed runtime or recovery policy at mainnet base');
  const policyCommitment = '0x' + Buffer.from(policy.unwrap().toU8a(true)).toString('hex');
  assert.match(policyCommitment, /^0x[0-9a-f]{64}$/, 'Invalid mainnet recovery policy commitment');
  return { base, at, version, deployed: { specVersion: version.specVersion.toNumber(),
    codeSha256: sha256(code.unwrap().toU8a(true)), policyCommitment } };
}

module.exports = { MAINNET_GENESIS, RECORDED_BLOCK, CODE_KEY, POLICY_KEY, sha256, candidateSpec, finalizedBlock, mainnetBaseline };
