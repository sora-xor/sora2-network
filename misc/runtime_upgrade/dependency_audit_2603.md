# Dependency Audit After Polkadot Stable 2603

`cargo audit` still reports vulnerable transitive crates after the upgrade to
`polkadot-stable2603-3`. These are not currently fixable with a normal
`cargo update` because the vulnerable versions are pinned by upstream SDK or
forked dependencies.

Last checked with:

```sh
cargo audit --json --file Cargo.lock --db /tmp/sora2-cargo-audit-db --stale --no-fetch
```

Result: 25 remaining RustSec vulnerability matches.

## Remaining Vulnerability Owners

- `tracing-subscriber 0.3.19`
  - Pulled by `sp-tracing 19.0.0` and `sc-tracing 45.0.0` from
    `polkadot-stable2603-3`.
  - `cargo update -p tracing-subscriber --precise 0.3.20` fails because
    `sp-tracing` requires `=0.3.19`.
- `wasmtime 35.0.0`
  - Pulled by `sc-executor-wasmtime 0.44.0` and `sp-wasm-interface 24.0.0`
    from `polkadot-stable2603-3`.
- `ring 0.16.20`, `rustls-webpki 0.101.7`, and `hickory-proto 0.24.4/0.25.2`
  - Pulled through the SDK networking stack, including `libp2p`, `libp2p-tls`,
    `libp2p-dns`, `libp2p-mdns`, and `litep2p`.
- `curve25519-dalek 3.2.0`
  - Pulled by the `sora2-ed25519-dalek-iroha` git dependency on `develop`.
  - Requires updating or replacing that fork with a version based on
    `curve25519-dalek >= 4.1.3`.

## Remediation Path

The low-risk path is to move to the next Polkadot SDK stable tag that includes
patched tracing, Wasmtime, libp2p, rustls-webpki, and Hickory dependencies, then
rerun `cargo audit`. Fixing these in `polkadot-stable2603-3` would require
vendoring and patching SDK internals such as tracing, executor, WASM interface,
and networking crates, which is substantially broader than a lockfile upgrade.

The separate non-SDK item is `sora2-ed25519-dalek-iroha`; that fork needs its
own upgrade to a patched Dalek release.
