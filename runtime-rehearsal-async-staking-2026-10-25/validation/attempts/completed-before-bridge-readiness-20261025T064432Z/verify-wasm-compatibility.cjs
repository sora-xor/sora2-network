/* Local-only compatibility inspection of the exact deployed and candidate Wasm.
 * Uses installed subwasm and the existing, self-checking V14 metadata comparator.
 * No remote RPC, credentials, transaction signing, or network submission.
 */
'use strict';
const assert = require('node:assert/strict');
const { readFileSync, writeFileSync, mkdirSync, mkdtempSync, rmSync, existsSync } = require('node:fs');
const { resolve, join } = require('node:path');
const { tmpdir } = require('node:os');
const { execFileSync } = require('node:child_process');
const { createHash } = require('node:crypto');
const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
const repositoryRoot = resolve(__dirname, '../..');
const packageRoot = resolve(__dirname, '..');
const baseline = resolve(option('--baseline', resolve(packageRoot, 'validation/live-runtime.compact.compressed.wasm')));
const candidate = resolve(option('--candidate', resolve(packageRoot, 'candidate.compact.compressed.wasm')));
const expectedSpec = Number(option('--candidate-spec', '137'));
const outputDir = resolve(option('--output-dir', resolve(packageRoot, 'validation')));
const localComparator = resolve(__dirname, 'compare-metadata.py');
const comparator = resolve(option('--comparator', existsSync(localComparator) ? localComparator
  : resolve(repositoryRoot, 'runtime-upgrade-4.8.9/validation/compare-metadata.py')));
const sha256 = data => createHash('sha256').update(data).digest('hex');
const report = { status: 'running', startedAt: new Date().toISOString(), localOnly: true, remoteRequests: 0, inputs: {}, checks: {} };
mkdirSync(outputDir, { recursive: true });
const output = join(outputDir, 'wasm-compatibility.json');
const working = mkdtempSync(join(tmpdir(), 'sora-equivocation-fee-compatibility-'));
const run = (command, params) => execFileSync(command, params, {
  env: { ...process.env, NO_COLOR: 'true' }, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024,
});

const { readInterfaces, defaultStaticMemoryPages, compareRuntimeExports } = require('./wasm-interfaces.cjs');

try {
  const snapshot = JSON.parse(readFileSync(resolve(__dirname, 'snapshot.json')));
  report.scriptSha256 = sha256(readFileSync(__filename));
  report.interfaceParserSha256 = sha256(readFileSync(resolve(__dirname, 'wasm-interfaces.cjs')));
  report.snapshotSha256 = sha256(readFileSync(resolve(__dirname, 'snapshot.json')));
  assert.equal(snapshot.status, 'passed');
  assert(snapshot.finality && snapshot.finality.selectedCanonicalHash === snapshot.blockHash);
  assert.equal(sha256(readFileSync(baseline)), snapshot.code.sha256, 'Baseline must match captured deployed Wasm');
  report.tools = { subwasm: run('subwasm', ['--version']).trim(), comparator, comparatorSha256: sha256(readFileSync(comparator)) };
  for (const [label, file] of [['baseline', baseline], ['candidate', candidate]]) {
    const bytes = readFileSync(file);
    const uncompressedPath = join(working, label + '.wasm');
    run('subwasm', ['decompress', file, uncompressedPath]);
    const decompressed = readFileSync(uncompressedPath);
    const module = new WebAssembly.Module(decompressed);
    const metadataPath = join(outputDir, label + '-metadata.json');
    const metadata = JSON.parse(run('subwasm', ['metadata', '--format', 'json', file]));
    writeFileSync(metadataPath, JSON.stringify(metadata, null, 2) + '\n');
    const info = JSON.parse(run('subwasm', ['info', '--json', file]));
    const interfaces = readInterfaces(decompressed, WebAssembly.Module.imports(module), WebAssembly.Module.exports(module));
    report.inputs[label] = { path: file, bytes: bytes.length, sha256: sha256(bytes),
      uncompressedBytes: decompressed.length, uncompressedSha256: sha256(decompressed),
      metadataPath, metadataSha256: sha256(readFileSync(metadataPath)), info,
      imports: WebAssembly.Module.imports(module), exports: WebAssembly.Module.exports(module),
      importedInterfaces: interfaces.imports, exportedInterfaces: interfaces.exports };
    {
      const metadataScalePath = join(outputDir, label + '-metadata.scale');
      run('subwasm', ['metadata', '--format', 'scale', '--output', metadataScalePath, file]);
      report.inputs[label].metadataScalePath = metadataScalePath;
      report.inputs[label].metadataScaleSha256 = sha256(readFileSync(metadataScalePath));
    }
  }
  const oldVersion = report.inputs.baseline.info.core_version;
  const newVersion = report.inputs.candidate.info.core_version;
  assert.equal(oldVersion.specVersion, snapshot.runtimeVersion.specVersion);
  assert.equal(newVersion.specVersion, expectedSpec, 'Candidate version must match explicit intended version');
  assert(newVersion.specVersion > oldVersion.specVersion, 'Candidate must strictly advance deployed specVersion');
  assert.equal(oldVersion.transactionVersion, 131, 'Pinned predecessor transaction version');
  assert.equal(newVersion.transactionVersion, 133, 'Async staking migration requires the reviewed transaction-version bump');
  for (const key of ['specName', 'implName', 'authoringVersion', 'implVersion', 'stateVersion', 'apis']) {
    assert.deepEqual(newVersion[key], oldVersion[key], 'Unchanged runtime version field: ' + key);
  }
  report.candidateVersion = { deployedSpecVersion: oldVersion.specVersion, candidateSpecVersion: newVersion.specVersion, transactionVersion: newVersion.transactionVersion, strictlyAdvances: true };
  const metadataReportPath = join(outputDir, 'metadata-compatibility.json');
  report.metadataComparison = JSON.parse(run('python3', [comparator, '--old', report.inputs.baseline.metadataPath,
    '--new', report.inputs.candidate.metadataPath, '--output', metadataReportPath, '--async-staking']));
  const metadata = JSON.parse(readFileSync(metadataReportPath));
  assert.equal(metadata.reviewedSdkMigrationCompatible, true, 'All unreviewed SDK SCALE changes must fail');
  assert.equal(metadata.reviewedAsyncStakingMigrationCompatible, true, 'All unreviewed async staking changes must fail');
  assert.deepEqual(metadata.requiresStorageMigration, { pallet: 'Staking', from: 16, to: 18, rawStateWitnessRequired: true });
  assert.equal(metadata.compatibleExistingScaleEncoding, false, 'CodeUpdated intentionally changes its existing event payload');
  assert.equal(metadata.reviewedSignedExtensionChange.added, 'AuthorizeCall');
  assert.equal(metadata.reviewedSignedExtensionChange.requiresTransactionVersion, newVersion.transactionVersion);
  assert.deepEqual(metadata.additions, [], 'Dependency migration must not introduce unreviewed ABI additions');
  assert(metadata.constantValueChanges.every(change => change.path === 'System.constants.Version'), 'Only the runtime version constant may change');
  const { verifyStakingQueryCompatibility } = require('./verify-staking-query-compatibility.cjs');
  report.stakingQueryCompatibility = verifyStakingQueryCompatibility({
    baselineMetadataPath: report.inputs.baseline.metadataScalePath,
    candidateMetadataPath: report.inputs.candidate.metadataScalePath,
    inventoryPath: resolve(__dirname, 'staking-public-inventory.json'),
    statePath: resolve(__dirname, 'staking-public-state.json.gz'), blockHash: snapshot.blockHash,
  });
  const identity = value => [value.module || '', value.name, value.kind].join('::');
  const sorted = values => [...values].sort((a,b) => identity(a).localeCompare(identity(b)));
  // Wasm import/export ordering is compiler-internal: runtime linkage uses names.
  // Require exact names/kinds and function signatures, with no blanket allowlist.
  assert.deepEqual(sorted(report.inputs.candidate.imports), sorted(report.inputs.baseline.imports), 'Host import names and kinds must be unchanged');
  const oldInterfaces = report.inputs.baseline.importedInterfaces;
  const newInterfaces = report.inputs.candidate.importedInterfaces;
  assert.deepEqual(sorted(newInterfaces.filter(value => value.kind === 'function')), sorted(oldInterfaces.filter(value => value.kind === 'function')), 'Host function signatures must be unchanged');
  const oldMemory = oldInterfaces.filter(value => value.kind === 'memory');
  const newMemory = newInterfaces.filter(value => value.kind === 'memory');
  assert.equal(oldMemory.length, 1); assert.equal(newMemory.length, 1);
  assert.deepEqual({ ...newMemory[0], minimumPages: oldMemory[0].minimumPages }, oldMemory[0], 'Only imported memory minimum may change');
  const memoryPath = resolve(repositoryRoot, 'vendor/polkadot-sdk/client/executor/common/src/runtime_blob/memory.rs');
  const strategyPath = resolve(repositoryRoot, 'vendor/polkadot-sdk/client/executor/common/src/wasm_runtime.rs');
  const memorySource = readFileSync(memoryPath, 'utf8'), strategySource = readFileSync(strategyPath, 'utf8');
  assert(memorySource.includes('initial.saturating_add(extra_pages)'));
  const pages = Number(strategySource.match(/DEFAULT_HEAP_ALLOC_PAGES: u32 = (\d+)/)?.[1]);
  assert(strategySource.includes('HeapAllocStrategy::Static { extra_pages: DEFAULT_HEAP_ALLOC_PAGES }'));
  const defaultBaselinePages = defaultStaticMemoryPages(oldMemory[0], pages);
  const defaultCandidatePages = defaultStaticMemoryPages(newMemory[0], pages);
  report.memoryCompatibility = { baseline: oldMemory[0], candidate: newMemory[0],
    minimumChangePages: newMemory[0].minimumPages - oldMemory[0].minimumPages,
    defaultStaticExtraHeapPages: pages, wasmPageBytes: 65536,
    defaultBaselinePages, defaultCandidatePages, wasm32MaximumPages: 65536,
    defaultEffectivePagesWithinWasm32Bound: true, candidateHostInstantiationProvenByThisCheck: false,
    maximumSharedAndAddressTypeUnchanged: true,
    currentSource: [{ path: memoryPath, sha256: sha256(readFileSync(memoryPath)) }, { path: strategyPath, sha256: sha256(readFileSync(strategyPath)) }],
    sourceContext: 'Current host source adds configured extra pages to module minimum. This is source context; exact candidate instantiation is exercised separately by local Wasm rehearsals.' };
  report.exportCompatibility = compareRuntimeExports(report.inputs.baseline.exportedInterfaces, report.inputs.candidate.exportedInterfaces);
  const hostFiles = [resolve(repositoryRoot, 'vendor/polkadot-sdk/client/executor/wasmtime/src/instance_wrapper.rs'),
    resolve(repositoryRoot, 'vendor/chopsticks-executor/smoldot/src/executor/host.rs')];
  report.exportCompatibility.hostSource = hostFiles.map(path => {
    const source = readFileSync(path, 'utf8');
    assert(source.includes('__heap_base') && !source.includes('__data_end'), 'Reviewed hosts must use heap base without reading the optional data-end marker');
    return { path, sha256: sha256(readFileSync(path)) };
  });
  report.exportCompatibility.reason = 'Only removal of the linker global __data_end is permitted. Neither inspected Wasmtime nor Chopsticks host reads it; both require __heap_base, which is preserved. Runtime functions, their signatures and all other named exports remain exact. Actual host instantiation is required in the subsequent local rehearsals.';
  report.hostImportOrderChanged = JSON.stringify(report.inputs.candidate.imports) !== JSON.stringify(report.inputs.baseline.imports);
  report.checks = { existingScaleEncodingCompatible: false, reviewedSdkMigrationCompatible: true,
    reviewedAsyncStakingMigrationCompatible: true, completePinnedStakingQueryJsonPreserved: true,
    codeUpdatedEventRequiresMetadataRefresh: true, noUnreviewedAbiAdditions: true,
    onlyRuntimeVersionConstantChanged: true, hostImportNamesAndKindsUnchanged: true,
    hostFunctionSignaturesUnchanged: true, importedMemoryShapePreserved: true, defaultStaticHeapWithinWasm32Bounds: true,
    wasmRequiredExportNamesAndKindsUnchanged: true, optionalLinkerGlobalRemovalReviewed: true, wasmExportedFunctionSignaturesUnchanged: true, runtimeApiVersionsUnchanged: true,
    transactionVersionAdvancedForReviewedAuthorizationExtension: true, specVersionStrictlyAdvanced: true };
  report.limitations = [
    'Metadata comparison covers declared SCALE layouts, indices, storage hashers/defaults and signed extensions; it does not execute migrations or prove custom codec behavior.',
    'Host-interface checks preserve named imports and signatures. Import/export ordering is compiler-internal and is recorded rather than treated as an ABI.',
    'Imported memory minimum changes satisfy the current default static strategy and wasm32 address-space bound; actual candidate host instantiation must pass separately in local rehearsals.',
    'No governance calls are encoded, signed or submitted. Subwasm authorize-upgrade call hashes are not used.',
  ];
  report.status = 'passed';
} catch (error) {
  report.status = 'failed'; report.error = { message: error.message, stack: error.stack };
  if (error.stdout) report.error.stdout = String(error.stdout).slice(-8000);
  if (error.stderr) report.error.stderr = String(error.stderr).slice(-8000);
  process.exitCode = 1;
} finally {
  rmSync(working, { recursive: true, force: true });
  report.finishedAt = new Date().toISOString();
  writeFileSync(output, JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify({ status: report.status, output, error: report.error?.message, candidateSha256: report.inputs.candidate?.sha256, checks: report.checks }));
}
