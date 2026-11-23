# SORA Nexus crypto interface migration

Based on the exact published `@polkadot/wasm-crypto` 7.5.4 archive; UPSTREAM.json records its registry integrity, commit and archive hash.

All 32 public crypto functions delegate to generated wasm-bindgen wrappers from the reviewed, source-built local `@polkadot/wasm-crypto-wasm` package. This replaces the old pointer bridge, static return pointer and prebuilt unsafe Wasm/asm.js implementations. Public argument order, result types, readiness checks, CJS and ESM entrypoints are retained. Modern wasm-bindgen owns allocation and return decoding. Node.js 24 or newer with WebAssembly is required by the supported tooling.

`waitReady` initializes once and reports initialization failure as false. `initNone` explicitly disables the interface; explicit Wasm initializers restore it. `initOnlyAsm` records an unsupported-asm.js error and leaves the interface unavailable. `initWasmAsm` uses only the reviewed Wasm implementation. Legacy raw-memory allocation helpers and arbitrary legacy pointer-ABI factories are no longer supported; custom factories must return the reviewed generated module interface. The unused UMD bundles embedding the retired implementations are omitted. No unsafe asm.js fallback is installed.

The Rust implementation and reproducible generated artifacts are maintained in the adjacent `polkadot-wasm-crypto` and `polkadot-wasm-crypto-wasm` directories. Compatibility tests compare the published implementation with this facade across deterministic outputs and cross-verification of randomized signatures.
