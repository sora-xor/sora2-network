# SORA Nexus Subxt signer security migration

Source: crates.io `subxt-signer` **0.50.3**, as required by Polkadot SDK stable2609.
Published archive SHA-256:
`674031b5bed3c7157a64f44133c38b45fb6bf2cd5cc6b2d6cfd839ea13a82ee2`.
The source and license are the actual upstream release, with its new interfaces.

The previously reviewed **bip32 0.6.0** migration remains necessary: upstream still
requests 0.5.2. The borrowed seed API is adapted in both implementation and upstream
test code. Wallet derivation paths, key bytes and signature policy are unchanged.
The retained `rustcrypto_migration` integration test is explicitly registered
because the published package disables automatic test discovery. It verifies fixed
public development wallets, signatures, tampering, seed lengths and paths.

This resolves the old k256/elliptic-curve/crypto-bigint branch using actual maintained
upstream releases. Keep the authoritative lock at crypto-bigint 0.7.6 or newer for
[RUSTSEC-2026-0387](https://rustsec.org/advisories/RUSTSEC-2026-0387.html).
No advisory is ignored or package version relabeled. The retired
`no_mangle_generic_items` compiler lint is removed; the compiler already enforces
the restriction as a hard error. The published development Cargo.lock is omitted
because this library is consumed under the application lock.
