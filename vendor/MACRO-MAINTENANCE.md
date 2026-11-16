# SORA Nexus macro dependency migrations

The maintained macro implementations are `pastey` 0.2.3 and `derive-where`
1.7.0. Consumers name the actual new packages in Cargo manifests; the original
consumer package versions are retained. No replacement advertises itself as a
new release of `paste`, `derivative`, or `proc-macro-error`.

`pastey` is a maintained fork recommended by the
[RustSec paste advisory](https://rustsec.org/advisories/RUSTSEC-2024-0436.html).
Consumers retain the `paste` dependency alias so existing macro invocations and
public re-exports keep their names. This covers the three ark-ff versions,
alloy-primitives, common, frame-benchmarking, frame-support, netlink-packet-core,
ORML traits/benchmarking, sc-executor-wasmtime, simba, sp-core, sp-runtime, and
tikv-jemalloc-ctl.

`derive-where` replaces `derivative` in ark-ec 0.4.2, ark-ff 0.3.0/0.4.2,
ark-poly 0.4.2, bridge-types, and presto. Its `safe` feature avoids unchecked
unreachable branches in generated comparisons. Explicit per-trait generic
bounds and ignored Debug fields remain explicit. SparsePolynomial keeps its
historical equality behavior through a direct PartialEq implementation that
compares terms while ignoring the declared variable count. Its existing Hash
behavior is retained; this migration does not change polynomial algorithms or
serialized data. See the maintained project's
[bound and field-skipping documentation](https://docs.rs/derive-where/1.7.0/derive_where/).

Aquamarine 0.5.0, multihash-derive 0.8.1, and subxt-macro 0.44.3 use their existing
maintained `syn` dependency to return spanned compile errors. There is no copied
panic interception layer. Multihash duplicate diagnostics retain both the
duplicate and previous-definition locations. Aquamarine's internal unexpected
diagram-entry warning remains a no-op on stable Rust, matching its previous
behavior; invalid attributes, unterminated diagrams and unreadable include files
still emit errors. Subxt metadata argument errors return ordinary compiler
diagnostics rather than panicking from the procedural macro.

Each new vendored registry consumer records its original crate archive SHA-256
in SORA-PATCH.md. SDK consumers retain source tag `polkadot-stable2603-7`, commit
`cd87d0b95e54c8ffc374334386f3f2b2c7a61f54`, with workspace dependencies materialized.

Validation on 2026-10-24:

- All five production ark packages compile together with the maintained macros.
- Upstream ark-ff tests pass: 16 for 0.3.0, 21 for 0.4.2, and 22 for 0.5.0.
- All eight ark-ec 0.4.2 tests pass.
- All 59 ark-poly 0.4.2 tests pass, including field/domain transforms and sparse
  polynomial operations. The five ark suites therefore total 126 passing tests.
- Eleven Aquamarine tests pass, including three new malformed-input regressions.
- Four multihash-derive tests pass, including unchanged successful generated
  tokens, duplicate diagnostics, and missing required attributes.
- Subxt's macro compiles with default features disabled. Four real procedural
  macro invocations verify compile errors for missing/conflicting metadata
  sources, absent OUT_DIR, and a URL feature that is not enabled.
- The standalone converter compiles with its committed lockfile. Its graph and
  the standalone sora2-common graph both audit with zero vulnerabilities and
  zero warnings, without advisory exceptions.

The isolated upstream ark test harness restores test-only ark-test-curves
dependencies omitted/commented out in published manifests. The 0.4.2 test
fixture also needs an explicit `pairing::{Bls12_381, Config, G1Prepared,
G2Prepared}` re-export to resolve its existing ambiguous glob imports on the
current compiler. Those fixture changes exist only under
`/tmp/sora2-macro-maintenance-tests`; production crates receive no such change.
Test compilation uses the persistent `/tmp/sora2-macro-maintenance-target` lane.
