'use strict';
const assert = require('node:assert/strict');
const { WsProvider } = require('@polkadot/rpc-provider');
const allowed = new Set(['chain_getBlock', 'chain_getBlockHash', 'chain_getHeader', 'chain_getFinalizedHead',
  'state_getStorage', 'state_getKeysPaged', 'state_queryStorageAt', 'state_getMetadata', 'state_getRuntimeVersion',
  'state_call', 'system_properties', 'system_chain', 'system_name', 'system_version', 'rpc_methods',
  'state_subscribeRuntimeVersion', 'state_unsubscribeRuntimeVersion']);
exports.installReadonlyRpc = function(report) {
  const original = WsProvider.prototype.send;
  WsProvider.prototype.send = function(method, ...params) {
    assert(allowed.has(method), 'Forbidden public RPC: ' + method);
    report.remoteRpcMethods[method] = (report.remoteRpcMethods[method] || 0) + 1;
    return original.call(this, method, ...params);
  };
  report.rpcGuard = { layer: 'WsProvider.prototype.send, before setup', allowlist: [...allowed].sort(), coversSetupAndPrivateCachedCalls: true };
  return () => { WsProvider.prototype.send = original; };
};
