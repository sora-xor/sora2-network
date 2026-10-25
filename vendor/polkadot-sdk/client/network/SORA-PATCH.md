# SORA Nexus network cryptography migration

Source: paritytech/polkadot-sdk `polkadot-stable2609`, commit
`7a8daa68e18718c18823c1109513fdc1c8aa71fe`, `substrate/client/network`,
actual package version **0.60.0**. Upstream sources, fixtures, benches and license
notices are retained. Nested SDK crates remain dependencies at the same tag.
Workspace metadata, dependency defaults/features and lint policy are materialized.

The new SDK's WebRTC certificate code requests P-256 0.13, reintroducing the
vulnerable crypto-bigint 0.5 branch. Migrate it to actual **p256 0.14.0**, using
`ToSec1Point`/`Sec1Point` and the referenced `Odd<U256>` group order. Key rejection
sampling, HMAC domain separation, serial-number and X.509 encodings are unchanged.
Existing upstream RFC6979, golden-certificate, certhash and verification fixtures
are retained rather than regenerated. The authoritative lock must resolve
crypto-bigint 0.7.6 or newer for RUSTSEC-2026-0387.

Upstream's signing test explicitly permits RFC6979 nonce reduction for SHA-256
prehashes at or above the P-256 order when moving to ECDSA 0.17. That rare case
can produce a different certificate hash; it does not change the underlying node
identity or certificate verification. WebRTC deterministic certificate derivation
is newly introduced by stable2609, and no earlier SORA Nexus customization is
replaced. This note does not claim a new constant-time proof of upstream arithmetic.

No old arithmetic implementation is copied and no security advisory is ignored.

The discovery bucket limit uses `u64::MAX` in place of the deprecated
`std::u64::MAX` constant, preserving the same value.
