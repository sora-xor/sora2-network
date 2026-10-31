# Reviewed Polkadot crypto Wasm for SORA Nexus

The source is `packages/wasm-crypto` from [polkadot-js/wasm commit
f55a3e75a2ab5408f6bf947f2ec9b294daef432d](https://github.com/polkadot-js/wasm/tree/f55a3e75a2ab5408f6bf947f2ec9b294daef432d/packages/wasm-crypto),
the source commit recorded by npm `@polkadot/wasm-crypto-wasm@7.5.4`.
The exact codeload archive SHA-256 is
`fef89c4248e7786f047147658d76b9cd0571ce7804f23f15dd098227619b0443`.
Upstream Apache-2.0 copyright notices and LICENSE are retained.

The published binary's native lock had five vulnerability matches, six unsound
advisories, three unmaintained packages, and one yanked package against the
2026-10-24 RustSec database. Its npm metadata did not expose that Rust closure.
This package rebuilds from maintained dependencies; it does not suppress these
findings or reuse the old binary. The independent standalone Cargo.lock is part
of the strict audit and rebuild gates.

## Source changes

- Ed25519 uses ed25519-dalek 2.2 and curve25519-dalek 4.1.3. Signing still derives
  its public key from the secret seed, ignoring the caller's supplied public key.
  Verification uses the standard maintained verifier without legacy scalar
  compatibility features.
- Sr25519 and VRF use schnorrkel 0.11.5, curve25519-dalek 4.1.3 and merlin 3.0.
  Half-Ed25519 96-byte keypair encoding, Ed25519 expansion, Substrate signing
  context, derivation chain codes and VRF transcript labels are retained.
  Signing rejects a supplied public key that does not match its secret. Truncated
  VRF proofs return false instead of panicking; historical trailing proof bytes
  remain ignored.
- Secp256k1 uses the maintained k256 0.14-backed
  [compatibility adapter](../sora-secp256k1-compat/README.md). This API continues to
  reject high-S recovery and accepts only 33/65-byte SEC1 public-key encodings,
  including correctly tagged hybrid keys. Signatures remain compact low-S bytes
  followed by the recovery identifier. A private HMAC-SHA256 nonce adapter
  preserves the old C wrapper's raw-digest RFC6979 input even when the digest is
  at or above the curve order; k256 still performs all signing arithmetic and
  low-S normalization. Nonce/key state and hash state are zeroized, and the
  rejection retry follows RFC6979 section 3.2.h. This wrapper-specific behavior
  does not change the shared runtime adapter.
- Mnemonics use bip39 2.2.2. Substrate mini-secrets still use entropy as the PBKDF2
  input and the raw password bytes; BTC/ETH seeds retain BIP39 normalization.
  PBKDF2/HMAC/SHA2 and Scrypt use maintained RustCrypto 0.13/0.13/0.11/0.12 APIs.
  Keyed and variable-length BLAKE2b uses blake2b_simd 1.0.4. Keccak and TwoX
  preserve their existing output encodings.
- The unmaintained wee_alloc is removed; Wasm uses the standard Rust allocator.
  JavaScript cryptographic randomness is explicitly enabled for both supported
  getrandom generations required by these upstream libraries. There is no
  predictable entropy fallback.
- wasm-bindgen is pinned to 0.2.129. The generated JS calls its typed bindings,
  replacing the former handwritten pointer ABI. All 32 upstream Rust exports
  remain available. The supported Node >=24 environment requires WebAssembly;
  the retired asm.js binary is not an operational fallback.

## Build and validation

On Linux x86-64, use the pinned `nightly-2026-10-23` toolchain with `wasm32-unknown-unknown`,
wasm-bindgen-cli **0.2.129** and a build-tools directory containing
`@swc/core` **1.16.20**. The script verifies tool versions, uses `cargo build
--locked`, and remaps repository and Cargo paths. Reuse an existing target lane.

```sh
cargo test --manifest-path vendor/polkadot-wasm-crypto/Cargo.toml --locked --lib
cargo audit --file vendor/polkadot-wasm-crypto/Cargo.lock --deny warnings
node vendor/polkadot-wasm-crypto/scripts/build-reviewed.mjs --build-tools-dir /path/to/tools
python3 vendor/polkadot-wasm-crypto/scripts/verify-artifacts.py
node vendor/polkadot-wasm-crypto/scripts/build-reviewed.mjs --build-tools-dir /path/to/tools --check
```

Set `WASM_BINDGEN` when the pinned CLI is not on PATH. `--check` rebuilds into a
fresh temporary packaging directory and compares every generated asset without
modifying committed outputs. Generated output is in
`../polkadot-wasm-crypto-wasm`; its `build-provenance.json` binds all Rust source,
manifest, lockfile, shared-adapter source, build-script, toolchain and npm-manifest
hashes to the generated ESM/CJS bindings and Wasm bytes. This binding makes stale
binaries fail CI even if their JavaScript package metadata appears current.

All 42 upstream Rust tests and eight added security/compatibility regressions pass. The added
checks cover public-key binding, malformed/canonical Ed25519 signatures,
truncated VRF proofs, VRF transcript/trailing-byte compatibility, strict mnemonic validation spacing,
the three raw-digest secp256k1 boundary signatures, and three successive nonce
outputs captured independently from the original C backend (secp256k1-sys 0.4.2). The final
standalone native graph currently contains 115 packages and has zero RustSec
vulnerabilities and zero warnings with `--deny warnings` and no ignored IDs.
Cross-version Wasm differential evidence is maintained in the adjacent test
harness and fixtures; random signatures are cross-verified rather than compared
as if their nonce bytes were deterministic.

The standalone workspace contains a small `adapter/Cargo.toml` that points its
library path at the single reviewed `../sora-secp256k1-compat/src/lib.rs` source.
This avoids Cargo hashing an absolute external package path into symbols while
keeping one implementation. The manifest and shared source are both bound into
build provenance. The canonical build pins the effective release profile
(`opt-level=s`, thin LTO, one codegen unit, panic abort) and remaps repository,
Cargo-home and Rust-sysroot paths. The shared adapter's repository license is
included in distributed packages as `SORA-LICENSE`.

Final Wasm is 676,030 bytes with SHA-256
`7f08ea9227b0d72df0953194994c54a56a34a7135a43690eb6c3f4867a4c316f`.
The final Linux checkout-path/cache/target comparison is recorded in
`../../misc/dependency_security/wasm-rebuild-2026-10-24.json`.
The 50 native tests pass. On Node 24.22.1, 2,928 checks against the published
reference bundle and 2,619 offline fixture checks pass, covering all 32 exported
functions. This includes 1,098 deterministic vectors, 96 signature/VRF cases,
and mutation, length, context-binding and canonicality regressions. See
[the independent fixture evidence](tests/README.md) for provenance and commands.

Canonical generated assets now require the Linux x86-64 GNU Rust host. Other
hosts should use [the pinned container rebuild helper](../../misc/dependency_security/WASM-REBUILDS.md).
The scripts enforce this host and the exact compiler commit, and map Rust sources
to `/rustc/<commit>` independently of whether `rust-src` is installed. Final
Linux checkout-path reproduction is recorded in the permanent rebuild evidence;
earlier macOS artifact comparisons are historical only.
