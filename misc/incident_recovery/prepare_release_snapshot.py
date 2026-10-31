#!/usr/bin/env python3
"""Prepare the validator database snapshot for a recovery release.

The completed predecessor reconstruction ends at block 28,191,816. This clones
it, records the BABE state-sync weight the reconstruction lacks, imports
checkpoint 28,191,817 with the exact release node and chain spec, then starts
that node offline on a further clone. A read-only inspector checks the persisted
GRANDPA authority and fresh round state; the node's passive observation journal
checks the actual client admission. Neither observation establishes voter health.
The import record binds the snapshot to the spec's policy commitment,
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
import stat
import subprocess
import sys
import time

import hashi_codec as scale
from node_identity import authority_key

CHAIN_ID = "sora-nexus-recovery-28191817"
CHECKPOINT_NUMBER = 28_191_817
PARENT_HASH = "0x60b63e40ce4de9b9c1408bf7455a6f5ced796825f2437a0ee8496476d80c56d6"
RECOVERY_SET_ID = 7957
VOTER_STOPPED = "GRANDPA remains stopped"
GENESIS = "0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5"
# Existing completed native reconstruction, pinned by retained-predecessor-process-v4
# stdout.log and its passed execution receipt. No new operator input is needed.
RECONSTRUCTION_SHA256 = "539e0b0cafc783c6b29ed4c022dc83e8d730ed795685eaee4e6349a63f98f52a"
MAX_INPUT = 2 * 1024**3
MAX_JSON = 64 * 1024**2
# The predecessor is an archive RocksDB; any other mode or engine refuses it.
DATABASE_ARGS = ["--database", "rocksdb", "--state-pruning", "archive", "--blocks-pruning", "archive"]


def sha(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def regular(path: Path, maximum: int) -> Path:
    path = path.absolute()
    info = path.lstat()
    scale.require(path.resolve(strict=True) == path and stat.S_ISREG(info.st_mode)
                  and info.st_nlink == 1 and info.st_size <= maximum, f"independent bounded file required: {path}")
    return path


def document(path: Path) -> dict:
    return json.loads(regular(path, MAX_JSON).read_bytes())


def write_json(path: Path, value: dict) -> None:
    with path.open("x") as stream:
        json.dump(value, stream, indent=2)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())


class OwnedInputs:
    """Bind the selected inputs before use and execute independent owned copies."""
    def __init__(self, directory: Path):
        directory.mkdir()
        self.directory, self.rows = directory, {}

    def add(self, name: str, source: Path, executable: bool = False) -> Path:
        source = regular(source, MAX_INPUT)
        before = {"sha256": sha(source), "bytes": source.stat().st_size}
        target = self.directory / name
        with source.open("rb") as src, target.open("xb") as dst:
            shutil.copyfileobj(src, dst, 1024**2)
            dst.flush()
            os.fsync(dst.fileno())
        target.chmod(0o500 if executable else 0o400)
        scale.require(sha(source) == before["sha256"] == sha(target)
                      and target.stat().st_size == before["bytes"], f"input changed while copying: {name}")
        self.rows[name] = {**before, "source": str(source), "owned": str(target)}
        return target

    def guard(self) -> None:
        for name, row in self.rows.items():
            for field in ("source", "owned"):
                path = regular(Path(row[field]), MAX_INPUT)
                scale.require(path.stat().st_size == row["bytes"] and sha(path) == row["sha256"],
                              f"bound input changed: {name} ({field})")


def reconstruction(path: Path) -> dict:
    receipt = regular(path / "RECONSTRUCTION.json", MAX_JSON)
    scale.require(sha(receipt) == RECONSTRUCTION_SHA256, "reconstruction differs from completed native evidence")
    value = document(receipt)
    scale.require(value.get("schema") == "sora-nexus-retained-predecessor-reconstruction-v1"
                  and value.get("bestNumber") == CHECKPOINT_NUMBER - 1 and value.get("bestHash") == PARENT_HASH
                  and value.get("grandpaCurrentSetId") == RECOVERY_SET_ID - 1
                  and all(value.get(k) is True for k in (
                      "checkpointAbsent", "clientDatabaseReconstructed", "fullCanonicalHistoryPersisted",
                      "parentStateReopenedAndCompletelyCompared", "noOldGrandpaVoteStateCopied", "noOldKeysCopied")),
                  "completed predecessor identity differs")
    return value


def verify_database_inventory(directory: Path, expected: dict) -> None:
    scale.require(directory.resolve(strict=True) == directory and directory.is_dir(), "canonical database copy required")
    scale.require(isinstance(expected, dict) and 0 < len(expected) <= 100_000, "database inventory bound")
    scale.require({p.name for p in directory.iterdir()} == set(expected), "database file inventory differs")
    for name, row in expected.items():
        scale.require(Path(name).name == name and name not in (".", "..") and "\\" not in name,
                      "invalid database inventory filename")
        path = regular(directory / name, 128 * 1024**3)
        scale.require(set(row) == {"size", "sha256"} and type(row["size"]) is int
                      and path.stat().st_size == row["size"] and sha(path) == row["sha256"],
                      f"database differs from completed reconstruction: {name}")
    scale.require({p.name for p in directory.iterdir()} == set(expected), "database membership changed")


def snapshot_layout(base: Path) -> Path:
    """Only the exact database may enter the archive, including after inspection."""
    directory = base
    for name in ("chains", CHAIN_ID, "db", "full"):
        scale.require(directory.resolve(strict=True) == directory and directory.is_dir()
                      and {p.name for p in directory.iterdir()} == {name}, "foreign snapshot archive layout")
        directory = directory / name
    scale.require(directory.resolve(strict=True) == directory and directory.is_dir(), "aliased snapshot database")
    return directory


def database_inventory(directory: Path) -> dict:
    names = sorted(p.name for p in directory.iterdir())
    scale.require(0 < len(names) <= 100_000, "database inventory bound")
    rows = {}
    for name in names:
        path = regular(directory / name, 128 * 1024**3)
        before = path.stat()
        rows[name] = {"size": before.st_size, "sha256": sha(path)}
        after = path.stat()
        scale.require((before.st_size, before.st_mtime_ns, before.st_ino)
                      == (after.st_size, after.st_mtime_ns, after.st_ino), "database file changed while hashing")
    scale.require(sorted(p.name for p in directory.iterdir()) == names, "database membership changed")
    return rows


def clone(source: Path, target: Path) -> None:
    """Copy-on-write where the filesystem supports it."""
    target.parent.mkdir(parents=True, exist_ok=True)
    command = ["cp", "-c", "-R"] if sys.platform == "darwin" else ["cp", "-a", "--reflink=auto"]
    subprocess.run(command + [str(source), str(target)], check=True)


def run(command: list[str], log: Path, env: dict | None = None) -> None:
    with log.open("xb") as stream:
        code = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT, env=env).returncode
    scale.require(code == 0, f"{command[0]} {command[1]} failed with exit {code}; see {log}")


def admitted_client(journal: Path, pid: int, policy: str) -> dict | None:
    """Consume the native client's observation, never its voter construction log."""
    rows = []
    for index, path in enumerate(sorted(journal.glob("*.json"))):
        scale.require(path.name == f"{index:08}.json", "passive journal has a gap")
        try:
            row = document(path)
        except json.JSONDecodeError:
            return None  # Recorder may still be writing its last create-new row.
        scale.require(row.get("schema") == "sora-nexus-passive-node-observation-v1"
                      and row.get("pid") == pid and row.get("sequence") == index, "foreign passive journal")
        rows.append(row)
    admissions = [r for r in rows if r.get("kind") == "database-admitted"]
    scale.require(len(admissions) <= 1, "duplicate client admission")
    if not admissions:
        return None
    row = admissions[0]
    scale.require(row.get("bestHash") == scale.CHECKPOINT_HASH and row.get("bestNumber") == CHECKPOINT_NUMBER
                  and row.get("finalizedHash") == PARENT_HASH and row.get("finalizedNumber") == CHECKPOINT_NUMBER - 1
                  and row.get("checkpointNumberIndexHash") == scale.CHECKPOINT_HASH
                  and row.get("checkpointHeaderPresentByHash") is True
                  and row.get("checkpointImportPolicy") == policy, "native client snapshot context differs")
    return row


def verify_offline_start(node: Path, spec: Path, base: Path, log: Path, timeout: int, policy: str) -> dict:
    """Observe passive client admission, with no authority keys or peer traffic."""
    journal = log.parent / "passive-client"
    env = {**os.environ, "RUST_LOG": "info,grandpa=debug"}
    command = [str(node), "--chain", str(spec), "--base-path", str(base), *DATABASE_ARGS,
               "--no-prometheus", "--no-telemetry", "--no-mdns", "--reserved-only",
               "--listen-addr", "/ip4/127.0.0.1/tcp/0", "--rpc-port", "0", "--name", "snapshot-verify",
               "--recovery-evidence-dir", str(journal)]
    with log.open("xb") as stream:
        process = subprocess.Popen(command, stdout=stream, stderr=subprocess.STDOUT, env=env)
    try:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            text = log.read_text(errors="replace")
            scale.require(VOTER_STOPPED not in text, f"recovery voter refused to start; see {log}")
            scale.require(process.poll() is None, f"node exited during verification; see {log}")
            admission = admitted_client(journal, process.pid, policy)
            # RPC readiness is only a startup milestone after service construction;
            # it is not treated as evidence that the voter has polled or is healthy.
            if admission is not None and "Running JSON-RPC server:" in text:
                return {"clientAdmission": admission, "voterHealthVerified": False,
                        "scope": "passive client database admission; no voting or finality test"}
            time.sleep(1)
        raise ValueError(f"passive client was not admitted within {timeout}s; see {log}")
    finally:
        process.terminate()
        try:
            process.wait(timeout=60)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()


def inspect_snapshot(tool: Path, database: Path, output: Path, recovery: dict) -> dict:
    """Read raw persisted metadata/aux using the fixed retained-client inspector."""
    log = output.parent / (output.name + ".stdout.json")
    run([str(tool), "--inspect-checkpoint-snapshot", str(database / "full"),
         str(output), recovery["policyCommitment"]], log)
    report = document(log)
    expected = sorted([["0x" + authority_key(key).hex(), weight] for key, weight in recovery["grandpaAuthorities"]])
    scale.require(report.get("schema") == "sora-nexus-checkpoint-snapshot-inspection-v1"
                  and report.get("database") == str(database / "full")
                  and report.get("engineReadOnly") is True and report.get("genesisHash") == GENESIS
                  and report.get("bestHash") == scale.CHECKPOINT_HASH and report.get("bestNumber") == CHECKPOINT_NUMBER
                  and report.get("finalizedHash") == PARENT_HASH and report.get("finalizedNumber") == CHECKPOINT_NUMBER - 1
                  and report.get("checkpointImportPolicy") == recovery["policyCommitment"]
                  and report.get("grandpaSetId") == RECOVERY_SET_ID
                  and report.get("grandpaAuthorities") == expected
                  and report.get("savedRoundSetId") == RECOVERY_SET_ID
                  and report.get("freshRoundStateVerified") is True
                  and report.get("voterHealthVerified") is False and report.get("productionReady") is False,
                  "persisted checkpoint snapshot inspection differs")
    artifacts = report.get("artifacts")
    scale.require(isinstance(artifacts, dict) and set(artifacts) == {
        "checkpoint-header.scale", "parent-header.scale", "grandpa-voters.scale", "grandpa-rounds.scale", "babe-epochs.scale"},
        "snapshot inspection artifacts differ")
    for name, row in artifacts.items():
        path = regular(output / name, MAX_JSON)
        scale.require(set(row) == {"file", "bytes", "sha256"} and row["file"] == name
                      and row["bytes"] == path.stat().st_size and row["sha256"] == sha(path),
                      "snapshot inspection artifact changed")
    return report


def evidence_inventory(directory: Path) -> dict:
    rows = {}
    for path in sorted(directory.rglob("*")):
        if path.is_dir() and not path.is_symlink():
            continue
        path = regular(path, MAX_INPUT)
        rows[str(path.relative_to(directory))] = {"bytes": path.stat().st_size, "sha256": sha(path)}
        scale.require(len(rows) <= 10_000, "snapshot evidence file bound")
    return rows


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
    destination = args.destination.absolute()
    destination.mkdir(parents=False)
    inputs = OwnedInputs(destination / "inputs")
    node = inputs.add("framenode", args.node, executable=True)
    seed_tool = inputs.add("retained-client-evidence", args.seed_tool, executable=True)
    chain_spec = inputs.add("chain-spec.json", args.chain_spec)
    checkpoint_block = inputs.add("checkpoint-block.json", args.checkpoint_block)
    predecessor = args.predecessor.absolute()
    original = reconstruction(predecessor)
    inputs.add("RECONSTRUCTION.json", predecessor / "RECONSTRUCTION.json")
    scale.require(inputs.rows["RECONSTRUCTION.json"]["sha256"] == RECONSTRUCTION_SHA256,
                  "reconstruction changed before its owned copy")
    spec = document(chain_spec)
    recovery = spec["recovery"]
    scale.require(spec["id"] == CHAIN_ID and recovery["checkpointHash"] == scale.CHECKPOINT_HASH,
                  "chain spec is not the recovery release")
    scale.require(recovery["previousGrandpaSetId"] + 1 == RECOVERY_SET_ID, "recovery GRANDPA set differs")
    checkpoint = document(checkpoint_block)
    scale.require(not checkpoint.get("justifications"), "checkpoint import must not carry abandoned-set justifications")
    block = checkpoint["block"]["header"]
    scale.require(int(block["number"], 16) == CHECKPOINT_NUMBER and block["parentHash"] == PARENT_HASH,
                  "checkpoint block file is not block 28,191,817")
    source = predecessor / "chains" / "sora-nexus-recovery-predecessor" / "db"
    base = destination / "base"
    database = base / "chains" / CHAIN_ID / "db"
    clone(source, database)
    scale.require(sorted(p.name for p in database.iterdir()) == ["full"], "foreign predecessor database layout")
    verify_database_inventory(database / "full", original["outputDatabaseFiles"])
    inputs.guard()
    write_json(destination / "INPUTS.json", {"schema": "sora-nexus-snapshot-inputs-v1", "files": inputs.rows,
               "reconstructionSha256": RECONSTRUCTION_SHA256, "copiedDatabaseInventoryVerified": True})
    logs = destination / "logs"
    logs.mkdir()
    seed = logs / "seed.json"
    with seed.open("xb") as stream, (logs / "seed.stderr.log").open("xb") as errors:
        code = subprocess.run([str(seed_tool), "--seed-predecessor-babe-weight", str(database / "full")],
                              stdout=stream, stderr=errors).returncode
    scale.require(code == 0, f"BABE weight seed failed; see {seed}")
    inputs.guard()
    run([str(node), "import-blocks", "--chain", str(chain_spec), "--base-path", str(base),
         *DATABASE_ARGS, str(checkpoint_block)], logs / "import.log")
    post_import = database_inventory(snapshot_layout(base))
    inspected = inspect_snapshot(seed_tool, database, logs / "persisted-import", recovery)
    verify = destination / "verify"
    clone(database, verify / "chains" / CHAIN_ID / "db")
    started = verify_offline_start(node, chain_spec, verify, logs / "verify-node.log", args.timeout,
                                   recovery["policyCommitment"])
    inspect_snapshot(seed_tool, verify / "chains" / CHAIN_ID / "db", logs / "persisted-restart", recovery)
    verify_database_inventory(snapshot_layout(base), post_import)
    # Keep failed copies for diagnosis; only this successful disposable clone is removed.
    shutil.rmtree(verify)
    evidence = evidence_inventory(logs)
    output = destination / f"{CHAIN_ID}-db.tar.zst"
    archive(base, output)
    verify_database_inventory(snapshot_layout(base), post_import)
    version = subprocess.run([str(node), "--version"], capture_output=True, text=True, check=True).stdout.strip()
    inputs.guard()
    scale.require(evidence_inventory(logs) == evidence, "snapshot evidence changed before registration")
    receipt = {
        "schema": "sora-nexus-validator-snapshot-v1",
        "archive": {"file": output.name, "bytes": output.stat().st_size, "sha256": sha(output),
                    "extractInto": "<base-path>", "contains": f"chains/{CHAIN_ID}/db/full",
                    "excludedDiagnostics": [name for name in post_import if name == "LOG" or name.startswith("LOG.old.")],
                    "extractedContentsVerified": False},
        "databaseSourceFiles": post_import,
        "bestBlock": {"number": inspected["bestNumber"], "hash": inspected["bestHash"]},
        "finalizedBlock": {"number": inspected["finalizedNumber"], "hash": inspected["finalizedHash"]},
        "policyCommitment": recovery["policyCommitment"],
        "recoveryGrandpaSetId": RECOVERY_SET_ID,
        "node": {"version": version, "sha256": inputs.rows["framenode"]["sha256"]},
        "chainSpecSha256": inputs.rows["chain-spec.json"]["sha256"],
        "checkpointBlockSha256": inputs.rows["checkpoint-block.json"]["sha256"],
        "predecessorReconstructionSha256": RECONSTRUCTION_SHA256,
        "seedTool": {"sha256": inputs.rows["retained-client-evidence"]["sha256"], "receipt": document(seed)},
        "persistedInspection": inspected,
        "evidenceFiles": evidence,
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
