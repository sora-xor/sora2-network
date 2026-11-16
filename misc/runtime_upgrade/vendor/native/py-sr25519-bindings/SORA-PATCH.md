# SORA Nexus native dependency migration

Based on the verified PyPI source distribution `py-sr25519-bindings==0.2.4`.

Upstream archive: https://files.pythonhosted.org/packages/cc/3c/a371fefdb2bf49978a79ae04fb9b866835ce0b536b1c0946db618f721983/py_sr25519_bindings-0.2.4.tar.gz

SHA-256: `19109c30a0e769ff09656e5cd815115e1913e0a627adb768c91c3c0074f2a863`.

Update PyO3 to 0.29.3, schnorrkel to 0.11.5, keccak to 0.1.6 and rand to 0.8.8 in the rebuilt extension. The upstream wrapper already uses the 0.29 API; no key or signature format changes are required. All 5 upstream Python tests pass on CPython 3.12 and 3.13.

Retain original Python and Cargo package names and versions: this is a local source patch, not a security version relabeling. Each package is an independent Cargo workspace with a committed Cargo.lock. Pin the PEP 517 build backend to maturin 1.15.0 and require locked Cargo builds. The containing native/rust-toolchain.toml pins Rust 1.99.1.

The regenerated lockfile reports zero RustSec vulnerabilities and zero warnings against the checked advisory database.

Omit obsolete upstream release-only build.sh/Dockerfile/uv.lock, generated PKG-INFO, and development requirements that install older published binary wheels. Preserve the Rust wrapper, upstream Python tests, README, and license. Use the containing native/README.md for the supported local build workflow.

The supported installer also supplies `MATURIN_PEP517_ARGS=--locked` before metadata generation. A stale-lock negative test confirms this rejects resolution changes without modifying Cargo.lock; pyproject locked=true alone is insufficient in Maturin 1.15.0 because its initial metadata call precedes project build-option loading.

Keep package-local Cargo linker configuration as tracked source (explicit native/.gitignore exceptions); normalize legacy .cargo/config to config.toml. Omit upstream CI/release workflows and obsolete .dockerignore from this local source distribution.
