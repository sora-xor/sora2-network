#!/usr/bin/env node
import fs from 'node:fs';
import { beginCell } from '@ton/core';

function parseBigIntLike(v, label = 'value') {
  if (typeof v === 'bigint') return v;
  if (typeof v === 'number') {
    if (!Number.isInteger(v)) throw new Error(`${label} must be an integer`);
    if (!Number.isSafeInteger(v)) {
      throw new Error(`${label} must be encoded as a string once above Number.MAX_SAFE_INTEGER`);
    }
    return BigInt(v);
  }
  if (typeof v !== 'string') throw new Error(`cannot parse bigint from ${typeof v}`);
  if (v.startsWith('0x') || v.startsWith('0X')) return BigInt(v);
  return BigInt(v);
}

function parseUintLike(v, bits, label) {
  const n = parseBigIntLike(v, label);
  if (n < 0n) {
    throw new Error(`${label} must be non-negative`);
  }
  const max = (1n << BigInt(bits)) - 1n;
  if (n > max) {
    throw new Error(`${label} exceeds ${bits} bits`);
  }
  return n;
}

function normalizeHex(v, label) {
  if (typeof v !== 'string') throw new Error(`${label} must be a hex string, got ${typeof v}`);
  const raw = v.startsWith('0x') || v.startsWith('0X') ? v.slice(2) : v;
  if (raw.length % 2 !== 0) {
    throw new Error(`${label} must have an even number of hex digits`);
  }
  if (!/^[0-9a-fA-F]*$/.test(raw)) {
    throw new Error(`${label} must contain only hex digits`);
  }
  return raw;
}

function hexToBuffer(v, expectedLen, label = 'value') {
  const raw = normalizeHex(v, label);
  const buf = Buffer.from(raw, 'hex');
  if (expectedLen !== undefined && buf.length !== expectedLen) {
    throw new Error(`${label} must be exactly ${expectedLen} bytes`);
  }
  return buf;
}

function hexToU256(v, label) {
  const buf = hexToBuffer(v, 32, label);
  return BigInt(`0x${buf.toString('hex')}`);
}

function buildItemsTail(items, startIndex) {
  const b = beginCell();
  let i = startIndex;
  while (i < items.length) {
    if (b.availableBits < 256) {
      b.storeRef(buildItemsTail(items, i));
      return b.endCell();
    }
    b.storeUint(items[i], 256);
    i += 1;
  }
  return b.endCell();
}

function buildItemsRef(itemsU256) {
  if (itemsU256.length > 0xffff) {
    throw new Error(`too many proof items: ${itemsU256.length}`);
  }
  const b = beginCell();
  b.storeUint(itemsU256.length, 16);
  let i = 0;
  while (i < itemsU256.length) {
    if (b.availableBits < 256) {
      b.storeRef(buildItemsTail(itemsU256, i));
      return b.endCell();
    }
    b.storeUint(itemsU256[i], 256);
    i += 1;
  }
  return b.endCell();
}

function buildProofCell(data) {
  const mmrProof = data.mmr_proof ?? data.proof;
  const leaf = data.mmr_leaf ?? data.latest_mmr_leaf ?? data.leaf;
  const digestScaleHex = data.digest_scale;
  if (!mmrProof) throw new Error('missing mmr_proof/proof');
  if (!leaf) throw new Error('missing mmr_leaf/latest_mmr_leaf/leaf');
  if (!digestScaleHex) throw new Error('missing digest_scale');

  const leafIndex = parseUintLike(mmrProof.leaf_index, 64, 'mmr_proof.leaf_index');
  const leafCount = parseUintLike(mmrProof.leaf_count, 64, 'mmr_proof.leaf_count');
  const itemsU256 = (mmrProof.items ?? []).map((item, index) => hexToU256(item, `mmr_proof.items[${index}]`));

  const version = parseUintLike(leaf.version, 8, 'mmr_leaf.version');
  const parentNumber = parseUintLike(leaf.parent_number, 32, 'mmr_leaf.parent_number');
  const parentHash = hexToU256(leaf.parent_hash, 'mmr_leaf.parent_hash');
  const nextSetId = parseUintLike(leaf.next_authority_set_id, 64, 'mmr_leaf.next_authority_set_id');
  const nextSetLen = parseUintLike(leaf.next_authority_set_len, 32, 'mmr_leaf.next_authority_set_len');
  const nextSetRoot = hexToU256(leaf.next_authority_set_root, 'mmr_leaf.next_authority_set_root');
  const randomSeed = hexToU256(leaf.random_seed, 'mmr_leaf.random_seed');

  const digestScale = hexToBuffer(digestScaleHex, undefined, 'digest_scale');
  if (digestScale.length > 127) {
    throw new Error('digest_scale exceeds the verifier single-cell limit of 127 bytes');
  }
  const digestRef = beginCell().storeBuffer(digestScale).endCell();

  const leafRef = beginCell()
    .storeUint(version, 8)
    .storeUint(parentNumber, 32)
    .storeUint(parentHash, 256)
    .storeUint(nextSetId, 64)
    .storeUint(nextSetLen, 32)
    .storeUint(nextSetRoot, 256)
    .storeUint(randomSeed, 256)
    .storeRef(digestRef)
    .endCell();

  const itemsRef = buildItemsRef(itemsU256);
  return beginCell()
    .storeUint(leafIndex, 64)
    .storeUint(leafCount, 64)
    .storeRef(itemsRef)
    .storeRef(leafRef)
    .endCell();
}

function requireFlagValue(argv, index, flag) {
  const value = argv[index + 1];
  if (value === undefined || value.startsWith('--')) {
    throw new Error(`missing value for ${flag}`);
  }
  return value;
}

function parseArgs(argv) {
  const out = { format: 'both' };
  for (let i = 2; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '--input') out.input = requireFlagValue(argv, i++, '--input');
    else if (a === '--output') out.output = requireFlagValue(argv, i++, '--output');
    else if (a === '--format') out.format = requireFlagValue(argv, i++, '--format');
    else throw new Error(`unknown arg: ${a}`);
  }
  if (!out.input) throw new Error('missing --input <sccp-proof-json>');
  if (!['hex', 'base64', 'both'].includes(out.format)) {
    throw new Error(`invalid --format: ${out.format}`);
  }
  return out;
}

function main() {
  const args = parseArgs(process.argv);
  const data = JSON.parse(fs.readFileSync(args.input, 'utf8'));
  const proofCell = buildProofCell(data);
  const boc = proofCell.toBoc({ idx: false });
  const hex = `0x${boc.toString('hex')}`;
  const b64 = boc.toString('base64');

  if (args.output) {
    fs.writeFileSync(args.output, boc);
  }

  if (args.format === 'hex' || args.format === 'both') {
    console.log(`boc_hex=${hex}`);
  }
  if (args.format === 'base64' || args.format === 'both') {
    console.log(`boc_base64=${b64}`);
  }
}

try {
  main();
} catch (e) {
  console.error(`error: ${e.message}`);
  process.exit(1);
}
