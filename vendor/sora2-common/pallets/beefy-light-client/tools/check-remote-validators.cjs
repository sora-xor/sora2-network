#!/usr/bin/env node
'use strict';

// Read-only activation check. The operator must supply the remote network;
// neither local validator limits nor chain-spec names identify that network.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');

async function main() {
  const args = process.argv.slice(2);
  const options = {};
  for (let i = 0; i < args.length; i += 2) {
    if (!['--endpoint', '--api-root', '--output', '--max-validators'].includes(args[i]) || !args[i + 1]) {
      throw new Error('Usage: check-remote-validators.cjs --endpoint URL --api-root NODE_PROJECT --output FILE [--max-validators 1024]');
    }
    options[args[i]] = args[i + 1];
  }
  for (const name of ['--endpoint', '--api-root', '--output']) {
    if (!options[name]) throw new Error(`Missing ${name}`);
  }
  const endpoint = new URL(options['--endpoint']);
  if (!['http:', 'https:', 'ws:', 'wss:'].includes(endpoint.protocol) || endpoint.username || endpoint.password) {
    throw new Error('Supply an HTTP(S) or WS(S) endpoint without embedded credentials');
  }
  const limit = Number(options['--max-validators'] || 1024);
  if (!Number.isSafeInteger(limit) || limit < 1 || limit > 0xffffffff) {
    throw new Error('max-validators must be a positive u32');
  }
  const apiModule = require.resolve('@polkadot/api', { paths: [path.resolve(options['--api-root'])] });
  const { ApiPromise, HttpProvider, WsProvider } = require(apiModule);
  const provider = endpoint.protocol.startsWith('http') ? new HttpProvider(endpoint.href) : new WsProvider(endpoint.href);
  let api;
  // A disconnected RPC must not leave a release check reconnecting indefinitely.
  const timeout = setTimeout(() => { console.error('Remote validator check timed out'); process.exit(1); }, 120000);
  try {
    api = await ApiPromise.create({ provider, noInitWarn: true });
    const block = await api.rpc.chain.getFinalizedHead();
    const at = await api.at(block);
    const beefy = at.query.beefy;
    if (!beefy?.authorities || !beefy?.nextAuthorities || !beefy?.validatorSetId) {
      throw new Error('Pinned metadata does not expose BEEFY current/next authority sets');
    }
    const entries = [beefy.authorities, beefy.nextAuthorities, beefy.validatorSetId];
    const keys = entries.map(entry => entry.key());
    const [current, next, setId, header, version, metadata, proof, raw] = await Promise.all([
      beefy.authorities(), beefy.nextAuthorities(), beefy.validatorSetId(),
      api.rpc.chain.getHeader(block), api.rpc.state.getRuntimeVersion(block),
      api.rpc.state.getMetadata(block), api.rpc.state.getReadProof(keys, block),
      Promise.all(keys.map(key => api.rpc.state.getStorage(key, block))),
    ]);
    const fits = current.length > 0 && next.length > 0 && current.length <= limit && next.length <= limit;
    const maximum = at.consts.beefy?.maxAuthorities?.toString() ?? null;
    const result = {
      checkedAt: new Date().toISOString(),
      endpoint: `${endpoint.protocol}//${endpoint.host}${endpoint.pathname}`,
      scope: 'Pinned current and next sets; this is not a guarantee about future governance changes',
      candidateMaxValidators: limit,
      genesisHash: api.genesisHash.toHex(),
      blockHash: block.toHex(),
      header: header.toJSON(),
      runtimeVersion: version.toJSON(),
      metadataSha256: crypto.createHash('sha256').update(Buffer.from(metadata.toHex().slice(2), 'hex')).digest('hex'),
      validatorSetId: setId.toString(),
      currentAuthorities: current.length,
      nextAuthorities: next.length,
      remoteConfiguredMaxAuthorities: maximum,
      remoteConfiguredMaximumFits: maximum === null ? null : BigInt(maximum) <= BigInt(limit),
      currentAndNextFit: fits,
      storage: keys.map((key, i) => ({ key: key.toHex ? key.toHex() : key, value: raw[i].toJSON() })),
      readProof: proof.toJSON(),
    };
    fs.writeFileSync(options['--output'], JSON.stringify(result, null, 2) + '\n', { flag: 'wx' });
    console.log(JSON.stringify({ blockHash: result.blockHash, current: current.length, next: next.length, limit, fits }));
    if (!fits) process.exitCode = 2;
  } finally {
    clearTimeout(timeout);
    if (api) await api.disconnect(); else await provider.disconnect();
  }
}

main().catch(error => { console.error(error.message); process.exitCode = 1; });
