const assert = require('node:assert/strict');
const { createRequire } = require('node:module');
const { dirname, resolve } = require('node:path');
const { pathToFileURL } = require('node:url');
const test = require('node:test');

const projectRequire = createRequire(resolve(process.cwd(), 'package.json'));
const coreRoot = process.argv[2] || resolve(dirname(projectRequire.resolve('@acala-network/chopsticks-core')), '../..');
const coreRequire = createRequire(resolve(coreRoot, 'package.json'));
const { genesisSchema } = coreRequire('./dist/cjs/schema/index.js');
const { dev_newBlock } = coreRequire('./dist/cjs/rpc/dev/new-block.js');
const { dev_setHead } = coreRequire('./dist/cjs/rpc/dev/set-head.js');
const { z } = coreRequire('zod');
const hash = '0x' + '12'.repeat(32);

function genesis() {
  return { id: 'fixture', name: 'Fixture', properties: { tokenDecimals: [18], tokenSymbol: ['XOR'] },
    genesis: { raw: { top: { '0x1234': '0xabcd' } } } };
}

test('CommonJS genesis validation preserves storage records and strips unknown keys', () => {
  assert.deepEqual(genesisSchema.parse({ ...genesis(), ignored: true }), genesis());
  const invalid = genesis();
  invalid.genesis.raw.top['0x1234'] = 42;
  assert.equal(genesisSchema.safeParse(invalid).success, false);
});

test('ESM genesis validation uses the same migrated schema', async () => {
  const esm = await import(pathToFileURL(resolve(coreRoot, 'dist/esm/schema/index.js')));
  assert.deepEqual(esm.genesisSchema.parse(genesis()), genesis());
});

test('block RPC preserves counts, messages and overrides', async () => {
  const calls = [];
  const context = { chain: { head: { number: 10 }, newBlock: async args => { calls.push(args); return { hash }; } } };
  const params = { count: 2, transactions: ['0x1234'], dmp: [{ sentAt: 1, msg: '0xab' }],
    hrmp: { 2000: [{ sentAt: 2, data: '0xcd' }] }, relayChainStateOverrides: [['0x11', null]],
    unsafeBlockHeight: 20, relayParentNumber: 7 };
  assert.equal(await dev_newBlock(context, [params]), hash);
  assert.equal(calls.length, 2);
  assert.deepEqual(calls[0].transactions, params.transactions);
  assert.deepEqual(calls[0].downwardMessages, params.dmp);
  assert.deepEqual(calls[0].horizontalMessages, params.hrmp);
  assert.deepEqual(calls[0].relayChainStateOverrides, params.relayChainStateOverrides);
  assert.equal(calls[0].unsafeBlockHeight, 20);
  assert.equal(calls[1].unsafeBlockHeight, undefined);
});

test('invalid block RPC inputs fail before executing any blocks', async () => {
  const context = { chain: { head: { number: 10 }, newBlock: async () => assert.fail('invalid input executed') } };
  for (const params of [{ transactions: [] }, { transactions: ['invalid'] }, { dmp: [{ msg: '0xab' }] }]) {
    await assert.rejects(dev_newBlock(context, [params]), error => error.name === 'ZodError');
  }
});

test('set-head RPC preserves hash and relative-number lookup', async () => {
  const lookups = [];
  const context = { chain: { head: { number: 10 },
    getBlockAt: async number => { lookups.push(number); return { hash }; },
    getBlock: async value => { lookups.push(value); return { hash }; }, setHead: async () => {} } };
  assert.equal(await dev_setHead(context, [-2]), hash);
  assert.equal(await dev_setHead(context, [hash]), hash);
  assert.deepEqual(lookups, [8, hash]);
  await assert.rejects(dev_setHead(context, ['bad hash']), error => error.name === 'ZodError');
});

test('patched constrained arrays reject oversize input before per-element validation', () => {
  let visited = 0;
  const schema = z.array(z.custom(() => { visited += 1; return false; })).max(2);
  const result = schema.safeParse(Array(100_000).fill(null));
  assert.equal(result.success, false);
  assert.equal(visited, 0);
  assert.equal(result.error.issues.length, 1);
});
