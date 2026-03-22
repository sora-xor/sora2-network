#!/usr/bin/env node
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, resolve } from 'node:path';

import { Address, Cell, Dictionary, SendMode, beginCell, contractAddress, toNano } from '@ton/core';

const ACK_TOKEN = 'I_UNDERSTAND_MAINNET_DEPLOY';
const STATE_VERSION = 1;

const TOP_UP_TONS = 0xd372158c;
const SCCP_SET_VERIFIER = 0x0f95e281;
const SCCP_VERIFIER_INITIALIZE = 0x35f2bca1;


function parseArgs(argv) {
  const valueFlags = new Set([
    'endpoint',
    'mnemonic-file',
    'governor-mnemonic-file',
    'governor',
    'sora-asset-id',
    'metadata-uri',
    'master-value',
    'verifier-value',
    'bind-verifier-value',
    'initialize-verifier-value',
    'latest-beefy-block',
    'current-validator-set-id',
    'current-validator-set-len',
    'current-validator-set-root',
    'next-validator-set-id',
    'next-validator-set-len',
    'next-validator-set-root',
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

function parseUint64(raw, name) {
  if (raw === undefined) {
    return null;
  }
  if (!/^\d+$/.test(raw)) {
    throw new Error(`${name} must be an unsigned integer`);
  }
  const value = BigInt(raw);
  if (value > 0xffffffffffffffffn) {
    throw new Error(`${name} must fit in uint64`);
  }
  return value;
}

function parseUint32(raw, name) {
  if (raw === undefined) {
    return null;
  }
  if (!/^\d+$/.test(raw)) {
    throw new Error(`${name} must be an unsigned integer`);
  }
  const value = Number(raw);
  if (!Number.isSafeInteger(value) || value < 0 || value > 0xffffffff) {
    throw new Error(`${name} must fit in uint32`);
  }
  return value;
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
  sccpExtraB.storeUint(0, 8); // tokenState = Paused until SORA activates via proof
  sccpExtraB.storeUint(0, 64); // nonce
  sccpExtraB.storeUint(0, 64); // inboundPausedMask
  sccpExtraB.storeUint(0, 64); // outboundPausedMask
  emptyBoolMap.store(sccpExtraB); // invalidatedInbound
  emptyBoolMap.store(sccpExtraB); // processedInbound
  emptyBoolMap.store(sccpExtraB); // processedGovernance
  emptyBurnsMap.store(sccpExtraB); // burns
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

function buildSetVerifierBody(newVerifier) {
  return beginCell()
    .storeUint(SCCP_SET_VERIFIER, 32)
    .storeUint(0, 64)
    .storeAddress(newVerifier ?? null)
    .endCell();
}

function buildVerifierInitializeBody({
  latestBeefyBlock,
  currentValidatorSetId,
  currentValidatorSetLen,
  currentValidatorSetRootU256,
  nextValidatorSetId,
  nextValidatorSetLen,
  nextValidatorSetRootU256,
}) {
  return beginCell()
    .storeUint(SCCP_VERIFIER_INITIALIZE, 32)
    .storeUint(0, 64)
    .storeUint(latestBeefyBlock, 64)
    .storeUint(currentValidatorSetId, 64)
    .storeUint(currentValidatorSetLen, 32)
    .storeUint(currentValidatorSetRootU256, 256)
    .storeUint(nextValidatorSetId, 64)
    .storeUint(nextValidatorSetLen, 32)
    .storeUint(nextValidatorSetRootU256, 256)
    .endCell();
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
  const bindVerifierTon = args['bind-verifier-value'] ?? '0.05';
  const initializeVerifierTon = args['initialize-verifier-value'] ?? '0.05';
  const execute = Boolean(args.execute);
  const resume = Boolean(args.resume);
  if (execute && args['ack-mainnet'] !== ACK_TOKEN) {
    throw new Error(`Mainnet execution requires --ack-mainnet ${ACK_TOKEN}`);
  }

  const verifierInitializeInputs = {
    latestBeefyBlock: parseUint64(args['latest-beefy-block'], 'latest-beefy-block'),
    currentValidatorSetId: parseUint64(args['current-validator-set-id'], 'current-validator-set-id'),
    currentValidatorSetLen: parseUint32(args['current-validator-set-len'], 'current-validator-set-len'),
    currentValidatorSetRootU256:
      args['current-validator-set-root'] === undefined
        ? null
        : parseHexU256(args['current-validator-set-root'], 'current-validator-set-root'),
    nextValidatorSetId: parseUint64(args['next-validator-set-id'], 'next-validator-set-id'),
    nextValidatorSetLen: parseUint32(args['next-validator-set-len'], 'next-validator-set-len'),
    nextValidatorSetRootU256:
      args['next-validator-set-root'] === undefined
        ? null
        : parseHexU256(args['next-validator-set-root'], 'next-validator-set-root'),
  };
  const verifierInitializeFields = [
    ['latestBeefyBlock', verifierInitializeInputs.latestBeefyBlock],
    ['currentValidatorSetId', verifierInitializeInputs.currentValidatorSetId],
    ['currentValidatorSetLen', verifierInitializeInputs.currentValidatorSetLen],
    ['currentValidatorSetRoot', verifierInitializeInputs.currentValidatorSetRootU256],
    ['nextValidatorSetId', verifierInitializeInputs.nextValidatorSetId],
    ['nextValidatorSetLen', verifierInitializeInputs.nextValidatorSetLen],
    ['nextValidatorSetRoot', verifierInitializeInputs.nextValidatorSetRootU256],
  ];
  const verifierInitializePresent = verifierInitializeFields.map(([, value]) => value !== null);
  const verifierInitializeInputsComplete = verifierInitializePresent.every(Boolean);
  const verifierInitializeInputsAny = verifierInitializePresent.some(Boolean);
  const verifierInitializeInputsPartial =
    verifierInitializeInputsAny && !verifierInitializeInputsComplete;
  const verifierInitializeMissingInputs = verifierInitializeFields
    .filter(([, value]) => value === null)
    .map(([name]) => name);
  const verifierInitializeBody = verifierInitializeInputsComplete
    ? buildVerifierInitializeBody(verifierInitializeInputs)
    : null;
  const verifierInitializeBodyBoc = verifierInitializeBody
    ? Buffer.from(verifierInitializeBody.toBoc())
    : null;

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
      bindVerifierTon,
      initializeVerifierTon,
    },
    masterCodeHashHex: masterArtifact.codeHashHex,
    verifierCodeHashHex: verifierArtifact.codeHashHex,
    walletCodeHashHex: walletArtifact.codeHashHex,
  });

  const setVerifierBody = buildSetVerifierBody(verifierAddress);
  const setVerifierBodyBoc = Buffer.from(setVerifierBody.toBoc());

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
      bindVerifier: bindVerifierTon,
      initializeVerifier: initializeVerifierTon,
    },
    bootstrap: {
      bindVerifier: {
        target: masterAddress.toString(),
        verifierAddress: verifierAddress.toString(),
        valueTon: bindVerifierTon,
        bodyBocBase64: setVerifierBodyBoc.toString('base64'),
        bodyBocHex: setVerifierBodyBoc.toString('hex'),
      },
      verifierInitialize: {
        target: verifierAddress.toString(),
        valueTon: initializeVerifierTon,
        inputsComplete: verifierInitializeInputsComplete,
        inputsPartial: verifierInitializeInputsPartial,
        missingInputs: verifierInitializeInputsComplete ? [] : verifierInitializeMissingInputs,
        bodyBocBase64: verifierInitializeBodyBoc ? verifierInitializeBodyBoc.toString('base64') : null,
        bodyBocHex: verifierInitializeBodyBoc ? verifierInitializeBodyBoc.toString('hex') : null,
        status: verifierInitializeInputsComplete ? 'ready-to-send' : 'pending-inputs',
        note: verifierInitializeInputsComplete
          ? 'Submit SccpVerifierInitialize from the configured governor wallet to bootstrap the verifier light client.'
          : 'Provide SORA-derived validator set inputs to build the SccpVerifierInitialize body.',
      },
    },
    stateFile,
    paramsHash,
    outPath,
    timestamp: new Date().toISOString(),
    note: 'Deploy only. The configured governor must still complete bootstrap actions on-chain. The script can auto-send SccpSetVerifier and SccpVerifierInitialize only when it has the governor wallet and full verifier bootstrap inputs.',
  };

  if (!execute) {
    result.mode = 'dry-run';
    result.note =
      'No transactions sent. Re-run with --execute --ack-mainnet I_UNDERSTAND_MAINNET_DEPLOY. If the governor wallet is also available, pass --governor-mnemonic-file (or use the same mnemonic as --governor) to auto-send SccpSetVerifier and, when validator inputs are provided, SccpVerifierInitialize after deployment.';
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

  let governorKeyPair = keyPair;
  let governorWallet = wallet;
  let governorWalletContract = walletContract;
  const governorMnemonicFile = args['governor-mnemonic-file'];
  if (governorMnemonicFile) {
    if (!existsSync(governorMnemonicFile)) {
      throw new Error(`Governor mnemonic file not found: ${governorMnemonicFile}`);
    }
    const governorMnemonic = readFileSync(governorMnemonicFile, 'utf8').trim();
    if (!governorMnemonic) {
      throw new Error(`Governor mnemonic file is empty: ${governorMnemonicFile}`);
    }
    const governorWords = governorMnemonic.split(/\s+/).filter(Boolean);
    if (governorWords.length < 12) {
      throw new Error('governor mnemonic should have at least 12 words');
    }
    governorKeyPair = await mnemonicToPrivateKey(governorWords);
    governorWallet = WalletContractV4.create({ workchain: 0, publicKey: governorKeyPair.publicKey });
    governorWalletContract = client.open(governorWallet);
  }

  // Connectivity + stronger identity guard.
  const lastBlock = await client.getLastBlock();
  if (!lastBlock?.last?.seqno || lastBlock.last.seqno <= 0) {
    throw new Error(`Unable to verify endpoint health for ${sanitizeEndpointHost(endpoint)}`);
  }

  const deployerAddress = wallet.address.toString();
  const governorSignerAddress = governorWallet.address.toString();
  const canAutoBindVerifier = governorSignerAddress === governor.toString();
  result.deployer = deployerAddress;
  result.bootstrap.bindVerifier.governorSigner = governorSignerAddress;
  result.bootstrap.bindVerifier.autoSendAvailable = canAutoBindVerifier;
  result.bootstrap.verifierInitialize.governorSigner = governorSignerAddress;
  result.bootstrap.verifierInitialize.autoSendAvailable =
    canAutoBindVerifier && verifierInitializeInputsComplete;
  if (!canAutoBindVerifier) {
    result.bootstrap.bindVerifier.status = 'pending-governor-wallet';
    result.bootstrap.bindVerifier.note =
      'Provide the governor wallet mnemonic with --governor-mnemonic-file, or send the encoded body from the configured governor address after deployment.';
    result.bootstrap.verifierInitialize.status = verifierInitializeInputsComplete
      ? 'pending-governor-wallet'
      : 'pending-inputs';
    if (verifierInitializeInputsComplete) {
      result.bootstrap.verifierInitialize.note =
        'Provide the governor wallet mnemonic with --governor-mnemonic-file, or send the encoded verifier initialize body from the configured governor address after deployment.';
    }
  } else {
    result.bootstrap.bindVerifier.status = 'ready-to-send';
    if (verifierInitializeInputsComplete) {
      result.bootstrap.verifierInitialize.status = 'ready-to-send';
    }
  }

  if (governorMnemonicFile && !canAutoBindVerifier) {
    throw new Error(
      `Governor wallet mismatch: derived ${governorSignerAddress}, expected configured governor ${governor.toString()}`,
    );
  }

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

  if (canAutoBindVerifier && !state.steps.bindVerifierSent?.done) {
    const seqnoBefore = await governorWalletContract.getSeqno();
    await governorWalletContract.sendTransfer({
      secretKey: governorKeyPair.secretKey,
      seqno: seqnoBefore,
      sendMode: SendMode.PAY_GAS_SEPARATELY,
      messages: [
        internal({
          to: masterAddress,
          value: toNano(bindVerifierTon),
          bounce: true,
          body: setVerifierBody,
        }),
      ],
    });
    state.steps.bindVerifierSent = {
      done: true,
      seqnoBefore,
      at: nowIso(),
      sender: governorSignerAddress,
    };
    persist();
  }

  if (canAutoBindVerifier && state.steps.bindVerifierSent?.done && !state.steps.bindVerifierSeqnoAdvanced?.done) {
    const seqnoAfterBind = await waitSeqnoAdvance(
      governorWalletContract,
      state.steps.bindVerifierSent.seqnoBefore,
    );
    state.steps.bindVerifierSeqnoAdvanced = {
      done: true,
      seqnoAfterBind,
      at: nowIso(),
    };
    persist();
  }

  if (
    canAutoBindVerifier &&
    verifierInitializeBody &&
    !state.steps.verifierInitializeSent?.done
  ) {
    const seqnoBefore = await governorWalletContract.getSeqno();
    await governorWalletContract.sendTransfer({
      secretKey: governorKeyPair.secretKey,
      seqno: seqnoBefore,
      sendMode: SendMode.PAY_GAS_SEPARATELY,
      messages: [
        internal({
          to: verifierAddress,
          value: toNano(initializeVerifierTon),
          bounce: true,
          body: verifierInitializeBody,
        }),
      ],
    });
    state.steps.verifierInitializeSent = {
      done: true,
      seqnoBefore,
      at: nowIso(),
      sender: governorSignerAddress,
    };
    persist();
  }

  if (
    canAutoBindVerifier &&
    state.steps.verifierInitializeSent?.done &&
    !state.steps.verifierInitializeSeqnoAdvanced?.done
  ) {
    const seqnoAfterInitialize = await waitSeqnoAdvance(
      governorWalletContract,
      state.steps.verifierInitializeSent.seqnoBefore,
    );
    state.steps.verifierInitializeSeqnoAdvanced = {
      done: true,
      seqnoAfterInitialize,
      at: nowIso(),
    };
    persist();
  }

  result.mode = 'execute';
  result.resumed = resume;
  result.wallet = {
    address: deployerAddress,
    steps: {
      deployBatchSent: state.steps.deployBatchSent,
      deploySeqnoAdvanced: state.steps.deploySeqnoAdvanced,
    },
  };
  if (canAutoBindVerifier) {
    result.bootstrap.bindVerifier.status = 'sent';
    result.bootstrap.bindVerifier.steps = {
      bindVerifierSent: state.steps.bindVerifierSent,
      bindVerifierSeqnoAdvanced: state.steps.bindVerifierSeqnoAdvanced,
    };
  }
  if (state.steps.verifierInitializeSent?.done) {
    result.bootstrap.verifierInitialize.status = 'sent';
    result.bootstrap.verifierInitialize.steps = {
      verifierInitializeSent: state.steps.verifierInitializeSent,
      verifierInitializeSeqnoAdvanced: state.steps.verifierInitializeSeqnoAdvanced,
    };
  }

  const pendingActions = [];
  if (!state.steps.bindVerifierSeqnoAdvanced?.done) {
    pendingActions.push(
      'Send SccpSetVerifier from the configured governor wallet to the deployed jetton master using the emitted bodyBoc.',
    );
  }
  if (!verifierInitializeInputsComplete) {
    pendingActions.push(
      'Provide the full SccpVerifierInitialize input set from SORA chain state: latest beefy block, current validator set id/len/root, and next validator set id/len/root.',
    );
  } else if (!state.steps.verifierInitializeSeqnoAdvanced?.done) {
    pendingActions.push(
      'Send SccpVerifierInitialize from the configured governor wallet to the deployed verifier using the emitted bodyBoc.',
    );
  }

  if (pendingActions.length > 0) {
    result.pendingActions = pendingActions;
    state.completed = false;
    delete state.completedAt;
  } else {
    state.completed = true;
    state.completedAt = nowIso();
  }
  persist();

  mkdirSync(dirname(outPath), { recursive: true });
  writeFileSync(outPath, `${JSON.stringify(result, null, 2)}\n`, 'utf8');

  console.log(JSON.stringify(result, null, 2));
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
