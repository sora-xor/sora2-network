#!/usr/bin/env node
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, resolve } from 'node:path';
import { loadEthers } from './load_ethers.mjs';

const ACK_TOKEN = 'I_UNDERSTAND_MAINNET_DEPLOY';
const STATE_VERSION = 2;
const BOOLEAN_FLAGS = new Set(['execute', 'resume']);

const CHAIN_CONFIG = {
  eth: { localDomain: 1, chainId: 1n },
  bsc: { localDomain: 2, chainId: 56n },
  tron: { localDomain: 5, chainId: 728126428n },
};

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (!a.startsWith('--')) {
      throw new Error(`Unexpected positional argument: ${a}`);
    }
    const key = a.slice(2);
    const nxt = argv[i + 1];
    if (BOOLEAN_FLAGS.has(key)) {
      if (nxt !== undefined && !nxt.startsWith('--')) {
        throw new Error(`Flag --${key} does not take a value`);
      }
      out[key] = true;
      continue;
    }
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

function parsePositiveInteger(v, name) {
  if (typeof v !== 'string' || !/^[0-9]+$/.test(v)) {
    throw new Error(`${name} must be a positive integer`);
  }
  const n = Number(v);
  if (!Number.isSafeInteger(n) || n <= 0) {
    throw new Error(`${name} must be a positive integer`);
  }
  return n;
}

function parseNonNegativeInteger(v, name) {
  if (typeof v !== 'string' || !/^[0-9]+$/.test(v)) {
    throw new Error(`${name} must be a non-negative integer`);
  }
  const n = Number(v);
  if (!Number.isSafeInteger(n)) {
    throw new Error(`${name} must be a non-negative integer`);
  }
  return n;
}

function parseNonNegativeBigInt(v, name) {
  if (typeof v !== 'string' || !/^[0-9]+$/.test(v)) {
    throw new Error(`${name} must be a non-negative integer`);
  }
  return BigInt(v);
}

function readRequiredFile(path, label) {
  if (!existsSync(path)) {
    throw new Error(`${label} file not found: ${path}`);
  }
  return readFileSync(path, 'utf8').trim();
}

function normalizeBytes32Hex(v, name) {
  if (typeof v !== 'string') {
    throw new Error(`${name} must be a hex string`);
  }
  const with0x = v.startsWith('0x') ? v : `0x${v}`;
  if (!/^0x[0-9a-fA-F]{64}$/.test(with0x)) {
    throw new Error(`${name} must be exactly 32 bytes (64 hex chars)`);
  }
  return with0x.toLowerCase();
}

function normalizePrivateKey(v) {
  if (typeof v !== 'string') {
    throw new Error('private key must be a string');
  }
  const with0x = v.startsWith('0x') ? v : `0x${v}`;
  if (!/^0x[0-9a-fA-F]{64}$/.test(with0x)) {
    throw new Error('private key must be 32-byte hex');
  }
  return with0x;
}

function readArtifact(path) {
  if (!existsSync(path)) {
    throw new Error(`Missing artifact: ${path}`);
  }
  return JSON.parse(readFileSync(path, 'utf8'));
}

function sortDeep(value) {
  if (typeof value === 'bigint') {
    return value.toString();
  }
  if (Array.isArray(value)) {
    return value.map(sortDeep);
  }
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

function sanitizeRpcHost(rpcUrl) {
  try {
    const u = new URL(rpcUrl);
    return u.host;
  } catch {
    return '<redacted>';
  }
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

function defaultStatePath(repoRoot, chainLabel, deployer) {
  const d = deployer.toLowerCase().replace(/^0x/, '').slice(0, 10);
  return resolve(repoRoot, 'deployments', 'state', `mainnet-${chainLabel}-${d}.json`);
}

function ensureStatePolicy({ execute, resume, stateFile }) {
  const exists = existsSync(stateFile);
  if (!execute) {
    return { exists };
  }
  if (resume && !exists) {
    throw new Error(`--resume requested but state file does not exist: ${stateFile}`);
  }
  if (!resume && exists) {
    throw new Error(
      `State file already exists: ${stateFile}. Use --resume or pass a different --state-file.`,
    );
  }
  return { exists };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const repoRoot = resolve(import.meta.dirname, '..');

  const chainLabel = requireArg(args, 'chain-label');
  const chainConfig = CHAIN_CONFIG[chainLabel];
  if (!chainConfig) {
    throw new Error(`Unsupported chain-label: ${chainLabel}`);
  }

  const rpcUrl = requireArg(args, 'rpc-url');
  const privateKeyFile = requireArg(args, 'private-key-file');
  const privateKeyRaw = readRequiredFile(privateKeyFile, 'private-key');
  const privateKey = normalizePrivateKey(privateKeyRaw.trim());

  const localDomain = parseNonNegativeInteger(requireArg(args, 'local-domain'), 'local-domain');
  const expectedChainId = parseNonNegativeBigInt(
    requireArg(args, 'expected-chain-id'),
    'expected-chain-id',
  );

  if (localDomain !== chainConfig.localDomain) {
    throw new Error(
      `Refusing deploy: local-domain ${localDomain} does not match ${chainLabel} expected ${chainConfig.localDomain}`,
    );
  }
  if (expectedChainId !== chainConfig.chainId) {
    throw new Error(
      `Refusing deploy: expected-chain-id ${expectedChainId.toString()} does not match ${chainLabel} expected ${chainConfig.chainId.toString()}`,
    );
  }

  const latestBeefyBlock = parseNonNegativeBigInt(
    requireArg(args, 'latest-beefy-block'),
    'latest-beefy-block',
  );
  const currentVset = {
    id: parseNonNegativeBigInt(requireArg(args, 'current-vset-id'), 'current-vset-id'),
    len: parsePositiveInteger(requireArg(args, 'current-vset-len'), 'current-vset-len'),
    root: normalizeBytes32Hex(requireArg(args, 'current-vset-root'), 'current-vset-root'),
  };
  const nextVset = {
    id: parseNonNegativeBigInt(requireArg(args, 'next-vset-id'), 'next-vset-id'),
    len: parsePositiveInteger(requireArg(args, 'next-vset-len'), 'next-vset-len'),
    root: normalizeBytes32Hex(requireArg(args, 'next-vset-root'), 'next-vset-root'),
  };

  if (nextVset.id <= currentVset.id) {
    throw new Error('next validator-set id must be strictly greater than current validator-set id');
  }

  const currentVsetJson = { ...currentVset, id: currentVset.id.toString() };
  const nextVsetJson = { ...nextVset, id: nextVset.id.toString() };

  const execute = Boolean(args.execute);
  const resume = Boolean(args.resume);
  const ack = args['ack-mainnet'];

  if (execute && ack !== ACK_TOKEN) {
    throw new Error(`Mainnet execution requires --ack-mainnet ${ACK_TOKEN}`);
  }

  const { ethers } = await loadEthers(repoRoot);
  const provider = new ethers.JsonRpcProvider(rpcUrl);
  const wallet = new ethers.Wallet(privateKey, provider);

  const network = await provider.getNetwork();
  if (network.chainId !== expectedChainId) {
    throw new Error(
      `Unexpected chain id: got ${network.chainId.toString()}, expected ${expectedChainId.toString()}`,
    );
  }

  const routerArtifactPath = resolve(repoRoot, 'artifacts/contracts/SccpRouter.sol/SccpRouter.json');
  const verifierArtifactPath = resolve(
    repoRoot,
    'artifacts/contracts/verifiers/SoraBeefyLightClientVerifier.sol/SoraBeefyLightClientVerifier.json',
  );
  const routerArtifact = readArtifact(routerArtifactPath);
  const verifierArtifact = readArtifact(verifierArtifactPath);

  const outPath =
    args.out ??
    resolve(
      repoRoot,
      'deployments',
      `mainnet-${chainLabel}-${new Date().toISOString().replace(/[:.]/g, '-')}.json`,
    );

  const stateFile = args['state-file'] ?? defaultStatePath(repoRoot, chainLabel, wallet.address);
  ensureStatePolicy({ execute, resume, stateFile });

  const paramsHash = hashParams({
    version: STATE_VERSION,
    chainLabel,
    localDomain,
    expectedChainId: expectedChainId.toString(),
    deployer: wallet.address,
    latestBeefyBlock: latestBeefyBlock.toString(),
    currentVset: currentVsetJson,
    nextVset: nextVsetJson,
    routerArtifact: routerArtifact.bytecode,
    verifierArtifact: verifierArtifact.bytecode,
  });

  const common = {
    chain: chainLabel,
    chainId: network.chainId.toString(),
    rpcHost: sanitizeRpcHost(rpcUrl),
    localDomain,
    deployer: wallet.address,
    latestBeefyBlock: latestBeefyBlock.toString(),
    currentVset: currentVsetJson,
    nextVset: nextVsetJson,
    outPath,
    stateFile,
    paramsHash,
    timestamp: new Date().toISOString(),
  };

  if (!execute) {
    const dryRun = {
      ...common,
      mode: 'dry-run',
      note:
        'No transactions sent. Re-run with --execute --ack-mainnet I_UNDERSTAND_MAINNET_DEPLOY',
    };
    console.log(JSON.stringify(dryRun, null, 2));
    return;
  }

  const nowIso = () => new Date().toISOString();
  let state;
  if (resume) {
    state = readJsonFile(stateFile);
    if (!state || state.version !== STATE_VERSION) {
      throw new Error(`Invalid state file version in ${stateFile}`);
    }
    if (state.paramsHash !== paramsHash) {
      throw new Error(`State params hash mismatch for ${stateFile}.`);
    }
  } else {
    state = {
      version: STATE_VERSION,
      chain: chainLabel,
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

  const verifierFactory = new ethers.ContractFactory(
    verifierArtifact.abi,
    verifierArtifact.bytecode,
    wallet,
  );
  const routerFactory = new ethers.ContractFactory(routerArtifact.abi, routerArtifact.bytecode, wallet);

  if (!state.steps.verifierDeployed?.done) {
    const verifier = await verifierFactory.deploy(latestBeefyBlock, currentVset, nextVset);
    const receipt = await verifier.deploymentTransaction().wait();
    state.steps.verifierDeployed = {
      done: true,
      address: await verifier.getAddress(),
      txHash: receipt.hash,
      blockNumber: receipt.blockNumber,
      at: nowIso(),
    };
    persist();
  }

  if (!state.steps.routerDeployed?.done) {
    const verifierAddress = state.steps.verifierDeployed.address;
    const router = await routerFactory.deploy(localDomain, verifierAddress);
    const receipt = await router.deploymentTransaction().wait();
    state.steps.routerDeployed = {
      done: true,
      address: await router.getAddress(),
      txHash: receipt.hash,
      blockNumber: receipt.blockNumber,
      at: nowIso(),
    };
    persist();
  }

  state.completed = true;
  state.completedAt = nowIso();
  persist();

  const output = {
    ...common,
    mode: 'execute',
    resumed: resume,
    verifier: state.steps.verifierDeployed,
    router: state.steps.routerDeployed,
    stateSummary: {
      completed: state.completed,
      completedAt: state.completedAt,
      steps: Object.keys(state.steps),
    },
  };

  mkdirSync(dirname(outPath), { recursive: true });
  writeFileSync(outPath, `${JSON.stringify(output, null, 2)}\n`, 'utf8');

  console.log(JSON.stringify(output, null, 2));
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
