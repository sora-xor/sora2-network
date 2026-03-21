import { expect } from 'chai';
import fc from 'fast-check';
import { network } from 'hardhat';

const U32_MAX = (1n << 32n) - 1n;
const U64_MAX = (1n << 64n) - 1n;
const U128_MAX = (1n << 128n) - 1n;
const DEFAULT_RUNS = 200;
const RUNS_ENV_VARS = ['SCCP_FUZZ_RUNS', 'SCCP_FASTCHECK_RUNS'];

function resolveRuns(defaultRuns) {
  for (const envName of RUNS_ENV_VARS) {
    const envRaw = process.env[envName];
    if (envRaw === undefined) {
      continue;
    }
    const parsed = Number(envRaw);
    if (!Number.isInteger(parsed) || parsed <= 0) {
      throw new Error(`${envName} must be a positive integer (got: ${envRaw})`);
    }
    return parsed;
  }

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

function bytesToHex(bytes) {
  return `0x${Buffer.from(bytes).toString('hex')}`;
}

async function expectRevert(promise) {
  let reverted = false;
  try {
    await promise;
  } catch {
    reverted = true;
  }
  expect(reverted).to.equal(true);
}

describe('SCCP codec fast-check fuzz', function () {
  let codec;

  before(async function () {
    const { ethers } = await network.connect();
    const CodecTest = await ethers.getContractFactory('SccpCodecTest');
    codec = await CodecTest.deploy();
    await codec.waitForDeployment();
  });

  it('encode/decode roundtrips across bounded arbitrary payload fields', async function () {
    const runs = FUZZ_RUNS;
    await fc.assert(
      fc.asyncProperty(
        fc.bigInt({ min: 0n, max: U32_MAX }),
        fc.bigInt({ min: 0n, max: U32_MAX }),
        fc.bigInt({ min: 0n, max: U64_MAX }),
        fc.uint8Array({ minLength: 32, maxLength: 32 }),
        fc.bigInt({ min: 0n, max: U128_MAX }),
        fc.uint8Array({ minLength: 32, maxLength: 32 }),
        async (sourceDomain, destDomain, nonce, soraAssetId, amount, recipient) => {
          const payload = await codec.encodeBurnPayloadV1(
            Number(sourceDomain),
            Number(destDomain),
            nonce,
            bytesToHex(soraAssetId),
            amount,
            bytesToHex(recipient),
          );
          expect((payload.length - 2) / 2).to.equal(97);

          const decoded = await codec.decodeBurnPayloadV1(payload);
          expect(BigInt(decoded[0])).to.equal(1n);
          expect(BigInt(decoded[1])).to.equal(sourceDomain);
          expect(BigInt(decoded[2])).to.equal(destDomain);
          expect(BigInt(decoded[3])).to.equal(nonce);
          expect(decoded[4].toLowerCase()).to.equal(bytesToHex(soraAssetId).toLowerCase());
          expect(BigInt(decoded[5])).to.equal(amount);
          expect(decoded[6].toLowerCase()).to.equal(bytesToHex(recipient).toLowerCase());
        },
      ),
      { numRuns: runs },
    );
  });

  it('decode fail-closes on arbitrary non-97-byte inputs', async function () {
    const runs = FUZZ_RUNS;
    await fc.assert(
      fc.asyncProperty(
        fc.uint8Array({ minLength: 0, maxLength: 256 }).filter((bytes) => bytes.length !== 97),
        async (bytes) => {
          await expectRevert(codec.decodeBurnPayloadV1(bytesToHex(bytes)));
        },
      ),
      { numRuns: runs },
    );
  });

  it('message id changes when fuzzed payload nonce changes', async function () {
    const runs = FUZZ_RUNS;
    await fc.assert(
      fc.asyncProperty(
        fc.bigInt({ min: 0n, max: U32_MAX }),
        fc.bigInt({ min: 0n, max: U32_MAX }),
        fc.bigInt({ min: 0n, max: U64_MAX - 1n }),
        fc.uint8Array({ minLength: 32, maxLength: 32 }),
        fc.bigInt({ min: 0n, max: U128_MAX }),
        fc.uint8Array({ minLength: 32, maxLength: 32 }),
        async (sourceDomain, destDomain, nonce, soraAssetId, amount, recipient) => {
          const payloadA = await codec.encodeBurnPayloadV1(
            Number(sourceDomain),
            Number(destDomain),
            nonce,
            bytesToHex(soraAssetId),
            amount,
            bytesToHex(recipient),
          );
          const payloadB = await codec.encodeBurnPayloadV1(
            Number(sourceDomain),
            Number(destDomain),
            nonce + 1n,
            bytesToHex(soraAssetId),
            amount,
            bytesToHex(recipient),
          );
          const idA = await codec.burnMessageId(payloadA);
          const idB = await codec.burnMessageId(payloadB);
          expect(idA).to.not.equal(idB);
        },
      ),
      { numRuns: runs },
    );
  });

  it('token add payload roundtrips and preserves fixed-width layout', async function () {
    const runs = FUZZ_RUNS;
    await fc.assert(
      fc.asyncProperty(
        fc.bigInt({ min: 0n, max: U32_MAX }),
        fc.bigInt({ min: 0n, max: U64_MAX }),
        fc.uint8Array({ minLength: 32, maxLength: 32 }),
        fc.integer({ min: 0, max: 255 }),
        fc.uint8Array({ minLength: 32, maxLength: 32 }),
        fc.uint8Array({ minLength: 32, maxLength: 32 }),
        async (targetDomain, nonce, soraAssetId, decimals, name, symbol) => {
          const payload = await codec.encodeTokenAddPayloadV1(
            Number(targetDomain),
            nonce,
            bytesToHex(soraAssetId),
            decimals,
            bytesToHex(name),
            bytesToHex(symbol),
          );
          expect((payload.length - 2) / 2).to.equal(110);

          const decoded = await codec.decodeTokenAddPayloadV1(payload);
          expect(BigInt(decoded[0])).to.equal(1n);
          expect(BigInt(decoded[1])).to.equal(targetDomain);
          expect(BigInt(decoded[2])).to.equal(nonce);
          expect(decoded[3].toLowerCase()).to.equal(bytesToHex(soraAssetId).toLowerCase());
          expect(BigInt(decoded[4])).to.equal(BigInt(decimals));
          expect(decoded[5].toLowerCase()).to.equal(bytesToHex(name).toLowerCase());
          expect(decoded[6].toLowerCase()).to.equal(bytesToHex(symbol).toLowerCase());
        },
      ),
      { numRuns: runs },
    );
  });

  it('token pause/resume payloads share bytes layout but stay message-id domain separated', async function () {
    const runs = FUZZ_RUNS;
    await fc.assert(
      fc.asyncProperty(
        fc.bigInt({ min: 0n, max: U32_MAX }),
        fc.bigInt({ min: 0n, max: U64_MAX }),
        fc.uint8Array({ minLength: 32, maxLength: 32 }),
        async (targetDomain, nonce, soraAssetId) => {
          const payloadPause = await codec.encodeTokenPausePayloadV1(
            Number(targetDomain),
            nonce,
            bytesToHex(soraAssetId),
          );
          const payloadResume = await codec.encodeTokenResumePayloadV1(
            Number(targetDomain),
            nonce,
            bytesToHex(soraAssetId),
          );
          expect(payloadPause.toLowerCase()).to.equal(payloadResume.toLowerCase());
          expect((payloadPause.length - 2) / 2).to.equal(45);

          const decodedPause = await codec.decodeTokenPausePayloadV1(payloadPause);
          const decodedResume = await codec.decodeTokenResumePayloadV1(payloadResume);
          expect(BigInt(decodedPause[0])).to.equal(1n);
          expect(BigInt(decodedPause[1])).to.equal(targetDomain);
          expect(BigInt(decodedPause[2])).to.equal(nonce);
          expect(decodedPause[3].toLowerCase()).to.equal(bytesToHex(soraAssetId).toLowerCase());
          expect(decodedResume[0]).to.equal(decodedPause[0]);
          expect(decodedResume[1]).to.equal(decodedPause[1]);
          expect(decodedResume[2]).to.equal(decodedPause[2]);
          expect(decodedResume[3].toLowerCase()).to.equal(decodedPause[3].toLowerCase());

          const pauseId = await codec.tokenPauseMessageId(payloadPause);
          const resumeId = await codec.tokenResumeMessageId(payloadResume);
          expect(pauseId).to.not.equal(resumeId);
        },
      ),
      { numRuns: runs },
    );
  });

  it('governance decoders fail-close on non-canonical lengths', async function () {
    const runs = FUZZ_RUNS;
    await fc.assert(
      fc.asyncProperty(
        fc.uint8Array({ minLength: 0, maxLength: 256 }).filter((bytes) => bytes.length !== 110),
        async (bytes) => {
          await expectRevert(codec.decodeTokenAddPayloadV1(bytesToHex(bytes)));
        },
      ),
      { numRuns: runs },
    );

    await fc.assert(
      fc.asyncProperty(
        fc.uint8Array({ minLength: 0, maxLength: 256 }).filter((bytes) => bytes.length !== 45),
        async (bytes) => {
          const payload = bytesToHex(bytes);
          await expectRevert(codec.decodeTokenPausePayloadV1(payload));
          await expectRevert(codec.decodeTokenResumePayloadV1(payload));
        },
      ),
      { numRuns: runs },
    );
  });
});
