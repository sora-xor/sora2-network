#!/usr/bin/env node
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, resolve } from 'node:path';

import { Address, Cell, Dictionary, SendMode, beginCell, contractAddress, toNano } from '@ton/core';

const ACK_TOKEN = 'I_UNDERSTAND_MAINNET_DEPLOY';
const STATE_VERSION = 1;

const TOP_UP_TONS = 0xd372158c;


function parseArgs(argv) {
  const valueFlags = new Set([
    'endpoint',
    'mnemonic-file',
    'governor',
    'sora-asset-id',
    'metadata-uri',
    'master-value',
    'verifier-value',
    'ack-mainnet',
    'out',
    'state-file',
  ]);
  const booleanFlags = new Set(['execute', 'resume']);
  const out = {};
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (!a.startsWith('--')) {
      throw new Error(`Unexpected positional argument: ${a}`);
    }
    const key = a.slice(2);
    if (!valueFlags.has(key) && !booleanFlags.has(key)) {
      throw new Error(`Unknown argument: ${a}`);
    }
    if (Object.hasOwn(out, key)) {
      throw new Error(`Duplicate argument: ${a}`);
    }
    if (booleanFlags.has(key)) {
      out[key] = true;
      continue;
    }
    const nxt = argv[i + 1];
    if (nxt === undefined || nxt.startsWith('--')) {
      throw new Error(`Missing value for --${key}`);
    }
    out[key] = nxt;
    i += 1;
  }
  return out;
}

function requireArg(args, key) {
  const v = args[key];
  if (v === undefined || v === null || v === '') {
    throw new Error(`Missing required --${key}`);
  }
  return v;
}

function parseHexU256(s, name) {
  const hex = s.startsWith('0x') ? s.slice(2) : s;
  if (!/^[0-9a-fA-F]{64}$/.test(hex)) {
    throw new Error(`${name} must be a 32-byte hex value (64 hex chars)`);
  }
  return BigInt(`0x${hex.toLowerCase()}`);
}

function loadArtifact(repoRoot, name) {
  const path = resolve(repoRoot, 'artifacts', name);
  if (!existsSync(path)) {
    throw new Error(`Missing artifact: ${path}`);
  }
  return JSON.parse(readFileSync(path, 'utf8'));
}

function codeFromArtifact(artifact) {
  return Cell.fromBoc(Buffer.from(artifact.codeBoc64, 'base64'))[0];
}

function sanitizeEndpointHost(endpoint) {
  try {
    return new URL(endpoint).host;
  } catch {
    return '<redacted>';
  }
}

function sortDeep(value) {
  if (Array.isArray(value)) return value.map(sortDeep);
  if (value && typeof value === 'object') {
    const out = {};
    for (const k of Object.keys(value).sort()) {
      out[k] = sortDeep(value[k]);
    }
    return out;
  }
  return value;
}

function hashParams(input) {
  const payload = JSON.stringify(sortDeep(input));
  return createHash('sha256').update(payload).digest('hex');
}

function readJsonFile(path) {
  return JSON.parse(readFileSync(path, 'utf8'));
}

function atomicWriteJson(path, value) {
  mkdirSync(dirname(path), { recursive: true });
  const tmp = `${path}.tmp`;
  writeFileSync(tmp, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
  renameSync(tmp, path);
}

function legacyStatePath(repoRoot, governorAddr) {
  const governorSuffix = governorAddr.hash.toString('hex').slice(0, 10);
  return resolve(repoRoot, 'deployments', 'state', `mainnet-ton-${governorSuffix}.json`);
}

function defaultStatePath(repoRoot, governorAddr, soraAssetIdU256) {
  const governorSuffix = governorAddr.hash.toString('hex').slice(0, 10);
  const assetSuffix = soraAssetIdU256.toString(16).slice(0, 10);
  return resolve(repoRoot, 'deployments', 'state', `mainnet-ton-${governorSuffix}-${assetSuffix}.json`);
}

function resolveStatePath({ args, repoRoot, governor, soraAssetIdU256, resume }) {
  if (args['state-file']) {
    return args['state-file'];
  }
  const nextPath = defaultStatePath(repoRoot, governor, soraAssetIdU256);
  if (!resume) {
    return nextPath;
  }
  const legacyPath = legacyStatePath(repoRoot, governor);
  if (!existsSync(nextPath) && existsSync(legacyPath)) {
    return legacyPath;
  }
  return nextPath;
}

function buildSnakeDataCell(data) {
  const chunkSize = 127;
  if (data.length === 0) {
    return beginCell().endCell();
  }

  let tail = null;
  for (let offset = data.length; offset > 0; offset -= chunkSize) {
    const start = Math.max(0, offset - chunkSize);
    const chunk = data.subarray(start, offset);
    const b = beginCell().storeBuffer(chunk);
    if (tail) {
      b.storeRef(tail);
    }
    tail = b.endCell();
  }

  return tail;
}

function ensureStatePolicy({ execute, resume, stateFile }) {
  const exists = existsSync(stateFile);
  if (!execute) return;
  if (resume && !exists) {
    throw new Error(`--resume requested but state file does not exist: ${stateFile}`);
  }
  if (!resume && exists) {
    throw new Error(
      `State file already exists: ${stateFile}. Use --resume or pass a different --state-file.`,
    );
  }
}

function buildMasterData({ governor, verifier, walletCode, metadataUri, soraAssetIdU256 }) {
  const emptyBoolMap = Dictionary.empty(Dictionary.Keys.BigUint(256), Dictionary.Values.Bool());
  const emptyBurnsMap = Dictionary.empty(Dictionary.Keys.BigUint(256), Dictionary.Values.Cell());

  const sccpExtraB = beginCell();
  sccpExtraB.storeUint(soraAssetIdU256, 256);
  sccpExtraB.storeUint(0, 64);
  sccpExtraB.storeUint(0, 64);
  sccpExtraB.storeUint(0, 64);
  emptyBoolMap.store(sccpExtraB);
  emptyBoolMap.store(sccpExtraB);
  emptyBurnsMap.store(sccpExtraB);
  const sccpExtra = sccpExtraB.endCell();

  const metadataCell = buildSnakeDataCell(Buffer.from(metadataUri ?? '', 'utf8'));

  return beginCell()
    .storeCoins(0n)
    .storeAddress(governor)
    .storeAddress(verifier ?? null)
    .storeRef(walletCode)
    .storeRef(metadataCell)
    .storeRef(sccpExtra)
    .endCell();
}

function buildVerifierData({ governor, jettonMaster, soraAssetIdU256 }) {
  const emptyMmrRootsMap = Dictionary.empty(Dictionary.Keys.Uint(16), Dictionary.Values.BigUint(256));
  const emptyKnownRootsMap = Dictionary.empty(Dictionary.Keys.BigUint(256), Dictionary.Values.Bool());

  const stB = beginCell();
  stB.storeBit(0);
  stB.storeUint(0, 64);
  stB.storeUint(0, 64);
  stB.storeUint(0, 32);
  stB.storeUint(0n, 256);
  stB.storeUint(0, 64);
  stB.storeUint(0, 32);
  stB.storeUint(0n, 256);
  stB.storeUint(0, 16);
  emptyMmrRootsMap.store(stB);
  emptyKnownRootsMap.store(stB);
  const stCell = stB.endCell();

  return beginCell()
    .storeAddress(governor)
    .storeAddress(jettonMaster)
    .storeUint(soraAssetIdU256, 256)
    .storeRef(stCell)
    .endCell();
}

function buildTopUpBody() {
  return beginCell().storeUint(TOP_UP_TONS, 32).endCell();
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitSeqnoAdvance(walletContract, fromSeqno, timeoutMs = 120000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const cur = await walletContract.getSeqno();
    if (cur > fromSeqno) {
      return cur;
    }
    await sleep(1500);
  }
  throw new Error(`Timed out waiting for wallet seqno > ${fromSeqno}`);
}

function assertEndpointLooksMainnet(endpoint) {
  const s = endpoint.toLowerCase();
  if (s.includes('testnet') || s.includes('sandbox')) {
    throw new Error(`Endpoint appears non-mainnet: ${sanitizeEndpointHost(endpoint)}`);
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));

  const endpoint = args.endpoint ?? 'https://mainnet-v4.tonhubapi.com';
  assertEndpointLooksMainnet(endpoint);

  const mnemonicFile = requireArg(args, 'mnemonic-file');
  if (!existsSync(mnemonicFile)) {
    throw new Error(`Mnemonic file not found: ${mnemonicFile}`);
  }
  const mnemonic = readFileSync(mnemonicFile, 'utf8').trim();
  if (!mnemonic) {
    throw new Error(`Mnemonic file is empty: ${mnemonicFile}`);
  }

  const governor = Address.parse(requireArg(args, 'governor'));
  const soraAssetIdU256 = parseHexU256(requireArg(args, 'sora-asset-id'), 'sora-asset-id');
  const metadataUri = args['metadata-uri'] ?? '';

  const masterDeployTon = args['master-value'] ?? '0.25';
  const verifierDeployTon = args['verifier-value'] ?? '0.45';
  const execute = Boolean(args.execute);
  const resume = Boolean(args.resume);
  if (execute && args['ack-mainnet'] !== ACK_TOKEN) {
    throw new Error(`Mainnet execution requires --ack-mainnet ${ACK_TOKEN}`);
  }

  const repoRoot = resolve(import.meta.dirname, '..');
  const masterArtifact = loadArtifact(repoRoot, 'sccp-jetton-master.compiled.json');
  const walletArtifact = loadArtifact(repoRoot, 'sccp-jetton-wallet.compiled.json');
  const verifierArtifact = loadArtifact(repoRoot, 'sccp-sora-verifier.compiled.json');

  const masterCode = codeFromArtifact(masterArtifact);
  const walletCode = codeFromArtifact(walletArtifact);
  const verifierCode = codeFromArtifact(verifierArtifact);

  const masterInit = {
    code: masterCode,
    data: buildMasterData({
      governor,
      verifier: null,
      walletCode,
      metadataUri,
      soraAssetIdU256,
    }),
  };
  const masterAddress = contractAddress(0, masterInit);

  const verifierInit = {
    code: verifierCode,
    data: buildVerifierData({
      governor,
      jettonMaster: masterAddress,
      soraAssetIdU256,
    }),
  };
  const verifierAddress = contractAddress(0, verifierInit);

  const outPath =
    args.out ??
    resolve(
      repoRoot,
      'deployments',
      `mainnet-ton-${new Date().toISOString().replace(/[:.]/g, '-')}.json`,
    );

  const stateFile = resolveStatePath({ args, repoRoot, governor, soraAssetIdU256, resume });
  ensureStatePolicy({ execute, resume, stateFile });

  const paramsHash = hashParams({
    version: STATE_VERSION,
    endpointHost: sanitizeEndpointHost(endpoint),
    governor: governor.toString(),
    soraAssetIdHex: `0x${soraAssetIdU256.toString(16).padStart(64, '0')}`,
    metadataUri,
    values: {
      masterDeployTon,
      verifierDeployTon,
    },
    masterCodeHashHex: masterArtifact.codeHashHex,
    verifierCodeHashHex: verifierArtifact.codeHashHex,
    walletCodeHashHex: walletArtifact.codeHashHex,
  });

  const result = {
    chain: 'ton',
    endpointHost: sanitizeEndpointHost(endpoint),
    governor: governor.toString(),
    soraAssetIdHex: `0x${soraAssetIdU256.toString(16).padStart(64, '0')}`,
    master: {
      address: masterAddress.toString(),
      accountIdHex: masterAddress.hash.toString('hex'),
      codeHashHex: masterArtifact.codeHashHex,
    },
    verifier: {
      address: verifierAddress.toString(),
      accountIdHex: verifierAddress.hash.toString('hex'),
      codeHashHex: verifierArtifact.codeHashHex,
    },
    walletCodeHashHex: walletArtifact.codeHashHex,
    valuesTon: {
      masterDeploy: masterDeployTon,
      verifierDeploy: verifierDeployTon,
    },
    stateFile,
    paramsHash,
    outPath,
    timestamp: new Date().toISOString(),
    note: 'Master/verifier binding happens when the verifier is initialized on-chain; no post-deploy setVerifier step is used.',
  };

  if (!execute) {
    result.mode = 'dry-run';
    result.note =
      'No transactions sent. Re-run with --execute --ack-mainnet I_UNDERSTAND_MAINNET_DEPLOY. Master/verifier binding happens when the verifier is initialized on-chain; no post-deploy setVerifier step is used.';
    console.log(JSON.stringify(result, null, 2));
    return;
  }

  let TonClient4;
  let WalletContractV4;
  let internal;
  let mnemonicToPrivateKey;

  try {
    ({ TonClient4, WalletContractV4, internal } = await import('@ton/ton'));
    ({ mnemonicToPrivateKey } = await import('@ton/crypto'));
  } catch (e) {
    throw new Error(
      `Missing TON runtime deps. Install with: npm install @ton/ton @ton/crypto\nOriginal error: ${e}`,
    );
  }

  const words = mnemonic.split(/\s+/).filter(Boolean);
  if (words.length < 12) {
    throw new Error('mnemonic should have at least 12 words');
  }

  const keyPair = await mnemonicToPrivateKey(words);
  const wallet = WalletContractV4.create({ workchain: 0, publicKey: keyPair.publicKey });
  const client = new TonClient4({ endpoint });
  const walletContract = client.open(wallet);

  // Connectivity + stronger identity guard.
  const lastBlock = await client.getLastBlock();
  if (!lastBlock?.last?.seqno || lastBlock.last.seqno <= 0) {
    throw new Error(`Unable to verify endpoint health for ${sanitizeEndpointHost(endpoint)}`);
  }

  const deployerAddress = wallet.address.toString();
  result.deployer = deployerAddress;

  const nowIso = () => new Date().toISOString();

  let state;
  if (resume) {
    state = readJsonFile(stateFile);
    if (!state || state.version !== STATE_VERSION) {
      throw new Error(`Invalid state file version in ${stateFile}`);
    }
    if (state.paramsHash !== paramsHash) {
      throw new Error(
        `State params hash mismatch for ${stateFile}. Refusing to resume with different inputs.`,
      );
    }
  } else {
    state = {
      version: STATE_VERSION,
      chain: 'ton',
      createdAt: nowIso(),
      updatedAt: nowIso(),
      paramsHash,
      steps: {},
    };
    atomicWriteJson(stateFile, state);
  }

  const persist = () => {
    state.updatedAt = nowIso();
    atomicWriteJson(stateFile, state);
  };

  if (!state.steps.deployBatchSent?.done) {
    const seqnoBefore = await walletContract.getSeqno();
    await walletContract.sendTransfer({
      secretKey: keyPair.secretKey,
      seqno: seqnoBefore,
      sendMode: SendMode.PAY_GAS_SEPARATELY,
      messages: [
        internal({
          to: masterAddress,
          value: toNano(masterDeployTon),
          bounce: false,
          init: masterInit,
          body: buildTopUpBody(),
        }),
        internal({
          to: verifierAddress,
          value: toNano(verifierDeployTon),
          bounce: false,
          init: verifierInit,
          body: buildTopUpBody(),
        }),
      ],
    });
    state.steps.deployBatchSent = {
      done: true,
      seqnoBefore,
      at: nowIso(),
    };
    persist();
  }

  if (!state.steps.deploySeqnoAdvanced?.done) {
    const seqnoAfterDeploy = await waitSeqnoAdvance(
      walletContract,
      state.steps.deployBatchSent.seqnoBefore,
    );
    state.steps.deploySeqnoAdvanced = {
      done: true,
      seqnoAfterDeploy,
      at: nowIso(),
    };
    persist();
  }

  state.completed = true;
  state.completedAt = nowIso();
  persist();

  result.mode = 'execute';
  result.resumed = resume;
  result.wallet = {
    address: deployerAddress,
    steps: {
      deployBatchSent: state.steps.deployBatchSent,
      deploySeqnoAdvanced: state.steps.deploySeqnoAdvanced,
    },
  };

  mkdirSync(dirname(outPath), { recursive: true });
  writeFileSync(outPath, `${JSON.stringify(result, null, 2)}\n`, 'utf8');

  console.log(JSON.stringify(result, null, 2));
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
