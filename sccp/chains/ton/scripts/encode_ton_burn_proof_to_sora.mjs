#!/usr/bin/env node
import { readFileSync } from 'node:fs';
import { Buffer } from 'node:buffer';
import { resolve } from 'node:path';

import { Address } from '@ton/core';
import { ethers } from 'ethers';

const repoRoot = resolve(import.meta.dirname, '..');
const SCCP_BURN_PREFIX = Buffer.from('sccp:burn:v1', 'utf8');
const DEFAULT_SOURCE_DOMAIN = 4;
const DEFAULT_DEST_DOMAIN = 0;

function usageAndExit(code) {
  // eslint-disable-next-line no-console
  console.error(
    [
      'Usage:',
      '  node scripts/encode_ton_burn_proof_to_sora.mjs \\',
      '    --jetton-master <ton_addr> \\',
      '    --sora-asset-id 0x<32-byte> \\',
      '    --recipient 0x<32-byte> \\',
      '    --amount <u128> \\',
      '    --nonce <u64> \\',
      '    --checkpoint-seqno <u32> --checkpoint-hash 0x<32-byte> \\',
      '    --target-seqno <u32> --target-hash 0x<32-byte> \\',
      '    --checkpoint-block-boc <0xhex|base64|@file> \\',
      '    --checkpoint-state-extra-proof <0xhex|base64|@file> \\',
      '    --target-block-proof-boc <0xhex|base64|@file> \\',
      '    --target-state-extra-proof <0xhex|base64|@file> \\',
      '    --shard-block-boc <0xhex|base64|@file> \\',
      '    --shard-state-accounts-proof <0xhex|base64|@file> \\',
      '    --account-proof <0xhex|base64|@file> \\',
      '    --burns-dict-proof <0xhex|base64|@file>',
      '',
      'Optional:',
      '  --ton-api <json-rpc-url>       Fetch latest masterchain head via getMasterchainInfo.',
      '  --jetton-master-account-id 0x<32-byte>',
      '  --master-code-hash 0x<32-byte> Defaults to local sccp-jetton-master artifact code hash.',
      '  --message-id 0x<32-byte>       Override canonical SCCP messageId.',
      '  --source-domain <u32>          Defaults to 4 (TON).',
      '  --dest-domain <u32>            Defaults to 0 (SORA).',
      '  --masterchain-proof <...>      Optional pre-encoded SCALE masterchain section.',
      '  --shard-proof <...>            Optional pre-encoded SCALE shard section.',
      '',
      'Proof byte inputs accept:',
      '  - 0x-prefixed hex',
      '  - base64',
      '  - @/path/to/file (raw bytes)',
    ].join('\n'),
  );
  process.exit(code);
}

function loadArtifact(name) {
  return JSON.parse(readFileSync(resolve(repoRoot, 'artifacts', name), 'utf8'));
}

function parseArgs(argv) {
  const valueFlags = new Set([
    'jetton-master',
    'jetton-master-account-id',
    'master-code-hash',
    'sora-asset-id',
    'recipient',
    'amount',
    'nonce',
    'checkpoint-seqno',
    'checkpoint-hash',
    'target-seqno',
    'target-hash',
    'checkpoint-block-boc',
    'checkpoint-state-extra-proof',
    'target-block-proof-boc',
    'target-state-extra-proof',
    'shard-block-boc',
    'shard-state-accounts-proof',
    'masterchain-proof',
    'shard-proof',
    'account-proof',
    'burns-dict-proof',
    'message-id',
    'ton-api',
    'source-domain',
    'dest-domain',
  ]);
  const out = {};
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--help' || arg === '-h') {
      usageAndExit(0);
    }
    if (!arg.startsWith('--')) {
      throw new Error(`unexpected positional argument: ${arg}`);
    }
    const key = arg.slice(2);
    if (!valueFlags.has(key)) {
      throw new Error(`unknown argument: ${arg}`);
    }
    const value = argv[i + 1];
    if (value === undefined || value.startsWith('--')) {
      throw new Error(`missing value for ${arg}`);
    }
    out[key] = value;
    i += 1;
  }
  return out;
}

function normalizeHex(value, label) {
  if (typeof value !== 'string' || !value.startsWith('0x')) {
    throw new Error(`${label} must be 0x-prefixed hex`);
  }
  const raw = value.slice(2);
  if (raw.length % 2 !== 0) {
    throw new Error(`${label} must have an even number of hex digits`);
  }
  if (!/^[0-9a-fA-F]*$/.test(raw)) {
    throw new Error(`${label} must contain only hex digits`);
  }
  return raw.toLowerCase();
}

function parseFixedHex(value, expectedLen, label) {
  const raw = normalizeHex(value, label);
  const out = Buffer.from(raw, 'hex');
  if (out.length !== expectedLen) {
    throw new Error(`${label} must be exactly ${expectedLen} bytes`);
  }
  return out;
}

function parseBigIntLike(value, label) {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${label} must be provided`);
  }
  if (value.startsWith('0x') || value.startsWith('0X')) {
    return BigInt(value);
  }
  return BigInt(value);
}

function parseUint(value, bits, label) {
  const out = parseBigIntLike(value, label);
  if (out < 0n) {
    throw new Error(`${label} must be non-negative`);
  }
  const max = (1n << BigInt(bits)) - 1n;
  if (out > max) {
    throw new Error(`${label} exceeds ${bits} bits`);
  }
  return out;
}

function toFixedBeBytes(value, width, label) {
  const bigint = typeof value === 'bigint' ? value : BigInt(value);
  const max = (1n << BigInt(width * 8)) - 1n;
  if (bigint < 0n || bigint > max) {
    throw new Error(`${label} exceeds ${width * 8} bits`);
  }
  return Buffer.from(bigint.toString(16).padStart(width * 2, '0'), 'hex');
}

function toFixedLeBytes(value, width, label) {
  return Buffer.from(toFixedBeBytes(value, width, label)).reverse();
}

function encodeCompactU32(value) {
  if (!Number.isInteger(value) || value < 0) {
    throw new Error('compact length must be a non-negative integer');
  }
  if (value < 1 << 6) {
    return Buffer.from([(value << 2) | 0]);
  }
  if (value < 1 << 14) {
    const v = (value << 2) | 1;
    return Buffer.from([v & 0xff, (v >> 8) & 0xff]);
  }
  if (value < 1 << 30) {
    const v = (value << 2) | 2;
    return Buffer.from([v & 0xff, (v >> 8) & 0xff, (v >> 16) & 0xff, (v >> 24) & 0xff]);
  }
  throw new Error('compact length too large');
}

function encodeScaleBytes(bytes) {
  return Buffer.concat([encodeCompactU32(bytes.length), bytes]);
}

function encodeMasterchainProofSection({
  checkpointBlockBoc,
  checkpointStateExtraProofBoc,
  targetBlockProofBoc,
  targetStateExtraProofBoc,
}) {
  return Buffer.concat([
    Buffer.from([1]),
    encodeScaleBytes(checkpointBlockBoc),
    encodeScaleBytes(checkpointStateExtraProofBoc),
    encodeScaleBytes(targetBlockProofBoc),
    encodeScaleBytes(targetStateExtraProofBoc),
  ]);
}

function encodeShardProofSection({
  shardBlockBoc,
  shardStateAccountsProofBoc,
}) {
  return Buffer.concat([
    Buffer.from([1]),
    encodeScaleBytes(shardBlockBoc),
    encodeScaleBytes(shardStateAccountsProofBoc),
  ]);
}

function parseProofBytesInput(value, label) {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${label} must be provided`);
  }
  if (value.startsWith('@')) {
    return readFileSync(value.slice(1));
  }
  if (value.startsWith('0x')) {
    return Buffer.from(normalizeHex(value, label), 'hex');
  }
  try {
    const out = Buffer.from(value, 'base64');
    if (out.length === 0 && value !== '') {
      throw new Error('empty base64 decode');
    }
    return out;
  } catch (error) {
    throw new Error(`${label} must be 0x hex, base64, or @file`);
  }
}

function resolveMasterchainProofSection(args) {
  if (args['masterchain-proof']) {
    return parseProofBytesInput(args['masterchain-proof'], 'masterchain-proof');
  }
  return encodeMasterchainProofSection({
    checkpointBlockBoc: parseProofBytesInput(args['checkpoint-block-boc'], 'checkpoint-block-boc'),
    checkpointStateExtraProofBoc: parseProofBytesInput(
      args['checkpoint-state-extra-proof'],
      'checkpoint-state-extra-proof',
    ),
    targetBlockProofBoc: parseProofBytesInput(args['target-block-proof-boc'], 'target-block-proof-boc'),
    targetStateExtraProofBoc: parseProofBytesInput(
      args['target-state-extra-proof'],
      'target-state-extra-proof',
    ),
  });
}

function resolveShardProofSection(args) {
  if (args['shard-proof']) {
    return parseProofBytesInput(args['shard-proof'], 'shard-proof');
  }
  return encodeShardProofSection({
    shardBlockBoc: parseProofBytesInput(args['shard-block-boc'], 'shard-block-boc'),
    shardStateAccountsProofBoc: parseProofBytesInput(
      args['shard-state-accounts-proof'],
      'shard-state-accounts-proof',
    ),
  });
}

function burnPayloadToBytes({
  sourceDomain,
  destDomain,
  nonce,
  soraAssetId,
  amount,
  recipient,
}) {
  return Buffer.concat([
    Buffer.from([1]),
    toFixedLeBytes(sourceDomain, 4, 'sourceDomain'),
    toFixedLeBytes(destDomain, 4, 'destDomain'),
    toFixedLeBytes(nonce, 8, 'nonce'),
    soraAssetId,
    toFixedLeBytes(amount, 16, 'amount'),
    recipient,
  ]);
}

function computeMessageId(fields) {
  const payload = burnPayloadToBytes(fields);
  return Buffer.from(ethers.getBytes(ethers.keccak256(Buffer.concat([SCCP_BURN_PREFIX, payload]))));
}

async function jsonRpc(endpoint, method, params) {
  const res = await fetch(endpoint, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 1,
      method,
      params,
    }),
  });
  if (!res.ok) {
    throw new Error(`TON RPC ${method} failed with HTTP ${res.status}`);
  }
  const body = await res.json();
  if (body.error) {
    throw new Error(`TON RPC ${method} error: ${JSON.stringify(body.error)}`);
  }
  return body.result;
}

function parseMasterchainInfo(result) {
  const last = result?.last ?? result;
  const seqno = last?.seqno;
  const rootHash = last?.root_hash ?? last?.rootHash;
  if (!Number.isInteger(seqno)) {
    throw new Error('getMasterchainInfo result missing integer last.seqno');
  }
  if (typeof rootHash !== 'string') {
    throw new Error('getMasterchainInfo result missing last.root_hash');
  }
  return {
    seqno,
    rootHash: parseFixedHex(rootHash.startsWith('0x') ? rootHash : `0x${rootHash}`, 32, 'last.root_hash'),
  };
}

async function resolveMasterchainCheckpoint(args) {
  const checkpointSeqno = args['checkpoint-seqno'];
  const checkpointHash = args['checkpoint-hash'];
  const targetSeqno = args['target-seqno'];
  const targetHash = args['target-hash'];

  if (checkpointSeqno && checkpointHash && targetSeqno && targetHash) {
    return {
      checkpointSeqno: Number(parseUint(checkpointSeqno, 32, 'checkpoint-seqno')),
      checkpointHash: parseFixedHex(checkpointHash, 32, 'checkpoint-hash'),
      targetSeqno: Number(parseUint(targetSeqno, 32, 'target-seqno')),
      targetHash: parseFixedHex(targetHash, 32, 'target-hash'),
    };
  }

  const endpoint = args['ton-api'];
  if (!endpoint) {
    throw new Error(
      'either provide checkpoint/target seqno+hash explicitly or supply --ton-api for getMasterchainInfo',
    );
  }

  const head = parseMasterchainInfo(await jsonRpc(endpoint, 'getMasterchainInfo', []));
  return {
    checkpointSeqno: checkpointSeqno ? Number(parseUint(checkpointSeqno, 32, 'checkpoint-seqno')) : head.seqno,
    checkpointHash: checkpointHash ? parseFixedHex(checkpointHash, 32, 'checkpoint-hash') : head.rootHash,
    targetSeqno: targetSeqno ? Number(parseUint(targetSeqno, 32, 'target-seqno')) : head.seqno,
    targetHash: targetHash ? parseFixedHex(targetHash, 32, 'target-hash') : head.rootHash,
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const sourceDomain = Number(parseUint(args['source-domain'] ?? `${DEFAULT_SOURCE_DOMAIN}`, 32, 'source-domain'));
  const destDomain = Number(parseUint(args['dest-domain'] ?? `${DEFAULT_DEST_DOMAIN}`, 32, 'dest-domain'));
  const nonce = parseUint(args.nonce, 64, 'nonce');
  const amount = parseUint(args.amount, 128, 'amount');
  const soraAssetId = parseFixedHex(args['sora-asset-id'], 32, 'sora-asset-id');
  const recipient = parseFixedHex(args.recipient, 32, 'recipient');

  let jettonMasterAccountId;
  if (args['jetton-master-account-id']) {
    jettonMasterAccountId = parseFixedHex(
      args['jetton-master-account-id'],
      32,
      'jetton-master-account-id',
    );
  } else if (args['jetton-master']) {
    const address = Address.parse(args['jetton-master']);
    jettonMasterAccountId = Buffer.from(address.hash);
  } else {
    throw new Error('either --jetton-master or --jetton-master-account-id is required');
  }

  const masterCodeHash = args['master-code-hash']
    ? parseFixedHex(args['master-code-hash'], 32, 'master-code-hash')
    : (() => {
        const raw = loadArtifact('sccp-jetton-master.compiled.json').codeHashHex;
        const normalized = raw.startsWith('0x') ? raw : `0x${raw}`;
        return parseFixedHex(normalized, 32, 'artifact master code hash');
      })();

  const {
    checkpointSeqno,
    checkpointHash,
    targetSeqno,
    targetHash,
  } = await resolveMasterchainCheckpoint(args);

  const messageId = args['message-id']
    ? parseFixedHex(args['message-id'], 32, 'message-id')
    : computeMessageId({
        sourceDomain,
        destDomain,
        nonce,
        soraAssetId,
        amount,
        recipient,
      });

  const masterchainProof = resolveMasterchainProofSection(args);
  const shardProof = resolveShardProofSection(args);
  const accountProof = parseProofBytesInput(args['account-proof'], 'account-proof');
  const burnsDictProof = parseProofBytesInput(args['burns-dict-proof'], 'burns-dict-proof');

  const proofBytes = Buffer.concat([
    Buffer.from([1]), // version
    toFixedLeBytes(BigInt(checkpointSeqno), 4, 'checkpoint-seqno'),
    checkpointHash,
    toFixedLeBytes(BigInt(targetSeqno), 4, 'target-seqno'),
    targetHash,
    jettonMasterAccountId,
    masterCodeHash,
    messageId,
    toFixedLeBytes(BigInt(destDomain), 4, 'dest-domain'),
    recipient,
    toFixedLeBytes(amount, 16, 'amount'),
    toFixedLeBytes(nonce, 8, 'nonce'),
    encodeScaleBytes(masterchainProof),
    encodeScaleBytes(shardProof),
    encodeScaleBytes(accountProof),
    encodeScaleBytes(burnsDictProof),
  ]);

  const out = {
    source_domain: sourceDomain,
    dest_domain: destDomain,
    jetton_master_account_id_hex: `0x${jettonMasterAccountId.toString('hex')}`,
    jetton_master_code_hash_hex: `0x${masterCodeHash.toString('hex')}`,
    trusted_checkpoint_seqno: checkpointSeqno,
    trusted_checkpoint_hash: `0x${checkpointHash.toString('hex')}`,
    target_mc_seqno: targetSeqno,
    target_mc_block_hash: `0x${targetHash.toString('hex')}`,
    message_id: `0x${messageId.toString('hex')}`,
    proof_scale_hex: `0x${proofBytes.toString('hex')}`,
    proof_scale_base64: proofBytes.toString('base64'),
    proof_sections: {
      masterchain_proof_bytes: masterchainProof.length,
      shard_proof_bytes: shardProof.length,
      account_proof_bytes: accountProof.length,
      burns_dict_proof_bytes: burnsDictProof.length,
    },
  };

  // eslint-disable-next-line no-console
  console.log(JSON.stringify(out, null, 2));
}

main().catch((error) => {
  // eslint-disable-next-line no-console
  console.error(`error: ${error.message}`);
  process.exit(1);
});
