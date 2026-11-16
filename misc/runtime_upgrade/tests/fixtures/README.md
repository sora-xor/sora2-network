# Offline compatibility fixtures

`bip32.json` copies the four derivation vectors from
[BIP32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki), including
their seeds, extended private keys and extended public keys.

`keypairs.json` was generated with the unmodified PyPI `substrate-interface`
1.8.1 wheel on 2026-10-24, using only public development keys and known example
mnemonics. It records addresses, public keys and signatures for the SR25519,
ED25519 and Ethereum ECDSA APIs. SR25519 signatures are randomized, so tests
verify the old signatures and round-trip new ones instead of comparing bytes.
ED25519 and ECDSA signatures are deterministic and must match exactly.

`governance.json` was generated with that same upstream wheel and the immutable
archived metadata file identified by its path and SHA-256. It records preimage,
council and technical-committee call bytes, signing payloads and extrinsic
encoding. The nonce is zero, the genesis/block hashes are synthetic zeros, the
runtime version is 132, the transaction version is a synthetic 1, and the
signature is 64 bytes of `0x01`. These extrinsics are deliberately invalid and
were never submitted. Tests also sign the payloads with the public Alice
development key and verify them offline.
