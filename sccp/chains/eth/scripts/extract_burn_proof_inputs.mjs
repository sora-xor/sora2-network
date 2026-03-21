#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { loadEthers } from './load_ethers.mjs';

const { Interface, concat, getAddress, getBytes, hexlify, keccak256, toUtf8Bytes } =
  await loadEthers(resolve(import.meta.dirname, '..'));

const EVENT_SIGNATURE = 'SccpBurned(bytes32,bytes32,address,uint128,uint32,bytes32,uint64,bytes)';
const EVENT_ABI =
  'event SccpBurned(bytes32 indexed messageId, bytes32 indexed soraAssetId, address indexed sender, uint128 amount, uint32 destDomain, bytes32 recipient, uint64 nonce, bytes payload)';
const EVENT_TOPIC0 = keccak256(toUtf8Bytes(EVENT_SIGNATURE));
const BURN_PREFIX = toUtf8Bytes('sccp:burn:v1');
const BURN_PAYLOAD_V1_LEN = 97;
const BURN_EVENT_IFACE = new Interface([
  EVENT_ABI,
]);

function usageAndExit(code) {
  console.error(
    [
      'Usage:',
      '  node scripts/extract_burn_proof_inputs.mjs --receipt-file <path> [--router 0x<address>] [--log-index <u64>]',
      '',
      'Extracts the canonical ETH -> SORA burn-proof public inputs from a transaction receipt JSON.',
      'The receipt JSON must contain a logs array in ethers or JSON-RPC shape.',
    ].join('\n'),
  );
  process.exit(code);
}

function parseArgs(argv) {
  const out = {};
  for (let i = 2; i < argv.length; i += 1) {
    const arg = argv[i];
    if (!arg.startsWith('--')) usageAndExit(2);
    const key = arg.slice(2);
    if (key !== 'receipt-file' && key !== 'router' && key !== 'log-index') usageAndExit(2);
    const value = argv[i + 1];
    if (value === undefined || value.startsWith('--')) usageAndExit(2);
    out[key] = value;
    i += 1;
  }
  return out;
}

function normalizeHex(value, label) {
  if (typeof value !== 'string') {
    throw new Error(`${label} must be a string`);
  }
  return hexlify(getBytes(value)).toLowerCase();
}

function normalizeAddress(value, label) {
  if (typeof value !== 'string') {
    throw new Error(`${label} must be a string`);
  }
  return getAddress(value);
}

function parseIndexValue(value, label) {
  if (typeof value === 'number') {
    if (!Number.isInteger(value) || value < 0) throw new Error(`${label} must be a non-negative integer`);
    return BigInt(value);
  }
  if (typeof value === 'bigint') {
    if (value < 0n) throw new Error(`${label} must be a non-negative integer`);
    return value;
  }
  if (typeof value !== 'string') {
    throw new Error(`${label} must be a string or integer`);
  }
  if (/^0x[0-9a-fA-F]+$/.test(value)) return BigInt(value);
  if (/^[0-9]+$/.test(value)) return BigInt(value);
  throw new Error(`${label} must be hex or decimal`);
}

function parseMaybeIndexValue(value, fallback, label) {
  if (value === undefined || value === null) return fallback;
  return parseIndexValue(value, label);
}

function readLE(bytes, offset, width) {
  let value = 0n;
  for (let i = 0; i < width; i += 1) {
    value |= BigInt(bytes[offset + i]) << BigInt(8 * i);
  }
  return value;
}

function bytes32Slice(bytes, offset) {
  return hexlify(bytes.slice(offset, offset + 32)).toLowerCase();
}

function decodeBurnPayloadV1(payloadHex) {
  const bytes = getBytes(payloadHex);
  if (bytes.length !== BURN_PAYLOAD_V1_LEN) {
    throw new Error(`burn payload must be ${BURN_PAYLOAD_V1_LEN} bytes, got ${bytes.length}`);
  }

  return {
    version: Number(bytes[0]),
    source_domain: Number(readLE(bytes, 1, 4)),
    dest_domain: Number(readLE(bytes, 5, 4)),
    nonce: readLE(bytes, 9, 8).toString(),
    sora_asset_id: bytes32Slice(bytes, 17),
    amount: readLE(bytes, 49, 16).toString(),
    recipient: bytes32Slice(bytes, 65),
  };
}

function selectBurnLog(receipt, router, requestedLogIndex) {
  if (!receipt || !Array.isArray(receipt.logs)) {
    throw new Error('receipt JSON must contain a logs array');
  }

  const matches = [];
  for (let i = 0; i < receipt.logs.length; i += 1) {
    const log = receipt.logs[i];
    if (!log || !Array.isArray(log.topics) || log.topics.length === 0) continue;
    if (normalizeHex(log.topics[0], 'log topic0') !== EVENT_TOPIC0) continue;

    const logAddress = normalizeAddress(log.address, 'log address');
    if (router !== null && logAddress !== router) continue;

    const logIndex = parseMaybeIndexValue(log.logIndex ?? log.index, BigInt(i), 'log index');
    if (requestedLogIndex !== null && logIndex !== requestedLogIndex) continue;

    matches.push({ log, logIndex });
  }

  if (matches.length === 0) {
    throw new Error('no matching SccpBurned log found in receipt');
  }
  if (matches.length > 1) {
    throw new Error('multiple matching SccpBurned logs found; pass --log-index or --router');
  }
  return matches[0];
}

function buildOutput(receipt, selected) {
  const parsed = BURN_EVENT_IFACE.parseLog(selected.log);
  if (!parsed) {
    throw new Error('failed to decode SccpBurned log');
  }

  const messageId = normalizeHex(parsed.args.messageId, 'messageId');
  const soraAssetId = normalizeHex(parsed.args.soraAssetId, 'soraAssetId');
  const sender = normalizeAddress(parsed.args.sender, 'sender');
  const amount = parsed.args.amount.toString();
  const destDomain = Number(parsed.args.destDomain);
  const recipient = normalizeHex(parsed.args.recipient, 'recipient');
  const nonce = parsed.args.nonce.toString();
  const payloadHex = normalizeHex(parsed.args.payload, 'payload');

  const decodedPayload = decodeBurnPayloadV1(payloadHex);
  const recomputedMessageId = keccak256(concat([BURN_PREFIX, getBytes(payloadHex)])).toLowerCase();

  if (recomputedMessageId !== messageId) {
    throw new Error(`burn payload messageId mismatch: expected ${messageId}, recomputed ${recomputedMessageId}`);
  }
  if (decodedPayload.sora_asset_id !== soraAssetId) {
    throw new Error('event soraAssetId does not match encoded payload');
  }
  if (decodedPayload.amount !== amount) {
    throw new Error('event amount does not match encoded payload');
  }
  if (decodedPayload.dest_domain !== destDomain) {
    throw new Error('event destDomain does not match encoded payload');
  }
  if (decodedPayload.recipient !== recipient) {
    throw new Error('event recipient does not match encoded payload');
  }
  if (decodedPayload.nonce !== nonce) {
    throw new Error('event nonce does not match encoded payload');
  }

  const router = normalizeAddress(selected.log.address, 'router address');
  const blockNumberValue = receipt.blockNumber ?? selected.log.blockNumber;
  if (blockNumberValue === undefined || blockNumberValue === null) {
    throw new Error('receipt blockNumber missing');
  }
  const blockNumber = parseIndexValue(blockNumberValue, 'receipt blockNumber');
  const status = receipt.status === undefined || receipt.status === null
    ? undefined
    : parseMaybeIndexValue(receipt.status, 0n, 'receipt status').toString();

  return {
    schema: 'sccp-eth-burn-proof-inputs/v1',
    event_name: 'SccpBurned',
    event_signature: EVENT_SIGNATURE,
    event_topic0: EVENT_TOPIC0,
    router,
    transaction_hash: normalizeHex(
      receipt.transactionHash ?? selected.log.transactionHash,
      'transactionHash',
    ),
    block_hash: normalizeHex(receipt.blockHash ?? selected.log.blockHash, 'blockHash'),
    block_number: blockNumber.toString(),
    log_index: selected.logIndex.toString(),
    receipt_status: status,
    message_id: messageId,
    payload_hex: payloadHex,
    indexed_event_fields: {
      message_id: messageId,
      sora_asset_id: soraAssetId,
      sender,
    },
    event_fields: {
      amount,
      dest_domain: destDomain,
      recipient,
      nonce,
    },
    decoded_payload: decodedPayload,
    proof_public_inputs: {
      router,
      event_topic0: EVENT_TOPIC0,
      message_id: messageId,
      payload_hex: payloadHex,
      source_domain: decodedPayload.source_domain,
      dest_domain: decodedPayload.dest_domain,
    },
  };
}

function main() {
  const args = parseArgs(process.argv);
  if (!args['receipt-file']) usageAndExit(2);

  const router = args.router ? normalizeAddress(args.router, 'router') : null;
  const requestedLogIndex = args['log-index'] ? parseIndexValue(args['log-index'], 'log-index') : null;
  const receipt = JSON.parse(readFileSync(args['receipt-file'], 'utf8'));
  const selected = selectBurnLog(receipt, router, requestedLogIndex);
  const output = buildOutput(receipt, selected);
  process.stdout.write(`${JSON.stringify(output, null, 2)}\n`);
}

main();
