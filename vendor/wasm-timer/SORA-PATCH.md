# SORA Nexus maintenance migration

Original consumer: `wasm-timer 0.2.5`, crates.io archive SHA-256 `be0ecb0db480561e9a7642b5d3e4187c128914e58aa84330b9493e3eb68c5e7f`. The package version identifies the original source; dependencies are actually replaced rather than relabeled or exempted from audit.

Migrate the WebAssembly timer registry mutex to parking_lot 0.12.5. This removes parking_lot 0.11 / parking_lot_core 0.8 and their abandoned instant dependency. Scope the dependency to the WebAssembly target where it is used. Native timing continues to use std::time; update the test executor to maintained smol 2.0.2. Timer APIs and scheduling semantics are unchanged.

Validation: all-feature native compilation and all 13 upstream timer tests pass.

The 2026-10-25 compiler-warning cleanup declares the existing optional
`assert_timer_heap_consistent` cfg to Cargo check-cfg without enabling or disabling
it. Explicitly ignoring Copy return values replaces no-op drops; the calls to
`into_usize()` (which relinquishes ownership) and `poll()` still execute exactly
once. No timing, ownership, or scheduling policy changes.
