#!/usr/bin/env node

'use strict';

const fs = require('fs');
const path = require('path');

function usage(message) {
  const lines = [
    'Usage:',
    '  node misc/sccp-e2e/src/fetch_nexus_bundle.js --torii-url <url> [--kind burn|governance] [--message-id 0x<32-byte>] [--poll-seconds <n>] [--max-attempts <n>] [--output-dir <path>]',
    '',
    'Reads message_id from --message-id or from SCCP_SCENARIO_CONTEXT_FILE when omitted.',
  ];
  if (message) {
    lines.push('', String(message));
  }
  process.stderr.write(`${lines.join('\n')}\n`);
  process.exit(1);
}

function parseArgs(argv) {
  const out = {
    toriiUrl: null,
    kind: null,
    messageId: null,
    pollSeconds: 5,
    maxAttempts: 10,
    outputDir: null,
  };

  for (let i = 2; i < argv.length; i += 1) {
    const arg = argv[i];
    const next = argv[i + 1];
    if (arg === '--torii-url' && next) {
      out.toriiUrl = next;
      i += 1;
    } else if (arg === '--kind' && next) {
      out.kind = next;
      i += 1;
    } else if (arg === '--message-id' && next) {
      out.messageId = next;
      i += 1;
    } else if (arg === '--poll-seconds' && next) {
      out.pollSeconds = Number(next);
      i += 1;
    } else if (arg === '--max-attempts' && next) {
      out.maxAttempts = Number(next);
      i += 1;
    } else if (arg === '--output-dir' && next) {
      out.outputDir = next;
      i += 1;
    } else if (arg === '--help' || arg === '-h') {
      usage();
    } else {
      usage(`Unknown or incomplete argument: ${arg}`);
    }
  }

  return out;
}

function readScenarioContext() {
  const filePath = process.env.SCCP_SCENARIO_CONTEXT_FILE;
  if (!filePath) {
    return null;
  }
  try {
    return JSON.parse(fs.readFileSync(filePath, 'utf8'));
  } catch (error) {
    usage(`Failed to read SCCP_SCENARIO_CONTEXT_FILE ${filePath}: ${error.message}`);
  }
}

function normalizeMessageId(value) {
  if (typeof value !== 'string' || !/^0x[0-9a-fA-F]{64}$/.test(value)) {
    usage(`message_id must be a 0x-prefixed 32-byte hex string, got '${value}'`);
  }
  return value.toLowerCase();
}

function normalizeKind(value) {
  if (value === 'burn' || value === 'governance') {
    return value;
  }
  usage(`kind must be 'burn' or 'governance', got '${value}'`);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function fetchBundleOnce(url, accept) {
  const response = await fetch(url, {
    headers: {
      accept,
    },
  });
  return response;
}

async function fetchBundleArtifacts(baseUrl, kind, messageId, pollSeconds, maxAttempts) {
  const bundleUrl = `${baseUrl.replace(/\/+$/, '')}/v1/sccp/proofs/${kind}/${messageId}`;
  let lastJsonStatus = null;
  let lastNoritoStatus = null;

  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    const jsonResponse = await fetchBundleOnce(bundleUrl, 'application/json');
    lastJsonStatus = jsonResponse.status;
    if (jsonResponse.status === 404) {
      if (attempt < maxAttempts) {
        await sleep(pollSeconds * 1000);
        continue;
      }
      usage(`Nexus bundle ${kind}/${messageId} was not found after ${maxAttempts} attempts`);
    }
    if (!jsonResponse.ok) {
      const body = await jsonResponse.text();
      usage(`Nexus JSON bundle fetch failed with HTTP ${jsonResponse.status}: ${body}`);
    }

    const noritoResponse = await fetchBundleOnce(bundleUrl, 'application/x-norito');
    lastNoritoStatus = noritoResponse.status;
    if (!noritoResponse.ok) {
      const body = await noritoResponse.text();
      usage(`Nexus Norito bundle fetch failed with HTTP ${noritoResponse.status}: ${body}`);
    }

    return {
      bundleUrl,
      jsonBundle: await jsonResponse.json(),
      noritoBytes: Buffer.from(await noritoResponse.arrayBuffer()),
      attempts: attempt,
    };
  }

  usage(
    `unreachable fetch loop state for ${kind}/${messageId} (last JSON status ${lastJsonStatus}, last Norito status ${lastNoritoStatus})`,
  );
}

async function main() {
  const args = parseArgs(process.argv);
  const context = readScenarioContext();
  const toriiUrl = args.toriiUrl || process.env.SCCP_NEXUS_TORII_URL;
  if (!toriiUrl) {
    usage('missing --torii-url and SCCP_NEXUS_TORII_URL');
  }

  const kind = normalizeKind(args.kind || process.env.SCCP_HUB_BUNDLE_KIND || 'burn');
  const messageId = normalizeMessageId(args.messageId || process.env.SCCP_MESSAGE_ID || context?.message_id);
  const outputDir = path.resolve(
    process.cwd(),
    args.outputDir || path.dirname(process.env.SCCP_SCENARIO_CONTEXT_FILE || '.'),
  );
  fs.mkdirSync(outputDir, { recursive: true });

  const { bundleUrl, jsonBundle, noritoBytes, attempts } = await fetchBundleArtifacts(
    toriiUrl,
    kind,
    messageId,
    args.pollSeconds,
    args.maxAttempts,
  );

  const stem = `nexus-${kind}-bundle-${messageId.slice(2)}`;
  const jsonPath = path.join(outputDir, `${stem}.json`);
  const noritoPath = path.join(outputDir, `${stem}.norito`);
  fs.writeFileSync(jsonPath, `${JSON.stringify(jsonBundle, null, 2)}\n`, 'utf8');
  fs.writeFileSync(noritoPath, noritoBytes);

  const result = {
    ok: true,
    kind,
    message_id: messageId,
    bundle_url: bundleUrl,
    attempts,
    bundle_json_path: jsonPath,
    bundle_norito_path: noritoPath,
    bundle_norito_hex: `0x${noritoBytes.toString('hex')}`,
    bundle_json: jsonBundle,
  };
  process.stdout.write(`${JSON.stringify(result)}\n`);
}

main().catch((error) => usage(error && error.message ? error.message : String(error)));
