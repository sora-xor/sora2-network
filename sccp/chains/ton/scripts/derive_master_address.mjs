import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { Address, beginCell, Cell, contractAddress, Dictionary } from '@ton/core';

const repoRoot = resolve(import.meta.dirname, '..');

function loadArtifact(name) {
  return JSON.parse(readFileSync(resolve(repoRoot, 'artifacts', name), 'utf8'));
}

function codeFromArtifact(artifact) {
  return Cell.fromBoc(Buffer.from(artifact.codeBoc64, 'base64'))[0];
}

function parseHexU256(s) {
  const hex = s.startsWith('0x') ? s.slice(2) : s;
  if (!/^[0-9a-fA-F]{64}$/.test(hex)) {
    throw new Error(`Expected 32-byte hex (64 chars), got: ${s}`);
  }
  return BigInt('0x' + hex.toLowerCase());
}

function usageAndExit(code) {
  // eslint-disable-next-line no-console
  console.error(
    [
      'Usage:',
      '  node scripts/derive_master_address.mjs --governor <legacy_seed_addr> --sora-asset-id <64hex> [--verifier <ton_addr>] [--metadata-uri <string>]',
      '',
      'Notes:',
      '  - governor is a legacy address-derivation seed only; it does not retain post-deploy admin powers',
      '  - verifier is usually omitted because the verifier self-registers during one-time bootstrap',
      '',
      'Outputs:',
      '  - master_address (friendly)',
      '  - master_account_id_hex (32 bytes)  => use as SORA `remote_token_id` for TON',
      '  - master_code_hash_hex (32 bytes)   => can be used as SORA TON `domain_endpoint` identifier',
    ].join('\n'),
  );
  process.exit(code);
}

function parseArgs(argv) {
  const valueFlags = new Set(['governor', 'sora-asset-id', 'verifier', 'metadata-uri']);
  const out = {};
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (!a.startsWith('--')) {
      throw new Error(`Unexpected positional argument: ${a}`);
    }
    const key = a.slice(2);
    if (!valueFlags.has(key)) {
      throw new Error(`Unknown argument: ${a}`);
    }
    if (Object.hasOwn(out, key)) {
      throw new Error(`Duplicate argument: ${a}`);
    }
    const next = argv[i + 1];
    if (next === undefined || next.startsWith('--')) {
      throw new Error(`Missing value for ${a}`);
    }
    out[key] = next;
    i += 1;
  }
  return out;
}

function buildSnakeDataCell(data) {
  const chunkSize = 127;
  if (data.length === 0) {
    return beginCell().endCell();
  }

  let tail = null;
  for (let offset = data.length; offset > 0; offset -= chunkSize) {
    const start = Math.max(0, offset - chunkSize);
    const chunk = data.subarray(start, offset);
    const b = beginCell().storeBuffer(chunk);
    if (tail) {
      b.storeRef(tail);
    }
    tail = b.endCell();
  }

  return tail;
}

function buildMasterData({ governor, verifier, walletCode, metadataUri, soraAssetIdU256 }) {
  const emptyBoolMap = Dictionary.empty(Dictionary.Keys.BigUint(256), Dictionary.Values.Bool());
  const emptyBurnsMap = Dictionary.empty(Dictionary.Keys.BigUint(256), Dictionary.Values.Cell());

  const sccpExtraB = beginCell();
  sccpExtraB.storeUint(soraAssetIdU256, 256);
  sccpExtraB.storeUint(0, 64); // nonce
  sccpExtraB.storeUint(0, 64); // inboundPausedMask
  sccpExtraB.storeUint(0, 64); // outboundPausedMask
  emptyBoolMap.store(sccpExtraB); // invalidatedInbound
  emptyBoolMap.store(sccpExtraB); // processedInbound
  emptyBurnsMap.store(sccpExtraB); // burns
  const sccpExtra = sccpExtraB.endCell();

  const metadataCell = buildSnakeDataCell(Buffer.from(metadataUri ?? '', 'utf8'));

  return beginCell()
    .storeCoins(0n) // totalSupply
    .storeAddress(governor)
    .storeAddress(verifier ?? null)
    .storeRef(walletCode)
    .storeRef(metadataCell)
    .storeRef(sccpExtra)
    .endCell();
}

async function main() {
  const rawArgv = process.argv.slice(2);
  if (rawArgv.includes('--help') || rawArgv.includes('-h')) {
    usageAndExit(0);
  }

  let args;
  try {
    args = parseArgs(rawArgv);
  } catch (e) {
    // eslint-disable-next-line no-console
    console.error(`Error: ${e.message}`);
    usageAndExit(2);
  }
  const governorStr = args.governor ?? null;
  const soraAssetIdStr = args['sora-asset-id'] ?? null;
  if (!governorStr || !soraAssetIdStr) {
    usageAndExit(2);
  }

  const verifierStr = args.verifier ?? null;
  const metadataUri = args['metadata-uri'] ?? '';

  const governor = Address.parse(governorStr);
  const verifier = verifierStr ? Address.parse(verifierStr) : null;
  const soraAssetIdU256 = parseHexU256(soraAssetIdStr);

  const masterArtifact = loadArtifact('sccp-jetton-master.compiled.json');
  const walletArtifact = loadArtifact('sccp-jetton-wallet.compiled.json');
  const masterCode = codeFromArtifact(masterArtifact);
  const walletCode = codeFromArtifact(walletArtifact);

  const data = buildMasterData({ governor, verifier, walletCode, metadataUri, soraAssetIdU256 });
  const init = { code: masterCode, data };
  const addr = contractAddress(0, init);

  const out = {
    master_address: addr.toString(),
    master_account_id_hex: addr.hash.toString('hex'),
    master_code_hash_hex: masterArtifact.codeHashHex,
    wallet_code_hash_hex: walletArtifact.codeHashHex,
  };

  // eslint-disable-next-line no-console
  console.log(JSON.stringify(out, null, 2));
}

main().catch((e) => {
  // eslint-disable-next-line no-console
  console.error(e);
  process.exit(1);
});
