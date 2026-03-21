#!/usr/bin/env node

import { mkdtempSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { basename, dirname, extname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, '..');
const contractsRoot = resolve(repoRoot, 'contracts');
const artifactsRoot = resolve(repoRoot, 'artifacts');
const contractsArtifactsRoot = resolve(artifactsRoot, 'contracts');
const tempOutDir = mkdtempSync(join(tmpdir(), 'sccp-forge-out-'));

function collectContracts(dir, out) {
  const entries = readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = join(dir, entry.name);
    if (entry.isDirectory()) {
      const relDir = relative(repoRoot, fullPath);
      if (relDir === 'contracts/test' || relDir === 'contracts/echidna') {
        continue;
      }
      collectContracts(fullPath, out);
      continue;
    }
    if (!entry.isFile() || extname(entry.name) !== '.sol') {
      continue;
    }
    out.push(relative(repoRoot, fullPath));
  }
}

function normalizeBytecode(value) {
  if (!value) {
    return '0x';
  }
  return value.startsWith('0x') ? value : `0x${value}`;
}

function buildHardhatArtifact(sourceName, foundryArtifact) {
  const contractName = basename(sourceName, '.sol');
  return {
    _format: 'hh-sol-artifact-1',
    contractName,
    sourceName,
    abi: foundryArtifact.abi ?? [],
    bytecode: normalizeBytecode(foundryArtifact.bytecode?.object),
    deployedBytecode: normalizeBytecode(foundryArtifact.deployedBytecode?.object),
    linkReferences: foundryArtifact.bytecode?.linkReferences ?? {},
    deployedLinkReferences: foundryArtifact.deployedBytecode?.linkReferences ?? {},
  };
}

function writeArtifact(sourceName, artifact) {
  const contractName = basename(sourceName, '.sol');
  const outputPath = join(artifactsRoot, sourceName, `${contractName}.json`);
  mkdirSync(dirname(outputPath), { recursive: true });
  writeFileSync(outputPath, `${JSON.stringify(artifact, null, 2)}\n`, 'utf8');
}

function main() {
  const contractFiles = [];
  collectContracts(contractsRoot, contractFiles);
  contractFiles.sort();

  if (contractFiles.length === 0) {
    throw new Error('[compile-artifacts] no production contracts found');
  }

  const forgeResult = spawnSync(
    'forge',
    ['build', '--force', '--out', tempOutDir],
    {
      cwd: repoRoot,
      stdio: 'inherit',
    },
  );

  if (forgeResult.error) {
    throw forgeResult.error;
  }
  if ((forgeResult.status ?? 1) !== 0) {
    process.exit(forgeResult.status ?? 1);
  }

  rmSync(contractsArtifactsRoot, { recursive: true, force: true });
  mkdirSync(artifactsRoot, { recursive: true });

  for (const sourceName of contractFiles) {
    const contractName = basename(sourceName, '.sol');
    const foundryArtifactPath = join(tempOutDir, `${contractName}.sol`, `${contractName}.json`);
    const foundryArtifact = JSON.parse(readFileSync(foundryArtifactPath, 'utf8'));
    writeArtifact(sourceName, buildHardhatArtifact(sourceName, foundryArtifact));
  }

  process.stdout.write(`[compile-artifacts] wrote ${contractFiles.length} Hardhat-style artifacts\n`);
}

try {
  main();
} finally {
  rmSync(tempOutDir, { recursive: true, force: true });
}
