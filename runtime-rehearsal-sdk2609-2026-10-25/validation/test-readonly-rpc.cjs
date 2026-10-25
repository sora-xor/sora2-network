'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { WsProvider } = require('@polkadot/rpc-provider');
const { installReadonlyRpc } = require('./readonly-rpc.cjs');

test('provider-level guard rejects submission and administrative methods before transport', () => {
  const real = WsProvider.prototype.send;
  let transported = 0;
  WsProvider.prototype.send = () => { transported++; };
  const report = { remoteRpcMethods: {} };
  const restore = installReadonlyRpc(report);
  try {
    for (const method of ['author_submitExtrinsic', 'author_submitAndWatchExtrinsic', 'author_insertKey', 'engine_createBlock', 'system_addReservedPeer']) {
      assert.throws(() => WsProvider.prototype.send.call({}, method, []), /Forbidden public RPC/);
    }
    assert.equal(transported, 0);
    assert.deepEqual(report.remoteRpcMethods, {});
  } finally { restore(); WsProvider.prototype.send = real; }
});

test('setup and cached reads pass through the same class with parameters and receiver intact', async () => {
  const real = WsProvider.prototype.send, calls = [], receiver = {};
  WsProvider.prototype.send = function(...params) { calls.push({ receiver: this, params }); return Promise.resolve('public-state'); };
  const report = { remoteRpcMethods: {} };
  const restore = installReadonlyRpc(report);
  try {
    assert.equal(await WsProvider.prototype.send.call(receiver, 'state_getStorage', ['0x1234', '0xabcd'], true), 'public-state');
    assert.equal(await WsProvider.prototype.send.call(receiver, 'system_chain', []), 'public-state');
    assert.deepEqual(calls[0], { receiver, params: ['state_getStorage', ['0x1234', '0xabcd'], true] });
    assert.deepEqual(report.remoteRpcMethods, { state_getStorage: 1, system_chain: 1 });
    assert.equal(report.rpcGuard.coversSetupAndPrivateCachedCalls, true);
  } finally { restore(); WsProvider.prototype.send = real; }
});

test('restoration removes the guard without replacing the original transport', () => {
  const real = WsProvider.prototype.send;
  const restore = installReadonlyRpc({ remoteRpcMethods: {} });
  assert.notEqual(WsProvider.prototype.send, real);
  restore();
  assert.equal(WsProvider.prototype.send, real);
});
