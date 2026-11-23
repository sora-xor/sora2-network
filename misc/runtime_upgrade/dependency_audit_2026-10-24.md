# SORA Nexus dependency security review

Reviewed on 2026-10-24. All eleven maintained Cargo lockfiles now have **zero
known vulnerability matches and zero RustSec warnings of any category**. The root
lock improved from **50 to 0 vulnerability matches**, **9 to 0 unsoundness
warnings**, **12 to 0 maintenance warnings**, and **1 to 0 yanked packages**
against the same advisory database. CI now fails on all RustSec warnings.

The locked node/native-runtime check and complete no-std runtime Wasm check
pass. The historical Iroha verifier was also executed in Wasm. Four native Python
extensions were rebuilt from audited source, closing vulnerabilities invisible to
Python package auditing. CPython 3.12 and 3.13 native builds and upstream tests
pass, including the complete governance compatibility suite on both interpreters.

[Machine-readable evidence](dependency-audit-2026-10-24.json) records the baseline,
current lockfile hashes, findings and validation stages. The comparison uses
`cargo-audit 0.22.1` and RustSec database commit
`21d909e4c8bd4e14c7aea58bf980c881a0053ab1` (1,352 advisories). CI uses 0.22.2;
changes in scanner treatment of Git/path dependencies are not remediation.
Consumer source patches were reviewed and tested separately. No advisory ignores
or synthetic safe dependency versions were introduced.

The initial scan found 49 matches. The database refresh added
RUSTSEC-2026-0387 for `crypto-bigint`; both the original and updated lockfiles were
rescanned against that same refreshed database. Full-lockfile matches include
optional and target-specific dependencies and do not prove production reachability.

| Maintained lockfile | Vulnerabilities | Unsound warnings | Unmaintained | Yanked |
| --- | ---: | ---: | ---: | ---: |
| Root `Cargo.lock` | 0 | 0 | 0 | 0 |
| `misc/farming_snapshot/convert/Cargo.lock` | 0 | 0 | 0 | 0 |
| `misc/rewards_snapshot/get_lp_tokens/Cargo.lock` | 0 | 0 | 0 | 0 |
| `vendor/sora2-common/Cargo.lock` | 0 | 0 | 0 | 0 |
| `misc/runtime_upgrade/vendor/native/py-bip39-bindings/Cargo.lock` | 0 | 0 | 0 | 0 |
| `misc/runtime_upgrade/vendor/native/py-ed25519-zebra-bindings/Cargo.lock` | 0 | 0 | 0 | 0 |
| `misc/runtime_upgrade/vendor/native/py-sr25519-bindings/Cargo.lock` | 0 | 0 | 0 | 0 |
| `misc/runtime_upgrade/vendor/native/pydantic-core/Cargo.lock` | 0 | 0 | 0 | 0 |
| `vendor/chopsticks-executor/Cargo.lock` | 0 | 0 | 0 | 0 |
| `vendor/polkadot-wasm-crypto/Cargo.lock` | 0 | 0 | 0 | 0 |
| `vendor/tikv-jemalloc-ctl-0.6.1/Cargo.lock` | 0 | 0 | 0 | 0 |

[Raw final Cargo audit responses](../dependency_security/cargo-audit-2026-10-24.json)
bind each result to its current lockfile hash and the advisory database commit.

## Completed dependency migrations

- Advanced reachable SDK references to **polkadot-stable2603-7**. Upstream source
  comparisons preserve the custom BABE, GRANDPA, preimage and staking changes.
  This remains the maintained release line used by the project; `stable2609`
  would entail an additional API/runtime migration.
- Backported the upstream Wasmtime 35-to-36 dependency upgrade into two SDK
  consumers, resolving **36.0.19** without changes to their upstream Rust source.
  See [SDK PR11793](https://github.com/paritytech/polkadot-sdk/pull/11793) and
  [local provenance](../../vendor/polkadot-sdk/client/executor/wasmtime/SORA-PATCH.md).
- Migrated the SDK, Alloy, libp2p identity and Subxt consumers to actual upstream
  **k256/p256 0.14, elliptic-curve 0.14.1, crypto-bigint 0.7.6 and bip32 0.6**.
  This removes `crypto-bigint 0.5.5` rather than backporting or relabeling it.
  Fixed vectors preserve public keys, signature bytes, recovery IDs, SEC1/DER
  encodings and wallet derivation. Libp2p normalizes high-S signatures before
  verification to preserve its previous accepted signature set. Provenance is
  recorded for [SDK core](../../vendor/polkadot-sdk/primitives/core/SORA-PATCH.md),
  [identity](../../vendor/libp2p-identity/SORA-PATCH.md),
  [Alloy](../../vendor/alloy-consensus/SORA-PATCH.md) and
  [Subxt](../../vendor/subxt-signer/SORA-PATCH.md).
- Replaced the Iroha Dalek v3 fork with **ed25519-dalek 2.2.0 /
  curve25519-dalek 4.1.3**. The verifier uses `raw_verify<Sha3_512>` over the
  existing `Sha3_256` message, preserving the Iroha transcript. It is not stock
  Ed25519ph. Verification was compared across 49,172 cases, with 768 generated
  signatures byte-identical to the legacy implementation; native pallet and
  production-verifier Wasm tests pass.
- Migrated DNS and resolver consumers to **Hickory 0.26.3**, with the new
  `hickory-net` runtime adapters. Migrated certificate handling to
  **rcgen 0.14.10 / rustls-webpki 0.103.16**, removing the old `ring 0.16` and
  webpki branches; the remaining ring version is **0.17.14**. UPnP and standalone
  RPC migrations remove the old h2 branch, leaving **h2 0.4.20**. Local resolver,
  certificate and authenticated transport tests pass. See
  [resolver provenance](../../vendor/libp2p-dns/SORA-PATCH.md) and
  [TLS provenance](../../vendor/libp2p-tls/SORA-PATCH.md).
- Migrated tracing, libp2p and Smoldot cache consumers to **lru 0.19.0**. Updated
  tracing-log's benchmark-only Criterion to **0.8.2** to remove the unsound
  `atty` dependency without dropping benchmark coverage. See
  [cache and benchmark provenance](../../vendor/tracing-log/SORA-PATCH.md).
- Migrated object/Smoldot decompression to **ruzstd 0.9.1**. Added excessive-window
  and output-limit regressions; source review also fixed a final raw-block
  output-limit bypass that persisted after the dependency update. Six tests pass,
  including restored runtime fixtures. See
  [decompression provenance](../../vendor/smoldot/SORA-PATCH.md).
- Updated **tracing-subscriber 0.3.23**, **webpki-roots 1.0.11** and
  **memmap2 0.9.12** through their consumers. Logging escapes untrusted terminal
  sequences; the trust-store test checks removal of emSign and retention of ISRG.
  The parity-db migration preserves its implementation and disk format.
- Updated compatible affected registry dependencies, including crossbeam-epoch,
  quinn-proto, rustls, ruint, webpki-root-certs, anyhow, cxx, event-listener and
  tokio. Replaced yanked spin 0.9.8 with 0.9.9. Removed the obsolete thread_local
  Git override that renamed upstream 1.0.1 to 1.9.9; the graph now uses maintained
  **1.1.10** with the actual subsequent fixes.
- Pinned Rust to **nightly-2026-10-23**, with `rust-src` and Wasm targets. Cargo's
  `WASM_BUILD_RUSTFLAGS=-C link-arg=--import-undefined` preserves the SDK's host
  function imports under Rust 1.96+, while undefined data symbols remain errors.
  `wasm32v1-none` matches the SDK's preferred feature baseline. See the
  [Rust linker change](https://blog.rust-lang.org/2026/04/04/changes-to-webassembly-targets-and-handling-undefined-symbols/).
- Migrated the farming converter to the same patched SDK core and refreshed its
  lockfile. Refreshed the maintained sora2-common standalone lock with the shared
  security patches and **jsonrpsee 0.24.11**. The root lock remains authoritative
  for the node; each standalone lock is independently audited.

These changes use secure supported release lines where feasible. They do not
claim that every dependency is on its newest major version.

## Python tooling and native-library review

The reviewed `substrate-interface` 1.8.1 source snapshot replaces vulnerable
Python `ecdsa` with **coincurve 21.0.0 / libsecp256k1** for BIP32 and ECDSA.
SR25519 remains the governance CLI's signing scheme. Offline tests cover official
BIP32 vectors, malformed derivation inputs, BIP39 normalization, existing key and
signature fixtures, and byte-identical governance call/extrinsic encodings.
[Source provenance and scope](vendor/README.md) describe the retained upstream
behavior and the local changes.

The unused Python `smoldot-light` native extension was removed. Its source lock
contains vulnerable ruzstd, lru and Wasmtime versions; the installed macOS binary
confirms the first two. Light-client transport is explicitly unsupported by the
CLI snapshot; HTTP/WebSocket RPC behavior is retained. See the
[native dependency evidence](vendor/native-dependency-review.json).

The current environment has **29 runtime distributions** (25 registry pins and
four local source builds); the strict installed-package audit also includes pip
and reports **zero vulnerabilities across 30 distributions**. The three Substrate
cryptographic bindings now use patched PyO3 0.29.3, rand 0.8.8 and keccak 0.1.6
as applicable. Pydantic-core uses LRU 0.19.0, including the upstream shared-iterator
Sync-bound fix even though that correction has no RustSec advisory yet.

All four native Cargo locks have **zero vulnerabilities and zero warnings**.
The actual rebuilt binaries were inspected for their Rust dependency versions.
All four wheels build with Rust 1.99.1 on CPython 3.12 and 3.13; **274 upstream
binding/Pydantic tests pass**. The complete CPython 3.12 and 3.13 environments each pass
all 12 offline governance and cryptographic compatibility tests (298 Python
tests in total including the upstream suites). The installer
works from an unrelated directory, preserves every Cargo.lock hash, and rejects
a stale lock without rewriting it. Explicit `MATURIN_PEP517_ARGS=--locked` is
required because Maturin reads its pyproject `locked=true` option after its first
metadata request. See [native source/build provenance](vendor/native/README.md).

Use a fresh environment and `install_dependencies.py`: updating requirements
alone does not uninstall removed `ecdsa` or `smoldot-light` distributions.

The pinned Python 3.13/Trixie and Rust 1.99.1 Docker build now passes on Linux
arm64, including all four native wheels and the complete runtime image. Its
installed wheels pass 149 offline tests; the final image repeats 39
compatibility/binding tests using its packaged application modules. The image
runs as UID 1000, passes CLI help and `pip check`, and its installed-path audit
reports zero vulnerabilities across 30 distributions including pip. The fresh
current-source runtime Wasm matches SHA-256
`82b5e49cc810af6a44d9f640a9a81872dfe103190f9544aef52728fa95e9d237`.
No Rust/C build tools remain in the runtime image. The local validation profile's
DNS repair survives a full restart without changing the global Docker context.
Image, wheel, native binary and lockfile hashes are in the
[container validation record](vendor/native-dependency-review.json).

## Completed maintenance migrations

The twelve formerly unmaintained root packages and the yanked core2 release are
absent from the resolved graph. No warning is suppressed.

| Retired dependency | Replacement through its consumers |
| --- | --- |
| async-std / async-std-resolver | Smol 2 networking, timers and task adapters; legacy feature names retain SDK source compatibility |
| core2 0.4.0 | cid 0.11.3, multihash 0.19.5, and no-std-io2 for multihash 0.17 |
| derivative | Standard derives for concrete types; derive-where 1.7 for generic bounds and skipped fields |
| fxhash | rustc-hash 2 in fxprof-processed-profile |
| instant | parking_lot 0.12 in wasm-timer |
| libsecp256k1 Rust crate | New k256-backed compatibility adapter preserving historical scalar parsing, high-S and recovery behavior |
| number_prefix | unit-prefix in indicatif |
| parity-util-mem | Removed unused pallet test dependencies |
| parity-wasm / wasm-instrument | wasmparser / wasm-encoder 0.262.0; explicit metadata, heap and deterministic stack-metering passes |
| paste | pastey 0.2.3 in SDK, ORML, Arkworks and remaining consumers |
| proc-macro-error / proc-macro-error2 | Maintained syn::Error diagnostics in Aquamarine, multihash-derive and Subxt macros |

The new secp256k1 adapter passes 14,094 differential checks against the historical
implementation, including 1,024 byte-identical deterministic signatures. Six
permanent regression tests exercise overflowing scalars, high-S verification,
hybrid keys and recovery IDs 2/3. All arithmetic is performed by maintained k256;
no old arithmetic implementation is copied. See
[adapter provenance](../../vendor/sora-secp256k1-compat/README.md).

Wasm changes preserve API tuple encoding, first-match custom-section replacement,
opaque debug metadata and wasm32 heap growth. Stack metering preserves historical
cost accounting and call thunks. Imported globals are rejected explicitly because
the old limiter used an incorrect counter index for them. See
[executor provenance](../../vendor/polkadot-sdk/client/executor/common/SORA-PATCH.md).

## JavaScript remediation and historical release evidence

Correcting the historical 4.8.9 package paths exposed 106 actual dependencies
and two high-severity audit entries: Zod's GHSA-mrhx-c7xc-m3w5 and its parent
Chopsticks. The previous empty-graph audit did not establish safety. Original
runtime and frontend manifests/locks are retained byte-for-byte as explicitly
non-operational [historical evidence](../dependency_security/historical-npm/README.md),
with SHA-256 hashes. Release Wasm, snapshots and existing rehearsal results remain
unchanged.

The four operational rehearsal graphs now use the reviewed Chopsticks 1.5.2
consumer, Zod 4.6.7 and the executor rebuilt from audited Rust source. The Zod
migration updates the record-schema API and both JavaScript module formats,
regenerates its TypeScript declarations, and passes six installed-package tests,
including the oversized-array regression. Its complete TypeScript source
compiles with the new types. The core public API loads with the rebuilt executor.

Native source inspection also exposed vulnerable Rust dependencies inside the
published Polkadot crypto Wasm and optional smoldot light client. The crypto
module is rebuilt from its audited source graph and uses modern generated
wasm-bindgen wrappers for all 32 public crypto functions. The old pointer bridge,
crypto initialization packages, and unsafe asm.js fallback are removed. The
reviewed RPC-provider manifest removes its unused optional `@substrate/connect`
dependency, retiring smoldot 2.0.26 from every operational graph. Its HTTP,
WebSocket and explicitly injected ScProvider JavaScript remain unchanged;
standalone structural declarations preserve the injected client's type contract.
The final installed crypto facade passes 2,928 differential and 2,619 offline
security assertions across all 32 exports, including malformed input,
randomized signature cross-verification and historical digest boundary cases.
Both native packages use a canonical Linux x86_64 build host, matching CI. The
scripts pin the effective release profile and remap source/toolchain paths;
generated host-dependent symbols are preserved. The
[container build recipe](../dependency_security/WASM-REBUILDS.md) documents the
supported environment and strict byte-for-byte checks. An independent Linux
checkout with different Cargo cache and target paths reproduces all 32 generated
assets exactly; the documented read-only container check also passes. See the
[final rebuild evidence](../dependency_security/wasm-rebuild-2026-10-24.json).
The [light-client review](../dependency_security/removed-light-client-review.json)
records the actual removed Wasm hash and vulnerable embedded versions.

CI stages the complete local package tree with original relative paths, rejects
empty or link-only graphs, and runs an offline dependency-tree check before
auditing. All five staged graphs pass: 90 dependency records in each runtime
graph and 51 in the frontend reproduction, with zero known npm vulnerabilities.
A guard rejects reintroduced retired bundles and registry duplicates of the
reviewed native packages. Fresh installation succeeds with lifecycle scripts
disabled. Seven audit-staging regression tests verify these rules, including removed or
replaced direct pins and canonical provider/executor endpoints. The
[npm evidence](../dependency_security/npm-audit-2026-10-24.json) records source
hashes, package counts and full audit responses. Both Wasm packages have separate
Cargo audits, source/asset provenance verification, reproducible source builds,
and installed-package execution tests in CI.

Seven installed integration tests pass under Node.js 24.22.1 LTS and 26.12.1:
CJS/ESM readiness, explicit disable/restoration, unsupported asm.js, absence of
retired packages, Ed25519/Sr25519/ECDSA keyring signing, and real local HTTP and
WebSocket JSON-RPC round trips. TypeScript declarations compile in strict mode,
including the original substrate-connect interface. `.nvmrc` now selects the
supported Node.js 24 line.

Polkadot API 16.5.6 remains pinned to preserve the frontend bug reproductions.
All three reproduction tests pass with the secure dependency closure; a
17.0.2 install changes the mixed-validator-claim result because upstream fixes
the bug. Updating that pin would destroy the evidence. The final deprecated
`node-domexception` shim is also removed: verified fetch-blob 3.2.0 source uses
the native Node.js DOMException that the old shim already exported on this
supported runtime. Three installed-package tests verify file reads,
slicing/streaming and changed-file error behavior. The retired packages are
absent, and the final fresh installation emits no deprecation notices.

Pre-existing runtime work and 4.8.12 artifacts were preserved. Their binaries
and rehearsals do not validate the updated dependencies. Fresh local runtime
Wasm was separately exercised through the rebuilt executor, including actual
Core_version execution; no production deployment or historical release rewrite
was performed.

## Validation stages

The following checks cover the completed migrations:

| Check | Status |
| --- | --- |
| Eleven maintained Cargo lockfiles | Zero vulnerabilities and zero warnings, with `--deny warnings` |
| Locked node binary and native runtime | Passed; fresh release-profile Wasm generated for the container check |
| RustCrypto consumers | 48 compatibility tests passed in an isolated workspace using the vendored sources and compatible root versions |
| SDK ECDSA | 23 tests passed; sp-core no-std/full_crypto checks passed natively and for Wasm |
| Alloy consensus no-std/k256 | Passed |
| Iroha verifier | 768 byte-identical signatures; 49,172 differential cases; 20 pallet tests passed |
| Complete runtime no-std Wasm compilation | Passed |
| Production Iroha verifier in Wasm | Passed: historical fixture accepted; altered message and noncanonical scalar rejected |
| Networking | 28 tests passed |
| Smoldot decompression | 6 tests passed |
| Installed Python environment | 29 runtime packages; strict audit of 30 distributions including pip is clean; 12 offline tests passed on each of CPython 3.12 and 3.13 |
| Python native wrappers | Four source builds on CPython 3.12/3.13; 274 upstream tests passed; four Cargo audits clean |
| Farming converter with migrated SDK core | Locked check passed |
| Runtime fee fixture using migrated Iroha signing | 5 tests passed |
| Tracing/LRU after Criterion 0.8.2 update | 14 tests passed; benchmark compilation passed |

The maintenance phase additionally passes 137 common tests, 18 rewards tests,
20 Iroha pallet tests, three metadata tests, nine Wasm builder tests and 11
executor memory/stack tests.
All 33 Wasmtime executor tests pass; its existing long-running instance-leak
stress test remains ignored by upstream in debug builds. Network and utility
migrations pass 87 focused tests/doctests. Macro migrations pass 126 Arkworks,
11 Aquamarine and four multihash-derive tests, plus four compiler-diagnostic
checks. The bundled TiKV allocator lock was also refreshed to match its pastey
manifest, and its locked all-feature library check passes. These results are
separate from the earlier phase totals below.

Earlier, **phase 1** passed 225 Rust tests with one upstream test ignored:
common 137, Wasmtime executor 33, logging 19, Wasm interface 2, WebSocket trust
store 1 and parity-db 33. That phase also passed the snapshot utility build,
formatting, lint-policy/runner checks, the stable-SDK guard and Actionlint.
Those results are retained as historical validation, **not represented as reruns
on the final dependency graph**.

Logging tests require `RUST_LOG` to be unset; the machine's inherited `warn`
filter suppresses upstream tests that expect `info` output. The node check uses
the installed LLVM 22 library paths. Existing compiler deprecation warnings
remain; a full warnings-as-errors Clippy gate was not run. Fresh runtime Wasm
generation and offline execution are verified. Production chain rehearsals and
promotion or recertification of archived release artifacts are separate release
requirements.

Weekly Dependabot updates cover maintained Cargo projects, active Python
requirements and Actions. The dependency-security workflow audits eleven Cargo
locks and five operational npm graphs on relevant PRs/pushes and weekly, with
`--deny warnings` for Rust. Graph completeness and local-package identities are
checked before npm auditing. Both embedded Wasm packages are rebuilt from locked
source and checked against their committed artifacts; installed-package tests
cover their public interfaces. Original npm locks are retained only as explicitly
non-operational historical evidence.

Reproduce the root Rust scan with a healthy, current advisory database:

```sh
cargo audit --db /tmp/sora2-rustsec-advisory-db --deny warnings --file Cargo.lock
```

Do not use `--stale` to substantiate a current review. Package auditing does not
inspect Rust libraries bundled inside Python wheels, and successful compilation
alone does not establish security.
