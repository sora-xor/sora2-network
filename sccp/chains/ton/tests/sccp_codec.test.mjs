import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { Blockchain } from '@ton/sandbox';
import { beginCell, Cell, contractAddress, SendMode } from '@ton/core';
import { ethers } from 'ethers';

const repoRoot = resolve(import.meta.dirname, '..');
const SCCP_BURN_PREFIX = Buffer.from('sccp:burn:v1', 'utf8');

function loadArtifact(name) {
  return JSON.parse(readFileSync(resolve(repoRoot, 'artifacts', name), 'utf8'));
}

function codeFromArtifact(artifact) {
  return Cell.fromBoc(Buffer.from(artifact.codeBoc64, 'base64'))[0];
}

function toFixedBeBytes(value, width, fieldName) {
  assert(Number.isInteger(width) && width > 0, `${fieldName} width must be a positive integer`);
  const n = BigInt(value);
  assert(n >= 0n, `${fieldName} must be non-negative`);
  const max = (1n << BigInt(width * 8)) - 1n;
  assert(n <= max, `${fieldName} exceeds ${width * 8} bits`);
  return Buffer.from(n.toString(16).padStart(width * 2, '0'), 'hex');
}

function toFixedLeBytes(value, width, fieldName) {
  return Buffer.from(toFixedBeBytes(value, width, fieldName)).reverse();
}

function burnPayloadToJsMessageId({
  sourceDomain,
  destDomain,
  nonce,
  soraAssetId,
  amount,
  recipient32,
}) {
  const payload = burnPayloadToJsBytes({
    sourceDomain,
    destDomain,
    nonce,
    soraAssetId,
    amount,
    recipient32,
  });

  return BigInt(ethers.keccak256(Buffer.concat([SCCP_BURN_PREFIX, payload])));
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
    Buffer.from([1]), // version
    toFixedLeBytes(sourceDomain, 4, 'sourceDomain'),
    toFixedLeBytes(destDomain, 4, 'destDomain'),
    toFixedLeBytes(nonce, 8, 'nonce'),
    toFixedBeBytes(soraAssetId, 32, 'soraAssetId'),
    toFixedLeBytes(amount, 16, 'amount'),
    toFixedBeBytes(recipient32, 32, 'recipient32'),
  ]);
}

async function openCodecContract() {
  const artifact = loadArtifact('sccp-codec-test.compiled.json');
  const code = codeFromArtifact(artifact);
  const blockchain = await Blockchain.create();
  const deployer = await blockchain.treasury('deployer');
  const c = blockchain.openContract(SccpCodecTest.createFromCode(code));
  await c.sendDeploy(deployer.getSender(), 1_000_000_000n);
  return c;
}

class SccpCodecTest {
  constructor(address, init) {
    this.address = address;
    this.init = init;
  }

  static createFromCode(code, workchain = 0) {
    const data = beginCell().endCell();
    const init = { code, data };
    return new SccpCodecTest(contractAddress(workchain, init), init);
  }

  async sendDeploy(provider, via, value) {
    // TopUpTons opcode (same as in `messages.tolk`).
    const TOP_UP_TONS = 0xd372158c;
    await provider.internal(via, {
      value,
      sendMode: SendMode.PAY_GAS_SEPARATELY,
      body: beginCell().storeUint(TOP_UP_TONS, 32).endCell(),
    });
  }

  async getMessageId(provider, sourceDomain, destDomain, nonce, soraAssetId, amount, recipient32) {
    const res = await provider.get('get_message_id', [
      { type: 'int', value: BigInt(sourceDomain) },
      { type: 'int', value: BigInt(destDomain) },
      { type: 'int', value: BigInt(nonce) },
      { type: 'int', value: BigInt(soraAssetId) },
      { type: 'int', value: BigInt(amount) },
      { type: 'int', value: BigInt(recipient32) },
    ]);
    return res.stack.readBigNumber();
  }
}

test('fixed-width byte helpers reject negative and overflow values', () => {
  assert.throws(
    () => toFixedBeBytes(0n, 0, 'amount'),
    /amount width must be a positive integer/,
  );
  assert.throws(
    () => toFixedBeBytes(0n, 1.5, 'amount'),
    /amount width must be a positive integer/,
  );
  assert.throws(
    () => toFixedBeBytes(-1n, 1, 'amount'),
    /amount must be non-negative/,
  );
  assert.throws(
    () => toFixedBeBytes(256n, 1, 'amount'),
    /amount exceeds 8 bits/,
  );
  assert.throws(
    () => toFixedLeBytes(1n << 64n, 8, 'nonce'),
    /nonce exceeds 64 bits/,
  );
});

test('little-endian helper reverses fixed-width big-endian bytes', () => {
  const be = toFixedBeBytes(0x11223344n, 4, 'sample');
  const le = toFixedLeBytes(0x11223344n, 4, 'sample');
  assert.equal(be.toString('hex'), '11223344');
  assert.equal(le.toString('hex'), '44332211');
});

test('SCCP messageId matches reference vector (ETH -> SORA fixture)', async () => {
  const c = await openCodecContract();

  const sourceDomain = 1; // ETH
  const destDomain = 0; // SORA
  const nonce = 777;
  const soraAssetId = BigInt('0x' + '11'.repeat(32));
  const amount = 10n;
  const recipient32 = BigInt('0x' + '22'.repeat(32));

  const expected = BigInt(
    '0x' +
      'f3cac8c5acfb0670a24e9ffeab7e409a9d54d1dc5e6dbaf0ee986462fe1ffb3a',
  );

  const got = await c.getMessageId(sourceDomain, destDomain, nonce, soraAssetId, amount, recipient32);
  assert.equal(got, expected);
});

test('SCCP JS encoder builds the exact reference payload bytes', () => {
  const payload = burnPayloadToJsBytes({
    sourceDomain: 1,
    destDomain: 0,
    nonce: 777n,
    soraAssetId: BigInt('0x' + '11'.repeat(32)),
    amount: 10n,
    recipient32: BigInt('0x' + '22'.repeat(32)),
  });

  const expectedHex =
    '01' +
    '01000000' +
    '00000000' +
    '0903000000000000' +
    '11'.repeat(32) +
    '0a' +
    '00'.repeat(15) +
    '22'.repeat(32);
  assert.equal(payload.toString('hex'), expectedHex);
});

test('SCCP JS payload encoder always returns a fixed 97-byte payload', () => {
  const payload = burnPayloadToJsBytes({
    sourceDomain: 5,
    destDomain: 4,
    nonce: 123n,
    soraAssetId: BigInt('0x' + 'aa'.repeat(32)),
    amount: 456n,
    recipient32: BigInt('0x' + 'bb'.repeat(32)),
  });
  assert.equal(payload.length, 97);
});

test('SCCP messageId helper is domain-separated from plain payload keccak', () => {
  const fixture = {
    sourceDomain: 1,
    destDomain: 4,
    nonce: 123n,
    soraAssetId: BigInt('0x' + '33'.repeat(32)),
    amount: 456n,
    recipient32: BigInt('0x' + '44'.repeat(32)),
  };
  const payload = burnPayloadToJsBytes(fixture);

  const prefixed = burnPayloadToJsMessageId(fixture);
  const plain = BigInt(ethers.keccak256(payload));
  assert.notEqual(prefixed, plain);

  const manualPrefixed = BigInt(ethers.keccak256(Buffer.concat([SCCP_BURN_PREFIX, payload])));
  assert.equal(prefixed, manualPrefixed);
});

test('SCCP JS payload encoder rejects out-of-range sourceDomain values', () => {
  assert.throws(
    () =>
      burnPayloadToJsBytes({
        sourceDomain: 1n << 32n,
        destDomain: 0,
        nonce: 1n,
        soraAssetId: 1n,
        amount: 1n,
        recipient32: 1n,
      }),
    /sourceDomain exceeds 32 bits/,
  );
});

test('SCCP JS payload encoder rejects out-of-range destination/nonce/amount/id/recipient values', () => {
  assert.throws(
    () =>
      burnPayloadToJsBytes({
        sourceDomain: 1,
        destDomain: 1n << 32n,
        nonce: 1n,
        soraAssetId: 1n,
        amount: 1n,
        recipient32: 1n,
      }),
    /destDomain exceeds 32 bits/,
  );

  assert.throws(
    () =>
      burnPayloadToJsBytes({
        sourceDomain: 1,
        destDomain: 0,
        nonce: 1n << 64n,
        soraAssetId: 1n,
        amount: 1n,
        recipient32: 1n,
      }),
    /nonce exceeds 64 bits/,
  );

  assert.throws(
    () =>
      burnPayloadToJsBytes({
        sourceDomain: 1,
        destDomain: 0,
        nonce: 1n,
        soraAssetId: 1n << 256n,
        amount: 1n,
        recipient32: 1n,
      }),
    /soraAssetId exceeds 256 bits/,
  );

  assert.throws(
    () =>
      burnPayloadToJsBytes({
        sourceDomain: 1,
        destDomain: 0,
        nonce: 1n,
        soraAssetId: 1n,
        amount: 1n << 128n,
        recipient32: 1n,
      }),
    /amount exceeds 128 bits/,
  );

  assert.throws(
    () =>
      burnPayloadToJsBytes({
        sourceDomain: 1,
        destDomain: 0,
        nonce: 1n,
        soraAssetId: 1n,
        amount: 1n,
        recipient32: 1n << 256n,
      }),
    /recipient32 exceeds 256 bits/,
  );
});

test('SCCP JS payload encoder rejects negative destination/nonce/amount/id/recipient values', () => {
  assert.throws(
    () =>
      burnPayloadToJsBytes({
        sourceDomain: -1n,
        destDomain: 0,
        nonce: 1n,
        soraAssetId: 1n,
        amount: 1n,
        recipient32: 1n,
      }),
    /sourceDomain must be non-negative/,
  );

  assert.throws(
    () =>
      burnPayloadToJsBytes({
        sourceDomain: 1,
        destDomain: -1n,
        nonce: 1n,
        soraAssetId: 1n,
        amount: 1n,
        recipient32: 1n,
      }),
    /destDomain must be non-negative/,
  );

  assert.throws(
    () =>
      burnPayloadToJsBytes({
        sourceDomain: 1,
        destDomain: 0,
        nonce: -1n,
        soraAssetId: 1n,
        amount: 1n,
        recipient32: 1n,
      }),
    /nonce must be non-negative/,
  );

  assert.throws(
    () =>
      burnPayloadToJsBytes({
        sourceDomain: 1,
        destDomain: 0,
        nonce: 1n,
        soraAssetId: -1n,
        amount: 1n,
        recipient32: 1n,
      }),
    /soraAssetId must be non-negative/,
  );

  assert.throws(
    () =>
      burnPayloadToJsBytes({
        sourceDomain: 1,
        destDomain: 0,
        nonce: 1n,
        soraAssetId: 1n,
        amount: -1n,
        recipient32: 1n,
      }),
    /amount must be non-negative/,
  );

  assert.throws(
    () =>
      burnPayloadToJsBytes({
        sourceDomain: 1,
        destDomain: 0,
        nonce: 1n,
        soraAssetId: 1n,
        amount: 1n,
        recipient32: -1n,
      }),
    /recipient32 must be non-negative/,
  );
});

test('SCCP messageId JS helper is unique across bounded nonce window', () => {
  const base = {
    sourceDomain: 1,
    destDomain: 3,
    soraAssetId: BigInt('0x' + '11'.repeat(32)),
    amount: 42n,
    recipient32: BigInt('0x' + '22'.repeat(32)),
  };

  const seen = new Set();
  for (let nonce = 0n; nonce < 128n; nonce += 1n) {
    const id = burnPayloadToJsMessageId({ ...base, nonce });
    const key = `0x${id.toString(16).padStart(64, '0')}`;
    assert(!seen.has(key), `messageId collision at nonce ${nonce.toString()}`);
    seen.add(key);
  }
  assert.equal(seen.size, 128);
});

test('SCCP messageId JS helper is unique across bounded recipient window', () => {
  const base = {
    sourceDomain: 2,
    destDomain: 4,
    nonce: 555n,
    soraAssetId: BigInt('0x' + '33'.repeat(32)),
    amount: 4242n,
  };

  const seen = new Set();
  for (let i = 0n; i < 128n; i += 1n) {
    const recipient32 = 1_000n + i;
    const id = burnPayloadToJsMessageId({ ...base, recipient32 });
    const key = `0x${id.toString(16).padStart(64, '0')}`;
    assert(!seen.has(key), `messageId collision at recipient offset ${i.toString()}`);
    seen.add(key);
  }
  assert.equal(seen.size, 128);
});

test('SCCP messageId JS helper is deterministic for repeated identical inputs', () => {
  const fixture = {
    sourceDomain: 5,
    destDomain: 1,
    nonce: 777n,
    soraAssetId: BigInt('0x' + '55'.repeat(32)),
    amount: 7777n,
    recipient32: BigInt('0x' + '66'.repeat(32)),
  };

  const a = burnPayloadToJsMessageId(fixture);
  const b = burnPayloadToJsMessageId(fixture);
  assert.equal(a, b);
});

test('SCCP messageId JS helper is unique across bounded amount window', () => {
  const base = {
    sourceDomain: 3,
    destDomain: 0,
    nonce: 100n,
    soraAssetId: BigInt('0x' + '77'.repeat(32)),
    recipient32: BigInt('0x' + '88'.repeat(32)),
  };

  const seen = new Set();
  for (let i = 0n; i < 128n; i += 1n) {
    const id = burnPayloadToJsMessageId({ ...base, amount: i });
    const key = `0x${id.toString(16).padStart(64, '0')}`;
    assert(!seen.has(key), `messageId collision at amount ${i.toString()}`);
    seen.add(key);
  }
  assert.equal(seen.size, 128);
});

test('SCCP messageId JS helper is unique across bounded sourceDomain window', () => {
  const base = {
    destDomain: 0,
    nonce: 999n,
    soraAssetId: BigInt('0x' + '99'.repeat(32)),
    amount: 9999n,
    recipient32: BigInt('0x' + 'aa'.repeat(32)),
  };

  const seen = new Set();
  for (let sourceDomain = 0; sourceDomain < 128; sourceDomain += 1) {
    const id = burnPayloadToJsMessageId({ ...base, sourceDomain });
    const key = `0x${id.toString(16).padStart(64, '0')}`;
    assert(!seen.has(key), `messageId collision at sourceDomain ${sourceDomain}`);
    seen.add(key);
  }
  assert.equal(seen.size, 128);
});

test('SCCP messageId JS helper is unique across bounded destination-domain window', () => {
  const base = {
    sourceDomain: 1,
    nonce: 777n,
    soraAssetId: BigInt('0x' + 'ab'.repeat(32)),
    amount: 700n,
    recipient32: BigInt('0x' + 'bc'.repeat(32)),
  };

  const seen = new Set();
  for (let destDomain = 0; destDomain < 128; destDomain += 1) {
    const id = burnPayloadToJsMessageId({ ...base, destDomain });
    const key = `0x${id.toString(16).padStart(64, '0')}`;
    assert(!seen.has(key), `messageId collision at destDomain ${destDomain}`);
    seen.add(key);
  }
  assert.equal(seen.size, 128);
});

test('SCCP messageId JS helper is unique across bounded soraAssetId window', () => {
  const base = {
    sourceDomain: 1,
    destDomain: 3,
    nonce: 1_234n,
    amount: 8_888n,
    recipient32: BigInt('0x' + 'de'.repeat(32)),
  };

  const seen = new Set();
  for (let i = 0n; i < 128n; i += 1n) {
    const id = burnPayloadToJsMessageId({ ...base, soraAssetId: 5_000n + i });
    const key = `0x${id.toString(16).padStart(64, '0')}`;
    assert(!seen.has(key), `messageId collision at soraAssetId offset ${i.toString()}`);
    seen.add(key);
  }
  assert.equal(seen.size, 128);
});

test('SCCP messageId JS helper is unique across bounded source/destination matrix', () => {
  const base = {
    nonce: 2_222n,
    soraAssetId: BigInt('0x' + 'e1'.repeat(32)),
    amount: 12_345n,
    recipient32: BigInt('0x' + 'f2'.repeat(32)),
  };

  const seen = new Set();
  for (let sourceDomain = 0; sourceDomain < 8; sourceDomain += 1) {
    for (let destDomain = 0; destDomain < 16; destDomain += 1) {
      const id = burnPayloadToJsMessageId({ ...base, sourceDomain, destDomain });
      const key = `0x${id.toString(16).padStart(64, '0')}`;
      assert(
        !seen.has(key),
        `messageId collision in source/destination matrix at ${sourceDomain}->${destDomain}`,
      );
      seen.add(key);
    }
  }
  assert.equal(seen.size, 128);
});

test('SCCP messageId JS helper is unique across bounded amount/recipient matrix', () => {
  const base = {
    sourceDomain: 3,
    destDomain: 1,
    nonce: 3_333n,
    soraAssetId: BigInt('0x' + 'a1'.repeat(32)),
  };

  const seen = new Set();
  for (let amount = 0n; amount < 8n; amount += 1n) {
    for (let i = 0n; i < 16n; i += 1n) {
      const recipient32 = 10_000n + i;
      const id = burnPayloadToJsMessageId({ ...base, amount, recipient32 });
      const key = `0x${id.toString(16).padStart(64, '0')}`;
      assert(
        !seen.has(key),
        `messageId collision in amount/recipient matrix at amount=${amount.toString()} recipientOffset=${i.toString()}`,
      );
      seen.add(key);
    }
  }
  assert.equal(seen.size, 128);
});

test('SCCP messageId JS helper is unique across bounded nonce/soraAssetId matrix', () => {
  const base = {
    sourceDomain: 4,
    destDomain: 2,
    amount: 22_222n,
    recipient32: BigInt('0x' + '9a'.repeat(32)),
  };

  const seen = new Set();
  for (let nonce = 0n; nonce < 8n; nonce += 1n) {
    for (let i = 0n; i < 16n; i += 1n) {
      const soraAssetId = 9_000n + i;
      const id = burnPayloadToJsMessageId({ ...base, nonce, soraAssetId });
      const key = `0x${id.toString(16).padStart(64, '0')}`;
      assert(
        !seen.has(key),
        `messageId collision in nonce/soraAssetId matrix at nonce=${nonce.toString()} assetOffset=${i.toString()}`,
      );
      seen.add(key);
    }
  }
  assert.equal(seen.size, 128);
});

test('SCCP messageId JS helper is unique across bounded source/destination/nonce matrix', () => {
  const base = {
    soraAssetId: BigInt('0x' + '5a'.repeat(32)),
    amount: 33_333n,
    recipient32: BigInt('0x' + '6b'.repeat(32)),
  };

  const seen = new Set();
  for (let sourceDomain = 0; sourceDomain < 4; sourceDomain += 1) {
    for (let destDomain = 0; destDomain < 4; destDomain += 1) {
      for (let nonce = 0n; nonce < 8n; nonce += 1n) {
        const id = burnPayloadToJsMessageId({ ...base, sourceDomain, destDomain, nonce });
        const key = `0x${id.toString(16).padStart(64, '0')}`;
        assert(
          !seen.has(key),
          `messageId collision in source/destination/nonce matrix at ${sourceDomain}->${destDomain} nonce=${nonce.toString()}`,
        );
        seen.add(key);
      }
    }
  }
  assert.equal(seen.size, 128);
});

test('SCCP messageId JS helper is unique across bounded source/destination/soraAssetId matrix', () => {
  const base = {
    nonce: 4_444n,
    amount: 44_444n,
    recipient32: BigInt('0x' + '7c'.repeat(32)),
  };

  const seen = new Set();
  for (let sourceDomain = 0; sourceDomain < 4; sourceDomain += 1) {
    for (let destDomain = 0; destDomain < 4; destDomain += 1) {
      for (let i = 0n; i < 8n; i += 1n) {
        const soraAssetId = 12_000n + i;
        const id = burnPayloadToJsMessageId({ ...base, sourceDomain, destDomain, soraAssetId });
        const key = `0x${id.toString(16).padStart(64, '0')}`;
        assert(
          !seen.has(key),
          `messageId collision in source/destination/soraAssetId matrix at ${sourceDomain}->${destDomain} assetOffset=${i.toString()}`,
        );
        seen.add(key);
      }
    }
  }
  assert.equal(seen.size, 128);
});

test('SCCP JS payload encoder preserves fixed field offsets at boundary-like values', () => {
  const payload = burnPayloadToJsBytes({
    sourceDomain: (1n << 32n) - 1n,
    destDomain: 0x01020304n,
    nonce: 0x0102030405060708n,
    soraAssetId: BigInt('0x' + 'aa'.repeat(32)),
    amount: (1n << 128n) - 1n,
    recipient32: BigInt('0x' + 'bb'.repeat(32)),
  });

  assert.equal(payload.length, 97);
  assert.equal(payload[0], 1);
  assert.equal(Buffer.from(payload.subarray(1, 5)).toString('hex'), 'ffffffff');
  assert.equal(Buffer.from(payload.subarray(5, 9)).toString('hex'), '04030201');
  assert.equal(Buffer.from(payload.subarray(9, 17)).toString('hex'), '0807060504030201');
  assert.equal(Buffer.from(payload.subarray(17, 49)).toString('hex'), 'aa'.repeat(32));
  assert.equal(Buffer.from(payload.subarray(49, 65)).toString('hex'), 'ff'.repeat(16));
  assert.equal(Buffer.from(payload.subarray(65, 97)).toString('hex'), 'bb'.repeat(32));
});

test('SCCP burn/attest prefix literals remain stable', () => {
  assert.equal(SCCP_BURN_PREFIX.toString('utf8'), 'sccp:burn:v1');
  assert.equal(Buffer.from('sccp:attest:v1', 'utf8').toString('utf8'), 'sccp:attest:v1');
});

test('SCCP message hash is sensitive to payload version byte changes', () => {
  const payloadV1 = burnPayloadToJsBytes({
    sourceDomain: 1,
    destDomain: 0,
    nonce: 500n,
    soraAssetId: BigInt('0x' + 'cd'.repeat(32)),
    amount: 999n,
    recipient32: BigInt('0x' + 'ef'.repeat(32)),
  });
  const payloadV2 = Buffer.from(payloadV1);
  payloadV2[0] = 2;

  const hashV1 = BigInt(ethers.keccak256(Buffer.concat([SCCP_BURN_PREFIX, payloadV1])));
  const hashV2 = BigInt(ethers.keccak256(Buffer.concat([SCCP_BURN_PREFIX, payloadV2])));
  assert.notEqual(hashV1, hashV2);
});

test('SCCP messageId JS reference encoder matches TVM for boundary values', async () => {
  const c = await openCodecContract();
  const fixture = {
    sourceDomain: 5, // TRON
    destDomain: 4, // TON
    nonce: (1n << 64n) - 1n,
    soraAssetId: BigInt('0x' + 'ff'.repeat(32)),
    amount: (1n << 128n) - 1n,
    recipient32: BigInt('0x' + 'aa'.repeat(32)),
  };

  const expected = burnPayloadToJsMessageId(fixture);
  const got = await c.getMessageId(
    fixture.sourceDomain,
    fixture.destDomain,
    fixture.nonce,
    fixture.soraAssetId,
    fixture.amount,
    fixture.recipient32,
  );

  assert.equal(got, expected);
});

test('SCCP messageId changes when payload fields change', async () => {
  const c = await openCodecContract();
  const base = {
    sourceDomain: 0,
    destDomain: 3,
    nonce: 42n,
    soraAssetId: BigInt('0x' + '11'.repeat(32)),
    amount: 123456789n,
    recipient32: BigInt('0x' + '22'.repeat(32)),
  };

  const baseExpected = burnPayloadToJsMessageId(base);
  const baseGot = await c.getMessageId(
    base.sourceDomain,
    base.destDomain,
    base.nonce,
    base.soraAssetId,
    base.amount,
    base.recipient32,
  );
  assert.equal(baseGot, baseExpected);

  const variants = [
    { ...base, sourceDomain: 1 },
    { ...base, destDomain: 4 },
    { ...base, nonce: base.nonce + 1n },
    { ...base, soraAssetId: BigInt('0x' + '33'.repeat(32)) },
    { ...base, amount: base.amount + 1n },
    { ...base, recipient32: BigInt('0x' + '44'.repeat(32)) },
  ];

  for (const variant of variants) {
    const expected = burnPayloadToJsMessageId(variant);
    const got = await c.getMessageId(
      variant.sourceDomain,
      variant.destDomain,
      variant.nonce,
      variant.soraAssetId,
      variant.amount,
      variant.recipient32,
    );
    assert.equal(got, expected);
    assert.notEqual(got, baseGot);
  }
});

test('SCCP messageId is direction-sensitive when source and destination are swapped', async () => {
  const c = await openCodecContract();
  const forward = {
    sourceDomain: 1,
    destDomain: 0,
    nonce: 9n,
    soraAssetId: BigInt('0x' + '55'.repeat(32)),
    amount: 77n,
    recipient32: BigInt('0x' + '66'.repeat(32)),
  };
  const reverse = {
    ...forward,
    sourceDomain: forward.destDomain,
    destDomain: forward.sourceDomain,
  };

  const forwardGot = await c.getMessageId(
    forward.sourceDomain,
    forward.destDomain,
    forward.nonce,
    forward.soraAssetId,
    forward.amount,
    forward.recipient32,
  );
  const reverseGot = await c.getMessageId(
    reverse.sourceDomain,
    reverse.destDomain,
    reverse.nonce,
    reverse.soraAssetId,
    reverse.amount,
    reverse.recipient32,
  );

  assert.equal(forwardGot, burnPayloadToJsMessageId(forward));
  assert.equal(reverseGot, burnPayloadToJsMessageId(reverse));
  assert.notEqual(forwardGot, reverseGot);
});

test('SCCP messageId JS reference encoder matches TVM for all-zero payload fields', async () => {
  const c = await openCodecContract();
  const fixture = {
    sourceDomain: 0,
    destDomain: 0,
    nonce: 0n,
    soraAssetId: 0n,
    amount: 0n,
    recipient32: 0n,
  };

  const expected = burnPayloadToJsMessageId(fixture);
  const got = await c.getMessageId(
    fixture.sourceDomain,
    fixture.destDomain,
    fixture.nonce,
    fixture.soraAssetId,
    fixture.amount,
    fixture.recipient32,
  );

  assert.equal(got, expected);
});

test('SCCP messageId JS reference encoder matches TVM for max uint32 domains', async () => {
  const c = await openCodecContract();
  const fixture = {
    sourceDomain: Number((1n << 32n) - 1n),
    destDomain: Number((1n << 32n) - 1n),
    nonce: 42n,
    soraAssetId: BigInt('0x' + '11'.repeat(32)),
    amount: 7n,
    recipient32: BigInt('0x' + '22'.repeat(32)),
  };

  const expected = burnPayloadToJsMessageId(fixture);
  const got = await c.getMessageId(
    fixture.sourceDomain,
    fixture.destDomain,
    fixture.nonce,
    fixture.soraAssetId,
    fixture.amount,
    fixture.recipient32,
  );

  assert.equal(got, expected);
});

test('SCCP messageId getter is deterministic for repeated identical inputs', async () => {
  const c = await openCodecContract();
  const fixture = {
    sourceDomain: 2,
    destDomain: 4,
    nonce: 12345n,
    soraAssetId: BigInt('0x' + '88'.repeat(32)),
    amount: 999n,
    recipient32: BigInt('0x' + '77'.repeat(32)),
  };

  const first = await c.getMessageId(
    fixture.sourceDomain,
    fixture.destDomain,
    fixture.nonce,
    fixture.soraAssetId,
    fixture.amount,
    fixture.recipient32,
  );
  const second = await c.getMessageId(
    fixture.sourceDomain,
    fixture.destDomain,
    fixture.nonce,
    fixture.soraAssetId,
    fixture.amount,
    fixture.recipient32,
  );

  assert.equal(first, second);
  assert.equal(first, burnPayloadToJsMessageId(fixture));
});

test('SCCP messageId getter matches JS helper over bounded source/destination matrix', async () => {
  const c = await openCodecContract();
  const base = {
    nonce: 4_321n,
    soraAssetId: BigInt('0x' + '12'.repeat(32)),
    amount: 65_535n,
    recipient32: BigInt('0x' + '34'.repeat(32)),
  };

  const seen = new Set();
  for (let sourceDomain = 0; sourceDomain < 8; sourceDomain += 1) {
    for (let destDomain = 0; destDomain < 8; destDomain += 1) {
      const fixture = {
        ...base,
        sourceDomain,
        destDomain,
      };
      const expected = burnPayloadToJsMessageId(fixture);
      const got = await c.getMessageId(
        fixture.sourceDomain,
        fixture.destDomain,
        fixture.nonce,
        fixture.soraAssetId,
        fixture.amount,
        fixture.recipient32,
      );

      assert.equal(got, expected);
      const key = `0x${got.toString(16).padStart(64, '0')}`;
      assert(
        !seen.has(key),
        `TVM messageId collision in source/destination matrix at ${sourceDomain}->${destDomain}`,
      );
      seen.add(key);
    }
  }
  assert.equal(seen.size, 64);
});

test('SCCP messageId getter matches JS helper over bounded source/destination extended matrix', async () => {
  const c = await openCodecContract();
  const base = {
    nonce: 8_765n,
    soraAssetId: BigInt('0x' + 'ef'.repeat(32)),
    amount: 12_345n,
    recipient32: BigInt('0x' + '01'.repeat(32)),
  };

  const seen = new Set();
  for (let sourceDomain = 0; sourceDomain < 8; sourceDomain += 1) {
    for (let destDomain = 0; destDomain < 16; destDomain += 1) {
      const fixture = {
        ...base,
        sourceDomain,
        destDomain,
      };
      const expected = burnPayloadToJsMessageId(fixture);
      const got = await c.getMessageId(
        fixture.sourceDomain,
        fixture.destDomain,
        fixture.nonce,
        fixture.soraAssetId,
        fixture.amount,
        fixture.recipient32,
      );
      assert.equal(got, expected);
      const key = `0x${got.toString(16).padStart(64, '0')}`;
      assert(
        !seen.has(key),
        `TVM messageId collision in extended source/destination matrix at ${sourceDomain}->${destDomain}`,
      );
      seen.add(key);
    }
  }
  assert.equal(seen.size, 128);
});

test('SCCP messageId getter matches JS helper over bounded source/destination/nonce matrix', async () => {
  const c = await openCodecContract();
  const base = {
    soraAssetId: BigInt('0x' + '23'.repeat(32)),
    amount: 54_321n,
    recipient32: BigInt('0x' + '45'.repeat(32)),
  };

  const seen = new Set();
  for (let sourceDomain = 0; sourceDomain < 4; sourceDomain += 1) {
    for (let destDomain = 0; destDomain < 4; destDomain += 1) {
      for (let nonce = 0n; nonce < 4n; nonce += 1n) {
        const fixture = {
          ...base,
          sourceDomain,
          destDomain,
          nonce,
        };
        const expected = burnPayloadToJsMessageId(fixture);
        const got = await c.getMessageId(
          fixture.sourceDomain,
          fixture.destDomain,
          fixture.nonce,
          fixture.soraAssetId,
          fixture.amount,
          fixture.recipient32,
        );
        assert.equal(got, expected);
        const key = `0x${got.toString(16).padStart(64, '0')}`;
        assert(
          !seen.has(key),
          `TVM messageId collision in source/destination/nonce matrix at ${sourceDomain}->${destDomain} nonce=${nonce.toString()}`,
        );
        seen.add(key);
      }
    }
  }
  assert.equal(seen.size, 64);
});

test('SCCP messageId getter matches JS helper over bounded nonce/amount matrix', async () => {
  const c = await openCodecContract();
  const base = {
    sourceDomain: 5,
    destDomain: 2,
    soraAssetId: BigInt('0x' + '56'.repeat(32)),
    recipient32: BigInt('0x' + '78'.repeat(32)),
  };

  const seen = new Set();
  for (let nonce = 0n; nonce < 8n; nonce += 1n) {
    for (let amount = 0n; amount < 8n; amount += 1n) {
      const fixture = {
        ...base,
        nonce,
        amount,
      };
      const expected = burnPayloadToJsMessageId(fixture);
      const got = await c.getMessageId(
        fixture.sourceDomain,
        fixture.destDomain,
        fixture.nonce,
        fixture.soraAssetId,
        fixture.amount,
        fixture.recipient32,
      );
      assert.equal(got, expected);
      const key = `0x${got.toString(16).padStart(64, '0')}`;
      assert(
        !seen.has(key),
        `TVM messageId collision in nonce/amount matrix at nonce=${nonce.toString()} amount=${amount.toString()}`,
      );
      seen.add(key);
    }
  }
  assert.equal(seen.size, 64);
});

test('SCCP messageId getter matches JS helper over bounded nonce/soraAssetId matrix', async () => {
  const c = await openCodecContract();
  const base = {
    sourceDomain: 3,
    destDomain: 1,
    amount: 87_654n,
    recipient32: BigInt('0x' + '67'.repeat(32)),
  };

  const seen = new Set();
  for (let nonce = 0n; nonce < 8n; nonce += 1n) {
    for (let i = 0n; i < 8n; i += 1n) {
      const fixture = {
        ...base,
        nonce,
        soraAssetId: 30_000n + i,
      };
      const expected = burnPayloadToJsMessageId(fixture);
      const got = await c.getMessageId(
        fixture.sourceDomain,
        fixture.destDomain,
        fixture.nonce,
        fixture.soraAssetId,
        fixture.amount,
        fixture.recipient32,
      );
      assert.equal(got, expected);
      const key = `0x${got.toString(16).padStart(64, '0')}`;
      assert(
        !seen.has(key),
        `TVM messageId collision in nonce/soraAssetId matrix at nonce=${nonce.toString()} assetOffset=${i.toString()}`,
      );
      seen.add(key);
    }
  }
  assert.equal(seen.size, 64);
});

test('SCCP messageId getter matches JS helper over bounded soraAssetId window', async () => {
  const c = await openCodecContract();
  const base = {
    sourceDomain: 1,
    destDomain: 4,
    nonce: 6_543n,
    amount: 98_765n,
    recipient32: BigInt('0x' + 'ab'.repeat(32)),
  };

  const seen = new Set();
  for (let i = 0n; i < 64n; i += 1n) {
    const fixture = {
      ...base,
      soraAssetId: 1_000n + i,
    };
    const expected = burnPayloadToJsMessageId(fixture);
    const got = await c.getMessageId(
      fixture.sourceDomain,
      fixture.destDomain,
      fixture.nonce,
      fixture.soraAssetId,
      fixture.amount,
      fixture.recipient32,
    );
    assert.equal(got, expected);
    const key = `0x${got.toString(16).padStart(64, '0')}`;
    assert(!seen.has(key), `TVM messageId collision in soraAssetId window at offset ${i.toString()}`);
    seen.add(key);
  }
  assert.equal(seen.size, 64);
});

test('SCCP messageId getter matches JS helper over bounded amount/recipient matrix', async () => {
  const c = await openCodecContract();
  const base = {
    sourceDomain: 2,
    destDomain: 5,
    nonce: 7_654n,
    soraAssetId: BigInt('0x' + 'cd'.repeat(32)),
  };

  const seen = new Set();
  for (let amount = 0n; amount < 8n; amount += 1n) {
    for (let i = 0n; i < 8n; i += 1n) {
      const fixture = {
        ...base,
        amount,
        recipient32: 20_000n + i,
      };
      const expected = burnPayloadToJsMessageId(fixture);
      const got = await c.getMessageId(
        fixture.sourceDomain,
        fixture.destDomain,
        fixture.nonce,
        fixture.soraAssetId,
        fixture.amount,
        fixture.recipient32,
      );
      assert.equal(got, expected);
      const key = `0x${got.toString(16).padStart(64, '0')}`;
      assert(
        !seen.has(key),
        `TVM messageId collision in amount/recipient matrix at amount=${amount.toString()} recipientOffset=${i.toString()}`,
      );
      seen.add(key);
    }
  }
  assert.equal(seen.size, 64);
});

test('SCCP messageId getter rejects out-of-range typed inputs', async () => {
  const c = await openCodecContract();
  const valid = {
    sourceDomain: 1n,
    destDomain: 0n,
    nonce: 1n,
    soraAssetId: 1n,
    amount: 1n,
    recipient32: 1n,
  };

  await assert.rejects(
    c.getMessageId(
      1n << 32n,
      valid.destDomain,
      valid.nonce,
      valid.soraAssetId,
      valid.amount,
      valid.recipient32,
    ),
    'sourceDomain > uint32 must be rejected',
  );

  await assert.rejects(
    c.getMessageId(
      valid.sourceDomain,
      1n << 32n,
      valid.nonce,
      valid.soraAssetId,
      valid.amount,
      valid.recipient32,
    ),
    'destDomain > uint32 must be rejected',
  );

  await assert.rejects(
    c.getMessageId(
      valid.sourceDomain,
      valid.destDomain,
      1n << 64n,
      valid.soraAssetId,
      valid.amount,
      valid.recipient32,
    ),
    'nonce > uint64 must be rejected',
  );

  await assert.rejects(
    c.getMessageId(
      valid.sourceDomain,
      valid.destDomain,
      valid.nonce,
      1n << 256n,
      valid.amount,
      valid.recipient32,
    ),
    'soraAssetId > uint256 must be rejected',
  );

  await assert.rejects(
    c.getMessageId(
      valid.sourceDomain,
      valid.destDomain,
      valid.nonce,
      valid.soraAssetId,
      valid.amount,
      1n << 256n,
    ),
    'recipient32 > uint256 must be rejected',
  );

  await assert.rejects(
    c.getMessageId(
      -1n,
      valid.destDomain,
      valid.nonce,
      valid.soraAssetId,
      valid.amount,
      valid.recipient32,
    ),
    'negative sourceDomain must be rejected',
  );

  await assert.rejects(
    c.getMessageId(
      valid.sourceDomain,
      -1n,
      valid.nonce,
      valid.soraAssetId,
      valid.amount,
      valid.recipient32,
    ),
    'negative destDomain must be rejected',
  );

  await assert.rejects(
    c.getMessageId(
      valid.sourceDomain,
      valid.destDomain,
      -1n,
      valid.soraAssetId,
      valid.amount,
      valid.recipient32,
    ),
    'negative nonce must be rejected',
  );

  await assert.rejects(
    c.getMessageId(
      valid.sourceDomain,
      valid.destDomain,
      valid.nonce,
      -1n,
      valid.amount,
      valid.recipient32,
    ),
    'negative soraAssetId must be rejected',
  );

  await assert.rejects(
    c.getMessageId(
      valid.sourceDomain,
      valid.destDomain,
      valid.nonce,
      valid.soraAssetId,
      valid.amount,
      -1n,
    ),
    'negative recipient32 must be rejected',
  );

  await assert.rejects(
    c.getMessageId(
      valid.sourceDomain,
      valid.destDomain,
      valid.nonce,
      valid.soraAssetId,
      1n << 128n,
      valid.recipient32,
    ),
    'amount > u128 must be rejected',
  );

  await assert.rejects(
    c.getMessageId(
      valid.sourceDomain,
      valid.destDomain,
      valid.nonce,
      valid.soraAssetId,
      -1n,
      valid.recipient32,
    ),
    'negative amount must be rejected',
  );
});
