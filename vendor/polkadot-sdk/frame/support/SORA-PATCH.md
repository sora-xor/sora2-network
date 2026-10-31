# SORA Nexus stable2609 customization provenance

Upstream: `paritytech/polkadot-sdk`, tag `polkadot-stable2609`, commit
`7a8daa68e18718c18823c1109513fdc1c8aa71fe`, path `substrate/frame/support`.

The actual stable2609 FRAME API removals and additions are adopted. The maintained pastey alias and k256 0.14 migration are retained, along with fixed secp256k1 encodings, Ethereum address slicing, short-circuit tuple predicates and generic CloneNoBound regressions. No removed SDK API is restored merely to bypass application migrations.

`Cargo.toml.orig` contains the exact upstream manifest. `Cargo.toml` materializes
upstream package, dependency and lint inheritance; SDK path dependencies are
pinned to the same release tag. Documented security overrides remain explicit.

[`stable2609-customizations.patch`](stable2609-customizations.patch) reproduces
the current Rust source, local formatter configuration and retained license copy
from this upstream directory. Its byte-for-byte replay was checked in an isolated
copy. [`stable2609-customizations.json`](stable2609-customizations.json) records
the patch hash and every changed source file hash. Earlier patch descriptions
below are historical rationale; their older base tags are not the current source.

## Earlier patch rationale

### SORA Nexus RustCrypto migration

Upstream: `frame-support` from `paritytech/polkadot-sdk` tag `polkadot-stable2603-7`,
commit `cd87d0b95e54c8ffc374334386f3f2b2c7a61f54`.
Sources and license notices are retained. Workspace dependencies and lints are
made explicit; internal SDK dependencies retain that tag.

Upgrade k256 to the actual stable 0.14.0 release, using elliptic-curve 0.14 and
crypto-bigint 0.7.6. sp-core explicitly requires crypto-bigint >=0.7.6 to preserve
the floor fixed by
[RUSTSEC-2026-0387](https://rustsec.org/advisories/RUSTSEC-2026-0387.html).
No vulnerable crate is relabeled and no advisory is ignored.

Use SEC1's renamed point encoding API and k256's slice parsing and infallible
recoverable signing API. Signature bytes, recovery IDs and public key encodings
remain unchanged. sp-core includes a fixed recoverable signature from unmodified
k256 0.13.4, also checked against its native secp256k1 backend. Validate native and
no-std configurations from the root workspace.

Validation also retains the SDK rustfmt configuration with `wrap_comments = false`.
The current formatter changes whitespace only in the two `union_of.rs` files
under frame-support; no runtime logic changes accompany those formatting edits.

The Ethereum-address conversion now passes `&hash[12..]` explicitly to `try_from`;
this resolves type inference ambiguity introduced by the wider dependency graph
without changing the selected 20 address bytes.

The unmaintained `paste` dependency is replaced by maintained `pastey` 0.2.3
under its original source alias; macro inputs and generated identifiers are retained.

The 2026-10-25 Clippy cleanup replaces deprecated integer limit functions with
identical associated constants. Tuple membership and queue-pause checks use
short-circuit OR accumulation, preserving left-to-right evaluation, stopping
predicate calls after the first true result, and false for the empty tuple. The
callback retains its original CloneNoBound derive; the procedural macro’s
generic-field cloning fix retains its absence of Clone bounds on Params/RuntimeCall.
Encoded fields and public behavior are unchanged.
