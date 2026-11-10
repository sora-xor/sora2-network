'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');
const sha256 = value => createHash('sha256').update(value).digest('hex');
exports.validateDependencies = function(directory) {
  const manifest = JSON.parse(fs.readFileSync(path.join(directory, 'package.json')));
  const report = { manifest, installed: {}, localSources: {}, nativeArtifacts: {}, lockSha256: sha256(fs.readFileSync(path.join(directory, 'package-lock.json'))) };
  for (const [name, spec] of Object.entries(manifest.dependencies)) {
    const installed = JSON.parse(fs.readFileSync(path.join(directory, 'node_modules', name, 'package.json')));
    let expected = spec;
    if (spec.startsWith('file:')) {
      const source = path.resolve(directory, spec.slice(5));
      const sourceBytes = fs.readFileSync(path.join(source, 'package.json'));
      const pkg = JSON.parse(sourceBytes);
      assert.equal(pkg.name, name); expected = pkg.version;
      report.localSources[name] = { path: source, version: pkg.version, manifestSha256: sha256(sourceBytes) };
    }
    assert.equal(installed.name, name); assert.equal(installed.version, expected);
    report.installed[name] = installed.version;
  }
  for (const [name, relative] of [['@acala-network/chopsticks-executor','../../vendor/chopsticks-executor'],['@polkadot/wasm-crypto-wasm','../../vendor/polkadot-wasm-crypto-wasm']]) {
    const source = path.resolve(directory, relative);
    const provBytes = fs.readFileSync(path.join(source, 'build-provenance.json'));
    const provenance = JSON.parse(provBytes);
    for (const [file, expected] of Object.entries(provenance.assets)) {
      assert.equal(sha256(fs.readFileSync(path.join(directory, 'node_modules', name, file))), expected, `${name}/${file}`);
    }
    report.nativeArtifacts[name] = { wasmSha256: provenance.wasm_sha256, wasmBytes: provenance.wasm_bytes, canonicalHost: provenance.canonical_host, assetsChecked: Object.keys(provenance.assets).length, provenanceSha256: sha256(provBytes) };
  }
  return report;
};
