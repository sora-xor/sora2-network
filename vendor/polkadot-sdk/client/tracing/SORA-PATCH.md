# SORA Nexus tracing dependency migration

Upstream: [`polkadot-stable2609`](https://github.com/paritytech/polkadot-sdk/tree/7a8daa68e18718c18823c1109513fdc1c8aa71fe/substrate/client/tracing),
commit `7a8daa68e18718c18823c1109513fdc1c8aa71fe`,
package `sc-tracing` 48.0.0. Original licenses and notices are retained.
The source includes upstream's block tracing changes. The local `contains_key`
cleanup preserves the field-presence check, including stored false values.
The separate `sc-tracing-proc-macro` remains on the same upstream SDK tag.

The manifest materializes upstream package metadata, lint policy, dependency
settings and features. The reviewed security floor for `tracing-subscriber`
remains **0.3.23**, replacing upstream's exact vulnerable `=0.3.19` pin in both
normal and development dependencies. The Cargo lock fixes the resolved release.
This addresses [RUSTSEC-2025-0055](https://rustsec.org/advisories/RUSTSEC-2025-0055.html),
fixed in 0.3.20 and newer; no security advisory is ignored.

The retained local `tests/ansi_escape.rs` regression verifies that logging an
untrusted terminal-control payload preserves its text without emitting a raw
escape byte. Logger-generated formatting is independent of untrusted event input.
No sanitization has been disabled for presentation compatibility.
