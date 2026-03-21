import { keccak_256 } from "@noble/hashes/sha3";
import { Connection, PublicKey } from "@solana/web3.js";

export const SCCP_DOMAIN_SORA = 0;
export const SCCP_DOMAIN_ETH = 1;
export const SCCP_DOMAIN_BSC = 2;
export const SCCP_DOMAIN_SOL = 3;
export const SCCP_DOMAIN_TON = 4;
export const SCCP_DOMAIN_TRON = 5;

export const SCCP_MSG_PREFIX_BURN_V1 = "sccp:burn:v1";
export const SCCP_SOL_BURN_PROOF_INPUTS_SCHEMA_V1 = "sccp-sol-burn-proof-inputs/v1";
export const BURN_PAYLOAD_V1_LEN = 97;
export const SOLANA_BURN_RECORD_ACCOUNT_DATA_LEN = 203;
export const SOLANA_FINALIZED_BURN_PROOF_VERSION_V1 = 1;
export const SCCP_SEED_PREFIX = "sccp";
export const SCCP_SEED_BURN = "burn";

const textEncoder = new TextEncoder();

function concatBytes(parts) {
  const size = parts.reduce((sum, part) => sum + part.length, 0);
  const out = new Uint8Array(size);
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.length;
  }
  return out;
}

function asUint8Array(value, name) {
  if (value instanceof Uint8Array) {
    return value;
  }
  if (typeof value === "string") {
    return hexToBytes(value, name);
  }
  if (Array.isArray(value)) {
    return Uint8Array.from(value);
  }
  throw new TypeError(`${name} must be a Uint8Array or byte array`);
}

function asFixedBytes(value, length, name) {
  const out = asUint8Array(value, name);
  if (out.length !== length) {
    throw new RangeError(`${name} must be ${length} bytes`);
  }
  return out;
}

function hexToBytes(value, name) {
  if (typeof value !== "string") {
    throw new TypeError(`${name} must be a hex string`);
  }
  const normalized = value.startsWith("0x") ? value.slice(2) : value;
  if (normalized.length === 0 || normalized.length % 2 !== 0 || !/^[0-9a-fA-F]+$/.test(normalized)) {
    throw new TypeError(`${name} must be a valid hex string`);
  }
  const out = new Uint8Array(normalized.length / 2);
  for (let i = 0; i < out.length; i += 1) {
    out[i] = Number.parseInt(normalized.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

function bytesEqual(left, right) {
  if (left.length !== right.length) {
    return false;
  }
  for (let i = 0; i < left.length; i += 1) {
    if (left[i] !== right[i]) {
      return false;
    }
  }
  return true;
}

function asPublicKey(value, name) {
  if (value instanceof PublicKey) {
    return value;
  }
  if (typeof value !== "string") {
    throw new TypeError(`${name} must be a base58 public key string`);
  }
  return new PublicKey(value);
}

function encodeU32LE(value) {
  if (!Number.isInteger(value) || value < 0 || value > 0xffffffff) {
    throw new RangeError("u32 value out of range");
  }
  const out = new Uint8Array(4);
  new DataView(out.buffer).setUint32(0, value, true);
  return out;
}

function encodeU64LE(value) {
  const normalized = BigInt(value);
  if (normalized < 0n || normalized > 0xffffffffffffffffn) {
    throw new RangeError("u64 value out of range");
  }
  const out = new Uint8Array(8);
  let cursor = normalized;
  for (let i = 0; i < out.length; i += 1) {
    out[i] = Number(cursor & 0xffn);
    cursor >>= 8n;
  }
  return out;
}

function decodeU64LE(bytes) {
  const fixed = asFixedBytes(bytes, 8, "slot");
  let out = 0n;
  for (let i = fixed.length - 1; i >= 0; i -= 1) {
    out = (out << 8n) | BigInt(fixed[i]);
  }
  return out;
}

function encodeU128LE(value) {
  const normalized = BigInt(value);
  if (normalized < 0n || normalized > ((1n << 128n) - 1n)) {
    throw new RangeError("u128 value out of range");
  }
  const out = new Uint8Array(16);
  let cursor = normalized;
  for (let i = 0; i < out.length; i += 1) {
    out[i] = Number(cursor & 0xffn);
    cursor >>= 8n;
  }
  return out;
}

function encodeCompactU32(length) {
  if (!Number.isInteger(length) || length < 0 || length >= 1 << 30) {
    throw new RangeError("compact SCALE length out of range");
  }
  if (length < 1 << 6) {
    return Uint8Array.of(length << 2);
  }
  if (length < 1 << 14) {
    const value = (length << 2) | 0x01;
    return Uint8Array.of(value & 0xff, (value >> 8) & 0xff);
  }
  const value = (length << 2) | 0x02;
  return Uint8Array.of(
    value & 0xff,
    (value >> 8) & 0xff,
    (value >> 16) & 0xff,
    (value >> 24) & 0xff,
  );
}

export function encodeBurnPayloadV1(payload) {
  if (payload?.version === undefined) {
    throw new TypeError("payload.version is required");
  }
  const recipient = asFixedBytes(payload.recipient, 32, "payload.recipient");
  const soraAssetId = asFixedBytes(payload.sora_asset_id, 32, "payload.sora_asset_id");
  if (!Number.isInteger(payload.version) || payload.version < 0 || payload.version > 0xff) {
    throw new RangeError("payload.version must fit in u8");
  }
  return concatBytes([
    Uint8Array.of(payload.version),
    encodeU32LE(payload.source_domain),
    encodeU32LE(payload.dest_domain),
    encodeU64LE(payload.nonce),
    soraAssetId,
    encodeU128LE(payload.amount),
    recipient,
  ]);
}

export function burnMessageId(payloadOrEncodedBytes) {
  const payloadBytes =
    payloadOrEncodedBytes instanceof Uint8Array
      ? asFixedBytes(payloadOrEncodedBytes, BURN_PAYLOAD_V1_LEN, "payload bytes")
      : encodeBurnPayloadV1(payloadOrEncodedBytes);
  return Uint8Array.from(
    keccak_256(concatBytes([textEncoder.encode(SCCP_MSG_PREFIX_BURN_V1), payloadBytes])),
  );
}

export function decodeBurnPayloadV1(payload) {
  const bytes = asFixedBytes(payload, BURN_PAYLOAD_V1_LEN, "payload");
  return {
    version: bytes[0],
    source_domain: new DataView(bytes.buffer, bytes.byteOffset + 1, 4).getUint32(0, true),
    dest_domain: new DataView(bytes.buffer, bytes.byteOffset + 5, 4).getUint32(0, true),
    nonce: decodeU64LE(bytes.slice(9, 17)),
    sora_asset_id: bytes.slice(17, 49),
    amount:
      decodeU64LE(bytes.slice(49, 57)) |
      (decodeU64LE(bytes.slice(57, 65)) << 64n),
    recipient: bytes.slice(65, 97),
  };
}

export function decodeSolanaBurnRecordAccountV1(accountData) {
  const bytes = asFixedBytes(
    accountData,
    SOLANA_BURN_RECORD_ACCOUNT_DATA_LEN,
    "accountData",
  );
  return {
    version: bytes[0],
    bump: bytes[1],
    messageId: bytes.slice(2, 34),
    payload: bytes.slice(34, 131),
    sender: bytes.slice(131, 163),
    mint: bytes.slice(163, 195),
    slot: decodeU64LE(bytes.slice(195, 203)),
    accountData: bytes,
  };
}

export function solanaBurnRecordDataHash(accountData) {
  return Uint8Array.from(
    keccak_256(
      asFixedBytes(accountData, SOLANA_BURN_RECORD_ACCOUNT_DATA_LEN, "accountData"),
    ),
  );
}

export function deriveBurnRecordPda({ routerProgramId, messageId }) {
  const programId = asPublicKey(routerProgramId, "routerProgramId");
  const messageIdBytes = asFixedBytes(messageId, 32, "messageId");
  const [burnRecordPda, bump] = PublicKey.findProgramAddressSync(
    [
      textEncoder.encode(SCCP_SEED_PREFIX),
      textEncoder.encode(SCCP_SEED_BURN),
      messageIdBytes,
    ],
    programId,
  );
  return {
    burnRecordPda,
    burnRecordPdaBytes: burnRecordPda.toBytes(),
    bump,
  };
}

export function assertCanonicalBurnRecord({
  routerProgramId,
  burnRecordPda,
  burnRecordOwner,
  burnRecordAccountData,
  expectedMessageId,
}) {
  const record = decodeSolanaBurnRecordAccountV1(burnRecordAccountData);
  const routerProgramIdBytes = asPublicKey(routerProgramId, "routerProgramId").toBytes();
  const burnRecordPdaBytes =
    burnRecordPda instanceof PublicKey
      ? burnRecordPda.toBytes()
      : asFixedBytes(burnRecordPda, 32, "burnRecordPda");
  const burnRecordOwnerBytes =
    burnRecordOwner instanceof PublicKey
      ? burnRecordOwner.toBytes()
      : asFixedBytes(burnRecordOwner, 32, "burnRecordOwner");
  const expectedMessage = asFixedBytes(expectedMessageId, 32, "expectedMessageId");
  const derived = deriveBurnRecordPda({
    routerProgramId,
    messageId: expectedMessage,
  });
  const recomputedMessageId = burnMessageId(record.payload);

  if (!bytesEqual(record.messageId, expectedMessage)) {
    throw new Error("burn record messageId does not match the requested messageId");
  }
  if (!bytesEqual(recomputedMessageId, expectedMessage)) {
    throw new Error("burn record payload does not hash to the requested messageId");
  }
  if (!bytesEqual(burnRecordPdaBytes, derived.burnRecordPdaBytes)) {
    throw new Error("burn record PDA does not match the canonical SCCP PDA derivation");
  }
  if (!bytesEqual(burnRecordOwnerBytes, routerProgramIdBytes)) {
    throw new Error("burn record owner does not match the configured SCCP router program id");
  }

  return {
    burnRecord: record,
    derivedBurnRecordPda: derived.burnRecordPda,
    derivedBurnRecordPdaBytes: derived.burnRecordPdaBytes,
  };
}

export function extractBurnProofInputs({
  routerProgramId,
  burnRecordPda,
  burnRecordOwner,
  burnRecordAccountData,
}) {
  const burnRecord = decodeSolanaBurnRecordAccountV1(burnRecordAccountData);
  return {
    schema: SCCP_SOL_BURN_PROOF_INPUTS_SCHEMA_V1,
    version: SOLANA_FINALIZED_BURN_PROOF_VERSION_V1,
    messageId: burnRecord.messageId,
    finalizedSlot: burnRecord.slot,
    routerProgramId: asFixedBytes(routerProgramId, 32, "routerProgramId"),
    burnRecordPda: asFixedBytes(burnRecordPda, 32, "burnRecordPda"),
    burnRecordOwner: asFixedBytes(burnRecordOwner, 32, "burnRecordOwner"),
    burnRecordDataHash: solanaBurnRecordDataHash(burnRecord.accountData),
    burnRecordPayload: burnRecord.payload,
    burnRecordSender: burnRecord.sender,
    burnRecordMint: burnRecord.mint,
  };
}

export async function fetchFinalizedBurnRecord({
  rpcEndpoint,
  routerProgramId,
  messageId,
}) {
  if (typeof rpcEndpoint !== "string" || rpcEndpoint.length === 0) {
    throw new TypeError("rpcEndpoint must be a non-empty string");
  }

  const programId = asPublicKey(routerProgramId, "routerProgramId");
  const expectedMessageId = asFixedBytes(messageId, 32, "messageId");
  const { burnRecordPda } = deriveBurnRecordPda({
    routerProgramId: programId,
    messageId: expectedMessageId,
  });

  const connection = new Connection(rpcEndpoint, "finalized");
  const { context, value } = await connection.getAccountInfoAndContext(
    burnRecordPda,
    {
      commitment: "finalized",
      encoding: "base64",
    },
  );
  if (value === null) {
    throw new Error("finalized burn record account was not found");
  }

  const burnRecordOwner = value.owner.toBytes();
  const burnRecordAccountData =
    value.data instanceof Uint8Array ? value.data : new Uint8Array(value.data);
  const validated = assertCanonicalBurnRecord({
    routerProgramId: programId,
    burnRecordPda,
    burnRecordOwner,
    burnRecordAccountData,
    expectedMessageId,
  });

  return {
    rpcContextSlot: BigInt(context.slot),
    routerProgramId: programId.toBytes(),
    burnRecordPda: burnRecordPda.toBytes(),
    burnRecordOwner,
    burnRecordAccountData,
    burnRecord: validated.burnRecord,
  };
}

export async function fetchFinalizedBurnProofInputs({
  rpcEndpoint,
  routerProgramId,
  messageId,
}) {
  const witness = await fetchFinalizedBurnRecord({
    rpcEndpoint,
    routerProgramId,
    messageId,
  });
  return {
    ...witness,
    publicInputs: extractBurnProofInputs({
      routerProgramId: witness.routerProgramId,
      burnRecordPda: witness.burnRecordPda,
      burnRecordOwner: witness.burnRecordOwner,
      burnRecordAccountData: witness.burnRecordAccountData,
    }),
  };
}

function encodeBool(value, name) {
  if (typeof value !== "boolean") {
    throw new TypeError(`${name} must be a boolean`);
  }
  return Uint8Array.of(value ? 1 : 0);
}

function encodeScaleBytes(value, name) {
  const bytes = asUint8Array(value, name);
  return concatBytes([encodeCompactU32(bytes.length), bytes]);
}

function encodeScaleVec(values, encoder, name) {
  if (!Array.isArray(values)) {
    throw new TypeError(`${name} must be an array`);
  }
  return concatBytes([
    encodeCompactU32(values.length),
    ...values.map((value, index) => encoder(value, `${name}[${index}]`)),
  ]);
}

function encodeOptionU64(value, name) {
  if (value === null || value === undefined) {
    return Uint8Array.of(0);
  }
  return concatBytes([Uint8Array.of(1), encodeU64LE(value)]);
}

function encodeSolanaMerkleProofV1(proof, name) {
  if (proof === null || typeof proof !== "object") {
    throw new TypeError(`${name} must be an object`);
  }
  return concatBytes([
    encodeScaleBytes(asUint8Array(proof.path ?? [], `${name}.path`), `${name}.path`),
    encodeScaleVec(
      proof.siblings ?? [],
      (siblings, levelName) =>
        encodeScaleVec(
          siblings,
          (sibling, siblingName) => asFixedBytes(sibling, 32, siblingName),
          levelName,
        ),
      `${name}.siblings`,
    ),
  ]);
}

function encodeSolanaAccountInfoV1(account, name) {
  if (account === null || typeof account !== "object") {
    throw new TypeError(`${name} must be an object`);
  }
  return concatBytes([
    asFixedBytes(account.pubkey, 32, `${name}.pubkey`),
    encodeU64LE(account.lamports),
    asFixedBytes(account.owner, 32, `${name}.owner`),
    encodeBool(account.executable, `${name}.executable`),
    encodeU64LE(account.rentEpoch),
    encodeScaleBytes(account.data ?? new Uint8Array(), `${name}.data`),
    encodeU64LE(account.writeVersion),
    encodeU64LE(account.slot),
  ]);
}

function encodeSolanaAccountDeltaProofV1(proof, name) {
  if (proof === null || typeof proof !== "object") {
    throw new TypeError(`${name} must be an object`);
  }
  return concatBytes([
    encodeSolanaAccountInfoV1(proof.account, `${name}.account`),
    encodeSolanaMerkleProofV1(proof.merkleProof, `${name}.merkleProof`),
  ]);
}

function encodeSolanaBankHashProofV1(proof, name) {
  if (proof === null || typeof proof !== "object") {
    throw new TypeError(`${name} must be an object`);
  }
  return concatBytes([
    encodeU64LE(proof.slot),
    asFixedBytes(proof.bankHash, 32, `${name}.bankHash`),
    asFixedBytes(proof.accountDeltaRoot, 32, `${name}.accountDeltaRoot`),
    asFixedBytes(proof.parentBankHash, 32, `${name}.parentBankHash`),
    asFixedBytes(proof.blockhash, 32, `${name}.blockhash`),
    encodeU64LE(proof.numSigs),
    encodeSolanaAccountDeltaProofV1(proof.accountProof, `${name}.accountProof`),
  ]);
}

function encodeSolanaVoteProofV1(proof, name) {
  if (proof === null || typeof proof !== "object") {
    throw new TypeError(`${name} must be an object`);
  }
  return concatBytes([
    asFixedBytes(proof.authorityPubkey, 32, `${name}.authorityPubkey`),
    asFixedBytes(proof.signature, 64, `${name}.signature`),
    encodeScaleBytes(proof.signedMessage, `${name}.signedMessage`),
    encodeU64LE(proof.voteSlot),
    asFixedBytes(proof.voteBankHash, 32, `${name}.voteBankHash`),
    encodeOptionU64(proof.rootedSlot, `${name}.rootedSlot`),
    encodeSolanaBankHashProofV1(proof.slotHashesProof, `${name}.slotHashesProof`),
  ]);
}

function normalizeFinalizedBankHash(publicInputs, burnProof) {
  const fromBurnProof = asFixedBytes(burnProof.bankHash, 32, "burnProof.bankHash");
  if (publicInputs.finalizedBankHash === undefined) {
    return fromBurnProof;
  }
  const fromPublicInputs = asFixedBytes(
    publicInputs.finalizedBankHash,
    32,
    "publicInputs.finalizedBankHash",
  );
  if (!bytesEqual(fromPublicInputs, fromBurnProof)) {
    throw new Error("publicInputs.finalizedBankHash must match burnProof.bankHash");
  }
  return fromPublicInputs;
}

export function encodeSolanaFinalizedBurnProofV1({
  publicInputs,
  burnProof,
  voteProofs,
}) {
  if (publicInputs === null || typeof publicInputs !== "object") {
    throw new TypeError("publicInputs must be an object");
  }
  if (burnProof === null || typeof burnProof !== "object") {
    throw new TypeError("burnProof must be an object");
  }
  if (!Array.isArray(voteProofs) || voteProofs.length === 0) {
    throw new TypeError("voteProofs must be a non-empty array");
  }
  if (BigInt(publicInputs.finalizedSlot) !== BigInt(burnProof.slot)) {
    throw new Error("publicInputs.finalizedSlot must match burnProof.slot");
  }
  const finalizedBankHash = normalizeFinalizedBankHash(publicInputs, burnProof);
  return concatBytes([
    Uint8Array.of(SOLANA_FINALIZED_BURN_PROOF_VERSION_V1),
    asFixedBytes(publicInputs.messageId, 32, "publicInputs.messageId"),
    encodeU64LE(publicInputs.finalizedSlot),
    finalizedBankHash,
    asFixedBytes(publicInputs.routerProgramId, 32, "publicInputs.routerProgramId"),
    asFixedBytes(publicInputs.burnRecordPda, 32, "publicInputs.burnRecordPda"),
    asFixedBytes(publicInputs.burnRecordOwner, 32, "publicInputs.burnRecordOwner"),
    asFixedBytes(publicInputs.burnRecordDataHash, 32, "publicInputs.burnRecordDataHash"),
    encodeSolanaBankHashProofV1(burnProof, "burnProof"),
    encodeScaleVec(voteProofs, encodeSolanaVoteProofV1, "voteProofs"),
  ]);
}

export function buildFinalizedBurnProof({
  routerProgramId,
  burnRecordPda,
  burnRecordOwner,
  burnRecordAccountData,
  burnProof,
  voteProofs,
  finalizedBankHash,
}) {
  const publicInputs = extractBurnProofInputs({
    routerProgramId,
    burnRecordPda,
    burnRecordOwner,
    burnRecordAccountData,
  });
  if (finalizedBankHash !== undefined) {
    publicInputs.finalizedBankHash = asFixedBytes(
      finalizedBankHash,
      32,
      "finalizedBankHash",
    );
  }
  return {
    publicInputs,
    proofBytes: encodeSolanaFinalizedBurnProofV1({
      publicInputs,
      burnProof,
      voteProofs,
    }),
  };
}
