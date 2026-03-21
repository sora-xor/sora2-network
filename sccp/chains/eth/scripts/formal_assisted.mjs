#!/usr/bin/env node

import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import { loadEthers } from './load_ethers.mjs';

const { Wallet, concat, getBytes, keccak256, toUtf8Bytes } =
  await loadEthers(resolve(import.meta.dirname, '..'));

const ATTEST_PREFIX = toUtf8Bytes('sccp:attest:v1');
const BURN_PREFIX = toUtf8Bytes('sccp:burn:v1');

function parseProfile(argv) {
  let profile = 'full';
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '--profile') {
      const v = argv[i + 1];
      if (!v) {
        throw new Error('missing value for --profile (expected fast|full)');
      }
      profile = v;
      i += 1;
      continue;
    }
    throw new Error(`unknown argument: ${a}`);
  }
  return profile;
}

function scaleCompactU32(n) {
  if (!Number.isInteger(n) || n < 0) {
    throw new Error('length must be a non-negative integer');
  }
  if (n < (1 << 6)) {
    return Uint8Array.from([(n << 2) | 0]);
  }
  if (n < (1 << 14)) {
    const v = (n << 2) | 1;
    return Uint8Array.from([v & 0xff, (v >> 8) & 0xff]);
  }
  if (n < (1 << 30)) {
    const v = (n << 2) | 2;
    return Uint8Array.from([v & 0xff, (v >> 8) & 0xff, (v >> 16) & 0xff, (v >> 24) & 0xff]);
  }
  throw new Error('length too large for compact u32 encoding');
}

function decodeCompactU32(bytes) {
  if (!(bytes instanceof Uint8Array) || bytes.length === 0) {
    throw new Error('compact bytes must be non-empty Uint8Array');
  }
  const mode = bytes[0] & 0b11;
  if (mode === 0) {
    return { value: bytes[0] >> 2, used: 1 };
  }
  if (mode === 1) {
    if (bytes.length < 2) {
      throw new Error('compact-2 decode requires 2 bytes');
    }
    const v = bytes[0] | (bytes[1] << 8);
    return { value: v >> 2, used: 2 };
  }
  if (mode === 2) {
    if (bytes.length < 4) {
      throw new Error('compact-4 decode requires 4 bytes');
    }
    const v = bytes[0] | (bytes[1] << 8) | (bytes[2] << 16) | (bytes[3] << 24);
    return { value: (v >>> 2), used: 4 };
  }
  throw new Error('big-integer compact mode is unsupported for this assistant check');
}

function assertHexBytes(hex, expectedLen, label) {
  if (typeof hex !== 'string' || !hex.startsWith('0x')) {
    throw new Error(`${label} must be 0x-prefixed hex`);
  }
  const b = getBytes(hex);
  if (b.length !== expectedLen) {
    throw new Error(`${label} must be ${expectedLen} bytes, got ${b.length}`);
  }
  return b;
}

function attestHash(messageIdHex) {
  const messageIdBytes = assertHexBytes(messageIdHex, 32, 'messageId');
  return keccak256(concat([ATTEST_PREFIX, messageIdBytes]));
}

function burnStyleHash(bytes) {
  return keccak256(concat([BURN_PREFIX, bytes]));
}

function encodeProof(signatures) {
  const buffers = [Buffer.from([1]), Buffer.from(scaleCompactU32(signatures.length))];
  for (const sig of signatures) {
    buffers.push(Buffer.from(sig));
  }
  return Buffer.concat(buffers);
}

function rng(seed) {
  let state = BigInt(seed);
  return () => {
    state ^= state >> 12n;
    state ^= state << 25n;
    state ^= state >> 27n;
    state = (state * 0x2545f4914f6cdd1dn) & ((1n << 64n) - 1n);
    return Number(state & 0xffn);
  };
}

function makeMessageId(seed) {
  const next = rng(seed);
  const out = new Uint8Array(32);
  for (let i = 0; i < out.length; i += 1) {
    out[i] = next();
  }
  return `0x${Buffer.from(out).toString('hex')}`;
}

function runScaleCompactChecks() {
  const boundaries = [0, 1, 63, 64, 65, 255, 16383, 16384, (1 << 30) - 1];
  for (const n of boundaries) {
    const enc = scaleCompactU32(n);
    const dec = decodeCompactU32(enc);
    assert.equal(dec.value, n, `compact roundtrip failed for ${n}`);
    assert.equal(dec.used, enc.length, `compact used mismatch for ${n}`);
  }

  assert.throws(() => scaleCompactU32(-1), /non-negative/);
  assert.throws(() => scaleCompactU32(1 << 30), /too large/);
  assert.throws(() => decodeCompactU32(Uint8Array.from([0x01])), /requires 2 bytes/);
  assert.throws(() => decodeCompactU32(Uint8Array.from([0x02, 0x00])), /requires 4 bytes/);
}

function runCompactEncodingLengthModeChecks() {
  const cases = [
    { n: 0, expectedLen: 1 },
    { n: 63, expectedLen: 1 },
    { n: 64, expectedLen: 2 },
    { n: 16383, expectedLen: 2 },
    { n: 16384, expectedLen: 4 },
    { n: (1 << 30) - 1, expectedLen: 4 },
  ];

  for (const { n, expectedLen } of cases) {
    const enc = scaleCompactU32(n);
    assert.equal(enc.length, expectedLen, `compact length mode mismatch for ${n}`);
  }
}

function runCompactModeBitsChecks() {
  const encMode0 = scaleCompactU32(63);
  const encMode1 = scaleCompactU32(64);
  const encMode2 = scaleCompactU32(16384);

  assert.equal(encMode0[0] & 0b11, 0, 'compact mode for <2^6 values must be 0');
  assert.equal(encMode1[0] & 0b11, 1, 'compact mode for <2^14 values must be 1');
  assert.equal(encMode2[0] & 0b11, 2, 'compact mode for <2^30 values must be 2');
}

function runCompactBoundaryTransitionChecks() {
  const transitions = [
    { lower: 63, upper: 64, lowerLen: 1, upperLen: 2, lowerMode: 0, upperMode: 1 },
    { lower: 16383, upper: 16384, lowerLen: 2, upperLen: 4, lowerMode: 1, upperMode: 2 },
  ];

  for (const t of transitions) {
    const lowerEnc = scaleCompactU32(t.lower);
    const upperEnc = scaleCompactU32(t.upper);
    const lowerDec = decodeCompactU32(lowerEnc);
    const upperDec = decodeCompactU32(upperEnc);

    assert.equal(lowerDec.value, t.lower, `compact lower bound roundtrip failed for ${t.lower}`);
    assert.equal(upperDec.value, t.upper, `compact upper bound roundtrip failed for ${t.upper}`);
    assert.equal(lowerEnc.length, t.lowerLen, `compact lower length mismatch for ${t.lower}`);
    assert.equal(upperEnc.length, t.upperLen, `compact upper length mismatch for ${t.upper}`);
    assert.equal(lowerEnc[0] & 0b11, t.lowerMode, `compact lower mode mismatch for ${t.lower}`);
    assert.equal(upperEnc[0] & 0b11, t.upperMode, `compact upper mode mismatch for ${t.upper}`);
  }
}

function runCompactKnownBytePatternChecks() {
  const cases = [
    { n: 0, hex: '00' },
    { n: 1, hex: '04' },
    { n: 63, hex: 'fc' },
    { n: 64, hex: '0101' },
    { n: 16383, hex: 'fdff' },
    { n: 16384, hex: '02000100' },
  ];

  for (const { n, hex } of cases) {
    const enc = Buffer.from(scaleCompactU32(n)).toString('hex');
    assert.equal(enc, hex, `compact canonical bytes mismatch for ${n}`);
  }
}

function runCompactSequentialRoundtripChecks() {
  for (let n = 0; n <= 1024; n += 1) {
    const enc = scaleCompactU32(n);
    const dec = decodeCompactU32(enc);
    assert.equal(dec.value, n, `compact sequential roundtrip failed for ${n}`);
    assert.equal(dec.used, enc.length, `compact used-length mismatch for ${n}`);
  }
}

function runCompactEncodedLengthMonotonicChecks() {
  let prevLen = 0;
  for (let n = 0; n <= 20000; n += 1) {
    const enc = scaleCompactU32(n);
    assert(enc.length >= prevLen, `compact encoded length must be monotonic at ${n}`);
    if (n < 64) {
      assert.equal(enc.length, 1, `compact encoded length must be 1 for ${n}`);
    } else if (n < 16384) {
      assert.equal(enc.length, 2, `compact encoded length must be 2 for ${n}`);
    } else {
      assert.equal(enc.length, 4, `compact encoded length must be 4 for ${n}`);
    }
    prevLen = enc.length;
  }
}

function runCompactModeWindowCoverageChecks() {
  const modes = new Set();
  for (let n = 0; n <= 20000; n += 1) {
    const enc = scaleCompactU32(n);
    modes.add(enc[0] & 0b11);
  }
  assert(modes.has(0), 'compact mode-0 must appear in bounded window');
  assert(modes.has(1), 'compact mode-1 must appear in bounded window');
  assert(modes.has(2), 'compact mode-2 must appear in bounded window');
}

function runCompactDecoderTrailingBytesChecks() {
  const enc1 = scaleCompactU32(63);
  const dec1 = decodeCompactU32(Uint8Array.from([...enc1, 0xaa, 0xbb]));
  assert.equal(dec1.value, 63, 'compact mode-1 value should decode with trailing bytes present');
  assert.equal(dec1.used, enc1.length, 'compact mode-1 used length should exclude trailing bytes');

  const enc2 = scaleCompactU32(300);
  const dec2 = decodeCompactU32(Uint8Array.from([...enc2, 0xcc]));
  assert.equal(dec2.value, 300, 'compact mode-2 value should decode with trailing bytes present');
  assert.equal(dec2.used, enc2.length, 'compact mode-2 used length should exclude trailing bytes');

  const enc4 = scaleCompactU32(70_000);
  const dec4 = decodeCompactU32(Uint8Array.from([...enc4, 0xdd]));
  assert.equal(dec4.value, 70_000, 'compact mode-4 value should decode with trailing bytes present');
  assert.equal(dec4.used, enc4.length, 'compact mode-4 used length should exclude trailing bytes');
}

function runCompactDecoderFailClosedChecks() {
  assert.throws(() => decodeCompactU32(new Uint8Array()), /non-empty Uint8Array/);
  assert.throws(() => decodeCompactU32([0x00]), /non-empty Uint8Array/);
  assert.throws(() => decodeCompactU32(Uint8Array.from([0x03])), /big-integer compact mode/);
}

function runHexInputValidationChecks() {
  assert.throws(() => assertHexBytes('11', 32, 'messageId'), /0x-prefixed hex/);
  assert.throws(() => assertHexBytes('0x11', 32, 'messageId'), /must be 32 bytes/);
  assert.throws(() => attestHash('0x11'), /must be 32 bytes/);
}

function runMessageIdSeedDeterminismChecks() {
  const a = makeMessageId(123n);
  const b = makeMessageId(123n);
  const c = makeMessageId(124n);
  assert.equal(a, b, 'same seed must produce identical synthetic messageId');
  assert.notEqual(a, c, 'different seeds should produce different synthetic messageIds');
}

function runPrefixLiteralChecks() {
  assert.equal(Buffer.from(BURN_PREFIX).toString('utf8'), 'sccp:burn:v1');
  assert.equal(Buffer.from(ATTEST_PREFIX).toString('utf8'), 'sccp:attest:v1');
}

function runAttestHashChecks() {
  const seen = new Set();
  const seenAttests = new Set();
  for (let i = 0; i < 64; i += 1) {
    const msg = makeMessageId(0x1234_5678n + BigInt(i));
    seen.add(msg);
    const a = attestHash(msg);
    const b = attestHash(msg);
    seenAttests.add(a);
    assert.equal(a, b, 'attest hash must be stable for same input');

    const msgBytes = getBytes(msg);
    const alt = new Uint8Array(msgBytes);
    alt[31] ^= 0x01;
    const changed = attestHash(`0x${Buffer.from(alt).toString('hex')}`);
    assert.notEqual(a, changed, 'attest hash must change with messageId byte flips');

    const burn = burnStyleHash(msgBytes);
    assert.notEqual(a, burn, 'attest hash must remain domain-separated from burn prefix');
  }
  assert.equal(seen.size, 64, 'bounded message-id sample should not collide');
  assert.equal(seenAttests.size, 64, 'bounded attest-hash sample should not collide');
}

function runAttestHashSingleByteSensitivityChecks() {
  const baseBytes = new Uint8Array(32);
  const baseHex = `0x${Buffer.from(baseBytes).toString('hex')}`;
  const baseDigest = attestHash(baseHex);
  const seen = new Set([baseDigest]);

  for (let i = 0; i < 32; i += 1) {
    const alt = new Uint8Array(baseBytes);
    alt[i] = 1;
    const digest = attestHash(`0x${Buffer.from(alt).toString('hex')}`);
    assert.notEqual(digest, baseDigest, `attest hash must change for byte index ${i}`);
    assert(!seen.has(digest), `attest hash collision for byte index ${i}`);
    seen.add(digest);
  }
}

function runAttestHashSequentialWindowChecks() {
  const seen = new Set();
  for (let i = 0; i < 256; i += 1) {
    const msgBytes = new Uint8Array(32);
    msgBytes[30] = (i >> 8) & 0xff;
    msgBytes[31] = i & 0xff;
    const msgHex = `0x${Buffer.from(msgBytes).toString('hex')}`;
    const digestA = attestHash(msgHex);
    const digestB = attestHash(msgHex);
    assert.equal(digestA, digestB, `attest hash must be stable within sequential window at ${i}`);
    assert(!seen.has(digestA), `attest hash collision within sequential window at ${i}`);
    seen.add(digestA);
  }
  assert.equal(seen.size, 256, 'attest hash sequential window must be collision-free');
}

function runAttestHashMultiByteSensitivityChecks() {
  const baseBytes = new Uint8Array(32);
  const baseHex = `0x${Buffer.from(baseBytes).toString('hex')}`;
  const baseDigest = attestHash(baseHex);
  const seen = new Set();

  for (let i = 0; i < 32; i += 1) {
    const alt = new Uint8Array(baseBytes);
    alt[i] = 1;
    alt[(i + 7) % 32] = 1;
    const digest = attestHash(`0x${Buffer.from(alt).toString('hex')}`);
    assert.notEqual(digest, baseDigest, `attest hash must change for multi-byte flip index ${i}`);
    assert(!seen.has(digest), `attest hash collision for multi-byte flip index ${i}`);
    seen.add(digest);
  }
  assert.equal(seen.size, 32, 'multi-byte flip window should produce unique attest hashes');
}

function makeSampleSignature(seed) {
  const digest = attestHash(makeMessageId(seed));
  return getBytes(new Wallet('0x' + '11'.repeat(32)).signingKey.sign(digest).serialized);
}

function runProofLayoutChecks() {
  const sampleSig = makeSampleSignature(99n);
  assert.equal(sampleSig.length, 65, 'wallet signature must be 65 bytes');

  const cases = [0, 1, 2, 63, 64, 300];
  for (const count of cases) {
    const signatures = [];
    for (let i = 0; i < count; i += 1) {
      signatures.push(new Uint8Array(sampleSig));
    }
    const proof = encodeProof(signatures);
    assert.equal(proof[0], 1, 'proof version byte must be 1');

    const decodedLen = decodeCompactU32(Uint8Array.from(proof.slice(1)));
    assert.equal(decodedLen.value, count, 'proof signature length prefix mismatch');

    const expectedTotal = 1 + decodedLen.used + (65 * count);
    assert.equal(proof.length, expectedTotal, 'proof byte length mismatch');
  }
}

function runProofHeaderBoundaryChecks() {
  const sampleSig = makeSampleSignature(123n);
  const counts = [0, 1, 63, 64, 65];
  for (const count of counts) {
    const signatures = [];
    for (let i = 0; i < count; i += 1) {
      signatures.push(new Uint8Array(sampleSig));
    }
    const proof = encodeProof(signatures);
    const encodedCount = Buffer.from(scaleCompactU32(count)).toString('hex');
    const proofHeader = proof.subarray(1, 1 + scaleCompactU32(count).length).toString('hex');
    assert.equal(proof[0], 1, `proof version byte mismatch at count ${count}`);
    assert.equal(proofHeader, encodedCount, `proof compact header mismatch at count ${count}`);
  }
}

function runProofCountWindowInvariantChecks() {
  const sampleSig = makeSampleSignature(124n);
  for (let count = 0; count <= 128; count += 1) {
    const signatures = [];
    for (let i = 0; i < count; i += 1) {
      signatures.push(new Uint8Array(sampleSig));
    }
    const proof = encodeProof(signatures);
    const decodedLen = decodeCompactU32(Uint8Array.from(proof.slice(1)));
    const expectedTotal = 1 + decodedLen.used + (65 * count);

    assert.equal(proof[0], 1, `proof version must remain 1 at count ${count}`);
    assert.equal(decodedLen.value, count, `proof compact count mismatch at count ${count}`);
    assert.equal(proof.length, expectedTotal, `proof length invariant mismatch at count ${count}`);
  }
}

function runProofHeaderRoundtripWindowChecks() {
  const sampleSig = makeSampleSignature(125n);
  for (let count = 0; count <= 128; count += 1) {
    const signatures = [];
    for (let i = 0; i < count; i += 1) {
      signatures.push(new Uint8Array(sampleSig));
    }
    const proof = encodeProof(signatures);
    const expectedHeader = scaleCompactU32(count);
    const decodedLen = decodeCompactU32(Uint8Array.from(proof.slice(1)));
    const actualHeader = Uint8Array.from(proof.slice(1, 1 + decodedLen.used));

    assert.equal(decodedLen.used, expectedHeader.length, `proof compact header size mismatch at count ${count}`);
    assert.deepEqual(actualHeader, expectedHeader, `proof compact header bytes mismatch at count ${count}`);
  }
}

function runProofPayloadSliceBoundaryChecks() {
  const sampleSig = makeSampleSignature(126n);
  const counts = [0, 1, 2, 5, 64];
  for (const count of counts) {
    const signatures = [];
    for (let i = 0; i < count; i += 1) {
      signatures.push(new Uint8Array(sampleSig));
    }
    const proof = encodeProof(signatures);
    const decodedLen = decodeCompactU32(Uint8Array.from(proof.slice(1)));
    const payloadStart = 1 + decodedLen.used;
    const payload = proof.slice(payloadStart);

    assert.equal(payload.length, count * 65, `proof payload byte-length mismatch at count ${count}`);
    for (let i = 0; i < count; i += 1) {
      const start = i * 65;
      const end = start + 65;
      const slice = payload.slice(start, end);
      assert.equal(slice.length, 65, `signature slice length mismatch at count ${count}, index ${i}`);
      assert.deepEqual(slice, Buffer.from(sampleSig), `signature slice bytes mismatch at count ${count}, index ${i}`);
    }
  }
}

function runProofLengthMonotonicityChecks() {
  const sig = makeSampleSignature(1500n);
  let prevLen = -1;
  for (let count = 0; count <= 16; count += 1) {
    const signatures = [];
    for (let i = 0; i < count; i += 1) {
      signatures.push(new Uint8Array(sig));
    }
    const proof = encodeProof(signatures);
    assert(proof.length > prevLen, `proof length should grow monotonically at count ${count}`);
    prevLen = proof.length;
  }
}

function runProofDeterminismChecks() {
  const sigA = makeSampleSignature(1000n);
  const sigB = makeSampleSignature(1001n);
  const proofA = encodeProof([sigA, sigB]);
  const proofB = encodeProof([sigA, sigB]);
  assert.deepEqual(proofA, proofB, 'proof encoding must be deterministic for same signatures');

  const mutSigB = new Uint8Array(sigB);
  mutSigB[0] ^= 0x01;
  const proofChanged = encodeProof([sigA, mutSigB]);
  assert.notDeepEqual(proofA, proofChanged, 'proof bytes must change when signatures change');

  const emptyProof = encodeProof([]);
  assert.equal(emptyProof.length, 2, 'empty proof must contain only version + compact length');
  assert.equal(emptyProof[0], 1, 'empty proof version must be 1');
  assert.equal(emptyProof[1], 0, 'empty proof compact length for zero signatures must be 0');
}

function runProofOrderSensitivityChecks() {
  const sigA = makeSampleSignature(2001n);
  const sigB = makeSampleSignature(2002n);
  const ordered = encodeProof([sigA, sigB]);
  const swapped = encodeProof([sigB, sigA]);
  assert.notDeepEqual(ordered, swapped, 'proof encoding must preserve signature order');
}

function runProofMalformedSignatureLengthChecks() {
  const sigA = makeSampleSignature(2000n);
  const malformed = sigA.slice(0, 64);
  const proof = encodeProof([malformed]);

  const decodedLen = decodeCompactU32(Uint8Array.from(proof.slice(1)));
  assert.equal(decodedLen.value, 1, 'malformed proof still encodes one signature entry');

  const expectedLenForCanonicalSigSize = 1 + decodedLen.used + 65;
  assert.notEqual(
    proof.length,
    expectedLenForCanonicalSigSize,
    'malformed 64-byte signature should violate canonical proof byte length invariant',
  );
}

function runProofLengthPrefixTamperingChecks() {
  const sig = makeSampleSignature(2100n);
  const canonical = encodeProof([sig]);
  const tampered = Buffer.from(canonical);
  tampered[1] = 0x08; // compact(2 signatures)

  const canonicalLen = decodeCompactU32(Uint8Array.from(canonical.slice(1)));
  const tamperedLen = decodeCompactU32(Uint8Array.from(tampered.slice(1)));
  assert.equal(canonicalLen.value, 1, 'canonical proof should encode one signature');
  assert.equal(tamperedLen.value, 2, 'tampered proof should decode as two signatures');

  const canonicalExpected = 1 + canonicalLen.used + (65 * canonicalLen.value);
  const tamperedExpected = 1 + tamperedLen.used + (65 * tamperedLen.value);
  assert.equal(canonical.length, canonicalExpected, 'canonical proof should satisfy length invariant');
  assert.notEqual(
    tampered.length,
    tamperedExpected,
    'tampered proof should violate canonical length invariant',
  );
}

function runProofPayloadDivisibilityChecks() {
  const sig = makeSampleSignature(2200n);
  const canonical = encodeProof([sig, sig, sig]);
  const canonicalLen = decodeCompactU32(Uint8Array.from(canonical.slice(1)));
  const canonicalPayloadLen = canonical.length - 1 - canonicalLen.used;
  assert.equal(canonicalPayloadLen, canonicalLen.value * 65, 'canonical proof payload length mismatch');
  assert.equal(canonicalPayloadLen % 65, 0, 'canonical proof payload must be divisible by 65');

  const malformed = Buffer.concat([canonical, Buffer.from([0xff])]);
  const malformedLen = decodeCompactU32(Uint8Array.from(malformed.slice(1)));
  const malformedPayloadLen = malformed.length - 1 - malformedLen.used;
  assert.notEqual(
    malformedPayloadLen,
    malformedLen.value * 65,
    'trailing-byte proof must violate canonical payload length',
  );
  assert.notEqual(malformedPayloadLen % 65, 0, 'trailing-byte proof payload must not be divisible by 65');
}

function runProofSignatureSizeInvariantChecks() {
  const sizes = [0, 1, 64, 65, 66];
  for (const size of sizes) {
    const entry = new Uint8Array(size);
    const proof = encodeProof([entry]);
    const decodedLen = decodeCompactU32(Uint8Array.from(proof.slice(1)));
    assert.equal(decodedLen.value, 1, 'single-entry proof should encode signature count 1');

    const expectedCanonical = 1 + decodedLen.used + 65;
    if (size === 65) {
      assert.equal(proof.length, expectedCanonical, '65-byte signature must satisfy canonical length');
    } else {
      assert.notEqual(
        proof.length,
        expectedCanonical,
        `${size}-byte signature should violate canonical length invariant`,
      );
    }
  }
}

function assertProfileSupported(profile) {
  if (profile !== 'fast' && profile !== 'full') {
    throw new Error(`unsupported --profile: ${profile} (expected fast|full)`);
  }
}

function runCoreChecks() {
  runScaleCompactChecks();
  runCompactEncodingLengthModeChecks();
  runCompactModeBitsChecks();
  runCompactBoundaryTransitionChecks();
  runCompactKnownBytePatternChecks();
  runCompactDecoderFailClosedChecks();
  runHexInputValidationChecks();
  runPrefixLiteralChecks();
  runAttestHashChecks();
  runProofLayoutChecks();
  runProofHeaderBoundaryChecks();
  runProofDeterminismChecks();
  runProofOrderSensitivityChecks();
  runProofMalformedSignatureLengthChecks();
  runProofLengthPrefixTamperingChecks();
  runProofSignatureSizeInvariantChecks();
}

function runExtendedChecks() {
  runCompactSequentialRoundtripChecks();
  runCompactEncodedLengthMonotonicChecks();
  runCompactModeWindowCoverageChecks();
  runCompactDecoderTrailingBytesChecks();
  runMessageIdSeedDeterminismChecks();
  runAttestHashSingleByteSensitivityChecks();
  runAttestHashSequentialWindowChecks();
  runAttestHashMultiByteSensitivityChecks();
  runProofCountWindowInvariantChecks();
  runProofHeaderRoundtripWindowChecks();
  runProofPayloadSliceBoundaryChecks();
  runProofLengthMonotonicityChecks();
  runProofPayloadDivisibilityChecks();
}

function main() {
  const profile = parseProfile(process.argv.slice(2));
  assertProfileSupported(profile);
  runCoreChecks();
  if (profile === 'full') {
    runExtendedChecks();
  }

  // eslint-disable-next-line no-console
  console.log(`[formal-assisted] ok (${profile}): compact+hash+proof invariants`);
}

main();
