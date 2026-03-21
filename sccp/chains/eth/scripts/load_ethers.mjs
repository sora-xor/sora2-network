import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

function readPackageJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'));
}

function readEthersSpec(repoRoot) {
  const packageJson = readPackageJson(resolve(repoRoot, 'package.json'));
  const ethersSpec = packageJson?.devDependencies?.ethers;
  if (typeof ethersSpec !== 'string' || ethersSpec.length === 0) {
    throw new Error(`Missing devDependencies.ethers in ${resolve(repoRoot, 'package.json')}`);
  }
  return ethersSpec;
}

function resolveEphemeralNodeModulesRoot(repoRoot, ethersSpec) {
  try {
    return execFileSync(
      'npx',
      [
        '-y',
        '-p',
        `ethers@${ethersSpec}`,
        '-c',
        'node -p "require(\\"path\\").resolve(process.env.PATH.split(require(\\"path\\").delimiter)[0], \\"..\\")"',
      ],
      {
        cwd: repoRoot,
        encoding: 'utf8',
      },
    ).trim();
  } catch (error) {
    const detail =
      error && typeof error === 'object' && 'stderr' in error && typeof error.stderr === 'string'
        ? error.stderr.trim()
        : '';
    throw new Error(
      detail
        ? `Unable to bootstrap ethers via npx: ${detail}`
        : 'Unable to bootstrap ethers via npx',
    );
  }
}

function resolveEthersEntrypoint(nodeModulesRoot) {
  const ethersRoot = resolve(nodeModulesRoot, 'ethers');
  const packageJsonPath = resolve(ethersRoot, 'package.json');
  if (!existsSync(packageJsonPath)) {
    throw new Error(`Bootstrapped ethers package.json not found: ${packageJsonPath}`);
  }
  const packageJson = readPackageJson(packageJsonPath);
  const moduleEntry = packageJson?.exports?.['.']?.import ?? packageJson?.module;
  if (typeof moduleEntry !== 'string' || moduleEntry.length === 0) {
    throw new Error(`Unable to resolve ethers ESM entrypoint from ${packageJsonPath}`);
  }
  return resolve(ethersRoot, moduleEntry.replace(/^\.\//, ''));
}

function isMissingEthers(error) {
  return (
    error?.code === 'ERR_MODULE_NOT_FOUND' && typeof error.message === 'string' && error.message.includes("'ethers'")
  );
}

export async function loadEthers(repoRoot) {
  try {
    return await import('ethers');
  } catch (error) {
    if (!isMissingEthers(error)) {
      throw error;
    }
  }

  const entrypoint = resolveEthersEntrypoint(
    resolveEphemeralNodeModulesRoot(repoRoot, readEthersSpec(repoRoot)),
  );
  return await import(pathToFileURL(entrypoint).href);
}
