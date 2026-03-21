import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';

import { ethers } from 'ethers';

const repoRoot = resolve(import.meta.dirname, '..');
const scriptPath = resolve(repoRoot, 'scripts', 'encode_ton_burn_proof_to_sora.mjs');
const artifactPath = resolve(repoRoot, 'artifacts', 'sccp-jetton-master.compiled.json');
const SCCP_BURN_PREFIX = Buffer.from('sccp:burn:v1', 'utf8');

function runEncoder(args) {
  const res = spawnSync(process.execPath, [scriptPath, ...args], {
    cwd: repoRoot,
    encoding: 'utf8',
  });
  return res;
}

function toFixedBeBytes(value, width) {
  const bigint = BigInt(value);
  return Buffer.from(bigint.toString(16).padStart(width * 2, '0'), 'hex');
}

function toFixedLeBytes(value, width) {
  return Buffer.from(toFixedBeBytes(value, width)).reverse();
}

function burnMessageId(fields) {
  const payload = Buffer.concat([
    Buffer.from([1]),
    toFixedLeBytes(fields.sourceDomain, 4),
    toFixedLeBytes(fields.destDomain, 4),
    toFixedLeBytes(fields.nonce, 8),
    Buffer.from(fields.soraAssetId.slice(2), 'hex'),
    toFixedLeBytes(fields.amount, 16),
    Buffer.from(fields.recipient.slice(2), 'hex'),
  ]);
  return Buffer.from(ethers.getBytes(ethers.keccak256(Buffer.concat([SCCP_BURN_PREFIX, payload]))));
}

function decodeCompactU32(buffer, offset) {
  const b0 = buffer[offset];
  const mode = b0 & 0x03;
  if (mode === 0) {
    return { value: b0 >> 2, used: 1 };
  }
  if (mode === 1) {
    const raw = buffer.readUInt16LE(offset);
    return { value: raw >> 2, used: 2 };
  }
  if (mode === 2) {
    const raw = buffer.readUInt32LE(offset);
    return { value: raw >>> 2, used: 4 };
  }
  throw new Error('big-integer compact encoding not expected in test fixture');
}

function readScaleBytes(buffer, offset) {
  const { value, used } = decodeCompactU32(buffer, offset);
  const start = offset + used;
  const end = start + value;
  return {
    bytes: buffer.subarray(start, end),
    next: end,
  };
}

function decodeMasterchainSection(buffer) {
  let offset = 0;
  const version = buffer[offset];
  offset += 1;
  const checkpointBlockBoc = readScaleBytes(buffer, offset);
  offset = checkpointBlockBoc.next;
  const checkpointStateExtraProofBoc = readScaleBytes(buffer, offset);
  offset = checkpointStateExtraProofBoc.next;
  const targetBlockProofBoc = readScaleBytes(buffer, offset);
  offset = targetBlockProofBoc.next;
  const targetStateExtraProofBoc = readScaleBytes(buffer, offset);
  offset = targetStateExtraProofBoc.next;
  return {
    version,
    checkpointBlockBoc: checkpointBlockBoc.bytes,
    checkpointStateExtraProofBoc: checkpointStateExtraProofBoc.bytes,
    targetBlockProofBoc: targetBlockProofBoc.bytes,
    targetStateExtraProofBoc: targetStateExtraProofBoc.bytes,
    next: offset,
  };
}

function decodeShardSection(buffer) {
  let offset = 0;
  const version = buffer[offset];
  offset += 1;
  const shardBlockBoc = readScaleBytes(buffer, offset);
  offset = shardBlockBoc.next;
  const shardStateAccountsProofBoc = readScaleBytes(buffer, offset);
  offset = shardStateAccountsProofBoc.next;
  return {
    version,
    shardBlockBoc: shardBlockBoc.bytes,
    shardStateAccountsProofBoc: shardStateAccountsProofBoc.bytes,
    next: offset,
  };
}

test('encodes TON burn proof bytes for SORA pallet consumption', () => {
  const res = runEncoder([
    '--jetton-master-account-id',
    `0x${'11'.repeat(32)}`,
    '--master-code-hash',
    `0x${'22'.repeat(32)}`,
    '--sora-asset-id',
    `0x${'33'.repeat(32)}`,
    '--recipient',
    `0x${'44'.repeat(32)}`,
    '--amount',
    '12345',
    '--nonce',
    '77',
    '--checkpoint-seqno',
    '10',
    '--checkpoint-hash',
    `0x${'55'.repeat(32)}`,
    '--target-seqno',
    '11',
    '--target-hash',
    `0x${'66'.repeat(32)}`,
    '--checkpoint-block-boc',
    '0xdeadbeef',
    '--checkpoint-state-extra-proof',
    '0xa1a2',
    '--target-block-proof-boc',
    '0xb1b2b3',
    '--target-state-extra-proof',
    '0xc1',
    '--shard-block-boc',
    Buffer.from('shard-block').toString('base64'),
    '--shard-state-accounts-proof',
    '0xd1d2',
    '--account-proof',
    '0x0102',
    '--burns-dict-proof',
    '0x030405',
  ]);

  assert.equal(res.status, 0, res.stderr);
  const out = JSON.parse(res.stdout);
  assert.equal(out.trusted_checkpoint_seqno, 10);
  assert.equal(out.target_mc_seqno, 11);

  const expectedMessageId = burnMessageId({
    sourceDomain: 4n,
    destDomain: 0n,
    nonce: 77n,
    soraAssetId: `0x${'33'.repeat(32)}`,
    amount: 12345n,
    recipient: `0x${'44'.repeat(32)}`,
  });
  assert.equal(out.message_id, `0x${expectedMessageId.toString('hex')}`);

  const proof = Buffer.from(out.proof_scale_hex.slice(2), 'hex');
  let offset = 0;
  assert.equal(proof[offset], 1);
  offset += 1;
  assert.equal(proof.readUInt32LE(offset), 10);
  offset += 4;
  assert.equal(proof.subarray(offset, offset + 32).toString('hex'), '55'.repeat(32));
  offset += 32;
  assert.equal(proof.readUInt32LE(offset), 11);
  offset += 4;
  assert.equal(proof.subarray(offset, offset + 32).toString('hex'), '66'.repeat(32));
  offset += 32;
  assert.equal(proof.subarray(offset, offset + 32).toString('hex'), '11'.repeat(32));
  offset += 32;
  assert.equal(proof.subarray(offset, offset + 32).toString('hex'), '22'.repeat(32));
  offset += 32;
  assert.equal(proof.subarray(offset, offset + 32).toString('hex'), expectedMessageId.toString('hex'));
  offset += 32;
  assert.equal(proof.readUInt32LE(offset), 0);
  offset += 4;
  assert.equal(proof.subarray(offset, offset + 32).toString('hex'), '44'.repeat(32));
  offset += 32;
  assert.equal(proof.subarray(offset, offset + 16).toString('hex'), toFixedLeBytes(12345n, 16).toString('hex'));
  offset += 16;
  assert.equal(proof.subarray(offset, offset + 8).toString('hex'), toFixedLeBytes(77n, 8).toString('hex'));
  offset += 8;

  let section = readScaleBytes(proof, offset);
  let masterchainSection = decodeMasterchainSection(section.bytes);
  assert.equal(masterchainSection.version, 1);
  assert.equal(masterchainSection.checkpointBlockBoc.toString('hex'), 'deadbeef');
  assert.equal(masterchainSection.checkpointStateExtraProofBoc.toString('hex'), 'a1a2');
  assert.equal(masterchainSection.targetBlockProofBoc.toString('hex'), 'b1b2b3');
  assert.equal(masterchainSection.targetStateExtraProofBoc.toString('hex'), 'c1');
  assert.equal(masterchainSection.next, section.bytes.length);
  offset = section.next;

  section = readScaleBytes(proof, offset);
  const shardSection = decodeShardSection(section.bytes);
  assert.equal(shardSection.version, 1);
  assert.equal(shardSection.shardBlockBoc.toString(), 'shard-block');
  assert.equal(shardSection.shardStateAccountsProofBoc.toString('hex'), 'd1d2');
  assert.equal(shardSection.next, section.bytes.length);
  offset = section.next;

  section = readScaleBytes(proof, offset);
  assert.equal(section.bytes.toString('hex'), '0102');
  offset = section.next;

  section = readScaleBytes(proof, offset);
  assert.equal(section.bytes.toString('hex'), '030405');
  offset = section.next;

  assert.equal(offset, proof.length);
});

test('uses the local master artifact code hash by default', () => {
  const artifact = JSON.parse(readFileSync(artifactPath, 'utf8'));
  const res = runEncoder([
    '--jetton-master-account-id',
    `0x${'11'.repeat(32)}`,
    '--sora-asset-id',
    `0x${'33'.repeat(32)}`,
    '--recipient',
    `0x${'44'.repeat(32)}`,
    '--amount',
    '1',
    '--nonce',
    '1',
    '--checkpoint-seqno',
    '10',
    '--checkpoint-hash',
    `0x${'55'.repeat(32)}`,
    '--target-seqno',
    '10',
    '--target-hash',
    `0x${'55'.repeat(32)}`,
    '--masterchain-proof',
    '0x01',
    '--shard-proof',
    '0x02',
    '--account-proof',
    '0x03',
    '--burns-dict-proof',
    '0x04',
  ]);

  assert.equal(res.status, 0, res.stderr);
  const out = JSON.parse(res.stdout);
  const expected = artifact.codeHashHex.startsWith('0x')
    ? artifact.codeHashHex.toLowerCase()
    : `0x${artifact.codeHashHex.toLowerCase()}`;
  assert.equal(out.jetton_master_code_hash_hex, expected);
});

test('accepts raw proof sections from files and rejects missing checkpoint source', () => {
  const dir = mkdtempSync(join(tmpdir(), 'sccp-ton-proof-'));
  const shardPath = join(dir, 'shard.bin');
  writeFileSync(shardPath, Buffer.from([0xaa, 0xbb, 0xcc]));

  const ok = runEncoder([
    '--jetton-master-account-id',
    `0x${'11'.repeat(32)}`,
    '--master-code-hash',
    `0x${'22'.repeat(32)}`,
    '--sora-asset-id',
    `0x${'33'.repeat(32)}`,
    '--recipient',
    `0x${'44'.repeat(32)}`,
    '--amount',
    '1',
    '--nonce',
    '1',
    '--checkpoint-seqno',
    '10',
    '--checkpoint-hash',
    `0x${'55'.repeat(32)}`,
    '--target-seqno',
    '10',
    '--target-hash',
    `0x${'55'.repeat(32)}`,
    '--masterchain-proof',
    '0x01',
    '--shard-proof',
    `@${shardPath}`,
    '--account-proof',
    '0x03',
    '--burns-dict-proof',
    '0x04',
  ]);
  assert.equal(ok.status, 0, ok.stderr);
  const out = JSON.parse(ok.stdout);
  assert.equal(out.proof_sections.shard_proof_bytes, 3);

  const bad = runEncoder([
    '--jetton-master-account-id',
    `0x${'11'.repeat(32)}`,
    '--master-code-hash',
    `0x${'22'.repeat(32)}`,
    '--sora-asset-id',
    `0x${'33'.repeat(32)}`,
    '--recipient',
    `0x${'44'.repeat(32)}`,
    '--amount',
    '1',
    '--nonce',
    '1',
    '--masterchain-proof',
    '0x01',
    '--shard-proof',
    '0x02',
    '--account-proof',
    '0x03',
    '--burns-dict-proof',
    '0x04',
  ]);
  assert.notEqual(bad.status, 0);
  assert.match(
    bad.stderr,
    /either provide checkpoint\/target seqno\+hash explicitly or supply --ton-api/,
  );
});
