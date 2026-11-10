'use strict';
const assert = require('node:assert/strict');
// WebAssembly.Module.imports omits function signatures and memory limits.
// Read those from the already engine-validated binary; reject unsupported forms
// instead of silently weakening the interface comparison for future artifacts.
function readInterfaces(bytes, reflectedImports, reflectedExports) {
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
  const functionTypes = [];
  const exports = [];
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
          const typeIndex = section.u32();
          const signature = types[typeIndex];
          functionTypes.push(typeIndex);
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
    } else if (id === 3) {
      const count = section.u32();
      for (let index = 0; index < count; index++) {
        const typeIndex = section.u32();
        assert(types[typeIndex], 'Invalid Wasm defined function type index');
        functionTypes.push(typeIndex);
      }
      section.done();
    } else if (id === 7) {
      const kinds = ['function', 'table', 'memory', 'global', 'tag'];
      const count = section.u32();
      for (let index = 0; index < count; index++) {
        const name = section.name(), kind = kinds[section.byte()], itemIndex = section.u32();
        assert(kind, 'Unsupported Wasm export kind');
        if (kind === 'function') {
          const typeIndex = functionTypes[itemIndex];
          assert(typeIndex !== undefined, 'Invalid Wasm exported function index');
          exports.push({ name, kind, ...types[typeIndex] });
        } else {
          exports.push({ name, kind });
        }
      }
      section.done();
    }
  }
  assert.deepEqual(imports.map(({ module, name, kind }) => ({ module, name, kind })), reflectedImports,
    'Parsed import names and kinds must match the independent WebAssembly engine');
  assert.deepEqual(exports.map(({ name, kind }) => ({ name, kind })), reflectedExports,
    'Parsed export names and kinds must match the independent WebAssembly engine');
  return { imports, exports };
}

function defaultStaticMemoryPages(memory, extraPages) {
  assert.equal(memory.addressType, 'i32', 'Only wasm32 memory is supported');
  assert.equal(memory.shared, false, 'Runtime memory must be unshared');
  assert(Number.isSafeInteger(extraPages) && extraPages >= 0, 'Invalid static extra heap pages');
  assert(Number.isSafeInteger(memory.minimumPages) && memory.minimumPages >= 0, 'Invalid imported memory minimum');
  const effectivePages = memory.minimumPages + extraPages;
  assert(effectivePages <= 65536, 'Default static heap exceeds wasm32 65,536-page address space');
  return effectivePages;
}
function compareRuntimeExports(baseline, candidate) {
  const identity = value => value.name + '::' + value.kind;
  const sorted = values => [...values].sort((a,b) => identity(a).localeCompare(identity(b)));
  // The linker may omit its unused data-end marker. It is not an allocator
  // contract: both inspected runtime hosts use __heap_base. This exception
  // permits only removal of that exact non-callable marker, never additions,
  // type changes, required host exports or any runtime function ABI change.
  const removedLinkerGlobals = baseline.filter(value => value.name === '__data_end' && value.kind === 'global'
    && !candidate.some(current => current.name === value.name));
  assert.deepEqual(sorted(candidate), sorted(baseline.filter(value => !removedLinkerGlobals.includes(value))),
    'Required Wasm exports and exported function signatures must be unchanged');
  return { removedLinkerGlobals, allExportNamesAndKindsUnchanged: removedLinkerGlobals.length === 0,
    requiredExportsAndFunctionSignaturesUnchanged: true };
}
module.exports = { readInterfaces, defaultStaticMemoryPages, compareRuntimeExports };
