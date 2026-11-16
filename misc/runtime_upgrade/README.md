# Runtime Upgrade

Release tooling for building, rehearsing, and submitting a runtime upgrade.

## Requirements

- Python 3.11+
- Rust 1.99.1 (`rustup toolchain install 1.99.1 --profile minimal`) for the four native bindings
- `cargo`
- `subwasm` for metadata review (`cargo install --locked subwasm`)

  Or

- Docker

### Python dependency maintenance

`requirements.txt` selects all 29 runtime dependencies for CPython 3.11+:
25 published Python pins and four reviewed local native sources. Each native
source has a locked Rust dependency graph and exact Maturin build backend.
Use a **new virtual environment** for this migration: updating requirements in
an existing environment does not remove the old vulnerable `ecdsa` and
`smoldot-light` packages.
From the repository root:

```bash
python3 -m venv misc/runtime_upgrade/.venv-secure
misc/runtime_upgrade/.venv-secure/bin/python misc/runtime_upgrade/install_dependencies.py
misc/runtime_upgrade/.venv-secure/bin/python -m pip check
misc/runtime_upgrade/.venv-secure/bin/python -m unittest discover -s misc/runtime_upgrade/tests -v
misc/runtime_upgrade/.venv-secure/bin/python misc/runtime_upgrade/main.py --help
```

The 2026-10-24 migration removes Python `ecdsa`, which has no patched release for
[CVE-2024-23342](https://github.com/advisories/GHSA-wj6h-64fc-37mp).
The helper now imports a reviewed source snapshot of `substrate-interface` 1.8.1
from [vendor/](vendor/README.md). Its BIP32 derivation and ECDSA backend use
`coincurve` 21.0.0 / libsecp256k1. SR25519 remains the governance CLI's signing
scheme. Existing URI, seed, mnemonic, governance call and signed-extrinsic formats
are covered by offline compatibility tests.

The unused Python `smoldot-light` 0.1.0 extension is also removed: its published
source lock includes vulnerable `ruzstd` 0.8.2, `lru` 0.12.5 and Wasmtime 36.0.3.
The installed macOS extension confirms the first two versions; pip-audit does
not inspect these bundled Rust libraries. Light-client mode is explicitly
unsupported by this CLI snapshot. HTTP and WebSocket governance RPC behavior
is unchanged. See [the native-dependency evidence](vendor/native-dependency-review.json).

The dependency refresh also updates `certifi` to 2026.10.23 and `urllib3` to 2.8.0.
The three Substrate cryptographic bindings and `pydantic-core` are built from
verified upstream source. Their rebuilt Rust dependencies remove vulnerable
PyO3 versions, old `keccak`/`rand`, and LRU's iterator auto-trait defect. The
installer resolves local source paths relative to its own directory, so it
works from any caller directory and inside Docker. Do not replace those source
paths with upstream wheel pins: that would restore the bundled defects.

Run `pip-audit --strict --path <venv-site-packages>` from a separate audit
environment after installation. Audit all four `vendor/native/*/Cargo.lock`
files with `cargo audit --deny warnings` as well; Python advisory scans do not
inspect Rust code inside wheels. CI performs both audits and runs the offline
compatibility suite. Upstream provenance and the maintenance procedure are
documented in [vendor/README.md](vendor/README.md).

The Docker image uses pinned official Python 3.13.17/Debian Trixie and Rust
1.99.1 build images. Its separate builder produces the reviewed native wheels;
the runtime image installs those wheels without retaining a Rust compiler.
On 2026-10-24, both stages built successfully on Linux arm64. The installed
wheels pass 149 offline tests. The final image includes a freshly generated
runtime WASM with its SHA-256 checked, runs as a non-root user, and passes CLI
help, `pip check`, and an installed-path audit with zero vulnerabilities across
30 distributions including pip. No Rust or C compiler remains in the runtime
image. See the [validation record](vendor/native-dependency-review.json) for the
image, wheel, native binary, lockfile, and WASM hashes.

Local validation uses the explicit `colima-sora-release-verify` Docker context.
Its DNS repair is confined to that profile and was checked after a full restart;
the global Docker context remains unchanged. Colima 0.10.3 can skip DNS setup
when dnsmasq has a pending upgrade ([upstream issue](https://github.com/abiosoft/colima/issues/1611)).
The profile's idempotent after-boot repair configures dnsmasq from the current
systemd-networkd DNS servers and interface address. This guest has no
systemd-resolved service, so replacing its resolver file with a systemd-resolved
symlink would not repair it.

Files inside past `runtime-upgrade-*` packages are retained release evidence with
checksums; dependency maintenance applies to this active tooling directory.

## Build The Candidate Runtime

```bash
cargo build --release --locked --features runtime-wasm --bin framenode
```

The candidate WASM lives at:

`target/release/wbuild/framenode-runtime/framenode_runtime.wasm`

The runtime blob that `WASM_BINARY` embeds for on-chain code storage is:

`target/release/wbuild/framenode-runtime/framenode_runtime.compact.compressed.wasm`

## Rehearse The Upgrade

Run the remote upgrade rehearsal before submitting the upgrade:

```bash
./misc/runtime_upgrade/run_remote_try_runtime.sh
```

Environment variables:

- `REMOTE_RPC_URL`
  Default: `wss://ws.mof.sora.org`
  Target HTTP(S) endpoint for `frame-remote-externalities`.
  This rehearsal replays the runtime upgrade and multi-block migrations against remote state and
  then checks the final storage versions on the fully migrated state.
- `WS`
  Deprecated alias for `REMOTE_RPC_URL`.
- `SNAP`
  Optional snapshot path for `frame-remote-externalities`.
  When set, the rehearsal uses the snapshot offline and falls back to `REMOTE_RPC_URL` if needed.
- `REMOTE_PALLETS`
  Optional comma-separated pallet list for `frame-remote-externalities`.
  When set, only those pallet storage prefixes are scraped instead of the full chain state.
- `REMOTE_CHILD_TRIE`
  Optional boolean flag for child trie scraping.
  Defaults to `true`; set `REMOTE_CHILD_TRIE=0` to skip child trie storage when the selected
  pallets do not rely on it.
- `REQUIRE_REMOTE=1`
  Fail closed instead of skipping when the remote externalities builder cannot connect or load state.
  `run_remote_try_runtime.sh` sets this by default for release use; set `REQUIRE_REMOTE=0` only for
  non-release local smoke runs.

Examples:

```bash
./misc/runtime_upgrade/run_remote_try_runtime.sh
REQUIRE_REMOTE=0 ./misc/runtime_upgrade/run_remote_try_runtime.sh
SNAP=/tmp/framenode.snap REQUIRE_REMOTE=1 ./misc/runtime_upgrade/run_remote_try_runtime.sh
```

This remote replay is the production rehearsal because it follows the actual runtime execution
path: single-block migrations run once, multi-block migrations are stepped to completion, and the
final storage versions are checked on the fully migrated state. Local `cargo test` runs with
`--features try-runtime` provide supplemental hook coverage for the runtime-owned migration wrappers.

## Capture Metadata For Review

Record the exact runtime version and metadata that downstream clients will consume:

```bash
./target/release/framenode --version
NO_COLOR=true subwasm info target/release/wbuild/framenode-runtime/framenode_runtime.wasm
NO_COLOR=true subwasm metadata --format json \
  target/release/wbuild/framenode-runtime/framenode_runtime.wasm \
  > /tmp/framenode-runtime-metadata.json
```

## Release Checklist

1. Build the candidate runtime WASM from the exact release commit.
2. Run `./misc/runtime_upgrade/run_remote_try_runtime.sh`.
3. Record whether the rehearsal used live `REMOTE_RPC_URL` state, a `SNAP` snapshot, or both.
4. Capture `framenode --version`, `subwasm info`, and the JSON metadata for downstream review.
5. Generate the preimage from `framenode_runtime.compact.compressed.wasm` with
   `generate_preimage.py`.
6. Submit `Preimage.note_preimage`.
7. Submit the council motion for `Democracy.external_propose_majority`.
8. Submit the technical committee motion for `Democracy.fast_track`.
9. Submit the runtime upgrade only after the remote rehearsal and metadata review are archived.

## Submit The Upgrade

The governance helper now has explicit subcommands for the live `wss://ws.mof.sora.org` flow:

- `note-preimage`
- `council-propose-majority`
- `tech-fast-track`
- `sudo-set-code` for dev-only networks that still expose `Sudo`

### Governance Flow

1. Install packages:

```bash
python misc/runtime_upgrade/install_dependencies.py
```

2. Upload the SCALE-encoded `set_code` preimage:

```bash
python misc/runtime_upgrade/main.py \
  --node-url wss://ws.mof.sora.org \
  --uri //Alice \
  note-preimage \
  --call-file-path target/release/wbuild/framenode-runtime/framenode_runtime.compact.compressed.wasm.preimage.call
```

3. Propose the external-majority referendum through council:

```bash
python misc/runtime_upgrade/main.py \
  --node-url wss://ws.mof.sora.org \
  --uri //Alice \
  council-propose-majority \
  --preimage-json target/release/wbuild/framenode-runtime/framenode_runtime.compact.compressed.wasm.preimage.json
```

4. Fast-track the externally proposed referendum through the technical committee:

```bash
python misc/runtime_upgrade/main.py \
  --node-url wss://ws.mof.sora.org \
  --uri //Alice \
  tech-fast-track \
  --preimage-json target/release/wbuild/framenode-runtime/framenode_runtime.compact.compressed.wasm.preimage.json
```

### Dev / Private-Net

For networks that still expose `Sudo`, the old direct path remains available as an explicitly named
dev helper:

```bash
python misc/runtime_upgrade/main.py \
  --node-url ws://127.0.0.1:9944 \
  --uri //Alice \
  sudo-set-code \
  --wasm-file-path /path/to/wasm-file
```

## Arguments

```
usage: Runtime Upgrade [-h] [--node-url NODE_URL]
     (--uri URI_KEYPAIR | --seed SEED | --mnemonic MNEMONIC)
     {note-preimage,council-propose-majority,tech-fast-track,sudo-set-code} ...

options:
-h, --help show this help message and exit
--node-url NODE_URL URL of the node to connect to
--uri URI_KEYPAIR URI of the keypair to use
--seed SEED Seed of the keypair to use
--mnemonic MNEMONIC Seed phrase of the keypair to use

```
