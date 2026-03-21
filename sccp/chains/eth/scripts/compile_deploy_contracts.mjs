#!/usr/bin/env node

import { readdirSync } from 'node:fs';
import { dirname, extname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, '..');
const contractsRoot = resolve(repoRoot, 'contracts');

function collectContracts(dir, out) {
  const entries = readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = join(dir, entry.name);
    if (entry.isDirectory()) {
      collectContracts(fullPath, out);
      continue;
    }
    if (!entry.isFile() || extname(entry.name) !== '.sol') {
      continue;
    }

    const rel = relative(repoRoot, fullPath);
    if (rel.startsWith('contracts/test/') || rel.startsWith('contracts/echidna/')) {
      continue;
    }
    out.push(rel);
  }
}

const contractFiles = [];
collectContracts(contractsRoot, contractFiles);
contractFiles.sort();

if (contractFiles.length === 0) {
  console.error('[compile:deploy] no production contracts found');
  process.exit(1);
}

console.log(`[compile:deploy] compiling ${contractFiles.length} production contract files`);

const result = spawnSync(
  'bash',
  ['./scripts/run_hardhat.sh', 'compile', '--no-tests', ...contractFiles],
  {
    cwd: repoRoot,
    stdio: 'inherit',
  },
);

if (result.error) {
  console.error(result.error);
  process.exit(1);
}
process.exit(result.status ?? 1);
