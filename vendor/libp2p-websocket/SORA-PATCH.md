# SORA Nexus WSS trust-store security backport

Upstream crate: `libp2p-websocket 0.44.0`, from
https://crates.io/crates/libp2p-websocket/0.44.0  
Crate archive SHA-256:
`888b2ff2e5d8dcef97283daab35ad1043d18952b65e05279eecbe02af4c6e347`  
Upstream repository: https://github.com/libp2p/rust-libp2p  
Commit recorded in `.cargo_vcs_info.json`:
`4725f46f0d0e14f3ef4e88edc085729c1312dff6`  
Source directory: `transports/websocket`

The published crate archive was verified against its crates.io checksum before
copying. MIT notices remain in the source files. `Cargo.toml.orig` and the
changelog retain the unmodified published contents; the normalized `Cargo.toml`
is the manifest used for this local patch.

The production changes are limited to raising `webpki-roots` from `0.25` to
`1.0.11` and adapting `client_root_store()` to clone the new shared
`rustls-pki-types::TrustAnchor` values. Transport behavior and TLS verification
are otherwise unchanged. The lockfile pins the selected roots release.

[RUSTSEC-2026-0381 / CVE-2026-111943](https://rustsec.org/advisories/RUSTSEC-2026-0381.html)
requires roots `>=1.0.10`, which remove `emSign ECC Root CA - G3`. Version
`1.0.11` was the latest stable registry release checked on 2026-10-24. The old
root was used when dialing WSS endpoints through this crate's default client
TLS configuration.

The local unit test in `tls::trust_store_tests` checks that the affected root's
public key is absent from the actual client store while the legitimate ISRG
Root X1 remains. It performs no network access. Its public-key fixture was
extracted from the `emSign ECC Root CA - G3` trust anchor in the published
`webpki-roots 0.25.4` source and is retained only as denylist regression data.

The development dependency rcgen is migrated from 0.11.3 to 0.14.10 to remove ring 0.16 (RUSTSEC-2025-0009). The TLS documentation example now obtains the private key and certificate DER from rcgen's CertifiedKey. Production WebSocket APIs remain unchanged.

Both WebSocket doctests pass with rcgen 0.14.10, including the TLS transport example.

Retire its development-only async-std runtime: transport doctests now use Tokio, while generic sink tests use the futures executor and an in-memory asynchronous writer. Production transport behavior is unchanged. The unrelated unused eth-bridge development dependency was also removed.

Validation: eight library tests and both transport doctests pass with the maintained development runtime.

The 2026-10-25 full Clippy gate replaces five equivalent
`io::Error::new(io::ErrorKind::Other, error)` conversions with
`io::Error::other`, preserving the wrapped errors and their kind.

The retained quick-sink error-propagation test uses the same `Error::other`
constructor cleanup; its error kind, message, and assertions are unchanged.
