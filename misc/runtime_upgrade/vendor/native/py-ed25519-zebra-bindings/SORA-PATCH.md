# SORA Nexus native dependency migration

Based on the verified PyPI source distribution `py-ed25519-zebra-bindings==1.3.0`.

Upstream archive: https://files.pythonhosted.org/packages/c1/0f/18ba2ae4eb2bc84fd05ebd3689d6d33609198707a602cf332e854944735f/py_ed25519_zebra_bindings-1.3.0.tar.gz

SHA-256: `c0c4478ab57d65671b310524978122a81b9921847dba07f005d0d2d218f610b5`.

Migrate PyO3 from 0.26 to the maintained 0.29 API series (locked at 0.29.3), with ed25519-zebra 4.2.0. Existing Bound/IntoPyObject wrapper APIs are compatible; key encoding and signature semantics remain unchanged. All 7 upstream Python tests pass on CPython 3.12 and 3.13.

Retain original Python and Cargo package names and versions: this is a local source patch, not a security version relabeling. Each package is an independent Cargo workspace with a committed Cargo.lock. Pin the PEP 517 build backend to maturin 1.15.0 and require locked Cargo builds. The containing native/rust-toolchain.toml pins Rust 1.99.1.

The regenerated lockfile reports zero RustSec vulnerabilities and zero warnings against the checked advisory database.

Omit obsolete upstream release-only build.sh/Dockerfile/uv.lock, generated PKG-INFO, and development requirements that install older published binary wheels. Preserve the Rust wrapper, upstream Python tests, README, and license. Use the containing native/README.md for the supported local build workflow.

The supported installer also supplies `MATURIN_PEP517_ARGS=--locked` before metadata generation. A stale-lock negative test confirms this rejects resolution changes without modifying Cargo.lock; pyproject locked=true alone is insufficient in Maturin 1.15.0 because its initial metadata call precedes project build-option loading.

Keep package-local Cargo linker configuration as tracked source (explicit native/.gitignore exceptions); normalize legacy .cargo/config to config.toml. Omit upstream CI/release workflows and obsolete .dockerignore from this local source distribution.
