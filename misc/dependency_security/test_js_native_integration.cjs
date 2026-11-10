#!/usr/bin/env node
'use strict';
const assert = require('node:assert/strict');
const { createRequire } = require('node:module');
const { pathToFileURL } = require('node:url');
const { execFileSync } = require('node:child_process');
const path = require('node:path');
const http = require('node:http');
const { once } = require('node:events');
const { test } = require('node:test');
const root = path.resolve(process.argv[2] || process.cwd());
const request = createRequire(path.join(root, 'package.json'));
const crypto = request('@polkadot/wasm-crypto');

test('retired embedded Wasm and asm.js packages are absent', () => {
  for (const name of ['@substrate/connect', 'smoldot', '@polkadot/wasm-crypto-asmjs', '@polkadot/wasm-crypto-init', '@polkadot/wasm-bridge', 'node-domexception']) {
    assert.throws(() => request.resolve(name), { code: 'MODULE_NOT_FOUND' });
  }
});
test('CJS readiness, explicit disable, unsupported asm.js, and restoration', async () => {
  assert.equal(crypto.isReady(), false);
  assert.throws(() => crypto.sha256(new Uint8Array()), /not been initialized/);
  assert.equal(await crypto.waitReady(), true);
  const known = 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855';
  assert.equal(Buffer.from(crypto.sha256(new Uint8Array())).toString('hex'), known);
  await request('@polkadot/wasm-crypto/initNone').initWasm();
  assert.equal(await crypto.waitReady(), false);
  await request('@polkadot/wasm-crypto/initOnlyAsm').initWasm();
  assert.equal(await crypto.waitReady(), false);
  assert.match(crypto.bridge.error, /asm.js crypto is no longer supported/);
  await request('@polkadot/wasm-crypto/initOnlyWasm').initWasm();
  assert.equal(crypto.isReady(), true);
  assert.equal(crypto.bridge.type, 'wasm');
  assert.equal(crypto.bridge.error, null);
});
test('ESM crypto entrypoint uses the reviewed generated module', () => {
  const cjs = request.resolve('@polkadot/wasm-crypto');
  const url = pathToFileURL(path.resolve(path.dirname(cjs), '../index.js')).href;
  const result = execFileSync(process.execPath, ['--input-type=module', '-e', `
    import { waitReady, blake2b } from ${JSON.stringify(url)};
    if (!await waitReady()) throw new Error('ESM Wasm initialization failed');
    console.log(Buffer.from(blake2b(new Uint8Array([1,2,3]), new Uint8Array(), 32)).toString('hex'));
  `], { encoding: 'utf8' }).trim();
  assert.equal(result, Buffer.from(crypto.blake2b(new Uint8Array([1,2,3]), new Uint8Array(), 32)).toString('hex'));
});
test('Polkadot keyring signs and verifies through the reviewed facade', async () => {
  const { Keyring } = request('@polkadot/keyring');
  const { cryptoWaitReady, signatureVerify } = request('@polkadot/util-crypto');
  assert.equal(await cryptoWaitReady(), true);
  for (const type of ['ed25519', 'sr25519', 'ecdsa']) {
    const pair = new Keyring({ type }).addFromUri('//Alice');
    const message = Uint8Array.from([1, 7, 9, 42]);
    const signature = pair.sign(message);
    assert.equal(pair.verify(message, signature, pair.publicKey), true, type);
    assert.equal(signatureVerify(message, signature, pair.publicKey).isValid, true, type);
    assert.equal(pair.verify(Uint8Array.from([2,7,9,42]), signature, pair.publicKey), false, type);
  }
});
test('HTTP provider retains local JSON-RPC request and result behavior', async () => {
  const { HttpProvider } = request('@polkadot/rpc-provider');
  const server = http.createServer((req,res) => {
    const chunks=[];
    req.on('data', chunk=>chunks.push(chunk));
    req.on('end',()=>{ const rpc=JSON.parse(Buffer.concat(chunks)); assert.equal(rpc.method,'system_chain'); assert.deepEqual(rpc.params,[]); res.setHeader('content-type','application/json'); res.end(JSON.stringify({jsonrpc:'2.0',id:rpc.id,result:'SORA Nexus test'})); });
  });
  server.listen(0,'127.0.0.1'); await once(server,'listening');
  try { const provider=new HttpProvider(`http://127.0.0.1:${server.address().port}`); assert.equal(await provider.send('system_chain',[]),'SORA Nexus test'); await provider.disconnect(); }
  finally { await new Promise(resolve=>server.close(resolve)); }
});
test('WebSocket provider retains local JSON-RPC request and result behavior', async () => {
  const { WsProvider } = request('@polkadot/rpc-provider');
  const { WebSocketServer } = request('ws');
  const server=new WebSocketServer({port:0,host:'127.0.0.1'}); await once(server,'listening');
  server.on('connection',socket=>socket.on('message',data=>{ const rpc=JSON.parse(data); socket.send(JSON.stringify({jsonrpc:'2.0',id:rpc.id,result:{method:rpc.method,params:rpc.params}})); }));
  const provider=new WsProvider(`ws://127.0.0.1:${server.address().port}`,false);
  try { await provider.connect(); await provider.isReady; assert.deepEqual(await provider.send('system_health',[]),{method:'system_health',params:[]}); }
  finally { await provider.disconnect(); for(const client of server.clients)client.terminate(); await new Promise(resolve=>server.close(resolve)); }
});
test('ScProvider continues to require an explicitly injected light client', async () => {
  const { ScProvider } = request('@polkadot/rpc-provider');
  assert.throws(()=>new ScProvider(undefined,'test'),/interface as first parameter/);
  const client={WellKnownChain:{test:'test'},createScClient:()=>{throw new Error('caller supplied client used');}};
  const provider=new ScProvider(client,'test');
  await assert.rejects(provider.connect(),/caller supplied client used/);
});
