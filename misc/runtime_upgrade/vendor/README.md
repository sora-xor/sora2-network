# Runtime-upgrade client snapshot

This directory contains the 21 Python source files from the published
[`substrate-interface` 1.8.1](https://pypi.org/project/substrate-interface/1.8.1/)
wheel. [substrate-interface-source.json](substrate-interface-source.json) records
the wheel URL, SHA-256 and original source-file hashes. The upstream Apache 2.0
license is retained in [LICENSE.substrate-interface](LICENSE.substrate-interface).
The upstream project is [JAMdotTech/py-polkadot-sdk](https://github.com/JAMdotTech/py-polkadot-sdk).

The upstream wheel requires Python `ecdsa`, whose
[CVE-2024-23342](https://github.com/advisories/GHSA-wj6h-64fc-37mp) has no fixed
release. This snapshot removes that dependency by using the maintained
[`coincurve`](https://github.com/ofek/coincurve) libsecp256k1 bindings. It is loaded
under `vendor.substrateinterface`, so an installed upstream client cannot silently
replace it. No substitute package version or advisory exclusion is used.

The source changes are limited to five files, each marked with a modification
notice:

- `utils/ecdsa_helpers.py`: derive public keys and add BIP32 private-key tweaks
  through libsecp256k1; require `CoinCurveECCBackend` for ECDSA signatures and
  recovery. Invalid BIP32 children retry the next index, failing at the hardened
  boundary or index exhaustion. Invalid masters and malformed derivation paths
  fail explicitly. BIP39 text receives the specification's NFKD normalization.
- `keypair.py`: require `CoinCurveECCBackend` for ECDSA address derivation.
- `storage.py` and `contracts.py`: use relative imports within the snapshot.
- `transport/smoldot.py`: reject light-client transport without importing a native
  backend. The governance CLI uses HTTP/WebSocket RPC. Python `smoldot-light`
  0.1.0 bundles vulnerable Rust dependencies that pip-audit cannot detect, so
  this unused package is removed. [Native-dependency evidence](native-dependency-review.json)
  records the source archive and confirmed binary version strings.

RPC handling, runtime metadata decoding, SCALE calls, extrinsic encoding and
SR25519/ED25519 behavior otherwise retain the upstream sources. The complete
active dependency set is pinned in `../requirements.txt`, including
`coincurve==21.0.0`. Four native distributions use local source builds in
[native/](native/README.md): the three Substrate cryptographic bindings and
`pydantic-core`. Their lockfiles and build backend are pinned separately.
The original `substrate-interface`, `ecdsa` and `smoldot-light` distributions
must not be installed in the tool's environment.

The offline tests cover all 17 nodes in the four official
[BIP32 derivation vectors](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki),
including leading-zero cases; invalid master/child/path handling; the BIP39
`TREZOR` vector and Unicode normalization; upstream SR25519/ED25519/ECDSA addresses
and signatures; and byte-identical preimage, council and fast-track calls,
signing payloads and extrinsics against archived production metadata. Fixed
extrinsic signatures in the fixtures are synthetic encoding inputs, never valid
transactions. The Sudo helper is checked with a mock, since production metadata
does not expose Sudo. Tests make no network requests.

For the next update, download the new upstream wheel from PyPI and verify its
published SHA-256 before comparing its Python sources against this manifest.
Review and reapply only the five documented modifications, refresh the exact
dependency lock in a new environment, and run the tests and `pip-audit --strict`
from the parent README. Review changes to upstream cryptography and wire formats
before replacing any compatibility fixture. Keep light-client mode disabled
unless its native dependency closure is migrated and audited as well. Remove
this snapshot once an upstream release provides the required safe cryptographic
backend and optional dependency handling.
