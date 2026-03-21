import { expect } from 'chai';
import { Interface, concat, encodeBytes32String, keccak256, toUtf8Bytes } from 'ethers';

import {
  buildOutput,
  decodeBurnPayloadV1,
  EXPORT_SCHEMA,
  LEGACY_SCHEMA,
  selectBurnLog,
} from '../scripts/extract_burn_export.mjs';

function encodeLE(value, width) {
  let v = BigInt(value);
  const out = Buffer.alloc(width);
  for (let i = 0; i < width; i += 1) {
    out[i] = Number(v & 0xffn);
    v >>= 8n;
  }
  return out;
}

function encodeBurnPayload({
  sourceDomain,
  destDomain,
  nonce,
  soraAssetId,
  amount,
  recipient,
}) {
  return `0x${Buffer.concat([
    Buffer.from([1]),
    encodeLE(sourceDomain, 4),
    encodeLE(destDomain, 4),
    encodeLE(nonce, 8),
    Buffer.from(soraAssetId.slice(2), 'hex'),
    encodeLE(amount, 16),
    Buffer.from(recipient.slice(2), 'hex'),
  ]).toString('hex')}`;
}

describe('extract_burn_export', function () {
  const iface = new Interface([
    'event SccpBurned(bytes32 indexed messageId, bytes32 indexed soraAssetId, address indexed sender, uint128 amount, uint32 destDomain, bytes32 recipient, uint64 nonce, bytes payload)',
  ]);

  it('produces a canonical burn export artifact from a valid receipt log', function () {
    const router = '0x1234567890123456789012345678901234567890';
    const sender = '0x9999999999999999999999999999999999999999';
    const soraAssetId = `0x${'11'.repeat(32)}`;
    const recipient = encodeBytes32String('sora-recipient');
    const payloadHex = encodeBurnPayload({
      sourceDomain: 5,
      destDomain: 0,
      nonce: 7,
      soraAssetId,
      amount: 25,
      recipient,
    });
    const messageId = keccak256(concat([toUtf8Bytes('sccp:burn:v1'), payloadHex]));
    const encoded = iface.encodeEventLog(
      iface.getEvent('SccpBurned'),
      [messageId, soraAssetId, sender, 25n, 0, recipient, 7n, payloadHex],
    );

    const receipt = {
      transactionHash: `0x${'aa'.repeat(32)}`,
      blockHash: `0x${'bb'.repeat(32)}`,
      blockNumber: '0x2a',
      status: '0x1',
      logs: [{
        address: router,
        topics: encoded.topics,
        data: encoded.data,
        logIndex: '0x3',
        transactionHash: `0x${'aa'.repeat(32)}`,
        blockHash: `0x${'bb'.repeat(32)}`,
      }],
    };

    const selected = selectBurnLog(receipt, router, null);
    const out = buildOutput(receipt, selected);

    expect(out.artifact_kind).to.equal('canonical_burn_export');
    expect(out.schema).to.equal(EXPORT_SCHEMA);
    expect(out.schema_aliases).to.deep.equal([LEGACY_SCHEMA]);
    expect(out.deprecated_fields).to.deep.equal(['proof_public_inputs']);
    expect(out.message_id).to.equal(messageId.toLowerCase());
    expect(out.payload_hex).to.equal(payloadHex.toLowerCase());
    expect(out.export_surface.source_domain).to.equal(5);
    expect(out.export_surface.dest_domain).to.equal(0);
    expect(out.proof_public_inputs).to.deep.equal(out.export_surface);
  });

  it('rejects a receipt whose event fields do not match the encoded payload', function () {
    const router = '0x1234567890123456789012345678901234567890';
    const sender = '0x9999999999999999999999999999999999999999';
    const soraAssetId = `0x${'11'.repeat(32)}`;
    const recipient = encodeBytes32String('sora-recipient');
    const payloadHex = encodeBurnPayload({
      sourceDomain: 5,
      destDomain: 0,
      nonce: 7,
      soraAssetId,
      amount: 25,
      recipient,
    });
    const messageId = keccak256(concat([toUtf8Bytes('sccp:burn:v1'), payloadHex]));
    const encoded = iface.encodeEventLog(
      iface.getEvent('SccpBurned'),
      [messageId, soraAssetId, sender, 26n, 0, recipient, 7n, payloadHex],
    );

    const receipt = {
      transactionHash: `0x${'aa'.repeat(32)}`,
      blockHash: `0x${'bb'.repeat(32)}`,
      blockNumber: '0x2a',
      logs: [{
        address: router,
        topics: encoded.topics,
        data: encoded.data,
      }],
    };

    const selected = selectBurnLog(receipt, router, null);
    expect(() => buildOutput(receipt, selected)).to.throw('event amount does not match encoded payload');
  });

  it('decodes canonical burn payload bytes with TRON source domain', function () {
    const payloadHex = encodeBurnPayload({
      sourceDomain: 5,
      destDomain: 0,
      nonce: 7,
      soraAssetId: `0x${'11'.repeat(32)}`,
      amount: 25,
      recipient: encodeBytes32String('sora-recipient'),
    });

    expect(decodeBurnPayloadV1(payloadHex)).to.deep.equal({
      version: 1,
      source_domain: 5,
      dest_domain: 0,
      nonce: '7',
      sora_asset_id: `0x${'11'.repeat(32)}`,
      amount: '25',
      recipient: encodeBytes32String('sora-recipient').toLowerCase(),
    });
  });
});
