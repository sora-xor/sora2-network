#!/usr/bin/env node

import {
  existsSync,
  linkSync,
  lstatSync,
  mkdirSync,
  rmSync,
  statSync,
} from 'node:fs';
import { basename, dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const canonicalRoot = join(__dirname, 'canonical');
const chainRoots = {
  eth: resolve(__dirname, '../../eth'),
  bsc: resolve(__dirname, '../../bsc'),
  tron: resolve(__dirname, '../../tron'),
};

const linkedFiles = [
  'contracts/SccpRouter.sol',
  'contracts/SccpToken.sol',
  'contracts/ISccpVerifier.sol',
  'contracts/SccpCodec.sol',
  'contracts/verifiers/AlwaysTrueVerifier.sol',
  'contracts/verifiers/AlwaysFalseVerifier.sol',
  'contracts/verifiers/SoraBeefyLightClientVerifier.sol',
  'contracts/test/SccpCodecTest.sol',
  'contracts/echidna/EchidnaSccpCodec.sol',
  'test/fuzz/SccpCodecFuzz.t.sol',
  'foundry.toml',
  'scripts/compile_artifacts.mjs',
  'scripts/load_ethers.mjs',
];

function parseArgs(argv) {
  let checkOnly = false;
  let chainRoot = null;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--check') {
      checkOnly = true;
      continue;
    }
    if (arg === '--chain-root') {
      const value = argv[i + 1];
      if (!value || value.startsWith('--')) {
        throw new Error('missing value for --chain-root');
      }
      chainRoot = resolve(value);
      i += 1;
      continue;
    }
    throw new Error(`unknown argument: ${arg}`);
  }

  return { checkOnly, chainRoot };
}

function selectedChains(chainRoot) {
  if (!chainRoot) {
    return Object.entries(chainRoots);
  }

  const chainName = basename(chainRoot);
  const knownRoot = chainRoots[chainName];
  if (!knownRoot) {
    throw new Error(`unsupported chain root: ${chainRoot}`);
  }
  if (resolve(knownRoot) !== resolve(chainRoot)) {
    throw new Error(`unexpected chain root for ${chainName}: ${chainRoot}`);
  }
  return [[chainName, chainRoot]];
}

function linkMatches(src, dest) {
  if (!existsSync(dest)) {
    return false;
  }

  const destStat = lstatSync(dest);
  if (destStat.isSymbolicLink()) {
    return false;
  }

  const srcStat = statSync(src);
  return srcStat.dev === destStat.dev && srcStat.ino === destStat.ino;
}

function ensureLinked(src, dest) {
  if (linkMatches(src, dest)) {
    return;
  }

  mkdirSync(dirname(dest), { recursive: true });
  rmSync(dest, { force: true });
  linkSync(src, dest);
}

function main() {
  const { checkOnly, chainRoot } = parseArgs(process.argv.slice(2));
  const targets = selectedChains(chainRoot);
  const drifted = [];

  for (const [, root] of targets) {
    for (const rel of linkedFiles) {
      const src = join(canonicalRoot, rel);
      const dest = join(root, rel);
      if (checkOnly) {
        if (!linkMatches(src, dest)) {
          drifted.push(dest);
        }
        continue;
      }
      ensureLinked(src, dest);
    }
  }

  if (checkOnly) {
    if (drifted.length > 0) {
      const message = [
        '[repo-hygiene] SCCP shared EVM hard links are out of sync:',
        ...drifted.map((path) => `  ${path}`),
        '[repo-hygiene] run: node ./sccp/chains/evm/shared/sync_sources.mjs',
      ].join('\n');
      process.stderr.write(`${message}\n`);
      process.exit(1);
    }
    return;
  }

  process.stdout.write('[sccp-evm-hardlinks] OK\n');
}

main();
