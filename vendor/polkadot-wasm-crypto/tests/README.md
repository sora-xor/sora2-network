# Published Wasm compatibility fixtures

`fixtures/wasm-crypto-7.5.4.json` records synthetic outputs from the published
`@polkadot/wasm-crypto@7.5.4` API and its `@polkadot/wasm-crypto-wasm@7.5.4`
dependency. It contains public test keys only; never fund these keys.
The retired binary and its dependency graph are not included in this repository.

The original npm packages were installed in an isolated temporary directory.
Their lockfile records these archive integrity values:

| npm package | SHA-512 integrity |
| --- | --- |
| `@polkadot/wasm-crypto@7.5.4` | `sha512-1seyClxa7Jd7kQjfnCzTTTfYhTa/KUTDUaD3DMHBk5Q4ZUN1D1unJgX+v1aUeXSPxmzocdZETPJJRZjhVOqg9g==` |
| `@polkadot/wasm-crypto-wasm@7.5.4` | `sha512-PsHgLsVTu43eprwSvUGnxybtOEuHPES6AbApcs7y5ZbM2PiDMzYbAjNul098xJK/CPtrxZ0ePDFnaQBmIJyTFw==` |

The source commit is
[`f55a3e75a2ab5408f6bf947f2ec9b294daef432d`](https://github.com/polkadot-js/wasm/tree/f55a3e75a2ab5408f6bf947f2ec9b294daef432d/packages/wasm-crypto).
Its source archive SHA-256 is
`fef89c4248e7786f047147658d76b9cd0571ce7804f23f15dd098227619b0443`.
The actual original Wasm is 335,277 bytes, SHA-256
`7edb3158a3ec97e5bd72f35476128a1c09273a109887fd445b366ad1ebc21b06`.
The frozen fixture SHA-256 is
`b0392e521c72d5a1a574551bffec1564b15eef7be07de8744d44a407859a3207`.

The [recorded migration evidence](compatibility-evidence.json) binds the frozen
fixture and harness to the final rebuilt Wasm
`7f08ea9227b0d72df0953194994c54a56a34a7135a43690eb6c3f4867a4c316f`
(676,030 bytes), built on the canonical Linux x86_64 host. Node 24.22.1 on
macOS arm64 executes this Linux-built Wasm in both the raw package and fresh
installed facade: all 2,928 differential checks and all 2,619 offline checks pass
with no failures. Build-byte reproducibility is checked separately on the
[canonical Linux build host](../../../misc/dependency_security/WASM-REBUILDS.md).

## Run without the retired dependency graph

From the repository root, the following gate loads only the maintained Wasm
package and inert JSON fixtures. It needs Node 24 or newer and no network access:

```sh
node vendor/polkadot-wasm-crypto/tests/compatibility.cjs \
  --candidate vendor/polkadot-wasm-crypto-wasm \
  --security-regressions true
```

To exercise the public facade after a staged `npm ci`, pass its installed package
directory instead:

```sh
node vendor/polkadot-wasm-crypto/tests/compatibility.cjs \
  --candidate /path/to/stage/validation/node_modules/@polkadot/wasm-crypto \
  --security-regressions true
```

To additionally cross-verify newly generated signatures and proofs with the
original published implementation, add
`--reference /path/to/isolated-original/node_modules/@polkadot/wasm-crypto`.
The harness requires its Wasm SHA-256 to match the provenance above. Do not add
the retired package to the current dependency graph to run this optional check.

The JSON result records actual candidate/reference Wasm hashes, the Node version,
fixture hash, checks performed, function coverage and failures. The offline gate
performs 2,619 assertions; the optional original implementation adds 309 checks,
for 2,928 total. Unexpected exceptions fail immediately. Use
`--collect-failures true` to collect deterministic mismatches before exit 1.

## Coverage and intended differences

The fixtures include 1,098 deterministic vectors, 96 signing/VRF cases and 16
derivation/key sets. All 32 exported cryptographic functions are covered:

- BIP39 generation at every supported strength, Unicode composed/decomposed and
  Japanese passphrases, strict validation versus whitespace-tolerant derivation.
- Ed25519 exact signatures, SR25519 hard/soft derivation and key agreement,
  randomized signatures and VRF proofs verified in both implementation directions.
  Soft-derived secret nonce bytes and randomized proof bytes are intentionally
  checked through their public keys and valid signatures instead of byte equality.
- Secp256k1 exact low-S signatures, public-key conversion and recovery for digest
  values 0, 1, n−1, n, n+1 and all-ones, with 16 secret keys. This caught the
  old C wrapper's raw-digest RFC6979 behavior at and above the curve order.
- SHA, HMAC, Keccak, variable/keyed BLAKE2b, TwoX, PBKDF2 and Scrypt with varied
  input lengths and parameters, including empty and 65,536-byte messages.
- Tampered messages, signatures and VRF transcripts; malformed Ed/SR key and
  signature lengths; malformed secp digest/signature lengths; strict high-S
  recovery rejection and historical trailing-byte VRF compatibility.

The security regression option checks two deliberate input-hardening changes:
SR25519 signing rejects a supplied valid public key belonging to another secret,
and truncated VRF proofs return false instead of panicking. It must not be used
when replaying the original binary as the candidate.

Fixture replay also caught mnemonic validation accepting whitespace that the
published validator rejected. Both that issue and the deterministic secp boundary
mismatch were fixed before recording passing migration evidence.

`--record-reference /path/to/original/package` explicitly regenerates fixtures.
Generation contains randomized mnemonics, SR signatures and VRF proofs, so this
is a reviewed fixture update, not part of normal CI. Preserve this provenance
and inspect any semantic changes before accepting regenerated outputs.

## Nonce retry reference

`reference-nonce.c` independently exercises the first three nonce candidates from
the original C HMAC generator, including its retry transitions. It is an optional
archive probe; normal CI uses the captured values in the private Rust unit test.
The original source lock pins `secp256k1-sys 0.4.2`, whose crate SHA-256 is
`957da2573cde917463ece3570eab4a0b3f19de6f1646cde62e6fd3868f566036`.
Its unmodified `depend/secp256k1/src/hash_impl.h` SHA-256 is
`5638c72220be432175d8401fd0bcea0512d4ba0d5c95e8ad710b69917068b0e9`.

On a little-endian machine, with that verified archive extracted outside the
repository:

```sh
cc -std=c99 -DSECP256K1_LITTLE_ENDIAN \
  -I /path/to/secp256k1-sys-0.4.2/depend/secp256k1/src \
  vendor/polkadot-wasm-crypto/tests/reference-nonce.c \
  -o /tmp/polkadot-reference-nonce
/tmp/polkadot-reference-nonce
```

The synthetic secret 1 and raw digest n produce, in order:

```text
0d9b7698bb66c2fd7edf1e91fe90d1c6247b694bd34cf1322fcf94e074a2c518
aae11b6fdf1da3ce91cba3553671815c23c48bf6f997daabf4bd4ff4474d23b3
54a641076afbce1882fafe98ed01042068f349075c6b384f5e4591693f53fffa
```

Testing successive candidates makes the retry schedule explicit without exposing
nonce injection or weakening production key generation. The original C source
is not a build or runtime dependency of the maintained package.
