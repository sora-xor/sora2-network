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
const candidate = resolve(option('--candidate', resolve(packageRoot, 'framenode-runtime-4.8.12.compact.compressed.wasm')));
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

// WebAssembly.Module.imports omits function signatures and memory limits.
// Read those from the already engine-validated binary; reject unsupported forms
// instead of silently weakening the interface comparison for future artifacts.
function importedInterfaces(bytes, reflectedImports) {
  class Reader {
    constructor(data) { this.data = data; this.offset = 0; }
    take(length) {
      assert(length <= this.data.length - this.offset, 'Truncated Wasm section');
      const value = this.data.subarray(this.offset, this.offset + length);
      this.offset += length;
      return value;
    }
    byte() { return this.take(1)[0]; }
    u32() {
      let value = 0;
      for (let index = 0; index < 5; index++) {
        const byte = this.byte();
        if (index === 4) assert(byte <= 0x0f, 'Invalid Wasm u32 LEB128');
        value += (byte & 0x7f) * 2 ** (7 * index);
        if (!(byte & 0x80)) return value;
      }
      throw new Error('Unterminated Wasm u32 LEB128');
    }
    name() { return new TextDecoder('utf-8', { fatal: true }).decode(this.take(this.u32())); }
    done() { assert.equal(this.offset, this.data.length, 'Trailing Wasm section bytes'); }
  }
  const types = [];
  const imports = [];
  const input = new Reader(bytes);
  assert.equal(input.take(8).toString('hex'), '0061736d01000000', 'Expected Wasm v1');
  const valueTypes = new Map([[0x7f, 'i32'], [0x7e, 'i64'], [0x7d, 'f32'],
    [0x7c, 'f64'], [0x7b, 'v128'], [0x70, 'funcref'], [0x6f, 'externref']]);
  const vector = reader => {
    const count = reader.u32();
    assert(count <= reader.data.length - reader.offset, 'Invalid Wasm value-type count');
    const values = [];
    for (let index = 0; index < count; index++) {
      const type = valueTypes.get(reader.byte());
      assert(type, 'Unsupported Wasm value type');
      values.push(type);
    }
    return values;
  };
  while (input.offset < bytes.length) {
    const id = input.byte();
    const section = new Reader(input.take(input.u32()));
    if (id === 1) {
      const count = section.u32();
      for (let index = 0; index < count; index++) {
        assert.equal(section.byte(), 0x60, 'Unsupported Wasm type declaration');
        types.push({ parameters: vector(section), results: vector(section) });
      }
      section.done();
    } else if (id === 2) {
      const count = section.u32();
      for (let index = 0; index < count; index++) {
        const imported = { module: section.name(), name: section.name() };
        const kind = section.byte();
        if (kind === 0) {
          const signature = types[section.u32()];
          assert(signature, 'Invalid Wasm imported function type index');
          imports.push({ ...imported, kind: 'function', ...signature });
        } else if (kind === 2) {
          const flags = section.u32();
          assert([0, 1, 3].includes(flags), 'Unsupported Wasm memory limits flags');
          const minimumPages = section.u32();
          const maximumPages = flags & 1 ? section.u32() : null;
          imports.push({ ...imported, kind: 'memory', minimumPages, maximumPages,
            shared: Boolean(flags & 2), addressType: 'i32' });
        } else {
          throw new Error('Unsupported Wasm import kind: ' + kind);
        }
      }
      section.done();
    }
  }
  assert.deepEqual(imports.map(({ module, name, kind }) => ({ module, name, kind })), reflectedImports,
    'Parsed import names and kinds must match the independent WebAssembly engine');
  return imports;
}

try {
  const snapshot = JSON.parse(readFileSync(resolve(__dirname, 'snapshot.json')));
  report.scriptSha256 = sha256(readFileSync(__filename));
  report.snapshotSha256 = sha256(readFileSync(resolve(__dirname, 'snapshot.json')));
  assert(snapshot.status === 'passed' && snapshot.runtimeVersion.specVersion < 134);
  assert.equal(sha256(readFileSync(baseline)), snapshot.code.sha256, 'Baseline must match captured deployed Wasm');
  report.tools = { subwasm: run('subwasm', ['--version']).trim(), comparator, comparatorSha256: sha256(readFileSync(comparator)) };
  for (const [label, file] of [['baseline', baseline], ['candidate', candidate]]) {
    const bytes = readFileSync(file);
    const uncompressedPath = join(working, label + '.wasm');
    run('subwasm', ['decompress', file, uncompressedPath]);
    const decompressed = readFileSync(uncompressedPath);
    const module = new WebAssembly.Module(decompressed);
    const metadataPath = join(outputDir, label + '-metadata.json');
    // This installed subwasm emits JSON on stdout even when --output is given.
    // Parse before writing so an empty or diagnostic response cannot look like metadata.
    const metadata = JSON.parse(run('subwasm', ['metadata', '--format', 'json', file]));
    writeFileSync(metadataPath, JSON.stringify(metadata, null, 2) + '\n');
    const info = JSON.parse(run('subwasm', ['info', '--json', file]));
    report.inputs[label] = { path: file, bytes: bytes.length, sha256: sha256(bytes),
      uncompressedBytes: decompressed.length, uncompressedSha256: sha256(decompressed),
      metadataPath, metadataSha256: sha256(readFileSync(metadataPath)),
      info, imports: WebAssembly.Module.imports(module), exports: WebAssembly.Module.exports(module),
      importedInterfaces: importedInterfaces(decompressed, WebAssembly.Module.imports(module)) };
  }
  const metadataReportPath = join(outputDir, 'metadata-compatibility.json');
  report.metadataComparison = JSON.parse(run('python3', [comparator, '--old', report.inputs.baseline.metadataPath,
    '--new', report.inputs.candidate.metadataPath, '--output', metadataReportPath]));
  const metadata = JSON.parse(readFileSync(metadataReportPath));
  assert.equal(metadata.compatibleExistingScaleEncoding, true, 'Existing SCALE encoding must be compatible');
  // The follow-up changes fee reservation and sponsorship admission only.
  // The current sealed faf9ab candidate already contains the capacity ABI.
  const previousMetadataPath = resolve(__dirname, 'previous-candidate-metadata.json');
  const previousMetadataSha256 = sha256(readFileSync(previousMetadataPath));
  assert.equal(previousMetadataSha256,
    'f610de2e428a46bca43c8756b04f0decd18efb56d5e898102503aefa04c1e020',
    'Predecessor metadata must match the sealed faf9ab 4.8.12 candidate');
  const predecessorMetadataPath = join(outputDir, 'predecessor-metadata-compatibility.json');
  run('python3', [comparator, '--old', previousMetadataPath,
    '--new', report.inputs.candidate.metadataPath, '--output', predecessorMetadataPath]);
  const predecessorComparison = JSON.parse(readFileSync(predecessorMetadataPath));
  assert.equal(predecessorComparison.compatibleExistingScaleEncoding, true,
    'Follow-up must preserve every sealed faf9ab SCALE encoding');
  assert.deepEqual(predecessorComparison.additions, [], 'Follow-up must not add an ABI variant or storage entry');
  assert.deepEqual(predecessorComparison.constantValueChanges, [], 'Follow-up must preserve sealed faf9ab constants');
  const currentModel = JSON.parse(readFileSync(report.inputs.candidate.metadataPath)).V14;
  const variants = (pallet, kind) => {
    const item = currentModel.pallets.find(item => item.name === pallet);
    return currentModel.types.types.find(type => type.id === item[kind].ty).type.def.variant.variants;
  };
  const cancellation = variants('EthBridge', 'calls').find(value => value.name === 'cancel_pending_multisig');
  assert.equal(cancellation.index, 18);
  assert.deepEqual(cancellation.fields.map(field => field.name), ['network_id', 'call_hash', 'timepoint']);
  const multisig = currentModel.pallets.find(item => item.name === 'BridgeMultisig');
  for (const name of ['PendingOperationsByProposer', 'ProposerCountedOperations', 'CancellationApprovals']) {
    assert(multisig.storage.entries.some(entry => entry.name === name), 'Capacity storage must remain present: ' + name);
  }
  report.predecessorMetadataComparison = { previousMetadataFile: 'previous-candidate-metadata.json',
    previousMetadataSha256,
    previousCandidateSha256: 'faf9ab84f3087639c4913ada9e17ea055fffc913b936b3c366fa2791ccd5ea59',
    candidateMetadataSha256: report.inputs.candidate.metadataSha256,
    reportFile: 'predecessor-metadata-compatibility.json', reportSha256: sha256(readFileSync(predecessorMetadataPath)),
    existingEncodingsPreserved: true, noAbiOrConstantChanges: true, capacityAbiRetained: true };
  // The fee repair adds sponsorship and bridge operation counters, plus append-only error,
  // event and call variants. The comparator checks every existing index and
  // encoding recursively; retain every addition for council review.
  report.metadataAdditions = metadata.additions;
  assert(metadata.constantValueChanges.every(change => change.path === 'System.constants.Version'), 'Only the runtime version constant may change');
  assert.deepEqual(report.inputs.candidate.imports, report.inputs.baseline.imports, 'Host import names, kinds and order must be unchanged');
  const oldInterfaces = report.inputs.baseline.importedInterfaces;
  const newInterfaces = report.inputs.candidate.importedInterfaces;
  assert.deepEqual(newInterfaces.filter(value => value.kind === 'function'), oldInterfaces.filter(value => value.kind === 'function'), 'Host function signatures must be unchanged');
  const oldMemory = oldInterfaces.filter(value => value.kind === 'memory');
  const newMemory = newInterfaces.filter(value => value.kind === 'memory');
  assert.equal(oldMemory.length, 1); assert.equal(newMemory.length, 1);
  assert.deepEqual({ ...newMemory[0], minimumPages: oldMemory[0].minimumPages }, oldMemory[0], 'Only imported memory minimum may change');
  const sdkCommit = 'e3737178ec726cffe506c907263aaaa417893fd0';
  assert(readFileSync(resolve(repositoryRoot, 'Cargo.lock'), 'utf8').includes('#' + sdkCommit));
  // Pinned executor source: static allocation adds its configured extra pages to
  // each module's own declared minimum. Existing policy is retained source context,
  // not a newly executed production-host test.
  report.memoryCompatibility = { baseline: oldMemory[0], candidate: newMemory[0],
    minimumChangePages: newMemory[0].minimumPages - oldMemory[0].minimumPages,
    defaultStaticExtraHeapPages: 2048, wasmPageBytes: 65536,
    defaultBaselinePages: oldMemory[0].minimumPages + 2048,
    defaultCandidatePages: newMemory[0].minimumPages + 2048,
    maximumSharedAndAddressTypeUnchanged: true, pinnedSdkCommit: sdkCommit,
    executorSourceReference: 'https://github.com/paritytech/polkadot-sdk/blob/' + sdkCommit + '/substrate/client/executor/common/src/runtime_blob/runtime_blob.rs',
    sourceContext: 'setup_memory_according_to_heap_alloc_strategy uses each memory initial() and adds static extra_pages; actual candidate instantiation is covered by the exact-Wasm fee rehearsal.' };
  assert.deepEqual(report.inputs.candidate.exports, report.inputs.baseline.exports, 'Wasm exports must be unchanged');
  const oldVersion = report.inputs.baseline.info.core_version;
  const newVersion = report.inputs.candidate.info.core_version;
  assert.equal(oldVersion.specVersion, snapshot.runtimeVersion.specVersion);
  assert.equal(newVersion.specVersion, 134);
  assert.equal(oldVersion.implVersion, 1);
  assert.equal(newVersion.implVersion, 1);
  assert.equal(newVersion.transactionVersion, 131);
  for (const key of ['specName', 'implName', 'authoringVersion', 'stateVersion', 'apis']) {
    assert.deepEqual(newVersion[key], oldVersion[key], 'Unchanged runtime version field: ' + key);
  }
  report.checks = { existingScaleEncodingCompatible: true, metadataAdditionsRecorded: true,
    previous4812EncodingsPreserved: true, noFollowupAbiOrConstantChanges: true, capacityAbiRetained: true,
    onlyRuntimeVersionConstantChanged: true, hostImportNamesKindsAndOrderUnchanged: true,
    hostFunctionSignaturesUnchanged: true, importedMemoryLimitsCompatible: true, memoryGrowthCompatibleWithPinnedSdkDefaultAllocation: true,
    wasmExportsUnchanged: true, runtimeApiVersionsUnchanged: true, transactionVersionRemains131: true,
    specVersionAdvancedTo134: true, implVersionRemains1: true };
  report.limitations = [
    'Structural metadata comparison covers declared SCALE layouts, indices, storage hashers/defaults and signed extension shapes; it does not execute migrations or prove custom codec behavior.',
    'Import comparison checks names, kinds, order, resolved function parameter/result types and imported memory limits/shared/address type. It does not prove host implementation behavior. Actual report rejection and signed fees are covered by the separate exact-Wasm rehearsal.',
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
