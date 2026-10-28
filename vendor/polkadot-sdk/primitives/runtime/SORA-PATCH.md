# SORA Nexus macro maintenance migration

Source: paritytech/polkadot-sdk, tag `polkadot-stable2609`, commit
`7a8daa68e18718c18823c1109513fdc1c8aa71fe`.

The `paste` dependency now names maintained `pastey` 0.2.3 under its
existing source alias. Runtime algorithms, layouts and generated identifiers
are unchanged. Workspace dependencies were materialized for this snapshot.

The current Rust formatter also adjusts the wrapping of one transaction-extension function signature; this is a whitespace-only change.

The 2026-10-25 Clippy cleanup replaces deprecated
`TransactionLongevity::max_value()` with the identical associated `MAX` constant.
Transaction longevity, encoded values, and validation policy are unchanged.

The legacy extrinsic compatibility decoder uses Option::zip for paired length
checks. remaining_len is still queried and its errors propagated even when the
initial length is unavailable; accepted encodings and length checks are unchanged.

## Stable 2609 rebase

Rebased on the exact upstream release above, retaining the previously reviewed
local source and dependency changes. `SORA-2609.patch` records only local Rust
source differences from this release; upstream source changes are included.
`SORA-2609-manifest-overrides.json` records the deliberate manifest policy
differences after workspace inheritance and SDK Git pins are materialized.
Validation is recorded in the root migration report, not inferred from this
source provenance.

The upstream versioned transaction-extension pipeline, preamble memory accounting,
and allocation-aware signature checking are retained. The legacy V4 decoder
keeps its previous length/error behavior; modern signed V4 payloads still hash
only when their encoded length exceeds 256 bytes. Local macro/decode cleanup
does not alter these rules.

The new upstream eager call-byte reservation is bounded by the known remaining
input and decoded-call limit. An untrusted compact length prefix cannot by itself
reserve nearly four GiB; unknown-length/error-reporting Inputs use incremental
allocation as before. `tests/extrinsic_allocation.rs` records actual allocator
requests while rejecting a forged `u32::MAX` prefix on a two-byte payload, for
both known- and unknown-length Inputs. Existing valid wire encodings and final
length validation remain unchanged.

The 2026-10-27 security migration raises only the `zstd` **dev-dependency**
from 0.12.4 to published 0.14.3, with its fixed zstd-safe 8.2.0 /
zstd-sys 2.1.2 dependencies. This dependency is used to encode test proof
fixtures; production no_std runtime code and its compressed-proof decoder are
unchanged. No compression package version is spoofed or advisory suppressed.
