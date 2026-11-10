# SORA Nexus maintained Wasm tooling migration

Upstream: [`polkadot-stable2609`](https://github.com/paritytech/polkadot-sdk/tree/7a8daa68e18718c18823c1109513fdc1c8aa71fe/substrate/client/executor/common),
commit `7a8daa68e18718c18823c1109513fdc1c8aa71fe`.
Original licenses and notices are retained. Workspace manifests are materialized
with the same SDK revision and actual upstream package version 0.29.0.

The previously reviewed migration from parity-wasm/wasm-instrument to maintained
wasmparser/wasm-encoder 0.262.0 is retained. RuntimeBlob stores module bytes and
uses re-encoding passes for memory conversion and deterministic stack metering.
Memory import conversion preserves the original export name; the deprecated
legacy heap rewrite API preserves its dynamic bounds and wasm32 saturating static
heap growth. Failed transformations leave the original blob intact. Custom
metadata remains opaque.

The stable2609 executor changes are integrated: WasmModule receives a heap
strategy when creating each instance, WasmInstance supports changing that strategy,
HeapAllocStrategy supports doubling, and PolkaVM validation errors retain their
message. The new `clear_memory_max_limit` transformation uses the maintained
parser. It clears only defined memories' maxima, preserving their initial pages,
other sections and opaque custom data; missing memory is accepted, as upstream.
A regression covers multiple memories, missing memory and idempotence. Wasmtime
now applies limits to each instance, rather than rewriting those limits at compile
time; the matching stable2609 executor implements that behavior.

The stack-metering policy remains the reviewed port from wasm-instrument 0.4.0,
including historical local/operand-stack costs, floating-point support, call
instrumentation and externally reachable thunks. Imported globals are rejected
explicitly. See [STACK-METERING.md](STACK-METERING.md) for original license
attribution and historical differential evidence. Those historical results do not
constitute validation of the new SDK migration.

[RUSTSEC-2022-0061](https://rustsec.org/advisories/RUSTSEC-2022-0061.html)
recommends maintained [wasm-tools](https://github.com/bytecodealliance/wasm-tools).
No old parser is copied or relabeled, and no advisory is suppressed.
