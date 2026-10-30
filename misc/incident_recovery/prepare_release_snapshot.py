#!/usr/bin/env python3
"""Prepare the validator database snapshot for a recovery release.

The completed predecessor reconstruction ends at block 28,191,816. This clones
it, records the BABE state-sync weight the reconstruction lacks, imports
checkpoint 28,191,817 with the exact release node and chain spec, then starts
that node offline on a further clone and requires it to open recovery GRANDPA
set 7957. The import record binds the snapshot to the spec's policy commitment,
so a snapshot is valid only with the release it was made with. The archive
holds only the database: no keys, network identity or node logs.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

import hashi_codec as scale

CHAIN_ID = "sora-nexus-recovery-28191817"
CHECKPOINT_NUMBER = 28_191_817
PARENT_HASH = "0x60b63e40ce4de9b9c1408bf7455a6f5ced796825f2437a0ee8496476d80c56d6"
RECOVERY_SET_ID = 7957
VOTER_STARTED = f"Starting new voter with set ID {RECOVERY_SET_ID}"
VOTER_STOPPED = "GRANDPA remains stopped"
CHECKPOINT_BEST = f"best: #{CHECKPOINT_NUMBER} ({scale.CHECKPOINT_HASH[:6]}…{scale.CHECKPOINT_HASH[-4:]})"
# The predecessor is an archive RocksDB; any other mode or engine refuses it.
DATABASE_ARGS = ["--database", "rocksdb", "--state-pruning", "archive", "--blocks-pruning", "archive"]


def sha(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def clone(source: Path, target: Path) -> None:
    """Copy-on-write where the filesystem supports it."""
    target.parent.mkdir(parents=True, exist_ok=True)
    command = ["cp", "-c", "-R"] if sys.platform == "darwin" else ["cp", "-a", "--reflink=auto"]
    subprocess.run(command + [str(source), str(target)], check=True)


def run(command: list[str], log: Path, env: dict | None = None) -> None:
    with log.open("xb") as stream:
        code = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT, env=env).returncode
    scale.require(code == 0, f"{command[0]} {command[1]} failed with exit {code}; see {log}")


def verify_offline_start(node: Path, spec: Path, base: Path, log: Path, timeout: int) -> dict:
    """Start the release node with no peers and wait for the recovery voter."""
    env = {**os.environ, "RUST_LOG": "info,grandpa=debug"}
    command = [str(node), "--chain", str(spec), "--base-path", str(base), *DATABASE_ARGS,
               "--no-prometheus", "--no-telemetry", "--no-mdns", "--reserved-only",
               "--listen-addr", "/ip4/127.0.0.1/tcp/0", "--rpc-port", "0", "--name", "snapshot-verify"]
    with log.open("xb") as stream:
        process = subprocess.Popen(command, stdout=stream, stderr=subprocess.STDOUT, env=env)
    try:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            text = log.read_text(errors="replace")
            scale.require(VOTER_STOPPED not in text, f"recovery voter refused to start; see {log}")
            scale.require(process.poll() is None, f"node exited during verification; see {log}")
            started = [line for line in text.splitlines() if VOTER_STARTED in line]
            best = [line for line in text.splitlines() if CHECKPOINT_BEST in line]
            if started and best:
                return {"voterStarted": started[0].strip(), "checkpointBest": best[0].strip()}
            time.sleep(1)
        raise ValueError(f"recovery voter did not start within {timeout}s; see {log}")
    finally:
        process.terminate()
        try:
            process.wait(timeout=60)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()


def archive(base: Path, output: Path) -> None:
    # RocksDB info logs (LOG*) are local diagnostics; its WAL (*.log) is data.
    tar = ["tar", "-C", str(base), "-cf", "-", "--exclude", "LOG", "--exclude", "LOG.old.*"]
    if sys.platform == "darwin":
        tar += ["--no-xattrs", "--no-mac-metadata"]
    env = {**os.environ, "COPYFILE_DISABLE": "1"}
    with output.open("xb") as stream:
        producer = subprocess.Popen(tar + ["chains"], stdout=subprocess.PIPE, env=env)
        compressor = subprocess.run(["zstd", "-q", "-T0", "-9", "-c"], stdin=producer.stdout, stdout=stream)
        producer.stdout.close()
        scale.require(producer.wait() == 0 and compressor.returncode == 0, "snapshot archive failed")
    subprocess.run(["zstd", "-q", "-t", str(output)], check=True)


def prepare(args: argparse.Namespace) -> dict:
    destination = args.destination
    destination.mkdir(parents=False)
    spec = json.loads(args.chain_spec.read_bytes())
    recovery = spec["recovery"]
    scale.require(spec["id"] == CHAIN_ID and recovery["checkpointHash"] == scale.CHECKPOINT_HASH,
                  "chain spec is not the recovery release")
    scale.require(recovery["previousGrandpaSetId"] + 1 == RECOVERY_SET_ID, "recovery GRANDPA set differs")
    block = json.loads(args.checkpoint_block.read_bytes())["block"]["header"]
    scale.require(int(block["number"], 16) == CHECKPOINT_NUMBER and block["parentHash"] == PARENT_HASH,
                  "checkpoint block file is not block 28,191,817")
    reconstruction = json.loads((args.predecessor / "RECONSTRUCTION.json").read_bytes())
    scale.require(reconstruction["bestNumber"] == CHECKPOINT_NUMBER - 1 and reconstruction["checkpointAbsent"],
                  "predecessor reconstruction is not complete before the checkpoint")
    source = args.predecessor / "chains" / "sora-nexus-recovery-predecessor" / "db"
    base = destination / "base"
    database = base / "chains" / CHAIN_ID / "db"
    clone(source, database)
    logs = destination / "logs"
    logs.mkdir()
    seed = logs / "seed.json"
    with seed.open("xb") as stream:
        code = subprocess.run([str(args.seed_tool), "--seed-predecessor-babe-weight", str(database / "full")],
                              stdout=stream, stderr=subprocess.PIPE).returncode
    scale.require(code == 0, f"BABE weight seed failed; see {seed}")
    run([str(args.node), "import-blocks", "--chain", str(args.chain_spec), "--base-path", str(base),
         *DATABASE_ARGS, str(args.checkpoint_block)], logs / "import.log")
    scale.require(sorted(os.listdir(base / "chains" / CHAIN_ID)) == ["db"], "import created files beside the database")
    verify = destination / "verify"
    clone(database, verify / "chains" / CHAIN_ID / "db")
    try:
        started = verify_offline_start(args.node, args.chain_spec, verify, logs / "verify-node.log", args.timeout)
    finally:
        shutil.rmtree(verify)
    output = destination / f"{CHAIN_ID}-db.tar.zst"
    archive(base, output)
    version = subprocess.run([str(args.node), "--version"], capture_output=True, text=True, check=True).stdout.strip()
    receipt = {
        "schema": "sora-nexus-validator-snapshot-v1",
        "archive": {"file": output.name, "bytes": output.stat().st_size, "sha256": sha(output),
                    "extractInto": "<base-path>", "contains": f"chains/{CHAIN_ID}/db/full"},
        "bestBlock": {"number": CHECKPOINT_NUMBER, "hash": scale.CHECKPOINT_HASH},
        "finalizedBlock": {"number": CHECKPOINT_NUMBER - 1, "hash": PARENT_HASH},
        "policyCommitment": recovery["policyCommitment"],
        "recoveryGrandpaSetId": RECOVERY_SET_ID,
        "node": {"version": version, "sha256": sha(args.node)},
        "chainSpecSha256": sha(args.chain_spec),
        "checkpointBlockSha256": sha(args.checkpoint_block),
        "predecessorReconstructionSha256": sha(args.predecessor / "RECONSTRUCTION.json"),
        "seedTool": {"sha256": sha(args.seed_tool), "receipt": json.loads(seed.read_bytes())},
        "offlineStart": started,
        "databaseArgs": DATABASE_ARGS,
        "productionReady": False,
    }
    raw = (json.dumps(receipt, indent=2) + "\n").encode()
    with (destination / "SNAPSHOT.json").open("xb") as stream:
        stream.write(raw)
    return {"receipt": str(destination / "SNAPSHOT.json"), "receiptSha256": hashlib.sha256(raw).hexdigest(),
            "archive": str(output), "archiveSha256": receipt["archive"]["sha256"],
            "archiveBytes": receipt["archive"]["bytes"], "policyCommitment": recovery["policyCommitment"]}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--predecessor", type=Path, required=True,
                        help="completed reconstruction attempt directory holding RECONSTRUCTION.json")
    parser.add_argument("--seed-tool", type=Path, required=True,
                        help="retained-client-evidence built with the retained-predecessor feature")
    parser.add_argument("--node", type=Path, required=True, help="exact release framenode binary")
    parser.add_argument("--chain-spec", type=Path, required=True, help="exact release chain spec")
    parser.add_argument("--checkpoint-block", type=Path, required=True, help="block 28,191,817 as import-blocks JSON")
    parser.add_argument("--destination", type=Path, required=True)
    parser.add_argument("--timeout", type=int, default=600)
    print(json.dumps(prepare(parser.parse_args()), indent=2))
