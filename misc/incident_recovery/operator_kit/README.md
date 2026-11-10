# SORA Nexus recovery restart kit

**First start:** stop → build/pull → unpack → run. **Later restarts:** run only.

## 1. Prepare

- Stop all old nodes using your validator keys, including backups, to avoid double-signing.
- Download `sora-nexus-recovery-28191817-db.tar.zst` (13.5 GB) from the release
  files. Allow another 20 GB free for unpacking.
- Replace `/srv/sora` below with your existing **base path**: the directory
  containing `chains/`, not `db/full`.

**Dropping a file into the old database is not enough.** `unpack` creates:

```text
/srv/sora/chains/sora-nexus-recovery-28191817/db/full
```

Keep the old chain directory: `run` reuses its validator and network keys.
For custom key locations, pass your existing `--keystore-path` and `--node-key-file`.

## 2. Choose one method

### From source (Linux)

Download the **full** `sora-nexus-recovery-28191817-kit.tar.gz`; the GitHub folder
alone lacks required files. Put it beside the snapshot. As the node's user:

```sh
tar -xzf sora-nexus-recovery-28191817-kit.tar.gz
cd sora-nexus-recovery-28191817-kit

# Build once (~30–60 minutes; ~40 GB build space).
./sora-recovery build

# Unpack once (requires zstd).
./sora-recovery unpack ../sora-nexus-recovery-28191817-db.tar.zst --base-path /srv/sora

# Start the node and recovery feeder (requires Python 3.8+).
./sora-recovery run --base-path /srv/sora --name "<your node name>"
```

For systemd, replace the old `ExecStart` with this `run` command using the
absolute kit path; set `TimeoutStopSec=120`.

### Docker

From the directory containing the snapshot:

```sh
docker pull sora2/substrate:recovery-28191817

docker run --rm -v /srv/sora:/chain -v "$PWD":/download:ro sora2/substrate:recovery-28191817 \
  unpack /download/sora-nexus-recovery-28191817-db.tar.zst

docker run -d --name sora-recovery --init --restart unless-stopped --stop-timeout 120 \
  -v /srv/sora:/chain -p 30333:30333 sora2/substrate:recovery-28191817 --name "<your node name>"

docker logs -f sora-recovery
```

Use the image digest from the release sheet. The data directory must be
writable by uid 10000.

Both methods start validators; bootnodes/RPC nodes add `--full-node`.
At least **11 of 15 validators** must start from the snapshot to establish finality.

## 3. Check progress

Confirm your usual peer ID appears. Validators: wait for `recovery-feeder: complete`;
keep the node running. If it reports `STOPPED`, keep the node running and
report that line. Do not edit/delete recovery files to bypass it.

## 4. Later restarts

From the same kit directory:

```sh
./sora-recovery run --base-path /srv/sora --name "<your node name>"
```

Or restart your updated systemd service / existing Docker container
(`docker restart sora-recovery`). **Do not repeat `build` or `unpack`.**

The feeder resumes unfinished work or exits after detecting completion.
Keep the kit, recovery chain spec, database and keys. Use this node release
until a compatible upgrade is provided; it is not required forever.
**Do not switch back to the old binary or old mainnet command.**
