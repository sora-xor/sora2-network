# SORA Nexus dependency security checks

The operational runtime rehearsal manifests consume the reviewed local
Chopsticks, Polkadot crypto and RPC-provider packages under vendor/. Keep `install-links=true`: npm must include
their full dependency records, rather than treating them as external symlinks.
Lifecycle scripts are disabled; the executor's reviewed build command is run
explicitly in CI.

To audit a release graph without changing the working copy:

```sh
audit_root=$(mktemp -d)
audit_project=$(python3 misc/dependency_security/prepare_npm_audit.py \
  runtime-upgrade-4.8.12/validation/package.json \
  runtime-upgrade-4.8.12/validation/package-lock.json "$audit_root")
cd "$audit_project"
npm ls --all --package-lock-only --offline
npm audit --package-lock-only --ignore-scripts --audit-level=low
npm ci --ignore-scripts --install-links
```

The helper preserves the repository-relative layout and recursively copies local
package dependencies. It records source hashes and package counts, rejects
empty/link-only graphs, and never changes the source manifest or lock. The five
operational project paths require their reviewed direct pins and exact local
package endpoints even if every direct pin is removed or replaced. For 4.8.9,
pass `rehearsal-package.json` and `rehearsal-package-lock.json`; the isolated copy
uses npm's standard names. For an actual rehearsal, use its explicit dependency
path option where available, or install beside its operational manifest. Do not
reuse the historical temporary dependency installation.

`historical-npm/` retains original records as evidence, not installation inputs.
The former malformed 4.8.9 graph hid a real Zod vulnerability; a successful audit
of an empty graph is not accepted. The current evidence file includes every
audited graph's package count and raw npm response.

The local Chopsticks schema tests can be run from an installed audit project:

```sh
node /absolute/repository/vendor/chopsticks-core/tests/zod-migration.cjs
```

The executor has separate source/asset verification and a locked rebuild under
`vendor/chopsticks-executor/scripts/`; its Cargo graph is audited independently
of npm metadata. This prevents an apparently current npm version from hiding an
old vulnerable Wasm dependency graph.

The source-built Polkadot crypto bundle has its own Cargo audit, reproducible
build, source/asset hashes, and published-version differential fixtures under
`vendor/polkadot-wasm-crypto/`. Its JavaScript facade preserves 32 crypto APIs
while using generated modern wasm-bindgen wrappers. The old pointer bridge and
asm.js fallback are removed. `@polkadot/rpc-provider` retains its unchanged
HTTP/WebSocket and injected ScProvider implementations, but no longer installs
the unused optional embedded smoldot light client. `test_js_native_integration.cjs`
checks the installed package closure, CJS/ESM readiness, keyring signatures, and
real local HTTP/WebSocket JSON-RPC round trips.

Use Node.js 24 or newer (`nvm use` follows `.nvmrc`). Validation includes Node.js
24.22.1 LTS, the [official current 24.x release](https://nodejs.org/en/blog/release/v24.22.1).
The reviewed fetch-blob 3.2.0 source uses Node.js's native DOMException, preserving
file/Blob behavior while retiring the deprecated node-domexception shim. Three
installed-package tests check file reads, slicing/streaming and changed-file errors.

The canonical Wasm build host is Linux x86_64. The
[final rebuild evidence](wasm-rebuild-2026-10-24.json) records exact matches from
an independent Linux checkout, Cargo cache and target, plus the documented
read-only container helper. macOS/Node.js checks execute those same Linux-built
portable Wasm assets; they are not cross-host byte-reproducibility claims.
