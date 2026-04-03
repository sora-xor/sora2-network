import test from 'node:test';
import assert from 'node:assert/strict';

import { ethers } from 'ethers';

import { buildTonMasterCallFromNexusBundle } from '../scripts/build_master_call_from_nexus_bundle.mjs';

const DOMAIN_TON = 4;

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
  out.push(...ethers.getBytes(payload.sora_asset_id));
  pushLe(out, payload.amount, 16);
  out.push(...ethers.getBytes(payload.recipient));
  return ethers.hexlify(Uint8Array.from(out)).toLowerCase();
}

function encodeTokenControlPayload(payload) {
  const out = [];
  out.push(payload.version);
  pushLe(out, payload.target_domain, 4);
  pushLe(out, payload.nonce, 8);
  out.push(...ethers.getBytes(payload.sora_asset_id));
  return ethers.hexlify(Uint8Array.from(out)).toLowerCase();
}

test('builds a TON mint master call from a Nexus burn bundle', () => {
  const payload = {
    version: 1,
    source_domain: 0,
    dest_domain: DOMAIN_TON,
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
  const messageId = ethers.keccak256(
    Buffer.concat([Buffer.from('sccp:burn:v1', 'utf8'), Buffer.from(payloadHex.slice(2), 'hex')]),
  ).toLowerCase();
  const bundle = {
    version: 1,
    commitment_root: `0x${'33'.repeat(32)}`,
    commitment: {
      version: 1,
      kind: 'Burn',
      target_domain: DOMAIN_TON,
      message_id: messageId,
      payload_hash: `0x${'44'.repeat(32)}`,
      parliament_certificate_hash: null,
    },
    merkle_proof: { steps: [] },
    payload,
    finality_proof: '0x1234',
  };

  const call = buildTonMasterCallFromNexusBundle({
    bundleJson: bundle,
    bundleScaleHex: '0xc0de',
    localDomain: DOMAIN_TON,
  });

  assert.equal(call.method, 'SccpMintFromVerifier');
  assert.equal(call.message_id, messageId);
  assert.equal(call.payload_hex, payloadHex);
  assert.equal(call.proof_hex, '0xc0de');
  assert.ok(call.body_boc_hex.startsWith('0x'));
});

test('builds a TON governance master call from a Nexus pause bundle', () => {
  const pausePayload = {
    version: 1,
    target_domain: DOMAIN_TON,
    nonce: '9',
    sora_asset_id: `0x${'55'.repeat(32)}`,
  };
  const payloadHex = encodeTokenControlPayload({
    version: pausePayload.version,
    target_domain: pausePayload.target_domain,
    nonce: BigInt(pausePayload.nonce),
    sora_asset_id: pausePayload.sora_asset_id,
  });
  const messageId = ethers.keccak256(
    Buffer.concat([Buffer.from('sccp:token:pause:v1', 'utf8'), Buffer.from(payloadHex.slice(2), 'hex')]),
  ).toLowerCase();
  const bundle = {
    version: 1,
    commitment_root: `0x${'66'.repeat(32)}`,
    commitment: {
      version: 1,
      kind: 'TokenPause',
      target_domain: DOMAIN_TON,
      message_id: messageId,
      payload_hash: `0x${'77'.repeat(32)}`,
      parliament_certificate_hash: `0x${'88'.repeat(32)}`,
    },
    merkle_proof: { steps: [] },
    payload: {
      Pause: pausePayload,
    },
    parliament_certificate: '0xbeef',
    finality_proof: '0xfeed',
  };

  const call = buildTonMasterCallFromNexusBundle({
    bundleJson: bundle,
    bundleScaleHex: '0x1234',
    localDomain: DOMAIN_TON,
  });

  assert.equal(call.method, 'SccpPauseTokenFromVerifier');
  assert.equal(call.message_id, messageId);
  assert.equal(call.payload_hex, payloadHex);
});

test('rejects a burn bundle that targets the wrong local domain', () => {
  const payload = {
    version: 1,
    source_domain: 0,
    dest_domain: 1,
    nonce: '3',
    sora_asset_id: `0x${'99'.repeat(32)}`,
    amount: '1',
    recipient: `0x${'aa'.repeat(32)}`,
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
  const messageId = ethers.keccak256(
    Buffer.concat([Buffer.from('sccp:burn:v1', 'utf8'), Buffer.from(payloadHex.slice(2), 'hex')]),
  ).toLowerCase();
  const bundle = {
    version: 1,
    commitment_root: `0x${'bb'.repeat(32)}`,
    commitment: {
      version: 1,
      kind: 'Burn',
      target_domain: 1,
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
      buildTonMasterCallFromNexusBundle({
        bundleJson: bundle,
        bundleScaleHex: '0x00',
        localDomain: DOMAIN_TON,
      }),
    /localDomain/,
  );
});

test('rejects a governance bundle with a mismatched message id', () => {
  const bundle = {
    version: 1,
    commitment_root: `0x${'dd'.repeat(32)}`,
    commitment: {
      version: 1,
      kind: 'TokenPause',
      target_domain: DOMAIN_TON,
      message_id: `0x${'ee'.repeat(32)}`,
      payload_hash: `0x${'ff'.repeat(32)}`,
      parliament_certificate_hash: `0x${'11'.repeat(32)}`,
    },
    merkle_proof: { steps: [] },
    payload: {
      Pause: {
        version: 1,
        target_domain: DOMAIN_TON,
        nonce: '5',
        sora_asset_id: `0x${'12'.repeat(32)}`,
      },
    },
    parliament_certificate: '0xbeef',
    finality_proof: '0xfeed',
  };

  assert.throws(
    () =>
      buildTonMasterCallFromNexusBundle({
        bundleJson: bundle,
        bundleScaleHex: '0x1234',
        localDomain: DOMAIN_TON,
      }),
    /commitment\.message_id/,
  );
});
