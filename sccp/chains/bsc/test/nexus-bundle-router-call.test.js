import { strict as assert } from 'node:assert';
import { resolve, dirname, basename } from 'node:path';
import { fileURLToPath } from 'node:url';

import { buildRouterCallFromNexusBundle } from '../scripts/build_router_call_from_nexus_bundle.mjs';
import { loadEthers } from '../scripts/load_ethers.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const repoRoot = resolve(__dirname, '..');
const { concat, getBytes, hexlify, keccak256, toUtf8Bytes } =
  await loadEthers(repoRoot);

const DOMAIN_BY_CHAIN = {
  eth: 1,
  bsc: 2,
  tron: 5,
};

function localDomain() {
  const chain = basename(repoRoot);
  const domain = DOMAIN_BY_CHAIN[chain];
  if (!domain) {
    throw new Error(`unsupported EVM chain root for Nexus bundle test: ${repoRoot}`);
  }
  return domain;
}

function pushLe(out, value, widthBytes) {
  let v = BigInt(value);
  for (let i = 0; i < widthBytes; i += 1) {
    out.push(Number(v & 0xffn));
    v >>= 8n;
  }
}

function encodeBurnPayload(payload) {
  const out = [];
  out.push(payload.version);
  pushLe(out, payload.source_domain, 4);
  pushLe(out, payload.dest_domain, 4);
  pushLe(out, payload.nonce, 8);
  out.push(...getBytes(payload.sora_asset_id));
  pushLe(out, payload.amount, 16);
  out.push(...getBytes(payload.recipient));
  return hexlify(Uint8Array.from(out)).toLowerCase();
}

function encodeTokenControlPayload(payload) {
  const out = [];
  out.push(payload.version);
  pushLe(out, payload.target_domain, 4);
  pushLe(out, payload.nonce, 8);
  out.push(...getBytes(payload.sora_asset_id));
  return hexlify(Uint8Array.from(out)).toLowerCase();
}

describe('Nexus bundle router call builder', function () {
  it('builds an EVM mintFromProof call from a Nexus burn bundle', function () {
    const destDomain = localDomain();
    const payload = {
      version: 1,
      source_domain: 0,
      dest_domain: destDomain,
      nonce: '7',
      sora_asset_id: `0x${'11'.repeat(32)}`,
      amount: '42',
      recipient: `0x${'22'.repeat(32)}`,
    };
    const payloadHex = encodeBurnPayload({
      version: payload.version,
      source_domain: payload.source_domain,
      dest_domain: payload.dest_domain,
      nonce: BigInt(payload.nonce),
      sora_asset_id: payload.sora_asset_id,
      amount: BigInt(payload.amount),
      recipient: payload.recipient,
    });
    const messageId = keccak256(concat([toUtf8Bytes('sccp:burn:v1'), getBytes(payloadHex)])).toLowerCase();
    const bundle = {
      version: 1,
      commitment_root: `0x${'99'.repeat(32)}`,
      commitment: {
        version: 1,
        kind: 'Burn',
        target_domain: destDomain,
        message_id: messageId,
        payload_hash: `0x${'44'.repeat(32)}`,
        parliament_certificate_hash: null,
      },
      merkle_proof: { steps: [] },
      payload,
      finality_proof: '0x1234',
    };

    const call = buildRouterCallFromNexusBundle({
      bundleJson: bundle,
      bundleScaleHex: '0xc0de',
      localDomain: destDomain,
    });
    assert.equal(call.method, 'mintFromProof');
    assert.equal(call.source_domain, 0);
    assert.equal(call.payload_hex, payloadHex);
    assert.equal(call.args[2], '0xc0de');
  });

  it('builds an EVM governance call from a Nexus pause bundle', function () {
    const targetDomain = localDomain();
    const pausePayload = {
      version: 1,
      target_domain: targetDomain,
      nonce: '9',
      sora_asset_id: `0x${'33'.repeat(32)}`,
    };
    const payloadHex = encodeTokenControlPayload({
      version: pausePayload.version,
      target_domain: pausePayload.target_domain,
      nonce: BigInt(pausePayload.nonce),
      sora_asset_id: pausePayload.sora_asset_id,
    });
    const messageId = keccak256(concat([toUtf8Bytes('sccp:token:pause:v1'), getBytes(payloadHex)])).toLowerCase();
    const bundle = {
      version: 1,
      commitment_root: `0x${'aa'.repeat(32)}`,
      commitment: {
        version: 1,
        kind: 'TokenPause',
        target_domain: targetDomain,
        message_id: messageId,
        payload_hash: `0x${'55'.repeat(32)}`,
        parliament_certificate_hash: `0x${'66'.repeat(32)}`,
      },
      merkle_proof: { steps: [] },
      payload: {
        Pause: pausePayload,
      },
      parliament_certificate: '0xbeef',
      finality_proof: '0xfeed',
    };

    const call = buildRouterCallFromNexusBundle({
      bundleJson: bundle,
      bundleScaleHex: '0x1234',
      localDomain: targetDomain,
    });
    assert.equal(call.method, 'pauseTokenFromProof');
    assert.equal(call.payload_hex, payloadHex);
    assert.equal(call.args[1], '0x1234');
  });

  it('rejects a burn bundle that targets the wrong local domain', function () {
    const destDomain = localDomain();
    const payload = {
      version: 1,
      source_domain: 0,
      dest_domain: destDomain + 1,
      nonce: '11',
      sora_asset_id: `0x${'77'.repeat(32)}`,
      amount: '5',
      recipient: `0x${'88'.repeat(32)}`,
    };
    const payloadHex = encodeBurnPayload({
      version: payload.version,
      source_domain: payload.source_domain,
      dest_domain: payload.dest_domain,
      nonce: BigInt(payload.nonce),
      sora_asset_id: payload.sora_asset_id,
      amount: BigInt(payload.amount),
      recipient: payload.recipient,
    });
    const messageId = keccak256(concat([toUtf8Bytes('sccp:burn:v1'), getBytes(payloadHex)])).toLowerCase();
    const bundle = {
      version: 1,
      commitment_root: `0x${'bb'.repeat(32)}`,
      commitment: {
        version: 1,
        kind: 'Burn',
        target_domain: payload.dest_domain,
        message_id: messageId,
        payload_hash: `0x${'cc'.repeat(32)}`,
        parliament_certificate_hash: null,
      },
      merkle_proof: { steps: [] },
      payload,
      finality_proof: '0x1234',
    };

    assert.throws(
      () =>
        buildRouterCallFromNexusBundle({
          bundleJson: bundle,
          bundleScaleHex: '0x00',
          localDomain: destDomain,
        }),
      /localDomain/,
    );
  });

  it('rejects a governance bundle with a mismatched message id', function () {
    const targetDomain = localDomain();
    const pausePayload = {
      version: 1,
      target_domain: targetDomain,
      nonce: '13',
      sora_asset_id: `0x${'99'.repeat(32)}`,
    };
    const bundle = {
      version: 1,
      commitment_root: `0x${'dd'.repeat(32)}`,
      commitment: {
        version: 1,
        kind: 'TokenPause',
        target_domain: targetDomain,
        message_id: `0x${'ee'.repeat(32)}`,
        payload_hash: `0x${'ff'.repeat(32)}`,
        parliament_certificate_hash: `0x${'aa'.repeat(32)}`,
      },
      merkle_proof: { steps: [] },
      payload: {
        Pause: pausePayload,
      },
      parliament_certificate: '0xbeef',
      finality_proof: '0xfeed',
    };

    assert.throws(
      () =>
        buildRouterCallFromNexusBundle({
          bundleJson: bundle,
          bundleScaleHex: '0x1234',
          localDomain: targetDomain,
        }),
      /commitment\.message_id/,
    );
  });
});
