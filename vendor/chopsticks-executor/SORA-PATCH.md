# SORA Nexus reviewed Chopsticks executor

This package retains the npm API and version of
`@acala-network/chopsticks-executor` 1.5.2 and replaces its prebuilt Wasm with a
source build. The executor Rust sources are unchanged from Chopsticks commit
`f6eab24db852e24c2c66e459600c3b9d68e2aa7b`. Its exact smoldot gitlink,
`d3006b601d2b10fc0002c3e8022954bd37f052c9` (smoldot 1.1.3), is retained under
`smoldot/`; workspace metadata is made explicit. Original licenses are retained.

The upstream executor lock contains a ruzstd 0.8.3 decompression bomb
(RUSTSEC-2026-0344), unsound event-listener 5.4.1 (RUSTSEC-2026-0221), retired
libsecp256k1 0.7.2, and yanked spin 0.9.8. An npm-only audit cannot inspect those
Rust libraries inside its Wasm. This migration:

- Updates ruzstd to 0.9.1 and enforces the decoded size after the final block,
  fixing the same output-limit bypass already reviewed in the node's smoldot.
- Updates event-listener to 5.4.2 and spin to 0.9.9 through the committed lock.
- Routes historical signing and recovery host functions through the maintained
  k256-backed `sora-secp256k1-compat` package, retaining their wire semantics.
- Rebuilds with wasm-bindgen 0.2.129 and regenerates matching ESM/CJS glue and
  declarations. Rust sources and JavaScript callback signatures are unchanged.

The independent `Cargo.lock` audits with zero vulnerabilities and zero warnings
using the current RustSec database and `--deny warnings`. No advisory is ignored
and no retired implementation is renamed. CI must audit this ninth lock and
verify/rebuild the bundled assets; auditing source manifests alone is insufficient.

Validation on 2026-10-24:

- The locked Wasm release build passes, as do all six integrated smoldot
  decompression tests, including oversized-window and final-block regressions.
- Twelve fixed state-root vectors, proof creation/decoding round trips, callbacks,
  malformed inputs, and runtime execution match the unmodified npm 1.5.2 reference.
- Both generated CJS and ESM packages pass the offline smoke suite. It rejects
  the compressed huge-window fixture and actually executes a fixed Wasm runtime's
  Core_version and integer-addition exports with signature mocking disabled.
- The current full SORA Nexus runtime's metadata and actual Core_version
  execution also match the historical executor. This is a local execution check,
  without live-chain queries or transactions.
- The canonical Linux rebuild compares all fourteen generated assets byte for byte.
  The embedded Wasm is 2,723,362 bytes; SHA-256:
  `e58f725beeb705f1b79234046ea2f12b5b6a73f9b8851980b0d7895252c41966`.

`build-provenance.json` binds the committed Rust source, manifests, lockfile,
shared compatibility adapter, build script, and generated assets. The verifier
fails if these inputs change without rebuilding. `build-reviewed.mjs --check`
rebuilds from the locked sources and compares all generated assets; it never
rewrites the committed outputs in check mode. The original yarn/wasm-pack build
path and intermediate generated pkg directory have been removed.

To reproduce on Linux x86-64, install `nightly-2026-10-23` with the `wasm32-unknown-unknown` target,
wasm-bindgen-cli 0.2.129, and the build-only npm packages @swc/core 1.16.20 and
fflate 0.8.2 in a temporary prefix. Every tool version is checked by the script.
Use a persistent Cargo target directory; for example, from the repository root:

```sh
python3 vendor/chopsticks-executor/scripts/verify-artifacts.py
CARGO_TARGET_DIR=/tmp/sora2-rustcrypto-migration-target \
  node vendor/chopsticks-executor/scripts/build-reviewed.mjs \
  --check --build-tools-dir /tmp/executor-build-tools
node vendor/chopsticks-executor/scripts/smoke.cjs \
  /path/to/installed/@acala-network/chopsticks-executor/dist/cjs/index.js
```

The smoke argument points to an installed/copied package so both ESM and CJS
resolve the same declared npm dependencies. Expected fixtures were captured from
the historical package independently of the new implementation. The small Wasm
fixture's WAT source is retained beside it.

Source/reference archive SHA-256 values:

- Chopsticks source: `c0f40d85b4cf90b84bd75d196bf3034179971401a1704071d9751e1ab6fc9f3e`
- Exact smoldot source: `13d68797c42d29970ce43f6fefc9b544235adc60ccb8c27810edf33753af7f20`
- Original executor npm archive: `592cfd3d9ce72e06e2d50748894513c7e5bdab0ec5bd39403d27114c73ecb693`

The temporary original npm/Rust references are validation inputs only; they are
not dependencies of the rebuilt package or committed application lockfiles.

The standalone workspace contains a small `adapter/Cargo.toml` that points its
library path at the single reviewed `../sora-secp256k1-compat/src/lib.rs` source.
This avoids Cargo hashing an absolute external package path into symbols while
keeping one implementation. The manifest and shared source are both bound into
build provenance. The canonical build pins the effective release profile
(`opt-level=s`, thin LTO, one codegen unit, panic abort) and remaps repository,
Cargo-home and Rust-sysroot paths. The shared adapter's repository license is
included in distributed packages as `SORA-LICENSE`.

The final Linux checkout-path/cache/target comparison is recorded in
`../../misc/dependency_security/wasm-rebuild-2026-10-24.json`.

Canonical generated assets now require the Linux x86-64 GNU Rust host. Other
hosts should use [the pinned container rebuild helper](../../misc/dependency_security/WASM-REBUILDS.md).
The scripts enforce this host and the exact compiler commit, and map Rust sources
to `/rustc/<commit>` independently of whether `rust-src` is installed. Final
Linux checkout-path reproduction is recorded in the permanent rebuild evidence;
earlier macOS artifact comparisons are historical only.
