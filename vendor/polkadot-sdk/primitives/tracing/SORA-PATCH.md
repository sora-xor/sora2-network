# SORA Nexus tracing dependency security backport

Upstream: https://github.com/paritytech/polkadot-sdk.git  
Tag: `polkadot-stable2609`  
Commit: `7a8daa68e18718c18823c1109513fdc1c8aa71fe`  
Source: `substrate/primitives/tracing`

The upstream Rust source files, benchmarks and README are copied without changes.
The upstream license declarations and file notices are retained. For `sc-tracing`,
its separate `sc-tracing-proc-macro` crate remains a dependency from the same SDK
tag rather than another local fork.

`Cargo.toml` materializes the upstream workspace package metadata, dependency
settings, enabled features and lint configuration. SDK workspace path dependencies
use the exact upstream release tag. The only dependency policy change is raising
`tracing-subscriber` from the SDK's exact `=0.3.19` pin to `0.3.23`, including
optional and development requirements. The lockfile fixes the resolved release;
the compatible requirement permits later patched releases.

This resolves [RUSTSEC-2025-0055 / CVE-2025-58160](https://rustsec.org/advisories/RUSTSEC-2025-0055.html),
which is fixed in `tracing-subscriber >=0.3.20`. Version `0.3.23` was the latest
stable registry release checked on 2026-10-24.

ANSI control sequences originating in log event input are intentionally escaped.
Messages that previously embedded terminal formatting can therefore display
escaped control characters instead of applying them. Do not disable the new
sanitization or restore the old exact pin to preserve that presentation behavior.
Logger-generated formatting is separate from untrusted event input.

`tests/ansi_escape.rs` is a local regression test; it is not part of the upstream
source snapshot. It verifies that logging a terminal-control payload preserves
its text without emitting a raw escape byte.

## Stable 2609 rebase

Rebased on the exact upstream release above, retaining the previously reviewed
local source and dependency changes. `SORA-2609.patch` records only local Rust
source differences from this release; upstream source changes are included.
`SORA-2609-manifest-overrides.json` records the deliberate manifest policy
differences after workspace inheritance and SDK Git pins are materialized.
Validation is recorded in the root migration report, not inferred from this
source provenance.
