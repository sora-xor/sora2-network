# SORA Nexus native dependency migration

Based on the verified PyPI source distribution `pydantic-core==2.50.1`.

Upstream archive: https://files.pythonhosted.org/packages/a6/24/af4ec4be49fbc810f35b0bdcacc3d433b56bbfa469fd3bfd4ae116cd9bf1/pydantic_core-2.50.1.tar.gz

SHA-256: `e50d7b94baac6c7d09927fa5ca5800a0c7ee5015c7fcff65beb3a1931b5a6e09`.

Migrate the regex cache consumer from LRU 0.18.5 to 0.19.0, which fixes the missing Sync bounds on sendable shared iterators. Keep the exact pydantic-core package version required by Pydantic. Regenerate the lockfile with PyO3 0.29.3 and compatible current dependencies. The existing NonZeroUsize cache constructor and locked regex cache require no adapter changes.

Retain original Python and Cargo package names and versions: this is a local source patch, not a security version relabeling. Each package is an independent Cargo workspace with a committed Cargo.lock. Pin the PEP 517 build backend to maturin 1.15.0 and require locked Cargo builds. The containing native/rust-toolchain.toml pins Rust 1.99.1.

The regenerated lockfile reports zero RustSec vulnerabilities and zero warnings against the checked advisory database.

Omit generated PKG-INFO so Maturin generates metadata from the patched project. Explicitly include Cargo.lock and patch provenance in source packages.

The supported installer also supplies `MATURIN_PEP517_ARGS=--locked` before metadata generation. A stale-lock negative test confirms this rejects resolution changes without modifying Cargo.lock; pyproject locked=true alone is insufficient in Maturin 1.15.0 because its initial metadata call precedes project build-option loading.

Validation: the rebuilt CPython 3.12 and 3.13 wheels each pass all 110 upstream string validator tests, exercising the regex-cache consumer.

Keep package-local Cargo linker configuration as tracked source (explicit native/.gitignore exceptions); normalize legacy .cargo/config to config.toml. Omit upstream CI/release workflows and obsolete .dockerignore from this local source distribution.
