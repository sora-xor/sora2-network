# SORA Nexus deterministic stack metering

`src/runtime_blob/stack_metering.rs` adapts the policy of wasm-instrument 0.4.0
to maintained wasmparser/wasm-encoder 0.262.0. The retired parity-wasm parser,
module representation, and serializer are absent from production dependencies.
The upstream algorithm is dual licensed MIT/Apache-2.0; the original notices
are retained in `LICENSE-stack-metering-MIT` and `LICENSE-stack-metering-APACHE`.

Compatibility is more specific than a generic maximum-stack estimator:

- A defined function costs its declared locals plus historical operand-stack
  maximum, whose initial activation charge is two. Parameters are not charged
  again. Host imports cost zero.
- The historical handling of unreachable control flow, `else`, `select`, and
  branch-table selectors is preserved, including estimator quirks that affect
  deterministic trap boundaries. Numeric instructions include floating point.
- Direct calls use the same increment/check/call/decrement instruction sequence
  and unsigned comparison. Exported functions, table entries, and start functions
  use thunks ordered by original function index. Identical function signatures
  reuse the first original type index.
- Custom metadata stays opaque. The old thunk builder's ordering is preserved:
  custom sections move to the end in reverse input order, including duplicates.
- Imported globals are rejected explicitly. The old implementation incorrectly
  used a defined-global index without accounting for imports, potentially writing
  the wrong counter. Exception tags and unsupported operators/types also fail
  explicitly rather than being assigned an invented stack cost.

Six permanent tests include fifteen small historical input/output module pairs,
an unsigned maximum-limit fixture, imported-global/exception-tag rejection,
opaque malformed debug payloads and duplicate custom names, invalid call indices,
local-count overflow, and the counter's position after existing globals. Expected
modules were generated using unmodified wasm-instrument 0.4.0; the replacement
does not generate its own expected results.

The separate reference harness compared fifteen modules at seventeen stack limits:
all 255 complete transformed modules were byte-identical. All 255 corresponding
pairs were instantiated in Node WebAssembly and produced identical repeated-call
results and traps, including recursion, indirect calls, start functions, and
floating point. Execution transcript SHA-256:
`4bb12f973655d147b992222b48a2aa118c8cb3af05e8b4c11709993e629a27f7`.

The archived 4.8.12 runtime was separately decompressed and checked read-only;
it is historical evidence, not a new release build. Its 13,542,031-byte input
produced byte-identical 16,469,186-byte instrumented modules. Input SHA-256:
`22248d39e04f50c93ce185dd97a1bf59edf6bf6a2af88ad0d01853e9bb44f040`.
Reference wasm-instrument 0.4.0 archive SHA-256:
`2a47ecb37b9734d1085eaa5ae1a81e60801fd8c28d4cabdd8aedb982021918bc`.

The newly built current-source runtime was checked independently on 2026-10-24.
Its 15,242,233-byte final input also produced byte-identical 18,064,352-byte outputs.
Input SHA-256: `10df94e3ddc2f1a87cd1f8ff98dea259fe4002c48d2959ad8d904713b389f4e3`.
Both instrumented outputs SHA-256:
`212e56ae630194e034cb67b938de11d20f4832fd5fbc070e0d7c35a98e570cc9`.

Optional reproduction from the repository root:

```sh
python3 vendor/polkadot-sdk/client/executor/common/verification/compare_stack_metering.py \
  --workspace /tmp/sora2-stack-metering-probe \
  --target-dir /tmp/sora2-rustcrypto-migration-target \
  --runtime-wasm target/debug/wbuild/framenode-runtime/framenode_runtime.compact.wasm
```

The runtime argument is optional and expects uncompressed Wasm. Only this
explicitly invoked external workspace resolves the retired reference library;
production and normal test graphs use maintained dependencies exclusively.
