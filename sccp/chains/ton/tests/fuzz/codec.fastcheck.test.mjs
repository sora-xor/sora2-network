import test from 'node:test';
import assert from 'node:assert/strict';
import fc from 'fast-check';
import { ethers } from 'ethers';

const SCCP_BURN_PREFIX = Buffer.from('sccp:burn:v1', 'utf8');

function toFixedBeBytes(value, width, fieldName) {
  const n = BigInt(value);
  assert(Number.isInteger(width) && width > 0, `${fieldName} width must be a positive integer`);
  assert(n >= 0n, `${fieldName} must be non-negative`);
  const max = (1n << BigInt(width * 8)) - 1n;
  assert(n <= max, `${fieldName} exceeds ${width * 8} bits`);
  return Buffer.from(n.toString(16).padStart(width * 2, '0'), 'hex');
}

function toFixedLeBytes(value, width, fieldName) {
  return Buffer.from(toFixedBeBytes(value, width, fieldName)).reverse();
}

function burnPayloadToJsBytes({
  sourceDomain,
  destDomain,
  nonce,
  soraAssetId,
  amount,
  recipient32,
}) {
  return Buffer.concat([
    Buffer.from([1]),
    toFixedLeBytes(sourceDomain, 4, 'sourceDomain'),
    toFixedLeBytes(destDomain, 4, 'destDomain'),
    toFixedLeBytes(nonce, 8, 'nonce'),
    toFixedBeBytes(soraAssetId, 32, 'soraAssetId'),
    toFixedLeBytes(amount, 16, 'amount'),
    toFixedBeBytes(recipient32, 32, 'recipient32'),
  ]);
}

function burnPayloadToMessageId(fields) {
  const payload = burnPayloadToJsBytes(fields);
  return ethers.keccak256(Buffer.concat([SCCP_BURN_PREFIX, payload]));
}

const U32_MAX = (1n << 32n) - 1n;
const U64_MAX = (1n << 64n) - 1n;
const U128_MAX = (1n << 128n) - 1n;
const U256_MAX = (1n << 256n) - 1n;
const DEFAULT_RUNS = 500;

function resolveRuns(defaultRuns) {
  for (let i = 0; i < process.argv.length; i += 1) {
    if (process.argv[i] === '--fuzz-runs') {
      const raw = process.argv[i + 1];
      const parsed = Number(raw);
      if (!Number.isInteger(parsed) || parsed <= 0) {
        throw new Error(`--fuzz-runs must be a positive integer (got: ${raw})`);
      }
      return parsed;
    }
  }
  return defaultRuns;
}
const FUZZ_RUNS = resolveRuns(DEFAULT_RUNS);

test('fuzz: payload encoder always returns fixed 97-byte layout', async () => {
  const runs = FUZZ_RUNS;
  await fc.assert(
    fc.property(
      fc.bigInt({ min: 0n, max: U32_MAX }),
      fc.bigInt({ min: 0n, max: U32_MAX }),
      fc.bigInt({ min: 0n, max: U64_MAX }),
      fc.bigInt({ min: 0n, max: U256_MAX }),
      fc.bigInt({ min: 0n, max: U128_MAX }),
      fc.bigInt({ min: 0n, max: U256_MAX }),
      (sourceDomain, destDomain, nonce, soraAssetId, amount, recipient32) => {
        const payload = burnPayloadToJsBytes({
          sourceDomain,
          destDomain,
          nonce,
          soraAssetId,
          amount,
          recipient32,
        });
        assert.equal(payload.length, 97);
        assert.equal(payload[0], 1);
      },
    ),
    { numRuns: runs },
  );
});

test('fuzz: message id is deterministic and nonce-sensitive', async () => {
  const runs = FUZZ_RUNS;
  await fc.assert(
    fc.property(
      fc.bigInt({ min: 0n, max: U32_MAX }),
      fc.bigInt({ min: 0n, max: U32_MAX }),
      fc.bigInt({ min: 0n, max: U64_MAX - 1n }),
      fc.bigInt({ min: 0n, max: U256_MAX }),
      fc.bigInt({ min: 0n, max: U128_MAX }),
      fc.bigInt({ min: 0n, max: U256_MAX }),
      (sourceDomain, destDomain, nonce, soraAssetId, amount, recipient32) => {
        const fields = {
          sourceDomain,
          destDomain,
          nonce,
          soraAssetId,
          amount,
          recipient32,
        };
        const idA = burnPayloadToMessageId(fields);
        const idA2 = burnPayloadToMessageId(fields);
        assert.equal(idA, idA2);

        const idB = burnPayloadToMessageId({
          ...fields,
          nonce: nonce + 1n,
        });
        assert.notEqual(idA, idB);
      },
    ),
    { numRuns: runs },
  );
});

test('fuzz: message id remains domain-separated from plain payload keccak', async () => {
  const runs = FUZZ_RUNS;
  await fc.assert(
    fc.property(
      fc.bigInt({ min: 0n, max: U32_MAX }),
      fc.bigInt({ min: 0n, max: U32_MAX }),
      fc.bigInt({ min: 0n, max: U64_MAX }),
      fc.bigInt({ min: 0n, max: U256_MAX }),
      fc.bigInt({ min: 0n, max: U128_MAX }),
      fc.bigInt({ min: 0n, max: U256_MAX }),
      (sourceDomain, destDomain, nonce, soraAssetId, amount, recipient32) => {
        const fields = {
          sourceDomain,
          destDomain,
          nonce,
          soraAssetId,
          amount,
          recipient32,
        };
        const payload = burnPayloadToJsBytes(fields);
        const prefixed = burnPayloadToMessageId(fields);
        const plain = ethers.keccak256(payload);
        assert.notEqual(prefixed, plain);
      },
    ),
    { numRuns: runs },
  );
});

test('fuzz: message id changes when source/destination domains are swapped', async () => {
  const runs = FUZZ_RUNS;
  await fc.assert(
    fc.property(
      fc.bigInt({ min: 0n, max: U32_MAX }),
      fc.bigInt({ min: 0n, max: U32_MAX }).filter((destDomain) => destDomain !== 0n),
      fc.bigInt({ min: 0n, max: U64_MAX }),
      fc.bigInt({ min: 0n, max: U256_MAX }),
      fc.bigInt({ min: 0n, max: U128_MAX }),
      fc.bigInt({ min: 0n, max: U256_MAX }),
      (sourceDomainRaw, destDelta, nonce, soraAssetId, amount, recipient32) => {
        const sourceDomain = sourceDomainRaw;
        const destDomain = (sourceDomainRaw + destDelta) & U32_MAX;
        if (sourceDomain === destDomain) {
          return;
        }

        const a = burnPayloadToMessageId({
          sourceDomain,
          destDomain,
          nonce,
          soraAssetId,
          amount,
          recipient32,
        });
        const b = burnPayloadToMessageId({
          sourceDomain: destDomain,
          destDomain: sourceDomain,
          nonce,
          soraAssetId,
          amount,
          recipient32,
        });
        assert.notEqual(a, b);
      },
    ),
    { numRuns: runs },
  );
});
