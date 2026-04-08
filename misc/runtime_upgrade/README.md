# Runtime Upgrade

Release tooling for building, rehearsing, and submitting a runtime upgrade.

## Requirements

- Python 3.11+
- `cargo`
- `subwasm` for metadata review (`cargo install --locked subwasm`)

  Or

- Docker

## Build The Candidate Runtime

```bash
cargo build --release --locked --features runtime-wasm --bin framenode
```

The candidate WASM lives at:

`target/release/wbuild/framenode-runtime/framenode_runtime.wasm`

## Rehearse The Upgrade

Run the remote upgrade rehearsal before submitting the upgrade:

```bash
./misc/runtime_upgrade/run_remote_try_runtime.sh
```

Environment variables:

- `REMOTE_RPC_URL`
  Default: `https://ws.mof.sora.org`
  Target HTTP(S) endpoint for `frame-remote-externalities`.
  This rehearsal replays the runtime upgrade and multi-block migrations against remote state and
  then checks the final storage versions on the fully migrated state.
- `WS`
  Deprecated alias for `REMOTE_RPC_URL`.
- `SNAP`
  Optional snapshot path for `frame-remote-externalities`.
  When set, the rehearsal uses the snapshot offline and falls back to `REMOTE_RPC_URL` if needed.
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
5. Generate the preimage if needed with `generate_preimage.py`.
6. Submit `Preimage.note_preimage`.
7. Submit the council motion for `Democracy.external_propose_majority`.
8. Submit the technical committee motion for `Democracy.fast_track`.
9. Submit the runtime upgrade only after the remote rehearsal and metadata review are archived.

## Submit The Upgrade

The governance helper now has explicit subcommands for the live `ws.mof.sora.org` flow:

- `note-preimage`
- `council-propose-majority`
- `tech-fast-track`
- `sudo-set-code` for dev-only networks that still expose `Sudo`

### Governance Flow

1. Install packages:

```bash
pip install -r requirements.txt
```

2. Upload the SCALE-encoded `set_code` preimage:

```bash
python misc/runtime_upgrade/main.py \
  --node-url wss://ws.mof.sora.org \
  --uri //Alice \
  note-preimage \
  --call-file-path target/release/wbuild/framenode-runtime/framenode_runtime.wasm.preimage.call
```

3. Propose the external-majority referendum through council:

```bash
python misc/runtime_upgrade/main.py \
  --node-url wss://ws.mof.sora.org \
  --uri //Alice \
  council-propose-majority \
  --preimage-json target/release/wbuild/framenode-runtime/framenode_runtime.wasm.preimage.json
```

4. Fast-track the externally proposed referendum through the technical committee:

```bash
python misc/runtime_upgrade/main.py \
  --node-url wss://ws.mof.sora.org \
  --uri //Alice \
  tech-fast-track \
  --preimage-json target/release/wbuild/framenode-runtime/framenode_runtime.wasm.preimage.json
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
