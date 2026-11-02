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
If your existing command uses `--node-key`, keep that argument instead; the wrapper
accepts it without requiring `network/secret_ed25519`.

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

### Bridge offchain workers

On recovery build `4.8.13-0acfac57d` (runtime 138), the presence of
`:sora-nexus:recovery:bridge-halt:v1` makes the bridge offchain worker return before
its first debug message and rejects ordinary bridge extrinsics with
`RecoveryHalted`. The check uses key existence: a stored `0x00` also keeps the
bridge halted. The node separately waits for verified recovery completion before
starting the offchain worker runner.

That release deliberately skips local bridge bootstrap, even with an Ethereum
key in the keystore. A fresh offchain database can therefore emit
`No network ids found in offchain storage`. **Clearing the bridge halt does not
initialize the missing network IDs, peer marker or RPC configuration.** Those
values are written by normal node startup, not by the offchain worker. Restarting
with the same recovery chain specification still skips bootstrap. An Ethereum
key plus `--offchain-worker always` does not establish bridge readiness on that
release.

The bridge bootstrap fix adds an explicit startup path for recovery bridge peers.
After installing a build containing the fix, retain the Ethereum key in the local
keystore, configure at least one network in the chain-local `bridge/eth.json`, and
start with HTTP RPC enabled and `--offchain-worker always`:

```sh
./sora-recovery run --base-path /srv/sora --name "<your node name>" \
  --offchain-worker always --rpc-port 9944 -leth_bridge=debug
```

With that explicit option, startup validates the configuration and writes the
public peer marker, network IDs and RPC settings before registering bridge
metrics. Missing, invalid or empty network configuration fails startup. Default
validator startup (`when-authority`) still skips recovery bridge bootstrap;
offline commands never initialize it. Keep `--offchain-worker always` in the
command on subsequent bridge-peer restarts.

The startup `Ethereum bridge peer initialized for networks` message confirms
local initialization. It does not establish on-chain peer membership, working
Ethereum RPC connectivity, successful signing or request import. Recovery
completion and the on-chain bridge halt remain independent gates: initialization
does not clear the halt or submit transactions. With the halt present, startup
also reports `Bridge halted, offchain worker idle`. Do not remove the halt key
manually to test readiness.

The chain-relative bridge configuration location is:

```text
<base-path>/chains/sora-nexus-recovery-28191817/bridge/eth.json
```

The old mainnet chain directory's `bridge/eth.json` is not used for this chain.
The original recovery release skips loading the file altogether. The fixed
build loads it at startup when recovery bridge bootstrap is explicitly enabled;
loading is never deferred to the first worker invocation. Network-specific
bridge metrics are also registered at startup; they are not discovered later by
the metrics polling loop.

For bridge diagnostics, pass `-leth_bridge=debug` at startup. The original build
is silent while the halt is present even at debug level. To allow later changes
through `system_addLogFilter`, also start with `--enable-log-reloading`; without
it, the RPC returns `No reload handle present`. The wrapper passes these flags
through to the node. Use `--offchain-worker never` when bridge activation timing
must remain under explicit operator control; it disables all offchain workers.

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
