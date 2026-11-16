# SORA Nexus RustCrypto migration

Upstream: `sp-core` from `paritytech/polkadot-sdk` tag `polkadot-stable2609`,
commit `7a8daa68e18718c18823c1109513fdc1c8aa71fe`.
Sources and license notices are retained. Workspace dependencies and lints are
made explicit; internal SDK dependencies retain that tag.

Upgrade k256 to the actual stable 0.14.0 release, using elliptic-curve 0.14 and
crypto-bigint 0.7.10. sp-core explicitly requires crypto-bigint >=0.7.10, preserving
the security fixes from
[RUSTSEC-2026-0387](https://rustsec.org/advisories/RUSTSEC-2026-0387.html)
and the subsequent arithmetic corrections. Upstream yanked 0.7.6 through 0.7.8
when [0.7.9 corrected the `NonZero<T>` multiplication implementation](https://github.com/RustCrypto/crypto-bigint/releases/tag/v0.7.9).
[0.7.10](https://github.com/RustCrypto/crypto-bigint/releases/tag/v0.7.10)
also retains the inversion/truncation and modular multiplication boundary fixes
from 0.7.7. The published 0.7.10 archive SHA-256 is
`9c035cb64cba03dad2fb921f3e218785e8cf9a41790da5eb3a1400f62fa2be11`,
with upstream commit `55af01b1957b1f90d718bf697ea9a6db7287151c`.
No vulnerable crate is relabeled and no advisory is ignored.

Use SEC1's renamed point encoding API and k256's slice parsing and infallible
recoverable signing API. Signature bytes, recovery IDs and public key encodings
remain unchanged. sp-core includes a fixed recoverable signature from unmodified
k256 0.13.4, also checked against its native secp256k1 backend. Validate native and
no-std configurations from the root workspace.

Validation also retains the SDK rustfmt configuration with `wrap_comments = false`.
The current formatter changes whitespace only in the two `union_of.rs` files
under frame-support; no runtime logic changes accompany those formatting edits.

Retire libsecp256k1 in the deprecated historical verification path by depending
on the new `sora-secp256k1-compat` adapter, which performs all cryptography through
maintained k256 while preserving the old overflowing-scalar behavior. Its README
records fixed fixtures and 14,094 differential compatibility checks. Replace the
unmaintained `paste` macro dependency with maintained `pastey` 0.2.3 using the
existing dependency alias; macro expansion call sites are unchanged.

## Stable 2609 rebase

Rebased on the exact upstream release above, retaining the previously reviewed
local source and dependency changes. `SORA-2609.patch` records only local Rust
source differences from this release; upstream source changes are included.
`SORA-2609-manifest-overrides.json` records the deliberate manifest policy
differences after workspace inheritance and SDK Git pins are materialized.
Validation is recorded in the root migration report, not inferred from this
source provenance.
