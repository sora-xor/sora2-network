'use strict';

const assert = require('node:assert/strict');
const path = require('node:path');

const RECORDED_BLOCK = '0x21ca4145bf9ee732a8508f0a99f8c877e5111b130401a2aa5f2d81600b6a1c3d';

function candidateSpec(input) {
  const packageName = path.basename(path.dirname(__dirname));
  const spec = input ?? { 'runtime-upgrade-4.8.14': '140', 'runtime-upgrade-4.8.15': '141' }[packageName];
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

module.exports = { RECORDED_BLOCK, candidateSpec, finalizedBlock };
