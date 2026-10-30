# SORA Nexus recovery restart kit

Chain `sora-nexus-recovery-28191817`, node `4.8.13-fb3ff0a3b`, built from
`sora-xor/sora2-network` commit `fb3ff0a3bf19d626ed277e2cddb030ee8dfb380c`.

## Before you start

- Stop every old node that holds your validator keys, including backups and
  sentries. Two nodes signing with the same keys get slashed.
- Download the snapshot `sora-nexus-recovery-28191817-db.tar.zst` (13.5 GB). The
  kit checks its SHA-256 before unpacking. Your data directory needs 20 GB free.
- Use the data directory your node ran with before: its `--base-path`, or the
  directory you mounted at `/chain`. The kit takes your keystore and network key
  from `chains/sora-substrate-main-net/keystore` and
  `chains/sora-substrate-main-net/network/secret_ed25519` there. If you kept them
  elsewhere, pass `--keystore-path` and `--node-key-file` to `run`.

## Docker

```
docker pull sora2/substrate:recovery-28191817

docker run --rm -v /srv/sora:/chain -v "$PWD":/download:ro sora2/substrate:recovery-28191817 \
  unpack /download/sora-nexus-recovery-28191817-db.tar.zst

docker run -d --name sora-recovery --init --restart unless-stopped --stop-timeout 120 \
  -v /srv/sora:/chain -p 30333:30333 sora2/substrate:recovery-28191817 --name "<your node name>"

docker logs -f sora-recovery
```

Replace `/srv/sora` with your data directory. The release sheet gives the image
digest to pull it by. Without registry access, load the same image from the
release files with `docker load -i sora-nexus-recovery-28191817-image.tar.gz`. The
image runs as uid 10000, like the previous release image. If that user cannot
write the directory, `unpack` says how to fix it. To build the image yourself,
run `docker build -t sora2/substrate:recovery-28191817 .` in this kit; it compiles
the same commit.

## From source (Linux)

```
./sora-recovery build
./sora-recovery unpack sora-nexus-recovery-28191817-db.tar.zst --base-path /srv/sora
./sora-recovery run --base-path /srv/sora --name "<your node name>"
```

`build` fetches the pinned commit and compiles it in 30-60 minutes. If a tool is
missing it prints the command that installs it. Run `unpack` and `run` as the
user that owns the data directory. Under systemd, use
`ExecStart=/path/to/kit/sora-recovery run --base-path /srv/sora --name "<your node name>"`
with `TimeoutStopSec=120`.

## Running

`run` starts the node as a validator on the recovery chain spec, with the
recovery feeder beside it. Add the options you normally use: `--name`,
`--public-addr`, `--port`, `--rpc-port`, `--prometheus-external`,
`--telemetry-url` and so on. The kit sets `--chain`, `--base-path`,
`--recovery-inherent-dir` and `--state-pruning archive` itself and rejects options
that conflict with them. `--print` shows the commands without starting them.

Bootnodes and RPC nodes add `--full-node`: no validator role and no feeder.
Bootnodes must keep their network keys so their peer IDs stay the same. A node
that joins after the restart can skip the snapshot with `--sync warp` once the
recovery chain has finalized blocks; at least 11 of the 15 validators must start
from the snapshot for that finality to exist.

What you should see:

- `sora-recovery: network identity 12D3Koo...`, your usual peer ID.
- `recovery-feeder: block N (Applying): wrote envelope K/2214 ...` for each new
  block while the maintenance work is applied. With every validator up this takes
  about 3 h 45 min.
- `recovery-feeder: complete`, then ordinary blocks.

If a `recovery-feeder` line says `STOPPED`, leave the node running and report
that line. Do not edit the kit or remove files to get past it.
