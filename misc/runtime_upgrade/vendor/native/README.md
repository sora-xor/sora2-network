# SORA Nexus Python native dependencies

The runtime-upgrade tooling builds these native extensions from reviewed local source because current Python wheels can embed older Rust dependencies. Python package versions are preserved for compatibility; each package's `SORA-PATCH.md` describes the actual source/dependency migration, and `UPSTREAM.json` records the verified PyPI archive URL and SHA-256.

| Package | Version | Migrated Rust dependencies |
| --- | --- | --- |
| py-bip39-bindings | 0.3.0 | PyO3 0.29.3, rand 0.8.8 |
| py-ed25519-zebra-bindings | 1.3.0 | PyO3 0.29.3, ed25519-zebra 4.2.0 |
| py-sr25519-bindings | 0.2.4 | PyO3 0.29.3, schnorrkel 0.11.5, keccak 0.1.6, rand 0.8.8 |
| pydantic-core | 2.50.1 | LRU 0.19.0, PyO3 0.29.3 |

Each directory is a standalone Cargo workspace with its own committed lockfile. The common toolchain is Rust 1.99.1. Every `pyproject.toml` pins Maturin 1.15.0 and sets `locked = true`. The runtime-upgrade installer additionally passes `MATURIN_PEP517_ARGS=--locked` before metadata generation, which prevents Cargo from changing the graph. Maturin reads its pyproject build options after an initial metadata call, so `locked = true` alone is insufficient; retain the explicit PEP 517 argument. The installer and requirements file select these local packages.

To build one package manually, enter its directory and use the intended interpreter:

```sh
MATURIN_PEP517_ARGS=--locked python -m pip wheel --no-deps --wheel-dir /tmp/sora2-native-wheels .
cargo audit --file Cargo.lock
```

Use CPython 3.11 or newer for the runtime-upgrade tooling. Native wheels are specific to the interpreter and operating system used to compile them. A macOS wheel is not a Linux deployment artifact; Linux CI and the Docker builder compile their own wheels from these same sources and lockfiles.

To update a package, verify the new source archive against PyPI's SHA-256, review the upstream differences, apply the dependency migration, and regenerate only that package's Cargo.lock. Audit the complete Rust graph, build with the pinned toolchain and locked Maturin configuration, and run upstream tests plus the runtime-upgrade signing/wire-compatibility tests. Review native dependency updates independently of Python package audit results: Python version metadata does not identify the Rust libraries embedded in a wheel. Keep licenses and upstream provenance with every source refresh.

Validation of this patch: all four locked release wheels build under Rust 1.99.1 for CPython 3.12.15 and 3.13.17 on macOS arm64. Each interpreter passes 27 upstream cryptographic binding tests and 110 Pydantic string/regex validator tests. All four native Rust lockfiles audit with zero vulnerabilities and zero warnings. A stale-lock PEP 517 negative test confirms that the explicit `--locked` argument rejects the build before changing Cargo.lock.

The pinned Linux arm64 Docker builder also builds all four wheels without changing their Cargo.lock hashes. Its installed wheels pass 149 offline tests: 12 governance/cryptographic compatibility tests, 27 upstream binding tests, and 110 Pydantic validator tests. The complete runtime image contains the freshly generated runtime WASM, runs as UID 1000, and passes CLI help, `pip check`, and a separate installed-path `pip-audit` of all 30 distributions including pip with zero vulnerabilities. It contains no Rust/C build tools. The final image repeats the 39 compatibility/binding tests using its packaged application modules. Image, wheel, binary, lockfile, and WASM hashes are recorded in [the validation record](../native-dependency-review.json).
