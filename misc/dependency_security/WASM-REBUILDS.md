# Canonical Wasm builds for SORA Nexus

Both reviewed npm bundles are built on **Linux x86-64**, using Rust's
`x86_64-unknown-linux-gnu` host toolchain and `wasm32-unknown-unknown` target.
The dependency-security CI job uses this same host. Native tests and the
JavaScript compatibility fixtures may run on other supported hosts.

This distinction matters: Cargo propagates build/proc-macro dependency metadata
into target symbol hashes. A macOS build can therefore expose different generated
closure symbols even with the same Rust sources and target. We preserve the
actual generated names and compare complete assets; we do not normalize symbols
or weaken byte-exact verification. Rust source paths are also mapped to the
canonical `/rustc/<commit>` prefix, so installing `rust-src` does not change the
bundle.

The direct build scripts reject unsupported hosts with a container instruction.
The checked-in container recipe pins:

- Ubuntu 24.04 image digest and Linux amd64 platform;
- rustup 1.29.2 and nightly-2026-10-23, rustc commit
  `7189d39d9ce936881b8132c6addeaf3e295cd455`;
- Node 24.22.1 and wasm-bindgen CLI 0.2.129, with archive SHA-256 checks;
- @swc/core 1.16.20 and fflate 0.8.2;
- release optimization `s`, thin LTO, one codegen unit, aborting panics and
  repository/Cargo/sysroot source-path remapping.

With Docker available, run from any checkout:

```sh
misc/dependency_security/rebuild_wasm_in_container.sh --check
```

The default check mounts the repository read-only, reuses the named build volume
`sora-reviewed-wasm-canonical`, and compares all 14 executor and 18 crypto assets.
It leaves the selected Docker context unchanged. Set `SORA_WASM_DOCKER_CONTEXT`
to use an explicit existing context, and optionally `SORA_WASM_BUILD_VOLUME` to
select another persistent build lane. The helper does not start or stop VMs.

After an intentional source or dependency change, regenerate both packages:

```sh
misc/dependency_security/rebuild_wasm_in_container.sh --write
```

This explicitly mounts the checkout writable and refreshes the generated files
and their provenance. Then rerun `--check`, the crypto fixtures, executor smoke
suite and installed-package integration tests before accepting the new output.

A native Linux x86-64 environment may run each `build-reviewed.mjs` directly,
with the same pinned tools and `--check --build-tools-dir <npm-tools-prefix>`.
The scripts enforce the Rust host/commit and tool versions. Cargo lockfiles remain
locked, and the verifier binds source, manifests, lockfiles, build scripts,
container recipe and generated output hashes. CI audits all eleven active Rust
lockfiles with `cargo audit --deny warnings`.

The permanent result record is
[wasm-rebuild-2026-10-24.json](wasm-rebuild-2026-10-24.json). Earlier macOS
checkout-path comparisons are historical evidence for the previous artifacts;
they do not establish byte identity for the final Linux-produced artifacts.
