#!/usr/bin/env node

import { createHash } from 'node:crypto';

function usage() {
  console.error(
    [
      'Usage:',
      '  tron_header.mjs --rpc <url> --block-number <n>',
      '',
      'Options:',
      '  --rpc <url>           TRON wallet RPC base URL',
      '  --block-number <n>    TRON block number to export',
      '  --help                Show this message',
    ].join('\n')
  );
}

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--help' || arg === '-h') {
      args.help = true;
      continue;
    }
    if (!arg.startsWith('--')) {
      throw new Error(`unexpected argument: ${arg}`);
    }
    const key = arg.slice(2);
    const value = argv[i + 1];
    if (value == null || value.startsWith('--')) {
      throw new Error(`missing value for --${key}`);
    }
    args[key] = value;
    i += 1;
  }
  return args;
}

function toBigEndianU64(value) {
  const n = BigInt(value);
  const out = Buffer.alloc(8);
  out.writeBigUInt64BE(n);
  return out;
}

function normalizeRpcBase(rpc) {
  return rpc.endsWith('/') ? rpc.slice(0, -1) : rpc;
}

function walletGetBlockByNumUrl(rpc) {
  return `${normalizeRpcBase(rpc)}/wallet/getblockbynum`;
}

function base64OfHex(hex) {
  return Buffer.from(hex, 'hex').toString('base64');
}

function hexOfBytes(bytes) {
  return Buffer.from(bytes).toString('hex');
}

function deriveBlockId(blockNumber, rawDataHex) {
  const rawHash = createHash('sha256').update(Buffer.from(rawDataHex, 'hex')).digest();
  const blockId = Buffer.from(rawHash);
  toBigEndianU64(blockNumber).copy(blockId, 0);
  return {
    blockId: blockId.toString('hex'),
    rawHash: rawHash.toString('hex'),
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    usage();
    process.exit(0);
  }

  const rpc = args.rpc;
  const blockNumberRaw = args['block-number'];
  if (!rpc || blockNumberRaw == null) {
    usage();
    process.exit(1);
  }

  const blockNumber = Number(blockNumberRaw);
  if (!Number.isInteger(blockNumber) || blockNumber < 0) {
    throw new Error(`invalid --block-number: ${blockNumberRaw}`);
  }

  const requestUrl = walletGetBlockByNumUrl(rpc);
  const response = await fetch(requestUrl, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ num: blockNumber }),
  });
  if (!response.ok) {
    throw new Error(`TRON RPC request failed: ${response.status} ${response.statusText}`);
  }

  const block = await response.json();
  const header = block?.block_header;
  const rawDataHex = header?.raw_data_hex;
  const witnessSignatureHex = block?.witness_signature;
  const rawData = header?.raw_data;

  if (typeof rawDataHex !== 'string' || rawDataHex.length === 0) {
    throw new Error('missing block_header.raw_data_hex');
  }
  if (typeof witnessSignatureHex !== 'string' || witnessSignatureHex.length === 0) {
    throw new Error('missing witness_signature');
  }
  if (!rawData || typeof rawData.number !== 'number') {
    throw new Error('missing block_header.raw_data.number');
  }

  const { blockId: derivedBlockId, rawHash } = deriveBlockId(rawData.number, rawDataHex);

  const out = {
    block_number: rawData.number,
    request_url: requestUrl,
    block_id: block?.blockID ?? null,
    derived_block_id: derivedBlockId,
    raw_data_hash: rawHash,
    raw_data_hex: rawDataHex,
    raw_data_base64: base64OfHex(rawDataHex),
    witness_signature_hex: witnessSignatureHex,
    witness_signature_base64: base64OfHex(witnessSignatureHex),
    raw_parent_hash:
      typeof rawData.parentHash === 'string' ? rawData.parentHash.toLowerCase() : null,
    raw_witness_address:
      typeof rawData.witness_address === 'string'
        ? rawData.witness_address.toLowerCase()
        : null,
    raw_account_state_root:
      typeof rawData.accountStateRoot === 'string'
        ? rawData.accountStateRoot.toLowerCase()
        : null,
    raw_data_len: Buffer.from(rawDataHex, 'hex').length,
    witness_signature_len: Buffer.from(witnessSignatureHex, 'hex').length,
    raw_data_sha256_bytes_hex: hexOfBytes(Buffer.from(rawHash, 'hex')),
  };

  console.log(JSON.stringify(out, null, 2));
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
