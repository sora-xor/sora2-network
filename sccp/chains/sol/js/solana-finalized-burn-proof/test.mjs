import assert from "node:assert/strict";

import { PublicKey } from "@solana/web3.js";

import {
  BURN_PAYLOAD_V1_LEN,
  SOLANA_BURN_RECORD_ACCOUNT_DATA_LEN,
  assertCanonicalBurnRecord,
  buildFinalizedBurnProof,
  burnMessageId,
  deriveBurnRecordPda,
  encodeBurnPayloadV1,
} from "./index.js";

function fixedByteArray(length, fill) {
  return Uint8Array.from({ length }, () => fill);
}

function encodeBurnRecordAccountData({
  bump,
  messageId,
  payload,
  sender,
  mint,
  slot,
}) {
  const out = new Uint8Array(SOLANA_BURN_RECORD_ACCOUNT_DATA_LEN);
  out[0] = 1;
  out[1] = bump;
  out.set(messageId, 2);
  out.set(payload, 34);
  out.set(sender, 131);
  out.set(mint, 163);

  let cursor = BigInt(slot);
  for (let i = 0; i < 8; i += 1) {
    out[195 + i] = Number(cursor & 0xffn);
    cursor >>= 8n;
  }
  return out;
}

const routerProgramId = new PublicKey(fixedByteArray(32, 0x14));
const payload = {
  version: 1,
  source_domain: 3,
  dest_domain: 0,
  nonce: 7n,
  sora_asset_id: fixedByteArray(32, 0x55),
  amount: 42n,
  recipient: fixedByteArray(32, 0x77),
};
const payloadBytes = encodeBurnPayloadV1(payload);
assert.equal(payloadBytes.length, BURN_PAYLOAD_V1_LEN);

const messageId = burnMessageId(payloadBytes);
const { burnRecordPda, bump } = deriveBurnRecordPda({
  routerProgramId,
  messageId,
});

const sender = fixedByteArray(32, 0x33);
const mint = fixedByteArray(32, 0x44);
const burnRecordAccountData = encodeBurnRecordAccountData({
  bump,
  messageId,
  payload: payloadBytes,
  sender,
  mint,
  slot: 42n,
});

const canonical = assertCanonicalBurnRecord({
  routerProgramId,
  burnRecordPda,
  burnRecordOwner: routerProgramId,
  burnRecordAccountData,
  expectedMessageId: messageId,
});

assert.deepEqual(Array.from(canonical.burnRecord.messageId), Array.from(messageId));
assert.deepEqual(
  Array.from(canonical.derivedBurnRecordPdaBytes),
  Array.from(burnRecordPda.toBytes()),
);

const burnProof = {
  slot: 42n,
  bankHash: fixedByteArray(32, 0x91),
  accountDeltaRoot: fixedByteArray(32, 0x92),
  parentBankHash: fixedByteArray(32, 0x93),
  blockhash: fixedByteArray(32, 0x94),
  numSigs: 3n,
  accountProof: {
    account: {
      pubkey: burnRecordPda.toBytes(),
      lamports: 1n,
      owner: routerProgramId.toBytes(),
      executable: false,
      rentEpoch: 0n,
      data: burnRecordAccountData,
      writeVersion: 1n,
      slot: 42n,
    },
    merkleProof: {
      path: new Uint8Array(),
      siblings: [],
    },
  },
};

const voteProofs = [
  {
    authorityPubkey: fixedByteArray(32, 0xa1),
    signature: fixedByteArray(64, 0xb2),
    signedMessage: fixedByteArray(12, 0xc3),
    voteSlot: 43n,
    voteBankHash: fixedByteArray(32, 0xd4),
    rootedSlot: 42n,
    slotHashesProof: {
      slot: 43n,
      bankHash: fixedByteArray(32, 0xe5),
      accountDeltaRoot: fixedByteArray(32, 0xe6),
      parentBankHash: fixedByteArray(32, 0xe7),
      blockhash: fixedByteArray(32, 0xe8),
      numSigs: 2n,
      accountProof: {
        account: {
          pubkey: fixedByteArray(32, 0xf1),
          lamports: 1n,
          owner: fixedByteArray(32, 0xf2),
          executable: false,
          rentEpoch: 0n,
          data: fixedByteArray(4, 0xf3),
          writeVersion: 1n,
          slot: 43n,
        },
        merkleProof: {
          path: new Uint8Array(),
          siblings: [],
        },
      },
    },
  },
];

const built = buildFinalizedBurnProof({
  routerProgramId: routerProgramId.toBytes(),
  burnRecordPda: burnRecordPda.toBytes(),
  burnRecordOwner: routerProgramId.toBytes(),
  burnRecordAccountData,
  burnProof,
  voteProofs,
});

assert.deepEqual(Array.from(built.publicInputs.messageId), Array.from(messageId));
assert.equal(built.publicInputs.finalizedSlot, 42n);
assert.equal(built.proofBytes[0], 1);
assert.throws(
  () =>
    buildFinalizedBurnProof({
      routerProgramId: routerProgramId.toBytes(),
      burnRecordPda: burnRecordPda.toBytes(),
      burnRecordOwner: routerProgramId.toBytes(),
      burnRecordAccountData,
      burnProof: { ...burnProof, slot: 41n },
      voteProofs,
    }),
  /finalizedSlot must match burnProof\.slot/,
);

console.log("solana-finalized-burn-proof tests passed");
