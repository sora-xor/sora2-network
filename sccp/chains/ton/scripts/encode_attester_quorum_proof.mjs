// Encode SCCP AttesterQuorum proof bytes for SORA `pallet-sccp`.
//
// Proof format:
// - version: u8 = 1
// - signatures: SCALE(Vec<[u8;65]>) where each signature is r||s||v (v may be 0/1 or 27/28)
//
// Signatures are over:
//   attestHash = keccak256("sccp:attest:v1" || messageId)
// (raw digest signature, no EIP-191 prefix)
//
// Usage:
//   node scripts/encode_attester_quorum_proof.mjs --message-id 0x.. --sig 0x.. --sig 0x..
//   node scripts/encode_attester_quorum_proof.mjs --message-id 0x.. --privkey 0x.. --privkey 0x..
//
import { Wallet, keccak256, concat, getBytes, toUtf8Bytes } from "ethers";
import { Buffer } from "node:buffer";

function usageAndExit(code) {
  const msg = [
    "Usage:",
    "  node scripts/encode_attester_quorum_proof.mjs --message-id 0x<32-byte> --sig 0x<65-byte> [--sig ...]",
    "  node scripts/encode_attester_quorum_proof.mjs --message-id 0x<32-byte> --privkey 0x<32-byte> [--privkey ...]",
  ].join("\n");
  // eslint-disable-next-line no-console
  console.error(msg);
  process.exit(code);
}

function parseArgs(argv) {
  let messageId = null;
  const sigs = [];
  const privkeys = [];
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--message-id") {
      const next = argv[i + 1];
      if (next === undefined || next.startsWith("--")) usageAndExit(2);
      messageId = next;
      i += 1;
      continue;
    }
    if (a === "--sig") {
      const next = argv[i + 1];
      if (next === undefined || next.startsWith("--")) usageAndExit(2);
      sigs.push(next);
      i += 1;
      continue;
    }
    if (a === "--privkey") {
      const next = argv[i + 1];
      if (next === undefined || next.startsWith("--")) usageAndExit(2);
      privkeys.push(next);
      i += 1;
      continue;
    }
    usageAndExit(2);
  }
  return { messageId, sigs, privkeys };
}

function normalizeHex(hex, label) {
  if (typeof hex !== "string" || !hex.startsWith("0x")) {
    throw new Error(`${label} must be 0x-prefixed hex`);
  }
  const raw = hex.slice(2);
  if (raw.length % 2 !== 0) {
    throw new Error(`${label} must have an even number of hex digits`);
  }
  if (!/^[0-9a-fA-F]*$/.test(raw)) {
    throw new Error(`${label} must contain only hex digits`);
  }
  return raw.toLowerCase();
}

function assertHexBytes(hex, expectedLen, label) {
  const raw = normalizeHex(hex, label);
  const b = getBytes(`0x${raw}`);
  if (b.length !== expectedLen) {
    throw new Error(`${label} must be ${expectedLen} bytes, got ${b.length}`);
  }
  return b;
}

// SCALE compact-encode a u32 (supports values < 2^30).
function scaleCompactU32(n) {
  if (!Number.isInteger(n) || n < 0) throw new Error("length must be a non-negative integer");
  if (n < 1 << 6) {
    return Uint8Array.from([(n << 2) | 0]);
  }
  if (n < 1 << 14) {
    const v = (n << 2) | 1;
    return Uint8Array.from([v & 0xff, (v >> 8) & 0xff]);
  }
  if (n < 1 << 30) {
    const v = (n << 2) | 2;
    return Uint8Array.from([v & 0xff, (v >> 8) & 0xff, (v >> 16) & 0xff, (v >> 24) & 0xff]);
  }
  throw new Error("length too large for compact u32 encoding");
}

function hexFromBytes(bytes) {
  return "0x" + Buffer.from(bytes).toString("hex");
}

function main() {
  const { messageId, sigs, privkeys } = parseArgs(process.argv);
  if (!messageId) usageAndExit(2);
  if (sigs.length > 0 && privkeys.length > 0) {
    throw new Error("use either --sig or --privkey, not both");
  }
  if (sigs.length === 0 && privkeys.length === 0) {
    throw new Error("at least one --sig or --privkey is required");
  }

  const messageIdBytes = assertHexBytes(messageId, 32, "messageId");
  const attestHash = keccak256(concat([toUtf8Bytes("sccp:attest:v1"), messageIdBytes]));

  const outSigs = [];
  const signers = [];
  if (privkeys.length > 0) {
    for (const k of privkeys) {
      const pk = typeof k === "string" && k.startsWith("0x") ? k : `0x${k}`;
      const normalizedPk = hexFromBytes(assertHexBytes(pk, 32, "private key"));
      const w = new Wallet(normalizedPk);
      const sig = w.signingKey.sign(attestHash).serialized; // 65-byte r||s||v (v=27/28)
      outSigs.push(assertHexBytes(sig, 65, "signature"));
      signers.push(w.address);
    }
  } else {
    for (const s of sigs) {
      outSigs.push(assertHexBytes(s, 65, "signature"));
    }
  }

  const len = scaleCompactU32(outSigs.length);
  const proofBytes = concat([Uint8Array.from([1]), len, ...outSigs]);
  const proofHex = hexFromBytes(proofBytes);
  const proofBase64 = Buffer.from(proofBytes).toString("base64");

  const out = {
    message_id: messageId,
    attest_hash: attestHash,
    signers: signers.length > 0 ? signers : undefined,
    proof_hex: proofHex,
    proof_base64: proofBase64,
  };

  // eslint-disable-next-line no-console
  console.log(JSON.stringify(out, null, 2));
}

try {
  main();
} catch (e) {
  // eslint-disable-next-line no-console
  console.error(`error: ${e.message}`);
  process.exit(1);
}
