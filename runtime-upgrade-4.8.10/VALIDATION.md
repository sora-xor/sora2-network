# 4.8.10 BABE repair validation

The candidate is `framenode-runtime-4.8.10.compact.compressed.wasm`, 3,055,007 bytes,
SHA-256 `a39de37798885ee253db508c553aca80321974fdd20fa0830bb94220cb9224c3`.
It advances runtime spec 131 to 132, resets impl version to 1, and retains
transaction version 131. The pinned SDK is
`e3737178ec726cffe506c907263aaaa417893fd0`.

## Runtime evidence

- Full native runtime suite: **195 passed, 0 failed, 4 existing ignored tests**.
- Focused migration suite with `try-runtime`: **10 passed, 0 failed**.
- Actual candidate Wasm executed against pinned mainnet state at block
  **27,734,948**. Migration scheduled Plain without rewriting current/next epochs;
  the first boundary announced it once; the second activated it. All three BABE
  APIs agreed afterward. Timestamp application, block finalization, authority and
  randomness promotion, and pending-change consumption were checked.
- Structural metadata comparison preserved existing encodings across 81 pallets,
  405 calls, 442 events, 967 errors, 485 storage entries, 155 constants, and 7 signed
  extensions. Only the runtime-version constant changed. Runtime API versions,
  all 54 host function signatures, imported memory requirements, and exports
  match deployed 4.8.9.
- The original client-side reproduction retained real signatures: 13 sampled
  canonical Plain headers fail under advertised VRF mode and pass with only the
  slot mode corrected; 7 primary controls pass. See the retained investigation
  in `misc/validator-production-2026-09-22/ROOT-CAUSE.md`.

The Wasm rehearsal uses synthetic unsealed headers, mocked signature hosts, and
slot jumps. A jump to an era boundary bypasses election preparation and logs
`SnapshotUnavailable`. It tests migration and BABE runtime behavior; it is not a
complete election or network consensus simulation. The independent real-header
verifier supplies the client-verification evidence. Full commands, dependencies,
script hashes and limitations are in
[`validation/WASM-VALIDATION.md`](validation/WASM-VALIDATION.md).

## Offline node recovery evidence

The PR review follow-up moved the recovery command's `String` to
`sc_cli::Error::Input` conversion into the CLI dispatcher. The default,
`try-runtime`, and extended workspace Clippy checks all pass with `-D warnings`.
The recovery behavior and packaged runtime candidate are unchanged by this
follow-up.

The complete node unit suite passed **26 tests, 0 failed**, including 16 focused
recovery tests. Actual RocksDB and ParityDB fixtures exercised guarded SDK backend
and client initialization, header/finality/trie reads, cache repair through
`AuxStore`, durable backup refusal paths, and subsequent ordinary SDK reopening.
They checked preservation of blocks, state, schema and unrelated auxiliary data.

The fixtures also reject incompatible database schemas, missing required headers,
ambiguous epoch trees, changed authority/randomness payloads, and unexpected
configuration changes. They verify actual SDK epoch selection after repair,
preserve legitimately announced future VRF configuration, and require explicit
backend and apply/backup arguments. Invalid runtime substitutes are stripped only
from the offline client's in-memory chain spec before evidence execution; the
ordinary mainnet spec and genesis remain unchanged.

The final native release build passed without `SKIP_WASM_BUILD`. Six binary CLI
checks passed, including refusal of missing databases without initialization.
The local executable is
`/Users/takemiyamakoto/dev/.sora2-pr1366-target/release/framenode`, macOS arm64,
96,380,960 bytes, SHA-256
`6eac1a97829d5195dc23a13b4218068bbb08d29056be6e23a854cf9d74f4fd6e`.
It is built from the recorded base plus `source.patch`, not the unmodified base
commit shown in its version string. Other operating systems need a build from
that same source; this package does not claim Linux binary validation.

The fresh build also generated runtime Wasm with SHA-256
`6d44dd474d1f511d2612c77133ed4059a733ea6ef66f2f21b2b4f69aa55f9cac`.
It has the same runtime version and metadata as the validated candidate, but
its executable bytes differ. The two builds embed different absolute checkout
paths in their Wasm, so this worktree build does not establish byte-for-byte
reproducibility of the candidate. The packaged `a39de377...` Wasm remains the
only artifact covered by the exact-Wasm state rehearsal and unsigned-call
checks. The macOS executable does not contain either compressed Wasm blob, so
the CLI checks do not validate the runtime candidate.

These fixtures are local tests. No affected validator's real database or signing
keys have been supplied or accessed. Operational success still requires checking
canonical imports, finality and authorship on each recovered validator.

## Unsigned package

Read-only preflight at finalized block **27,741,256**, hash
`0xa3a31ebb005cf8f0be84c1217fc56a8cb6d0fd947bff3937ccbfc41f2bb7aa20`, confirmed:

- Expected mainnet genesis and exact deployed spec-131 Wasm.
- Current and next VRF `(1,4)`, no pending configuration, no repair marker.
- Canonical Plain headers in the last 20 finalized ancestors and no SecondaryVRF
  headers in that sample.

The unsigned `system.setCode` call is **3,055,013 bytes**, proposal hash
`0xcb822d1ad3293838ba0ecfb4eb9ded109c040693f626e406f07b184ecc5e6713`.
The offline package verifier independently reconstructs its SCALE encoding from
deployed metadata, verifies the embedded Wasm byte-for-byte, and checks the
preimage call and both successful exact-binary validation reports.

Package preparation read public chain state only. It did not sign or submit a
transaction, read private keys, access an affected validator database, restart a
node, or enact the runtime. Repeat the live preflight before governance use.

## Build and source

Source provenance and a patch against the recorded base commit are included in
`validation/source-provenance.json` and `source.patch`. The candidate runtime
build reused the existing `target` directory with the repository's LLVM wrapper:

```sh
env -u SKIP_WASM_BUILD \
  CC_wasm32_unknown_unknown=/opt/homebrew/opt/llvm@21/bin/clang \
  AR_wasm32_unknown_unknown=/opt/homebrew/opt/llvm@21/bin/llvm-ar \
  LIBCLANG_PATH=/opt/homebrew/opt/llvm@21/lib \
  LLVM_CONFIG_PATH=/opt/homebrew/opt/llvm@21/bin/llvm-config \
  scripts/with_llvm_env.sh cargo build --release --offline \
  -p framenode-runtime --features build-wasm-binary
```

The follow-up node release build used a separate fixed target directory,
`/Users/takemiyamakoto/dev/.sora2-pr1366-target`, and set
`WASM_BUILD_WORKSPACE_HINT` to this worktree to keep the generated Wasm build
locked to the workspace dependencies.

Beyond the three local package version bumps, Cargo.lock changes only add the
node's links to already-pinned database/epoch crates. No third-party dependency
versions changed. Subsequent node builds use `--locked`. The runtime Wasm is
portable; a locally built macOS node binary is not a Linux deployment artifact.
