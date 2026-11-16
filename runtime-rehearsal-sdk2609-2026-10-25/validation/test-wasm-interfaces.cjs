'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { readInterfaces, defaultStaticMemoryPages, compareRuntimeExports } = require('./wasm-interfaces.cjs');
const u32 = value => { const bytes = []; do { const b = value & 127; value >>>= 7; bytes.push(b | (value ? 128 : 0)); } while (value); return bytes; };
const name = value => [...u32(Buffer.byteLength(value)), ...Buffer.from(value)];
const section = (id, data) => [id, ...u32(data.length), ...data];
function fixture(parameters = [0x7f], reverseTypes = false) {
  const signatures = [[0x60, 0, 1, 0x7f], [0x60, parameters.length, ...parameters, 1, 0x7f]];
  const types = reverseTypes ? signatures.reverse() : signatures;
  const bytes = Buffer.from([0,97,115,109,1,0,0,0,
    ...section(1, [2, ...types.flat()]),
    ...section(2, [1, ...name('env'), ...name('host'), 0, reverseTypes ? 1 : 0]),
    ...section(3, [1, reverseTypes ? 0 : 1]),
    ...section(7, [2, ...name('forwarded'), 0, 0, ...name('Runtime_api'), 0, 1]),
    ...section(10, [1, 4, 0, 0x41, 0, 0x0b])]);
  const module = new WebAssembly.Module(bytes);
  return { bytes, imports: WebAssembly.Module.imports(module), exports: WebAssembly.Module.exports(module) };
}
const parse = fixture => readInterfaces(fixture.bytes, fixture.imports, fixture.exports);
test('resolves imported and defined function indices to their exported signatures', () => {
  const result = parse(fixture());
  assert.deepEqual(result.exports, [
    { name: 'forwarded', kind: 'function', parameters: [], results: ['i32'] },
    { name: 'Runtime_api', kind: 'function', parameters: ['i32'], results: ['i32'] },
  ]);
});
test('type table ordering does not change the named ABI', () => {
  assert.deepEqual(parse(fixture([0x7f], true)), parse(fixture()));
});
test('exported arity/type changes are observable even with identical names and kinds', () => {
  const original = fixture(), changed = fixture([0x7e, 0x7f]);
  assert.deepEqual(original.exports, changed.exports);
  assert.notDeepEqual(parse(original).exports, parse(changed).exports);
});
test('independent engine export reflection must agree', () => {
  const value = fixture();
  assert.throws(() => readInterfaces(value.bytes, value.imports, []), /independent WebAssembly engine/);
});
test('default static heap accepts the exact wasm32 bound and rejects one extra page', () => {
  const memory = { addressType: 'i32', shared: false, minimumPages: 63488 };
  assert.equal(defaultStaticMemoryPages(memory, 2048), 65536);
  assert.throws(() => defaultStaticMemoryPages({ ...memory, minimumPages: 63489 }, 2048), /exceeds wasm32/);
});
test('unsupported memory widths, sharing and malformed page counts are rejected', () => {
  const memory = { addressType: 'i32', shared: false, minimumPages: 20 };
  for (const changed of [{ addressType: 'i64' }, { shared: true }, { minimumPages: -1 }]) {
    assert.throws(() => defaultStaticMemoryPages({ ...memory, ...changed }, 2048));
  }
  assert.throws(() => defaultStaticMemoryPages(memory, NaN));
});

const runtimeExports = [
  { name: 'Core_version', kind: 'function', parameters: ['i32', 'i32'], results: ['i64'] },
  { name: '__heap_base', kind: 'global' }, { name: 'memory', kind: 'memory' },
  { name: '__data_end', kind: 'global' },
];
test('only the unused data-end linker global may be removed', () => {
  const result = compareRuntimeExports(runtimeExports, runtimeExports.filter(value => value.name !== '__data_end'));
  assert.deepEqual(result.removedLinkerGlobals, [{ name: '__data_end', kind: 'global' }]);
  assert.equal(result.requiredExportsAndFunctionSignaturesUnchanged, true);
  assert.equal(result.allExportNamesAndKindsUnchanged, false);
  assert.equal(compareRuntimeExports(runtimeExports, runtimeExports).allExportNamesAndKindsUnchanged, true);
});
test('required allocator, memory and runtime function exports cannot be removed', () => {
  for (const name of ['__heap_base', 'memory', 'Core_version']) {
    assert.throws(() => compareRuntimeExports(runtimeExports, runtimeExports.filter(value => value.name !== name)), /Required Wasm exports/);
  }
});
test('linker kind changes, unreviewed additions and runtime signature changes fail', () => {
  for (const candidate of [
    runtimeExports.map(value => value.name === '__data_end' ? { ...value, kind: 'function', parameters: [], results: [] } : value),
    [...runtimeExports, { name: 'unexpected', kind: 'global' }],
    runtimeExports.map(value => value.name === 'Core_version' ? { ...value, parameters: ['i32'] } : value),
  ]) assert.throws(() => compareRuntimeExports(runtimeExports, candidate), /Required Wasm exports/);
});
