#!/usr/bin/env node

const { spawn } = require('child_process');
const fs = require('fs');
const path = require('path');
const process = require('process');
const { ApiPromise, Keyring, WsProvider } = require('@polkadot/api');
const { cryptoWaitReady } = require('@polkadot/util-crypto');

class McpStdioClient {
  constructor({ cmd, args, cwd, env }) {
    this.cmd = cmd;
    this.args = args;
    this.cwd = cwd;
    this.env = env;
    this.nextId = 1;
    this.pending = new Map();
    this.buffer = Buffer.alloc(0);
    this.child = null;
    this.stderr = '';
  }

  async start() {
    this.child = spawn(this.cmd, this.args, {
      cwd: this.cwd,
      env: this.env,
      stdio: ['pipe', 'pipe', 'pipe'],
    });

    this.child.stdout.on('data', (chunk) => {
      this.buffer = Buffer.concat([this.buffer, chunk]);
      this.processFrames();
    });

    this.child.stderr.on('data', (chunk) => {
      this.stderr += chunk.toString('utf8');
    });

    this.child.on('exit', (code, signal) => {
      const reason = `MCP process exited (code=${code}, signal=${signal})`;
      for (const { reject } of this.pending.values()) {
        reject(new Error(`${reason}\n${this.stderr}`));
      }
      this.pending.clear();
    });
  }

  async request(method, params) {
    const id = this.nextId++;
    const payload = {
      jsonrpc: '2.0',
      id,
      method,
      params,
    };

    const body = Buffer.from(JSON.stringify(payload), 'utf8');
    const header = Buffer.from(`Content-Length: ${body.length}\r\n\r\n`, 'utf8');

    const responsePromise = new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
    });

    this.child.stdin.write(Buffer.concat([header, body]));
    return responsePromise;
  }

  async toolCall(name, argumentsObject) {
    const response = await this.request('tools/call', {
      name,
      arguments: argumentsObject,
    });

    if (response.error) {
      throw new Error(`MCP tools/call error for ${name}: ${JSON.stringify(response.error)}`);
    }

    const result = response.result || {};
    if (result.isError) {
      throw new Error(`MCP tool ${name} returned isError=true: ${JSON.stringify(result)}`);
    }

    if (result.structuredContent) {
      return result.structuredContent;
    }

    const firstText = result.content && result.content[0] && result.content[0].text;
    if (typeof firstText === 'string') {
      return JSON.parse(firstText);
    }

    return result;
  }

  processFrames() {
    while (true) {
      const headerEnd = this.buffer.indexOf(Buffer.from('\r\n\r\n'));
      if (headerEnd === -1) {
        return;
      }

      const headerRaw = this.buffer.slice(0, headerEnd).toString('utf8');
      const match = headerRaw.match(/Content-Length:\s*(\d+)/i);
      if (!match) {
        throw new Error(`Missing Content-Length header in MCP frame: ${headerRaw}`);
      }

      const contentLength = Number(match[1]);
      const frameStart = headerEnd + 4;
      const frameEnd = frameStart + contentLength;
      if (this.buffer.length < frameEnd) {
        return;
      }

      const jsonBody = this.buffer.slice(frameStart, frameEnd).toString('utf8');
      this.buffer = this.buffer.slice(frameEnd);

      const message = JSON.parse(jsonBody);
      if (message.id === undefined || message.id === null) {
        continue;
      }

      const pending = this.pending.get(message.id);
      if (!pending) {
        continue;
      }
      this.pending.delete(message.id);
      pending.resolve(message);
    }
  }

  async close() {
    if (!this.child) {
      return;
    }
    this.child.stdin.end();
    this.child.kill('SIGTERM');
  }
}

async function waitForFinalizedTx(api, txHash, timeoutMs) {
  const lowerHash = txHash.toLowerCase();

  return new Promise(async (resolve, reject) => {
    let done = false;
    let unsub = null;

    const timeout = setTimeout(() => {
      finish(
        new Error(
          `Timed out waiting for finalized inclusion of extrinsic ${txHash} after ${timeoutMs} ms`
        )
      );
    }, timeoutMs);

    async function finish(err, result) {
      if (done) {
        return;
      }
      done = true;
      clearTimeout(timeout);
      try {
        if (unsub) {
          await unsub();
        }
      } catch (_) {}

      if (err) {
        reject(err);
      } else {
        resolve(result);
      }
    }

    try {
      unsub = await api.rpc.chain.subscribeFinalizedHeads(async (header) => {
        try {
          const blockHash = header.hash.toHex();
          const block = await api.rpc.chain.getBlock(blockHash);
          const index = block.block.extrinsics.findIndex(
            (ex) => ex.hash.toHex().toLowerCase() === lowerHash
          );

          if (index < 0) {
            return;
          }

          const allEvents = await api.query.system.events.at(blockHash);
          const extrinsicEvents = allEvents
            .filter(
              ({ phase }) => phase.isApplyExtrinsic && phase.asApplyExtrinsic.toNumber() === index
            )
            .map(({ event }) => ({
              section: event.section,
              method: event.method,
              data: event.data.toHuman(),
            }));

          const hasSuccess = extrinsicEvents.some(
            (evt) => evt.section === 'system' && evt.method === 'ExtrinsicSuccess'
          );
          const hasFailed = extrinsicEvents.some(
            (evt) => evt.section === 'system' && evt.method === 'ExtrinsicFailed'
          );
          const outcome = hasSuccess ? 'success' : hasFailed ? 'failed' : 'unknown';

          await finish(null, {
            blockHash,
            extrinsicIndex: index,
            events: extrinsicEvents,
            outcome,
          });
        } catch (err) {
          await finish(err);
        }
      });
    } catch (err) {
      await finish(err);
    }
  });
}

function requiredString(value, field) {
  if (!value || typeof value[field] !== 'string') {
    throw new Error(`Missing string field '${field}' in object: ${JSON.stringify(value)}`);
  }
  return value[field];
}

async function main() {
  const mcpDir = path.resolve(__dirname, '..');

  const wsUrl = process.env.SORA_WS_URL || 'ws://127.0.0.1:9944';
  const connectTimeoutMs = Number(process.env.CONNECT_TIMEOUT_MS || 15000);
  const mcpNetwork = process.env.MCP_NETWORK || 'sora_testnet';
  const signerUri = process.env.SIGNER_URI || '//Alice';
  const timeoutMs = Number(process.env.WATCH_TIMEOUT_MS || 120000);
  const callName = process.env.SCCP_CALL_NAME || 'set_outbound_domain_paused';
  const callArgs = process.env.SCCP_CALL_ARGS
    ? JSON.parse(process.env.SCCP_CALL_ARGS)
    : { domain_id: 1, paused: false };

  const defaultConfig = fs.existsSync(path.join(mcpDir, 'config.toml'))
    ? path.join(mcpDir, 'config.toml')
    : path.join(mcpDir, 'config.example.toml');
  const mcpConfig = process.env.SCCP_MCP_CONFIG || defaultConfig;

  const provider = new WsProvider(wsUrl);
  let api;
  try {
    api = await withTimeout(
      ApiPromise.create({ provider }),
      connectTimeoutMs,
      `Timed out connecting to SORA WS endpoint ${wsUrl}`
    );
  } catch (err) {
    try {
      await provider.disconnect();
    } catch (_) {}
    throw err;
  }
  await cryptoWaitReady();

  const keyring = new Keyring({ type: 'sr25519' });
  const signer = keyring.addFromUri(signerUri);

  const mcp = new McpStdioClient({
    cmd: 'cargo',
    args: ['run', '--quiet'],
    cwd: mcpDir,
    env: {
      ...process.env,
      SCCP_MCP_CONFIG: mcpConfig,
    },
  });

  await mcp.start();

  try {
    const initResponse = await mcp.request('initialize', {});
    if (initResponse.error) {
      throw new Error(`MCP initialize failed: ${JSON.stringify(initResponse.error)}`);
    }

    const buildCall = await mcp.toolCall('sora_sccp_build_call', {
      network: mcpNetwork,
      call_name: callName,
      signer: signer.address,
      args: callArgs,
    });

    const callDataHex = requiredString(buildCall, 'call_data_hex');
    const call = api.registry.createType('Call', callDataHex);
    const tx = api.tx(call);
    await tx.signAsync(signer);
    const signedExtrinsicHex = tx.toHex();

    const submitResult = await mcp.toolCall('sora_sccp_submit_signed_extrinsic', {
      network: mcpNetwork,
      signed_extrinsic_hex: signedExtrinsicHex,
    });
    const txHash = requiredString(submitResult, 'tx_hash');

    const finalized = await waitForFinalizedTx(api, txHash, timeoutMs);

    if (!Array.isArray(finalized.events) || finalized.events.length === 0) {
      throw new Error(
        `Finalized extrinsic ${txHash} had no decoded events for its extrinsic index`
      );
    }

    if (finalized.outcome === 'unknown') {
      throw new Error(
        `Finalized extrinsic ${txHash} had decoded events but no ExtrinsicSuccess/ExtrinsicFailed`
      );
    }

    const summary = {
      ws_url: wsUrl,
      network: mcpNetwork,
      signer: signer.address,
      call_name: callName,
      call_args: callArgs,
      tx_hash: txHash,
      finalized_block: finalized.blockHash,
      extrinsic_index: finalized.extrinsicIndex,
      outcome: finalized.outcome,
      events: finalized.events,
    };

    process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
    process.stdout.write(
      `[integration] finalized extrinsic observed with decoded events (${finalized.outcome})\n`
    );
  } finally {
    await mcp.close();
    await api.disconnect();
  }
}

async function withTimeout(promise, timeoutMs, message) {
  let timer = null;
  const timeoutPromise = new Promise((_, reject) => {
    timer = setTimeout(() => reject(new Error(message)), timeoutMs);
  });

  try {
    return await Promise.race([promise, timeoutPromise]);
  } finally {
    if (timer) {
      clearTimeout(timer);
    }
  }
}

main().catch((err) => {
  process.stderr.write(`[integration] failed: ${err.stack || err.message}\n`);
  process.exit(1);
});
