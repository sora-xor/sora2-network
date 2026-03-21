import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';

import pkg from '@ton/tolk-js';

const { runTolkCompiler, getTolkCompilerVersion } = pkg;

const repoRoot = resolve(import.meta.dirname, '..');
const contractsDir = resolve(repoRoot, 'contracts');
const artifactsDir = resolve(repoRoot, 'artifacts');

function ensureDir(p) {
  mkdirSync(p, { recursive: true });
}

async function compileOne(entrypoint, outName) {
  const outPath = resolve(artifactsDir, outName);
  ensureDir(dirname(outPath));

  const relEntrypoint = entrypoint;
  const result = await runTolkCompiler({
    entrypointFileName: relEntrypoint,
    fsReadCallback: (p) => {
      // Stdlib imports (e.g. "@stdlib/..") are handled by tolk-js itself.
      return readFileSync(resolve(contractsDir, p), 'utf8');
    },
    optimizationLevel: 2,
    withSrcLineComments: true,
    withStackComments: true,
  });

  if (result.status !== 'ok') {
    throw new Error(result.message);
  }

  writeFileSync(
    outPath,
    JSON.stringify(
      {
        entrypoint,
        compiler: {
          name: 'tolk',
          version: await getTolkCompilerVersion(),
        },
        fiftCode: result.fiftCode,
        codeBoc64: result.codeBoc64,
        codeHashHex: result.codeHashHex,
      },
      null,
      2,
    ) + '\n',
    'utf8',
  );

  return { outPath };
}

async function main() {
  ensureDir(artifactsDir);

  const outputs = [];
  outputs.push(await compileOne('sccp-jetton-wallet.tolk', 'sccp-jetton-wallet.compiled.json'));
  outputs.push(await compileOne('sccp-jetton-master.tolk', 'sccp-jetton-master.compiled.json'));
  outputs.push(await compileOne('sccp-sora-verifier.tolk', 'sccp-sora-verifier.compiled.json'));
  outputs.push(await compileOne('sccp-codec-test.tolk', 'sccp-codec-test.compiled.json'));

  // Small sanity check: artifacts exist and have code.
  for (const { outPath } of outputs) {
    const parsed = JSON.parse(readFileSync(outPath, 'utf8'));
    if (typeof parsed.codeBoc64 !== 'string' || parsed.codeBoc64.length === 0) {
      throw new Error(`Missing codeBoc64 in ${outPath}`);
    }
  }
}

main().catch((e) => {
  // eslint-disable-next-line no-console
  console.error(e);
  process.exit(1);
});
