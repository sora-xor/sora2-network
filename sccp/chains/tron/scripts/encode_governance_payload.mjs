#!/usr/bin/env node

import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { loadEthers } from './load_ethers.mjs';

const { keccak256, concat, toUtf8Bytes, getBytes } = await loadEthers(resolve(import.meta.dirname, '..'));

const ACTIONS = new Set(['add', 'pause', 'resume']);
const ACTION_ALLOWED_ARGS = {
  add: new Set(['action', 'target-domain', 'nonce', 'sora-asset-id', 'decimals', 'name', 'symbol']),
  pause: new Set(['action', 'target-domain', 'nonce', 'sora-asset-id']),
  resume: new Set(['action', 'target-domain', 'nonce', 'sora-asset-id']),
};
const KNOWN_ARGS = new Set(['action', 'target-domain', 'nonce', 'sora-asset-id', 'decimals', 'name', 'symbol']);

function usageAndExit(code) {
  console.error(
    [
      'Usage:',
      '  node scripts/encode_governance_payload.mjs --action add --target-domain <u32> --nonce <u64> --sora-asset-id 0x<32-byte> --decimals <u8> --name <text> --symbol <text>',
      '  node scripts/encode_governance_payload.mjs --action pause --target-domain <u32> --nonce <u64> --sora-asset-id 0x<32-byte>',
      '  node scripts/encode_governance_payload.mjs --action resume --target-domain <u32> --nonce <u64> --sora-asset-id 0x<32-byte>',
    ].join('\n'),
  );
  process.exit(code);
}

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (!a.startsWith('--')) {
      throw new Error(`Unexpected positional argument: ${a}`);
    }
    const key = a.slice(2);
    if (!KNOWN_ARGS.has(key)) {
      throw new Error(`Unknown argument: --${key}`);
    }
    if (Object.hasOwn(out, key)) {
      throw new Error(`Duplicate argument: --${key}`);
    }
    const value = argv[i + 1];
    if (value === undefined || value.startsWith('--')) {
      throw new Error(`Missing value for --${key}`);
    }
    out[key] = value;
    i += 1;
  }
  return out;
}

function parseU32(v, label) {
  if (!/^[0-9]+$/.test(v)) throw new Error(`${label} must be a decimal integer`);
  const n = Number(v);
  if (!Number.isInteger(n) || n < 0 || n > 0xffffffff) throw new Error(`${label} out of u32 range`);
  return n;
}

function parseU64(v, label) {
  if (!/^[0-9]+$/.test(v)) throw new Error(`${label} must be a decimal integer`);
  const n = BigInt(v);
  if (n < 0n || n > 0xffffffffffffffffn) throw new Error(`${label} out of u64 range`);
  return n;
}

function parseU8(v, label) {
  if (!/^[0-9]+$/.test(v)) throw new Error(`${label} must be a decimal integer`);
  const n = Number(v);
  if (!Number.isInteger(n) || n < 0 || n > 255) throw new Error(`${label} out of u8 range`);
  return n;
}

function parseAssetId(v) {
  const with0x = v.startsWith('0x') ? v : `0x${v}`;
  if (!/^0x[0-9a-fA-F]{64}$/.test(with0x)) {
    throw new Error('sora-asset-id must be exactly 32 bytes hex');
  }
  return with0x.toLowerCase();
}

function encodeLE32(n) {
  const out = new Uint8Array(4);
  out[0] = n & 0xff;
  out[1] = (n >> 8) & 0xff;
  out[2] = (n >> 16) & 0xff;
  out[3] = (n >> 24) & 0xff;
  return out;
}

function encodeLE64(n) {
  const out = new Uint8Array(8);
  let v = n;
  for (let i = 0; i < 8; i += 1) {
    out[i] = Number(v & 0xffn);
    v >>= 8n;
  }
  return out;
}

function encodeAsciiBytes32(value, label) {
  const raw = toUtf8Bytes(value);
  if (raw.length === 0 || raw.length > 32) {
    throw new Error(`${label} must be 1..32 bytes UTF-8`);
  }
  for (const b of raw) {
    if (b < 0x20 || b > 0x7e) {
      throw new Error(`${label} must use printable ASCII`);
    }
  }
  const out = new Uint8Array(32);
  out.set(raw);
  return out;
}

function prefixForAction(action) {
  if (action === 'add') return toUtf8Bytes('sccp:token:add:v1');
  if (action === 'pause') return toUtf8Bytes('sccp:token:pause:v1');
  return toUtf8Bytes('sccp:token:resume:v1');
}

function toHex(bytes) {
  return `0x${Buffer.from(bytes).toString('hex')}`;
}

function asHex(v) {
  if (typeof v === 'string') return v.toLowerCase();
  return toHex(v);
}

function validateActionArgs(args) {
  const action = args.action;
  if (!ACTIONS.has(action)) throw new Error('action must be one of: add, pause, resume');

  const allowed = ACTION_ALLOWED_ARGS[action];
  for (const key of Object.keys(args)) {
    if (!allowed.has(key)) {
      throw new Error(`--${key} is not valid with --action ${action}`);
    }
  }

  for (const required of ['action', 'target-domain', 'nonce', 'sora-asset-id']) {
    if (!args[required]) {
      throw new Error(`Missing required --${required}`);
    }
  }

  if (action === 'add' && (!args.decimals || !args.name || !args.symbol)) {
    throw new Error('add action requires --decimals, --name, and --symbol');
  }
}

function encodePayload(args) {
  validateActionArgs(args);

  const action = args.action;

  const targetDomain = parseU32(args['target-domain'], 'target-domain');
  const nonce = parseU64(args.nonce, 'nonce');
  const soraAssetId = parseAssetId(args['sora-asset-id']);
  const soraAssetIdBytes = getBytes(soraAssetId);

  if (action === 'add') {
    const decimals = parseU8(args.decimals, 'decimals');
    const name = encodeAsciiBytes32(args.name, 'name');
    const symbol = encodeAsciiBytes32(args.symbol, 'symbol');
    const payload = concat([
      Uint8Array.from([1]),
      encodeLE32(targetDomain),
      encodeLE64(nonce),
      soraAssetIdBytes,
      Uint8Array.from([decimals]),
      name,
      symbol,
    ]);
    return { action, payload, targetDomain, nonce: nonce.toString(), soraAssetId, decimals, name: args.name, symbol: args.symbol };
  }

  const payload = concat([
    Uint8Array.from([1]),
    encodeLE32(targetDomain),
    encodeLE64(nonce),
    soraAssetIdBytes,
  ]);

  return { action, payload, targetDomain, nonce: nonce.toString(), soraAssetId };
}

function main() {
  try {
    const args = parseArgs(process.argv.slice(2));
    const encoded = encodePayload(args);
    const messageId = keccak256(concat([prefixForAction(encoded.action), encoded.payload]));

    console.log(
      JSON.stringify(
        {
          action: encoded.action,
          target_domain: encoded.targetDomain,
          nonce: encoded.nonce,
          sora_asset_id: encoded.soraAssetId,
          decimals: encoded.decimals,
          name: encoded.name,
          symbol: encoded.symbol,
          payload_hex: asHex(encoded.payload),
          message_id: messageId,
        },
        null,
        2,
      ),
    );
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    usageAndExit(2);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}

export { encodePayload, parseArgs };
