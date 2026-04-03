#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

import { beginCell } from '@ton/core';
import { ethers } from 'ethers';

const OPCODES = {
  mint: 0x23e4c1a0,
  add: 0x23e4c1a1,
  pause: 0x23e4c1a2,
  resume: 0x23e4c1a3,
};

const BURN_PREFIX = Buffer.from('sccp:burn:v1', 'utf8');
const TOKEN_ADD_PREFIX = Buffer.from('sccp:token:add:v1', 'utf8');
const TOKEN_PAUSE_PREFIX = Buffer.from('sccp:token:pause:v1', 'utf8');
const TOKEN_RESUME_PREFIX = Buffer.from('sccp:token:resume:v1', 'utf8');

function usage(message) {
  const lines = [
    'Usage:',
    '  node scripts/build_master_call_from_nexus_bundle.mjs [--bundle-json-file <path>] [--bundle-norito-file <path> | --bundle-norito-hex 0x<...>] [--local-domain <u32>]',
    '',
    'When omitted, bundle paths and local domain are read from SCCP_SCENARIO_CONTEXT_FILE / SCCP_HUB_BUNDLE_* env vars.',
  ];
  if (message) {
    lines.push('', String(message));
  }
  throw new Error(lines.join('\n'));
}

function parseArgs(argv) {
  const out = {
    bundleJsonFile: null,
    bundleNoritoFile: null,
    bundleNoritoHex: null,
    bundleScaleFile: null,
    bundleScaleHex: null,
    localDomain: null,
  };
  for (let i = 2; i < argv.length; i += 1) {
    const arg = argv[i];
    const next = argv[i + 1];
    if (arg === '--bundle-json-file' && next) {
      out.bundleJsonFile = next;
      i += 1;
    } else if (arg === '--bundle-norito-file' && next) {
      out.bundleNoritoFile = next;
      i += 1;
    } else if (arg === '--bundle-norito-hex' && next) {
      out.bundleNoritoHex = next;
      i += 1;
    } else if (arg === '--bundle-scale-file' && next) {
      out.bundleScaleFile = next;
      i += 1;
    } else if (arg === '--bundle-scale-hex' && next) {
      out.bundleScaleHex = next;
      i += 1;
    } else if (arg === '--local-domain' && next) {
      out.localDomain = next;
      i += 1;
    } else if (arg === '--help' || arg === '-h') {
      usage();
    } else {
      usage(`Unknown or incomplete argument: ${arg}`);
    }
  }
  return out;
}

function readScenarioContext() {
  const filePath = process.env.SCCP_SCENARIO_CONTEXT_FILE;
  if (!filePath) {
    return null;
  }
  return JSON.parse(readFileSync(filePath, 'utf8'));
}

function normalizeHex(value, label, expectedBytes = null) {
  if (typeof value !== 'string' || !/^0x[0-9a-fA-F]*$/.test(value)) {
    usage(`${label} must be a 0x-prefixed hex string`);
  }
  const normalized = ethers.hexlify(ethers.getBytes(value)).toLowerCase();
  if (expectedBytes !== null && ethers.getBytes(normalized).length !== expectedBytes) {
    usage(`${label} must be ${expectedBytes} bytes`);
  }
  return normalized;
}

function parseU32(value, label) {
  if (typeof value === 'number' && Number.isInteger(value) && value >= 0 && value <= 0xffff_ffff) {
    return value;
  }
  if (typeof value === 'string' && /^[0-9]+$/.test(value)) {
    const parsed = Number(value);
    if (Number.isSafeInteger(parsed) && parsed >= 0 && parsed <= 0xffff_ffff) {
      return parsed;
    }
  }
  usage(`${label} must be a safe u32 integer or decimal string`);
}

function parseBigIntDecimal(value, label) {
  if (typeof value === 'bigint') {
    return value;
  }
  if (typeof value === 'number' && Number.isSafeInteger(value) && value >= 0) {
    return BigInt(value);
  }
  if (typeof value === 'string' && /^[0-9]+$/.test(value)) {
    return BigInt(value);
  }
  usage(`${label} must be a non-negative integer or decimal string`);
}

function pushLe(out, value, widthBytes) {
  let v = value;
  for (let i = 0; i < widthBytes; i += 1) {
    out.push(Number(v & 0xffn));
    v >>= 8n;
  }
  if (v !== 0n) {
    usage(`value does not fit in ${widthBytes} bytes`);
  }
}

function encodeBurnPayload(payload) {
  const out = [];
  out.push(parseU32(payload.version ?? 0, 'payload.version'));
  pushLe(out, BigInt(parseU32(payload.source_domain, 'payload.source_domain')), 4);
  pushLe(out, BigInt(parseU32(payload.dest_domain, 'payload.dest_domain')), 4);
  pushLe(out, parseBigIntDecimal(payload.nonce, 'payload.nonce'), 8);
  out.push(...ethers.getBytes(normalizeHex(payload.sora_asset_id, 'payload.sora_asset_id', 32)));
  pushLe(out, parseBigIntDecimal(payload.amount, 'payload.amount'), 16);
  out.push(...ethers.getBytes(normalizeHex(payload.recipient, 'payload.recipient', 32)));
  return ethers.hexlify(Uint8Array.from(out)).toLowerCase();
}

function encodeTokenAddPayload(payload) {
  const out = [];
  out.push(parseU32(payload.version ?? 0, 'payload.version'));
  pushLe(out, BigInt(parseU32(payload.target_domain, 'payload.target_domain')), 4);
  pushLe(out, parseBigIntDecimal(payload.nonce, 'payload.nonce'), 8);
  out.push(...ethers.getBytes(normalizeHex(payload.sora_asset_id, 'payload.sora_asset_id', 32)));
  out.push(parseU32(payload.decimals, 'payload.decimals'));
  out.push(...ethers.getBytes(normalizeHex(payload.name, 'payload.name', 32)));
  out.push(...ethers.getBytes(normalizeHex(payload.symbol, 'payload.symbol', 32)));
  return ethers.hexlify(Uint8Array.from(out)).toLowerCase();
}

function encodeTokenControlPayload(payload) {
  const out = [];
  out.push(parseU32(payload.version ?? 0, 'payload.version'));
  pushLe(out, BigInt(parseU32(payload.target_domain, 'payload.target_domain')), 4);
  pushLe(out, parseBigIntDecimal(payload.nonce, 'payload.nonce'), 8);
  out.push(...ethers.getBytes(normalizeHex(payload.sora_asset_id, 'payload.sora_asset_id', 32)));
  return ethers.hexlify(Uint8Array.from(out)).toLowerCase();
}

function bocOutputs(cell) {
  const boc = cell.toBoc({ idx: false });
  return {
    body_boc_hex: `0x${Buffer.from(boc).toString('hex')}`,
    body_boc_base64: Buffer.from(boc).toString('base64'),
  };
}

export function buildTonMasterCallFromNexusBundle({ bundleJson, bundleProofHex, localDomain }) {
  if (!bundleJson || typeof bundleJson !== 'object') {
    usage('bundleJson must be an object');
  }

  const proofHex = normalizeHex(bundleProofHex, 'bundleProofHex');
  const normalizedLocalDomain = parseU32(localDomain, 'localDomain');
  const commitment = bundleJson.commitment;
  if (!commitment || typeof commitment !== 'object') {
    usage('bundleJson.commitment must be an object');
  }

  if (bundleJson.payload && typeof bundleJson.payload === 'object' && !Array.isArray(bundleJson.payload)) {
    const payload = bundleJson.payload;
    if ('source_domain' in payload && 'dest_domain' in payload) {
      const payloadHex = encodeBurnPayload(payload);
      const messageId = ethers.keccak256(Buffer.concat([BURN_PREFIX, Buffer.from(payloadHex.slice(2), 'hex')])).toLowerCase();
      if (normalizeHex(commitment.message_id, 'commitment.message_id', 32) !== messageId) {
        usage('burn bundle commitment.message_id does not match the canonical payload message_id');
      }
      if (parseU32(payload.dest_domain, 'payload.dest_domain') !== normalizedLocalDomain) {
        usage('burn bundle payload.dest_domain does not match localDomain');
      }
      const body = beginCell()
        .storeUint(OPCODES.mint, 32)
        .storeUint(0, 64)
        .storeUint(parseU32(payload.source_domain, 'payload.source_domain'), 32)
        .storeUint(parseBigIntDecimal(payload.nonce, 'payload.nonce'), 64)
        .storeCoins(parseBigIntDecimal(payload.amount, 'payload.amount'))
        .storeUint(BigInt(normalizeHex(payload.recipient, 'payload.recipient', 32)), 256)
        .storeAddress(null)
        .endCell();
      return {
        ok: true,
        method: 'SccpMintFromVerifier',
        opcode: `0x${OPCODES.mint.toString(16)}`,
        message_id: messageId,
        payload_hex: payloadHex,
        proof_hex: proofHex,
        source_domain: parseU32(payload.source_domain, 'payload.source_domain'),
        burn_nonce: parseBigIntDecimal(payload.nonce, 'payload.nonce').toString(),
        jetton_amount: parseBigIntDecimal(payload.amount, 'payload.amount').toString(),
        recipient32: normalizeHex(payload.recipient, 'payload.recipient', 32),
        ...bocOutputs(body),
      };
    }

    const governanceEntries = Object.entries(payload);
    if (governanceEntries.length !== 1) {
      usage('governance bundle payload must be an externally-tagged object with one variant');
    }
    const [variant, variantPayload] = governanceEntries[0];
    const targetDomain = parseU32(variantPayload.target_domain, 'payload.target_domain');
    if (targetDomain !== normalizedLocalDomain) {
      usage('governance bundle payload.target_domain does not match localDomain');
    }

    if (variant === 'Add') {
      const payloadHex = encodeTokenAddPayload(variantPayload);
      const messageId = ethers.keccak256(Buffer.concat([TOKEN_ADD_PREFIX, Buffer.from(payloadHex.slice(2), 'hex')])).toLowerCase();
      if (normalizeHex(commitment.message_id, 'commitment.message_id', 32) !== messageId) {
        usage('governance add bundle commitment.message_id does not match the canonical payload message_id');
      }
      const body = beginCell()
        .storeUint(OPCODES.add, 32)
        .storeUint(0, 64)
        .storeUint(BigInt(messageId), 256)
        .storeUint(BigInt(normalizeHex(variantPayload.sora_asset_id, 'payload.sora_asset_id', 32)), 256)
        .endCell();
      return {
        ok: true,
        method: 'SccpAddTokenFromVerifier',
        opcode: `0x${OPCODES.add.toString(16)}`,
        message_id: messageId,
        payload_hex: payloadHex,
        proof_hex: proofHex,
        sora_asset_id: normalizeHex(variantPayload.sora_asset_id, 'payload.sora_asset_id', 32),
        ...bocOutputs(body),
      };
    }

    if (variant === 'Pause' || variant === 'Resume') {
      const payloadHex = encodeTokenControlPayload(variantPayload);
      const prefix = variant === 'Pause' ? TOKEN_PAUSE_PREFIX : TOKEN_RESUME_PREFIX;
      const opcode = variant === 'Pause' ? OPCODES.pause : OPCODES.resume;
      const method =
        variant === 'Pause' ? 'SccpPauseTokenFromVerifier' : 'SccpResumeTokenFromVerifier';
      const messageId = ethers.keccak256(Buffer.concat([prefix, Buffer.from(payloadHex.slice(2), 'hex')])).toLowerCase();
      if (normalizeHex(commitment.message_id, 'commitment.message_id', 32) !== messageId) {
        usage(`governance ${variant.toLowerCase()} bundle commitment.message_id does not match the canonical payload message_id`);
      }
      const body = beginCell()
        .storeUint(opcode, 32)
        .storeUint(0, 64)
        .storeUint(BigInt(messageId), 256)
        .storeUint(BigInt(normalizeHex(variantPayload.sora_asset_id, 'payload.sora_asset_id', 32)), 256)
        .endCell();
      return {
        ok: true,
        method,
        opcode: `0x${opcode.toString(16)}`,
        message_id: messageId,
        payload_hex: payloadHex,
        proof_hex: proofHex,
        sora_asset_id: normalizeHex(variantPayload.sora_asset_id, 'payload.sora_asset_id', 32),
        ...bocOutputs(body),
      };
    }
  }

  usage('unsupported Nexus bundle JSON shape');
}

async function main() {
  const args = parseArgs(process.argv);
  const context = readScenarioContext();
  const bundleJsonFile = args.bundleJsonFile || context?.hub_bundle_json_path || process.env.SCCP_HUB_BUNDLE_JSON_PATH;
  const bundleNoritoFile =
    args.bundleNoritoFile ||
    context?.hub_bundle_norito_path ||
    process.env.SCCP_HUB_BUNDLE_NORITO_PATH ||
    args.bundleScaleFile ||
    context?.hub_bundle_scale_path ||
    process.env.SCCP_HUB_BUNDLE_SCALE_PATH;
  const bundleNoritoHex =
    args.bundleNoritoHex ||
    context?.hub_bundle_norito_hex ||
    process.env.SCCP_HUB_BUNDLE_NORITO_HEX ||
    args.bundleScaleHex ||
    context?.hub_bundle_scale_hex ||
    process.env.SCCP_HUB_BUNDLE_SCALE_HEX;
  const localDomain = args.localDomain || process.env.SCCP_DEST_DOMAIN;

  if (!bundleJsonFile) {
    usage('missing --bundle-json-file and no hub bundle JSON path in scenario context');
  }
  if (!bundleNoritoFile && !bundleNoritoHex) {
    usage('missing --bundle-norito-file / --bundle-norito-hex and no hub bundle proof bytes in scenario context');
  }
  if (!localDomain) {
    usage('missing --local-domain and SCCP_DEST_DOMAIN');
  }

  const bundleJson = JSON.parse(readFileSync(bundleJsonFile, 'utf8'));
  const proofHex = bundleNoritoHex
    ? normalizeHex(bundleNoritoHex, 'bundleNoritoHex')
    : `0x${readFileSync(bundleNoritoFile).toString('hex')}`;
  const result = buildTonMasterCallFromNexusBundle({
    bundleJson,
    bundleProofHex: proofHex,
    localDomain,
  });
  process.stdout.write(`${JSON.stringify(result)}\n`);
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  try {
    await main();
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exit(1);
  }
}
