#!/usr/bin/env python3
"""Deterministic bounded archive shards for SORA Nexus raw Burn-event evidence.

This binds evidence bytes and their complete reference graph. It does not verify
trie proofs, reward allocations, finality or production readiness. Use the native
burn-history verifier on the reconstructed files before any release admission.
"""
from __future__ import annotations
import argparse
from contextlib import closing
import gzip
import hashlib
import io
import json
from pathlib import Path
import sqlite3
import tarfile
import tempfile

import hashi_codec as scale
import release_package as package

SCHEMA = "sora-nexus-burn-evidence-archive-v1"
MANIFEST_NAME = "burn-history-manifest.json"
INVENTORY = "__sora_nexus_inventory.jsonl"
MAX_ENTRY = 128 * 1024**2
MAX_PAGE = 64 * 1024**2
MAX_SHARDS = 16300
DEFAULT_SHARD = 256 * 1024**2


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode() + b"\n"


def db_new(path):
    db = sqlite3.connect(path)
    try:
        db.execute("CREATE TABLE expected(path TEXT PRIMARY KEY, sha TEXT, kind TEXT)")
        db.execute("CREATE TABLE actual(path TEXT PRIMARY KEY, sha TEXT, size INTEGER, shard INTEGER, checked INTEGER DEFAULT 0)")
    except BaseException:
        db.close()
        raise
    return db


def expected(db, relative, sha, kind):
    scale.require(isinstance(relative, str) and relative != INVENTORY and 0 < len(relative.encode()) <= 100, "Invalid archived evidence path")
    scale.require(isinstance(sha, str) and len(sha) == 64 and all(c in "0123456789abcdef" for c in sha), "Invalid evidence SHA256")
    old = db.execute("SELECT sha,kind FROM expected WHERE path=?", (relative,)).fetchone()
    scale.require(old is None or old == (sha, kind), "Contradictory evidence reference")
    if old is None:
        db.execute("INSERT INTO expected VALUES(?,?,?)", (relative, sha, kind))


def manifest_refs(db, data):
    manifest = json.loads(data)
    scale.require(manifest.get("schema") == "sora-nexus-burn-proof-stream-v1" and manifest.get("checkpointHash") == scale.CHECKPOINT_HASH, "Wrong Burn history checkpoint")
    scale.require(type(manifest.get("first")) is int and type(manifest.get("last")) is int and 0 < manifest["first"] <= manifest["last"] <= scale.CHECKPOINT, "Invalid Burn history range")
    initial = manifest["initial"]
    expected(db, initial["file"], initial["sha256"], "initial")
    scale.require(isinstance(manifest.get("pages"), list) and 0 < len(manifest["pages"]) <= 4_000_000, "Burn page inventory bound")
    seen = {initial["file"]}
    for page in manifest["pages"]:
        scale.require(set(page) == {"file", "sha256"} and page["file"] not in seen, "Duplicate or malformed Burn page")
        seen.add(page["file"])
        expected(db, page["file"], page["sha256"], "page")
    return manifest


def proof_refs(db, record):
    refs = record["proof"]
    scale.require(isinstance(refs, list) and 0 < len(refs) <= 16384 and len(set(refs)) == len(refs), "Invalid proof-node reference list")
    for digest in refs:
        expected(db, "objects/" + digest, digest, "proof")


def metadata_refs(db, kind, data):
    if kind == "initial":
        proof_refs(db, json.loads(data))
    elif kind == "page":
        with gzip.GzipFile(fileobj=io.BytesIO(data)) as source:
            decoded = source.read(MAX_PAGE + 1)
        scale.require(len(decoded) <= MAX_PAGE, "Burn page decompression bound")
        page = json.loads(decoded)
        scale.require(page.get("schema") == "sora-nexus-burn-proof-page-v1" and isinstance(page.get("records"), list) and 0 < len(page["records"]) <= 16, "Invalid Burn proof page")
        for record in page["records"]:
            proof_refs(db, record)


def checked_data(root, relative, sha):
    path = package.local_file(root, relative)
    scale.require(path.stat().st_size <= MAX_ENTRY, "Evidence entry bound")
    data = path.read_bytes()
    scale.require(hashlib.sha256(data).hexdigest() == sha, "Evidence SHA256 mismatch")
    return path, data


def tar_info(name, size):
    info = tarfile.TarInfo(name)
    info.size = size
    info.mode = 0o444
    info.uid = info.gid = info.mtime = 0
    info.uname = info.gname = ""
    return info


def archive(manifest_path, destination, shard_bytes=DEFAULT_SHARD, *, graph=None):
    # Only internal fixed graph adapters use this extension. The public raw
    # proof archive CLI and its schema remain unchanged.
    manifest_name = graph.MANIFEST_NAME if graph else MANIFEST_NAME
    schema = graph.SCHEMA if graph else SCHEMA
    index_name = graph.INDEX_NAME if graph else "burn-evidence-archive.json"
    parse_manifest = graph.manifest_refs if graph else manifest_refs
    parse_metadata = graph.metadata_refs if graph else metadata_refs
    scale.require(not destination.exists(), "Archive destination must be new")
    scale.require(1024 * 1024 <= shard_bytes <= 1024 * 1024**2, "Archive shard bound")
    scale.require(manifest_path.name == manifest_name and manifest_path.stat().st_size <= MAX_ENTRY, "Raw manifest name/size")
    raw = manifest_path.read_bytes()
    manifest_sha = hashlib.sha256(raw).hexdigest()
    root = manifest_path.parent
    destination.mkdir(parents=True)
    database = destination / ".references.sqlite"
    with closing(db_new(database)) as db:
        expected(db, manifest_name, manifest_sha, "manifest")
        manifest = parse_manifest(db, raw)
        for relative, sha, kind in db.execute("SELECT path,sha,kind FROM expected WHERE kind IN ('initial','page') ORDER BY path").fetchall():
            _, data = checked_data(root, relative, sha)
            parse_metadata(db, kind, data)
        db.commit()
        shards, current, inventory = [], None, None
        current_size = 0
        current_count = 0

        def finish():
            nonlocal current, inventory
            if current is None:
                return
            inventory.seek(0, 2)
            size = inventory.tell()
            inventory.seek(0)
            current.addfile(tar_info(INVENTORY, size), inventory)
            name = current.name
            current.close()
            inventory.close()
            actual = package.digest_file(Path(name))
            scale.require(actual["bytes"] <= shard_bytes, "Completed archive shard bound")
            shards.append({"file": Path(name).name, **actual, "entries": current_count})
            current = inventory = None

        try:
            # The raw manifest is first, allowing a verifier to validate references
            # while streaming later metadata and proof members without extraction.
            rows = db.execute("SELECT path,sha FROM expected ORDER BY CASE kind WHEN 'manifest' THEN 0 ELSE 1 END,path")
            for relative, sha in rows:
                _, data = checked_data(root, relative, sha)
                line = canonical({"path": relative, "sha256": sha, "bytes": len(data)})
                padded = 512 + ((len(data) + 511) // 512) * 512
                scale.require(padded + len(line) + 20480 < shard_bytes, "Evidence file requires a larger shard size")
                if current is not None and current_size + padded + len(line) + 20480 > shard_bytes:
                    finish()
                if current is None:
                    scale.require(len(shards) < MAX_SHARDS, "Too many evidence shards")
                    current = tarfile.open(destination / f"burn-evidence-{len(shards):05d}.tar", "w", format=tarfile.USTAR_FORMAT)
                    inventory = tempfile.SpooledTemporaryFile(max_size=8 * 1024**2)
                    current_size = current_count = 0
                current.addfile(tar_info(relative, len(data)), io.BytesIO(data))
                inventory.write(line)
                current_size += padded + len(line)
                current_count += 1
            finish()
            report = {"schema": schema, "checkpointHash": scale.CHECKPOINT_HASH, "first": manifest["first"], "last": manifest["last"],
                      "sourceManifestSha256": manifest_sha, "manifestPath": manifest_name, "files": db.execute("SELECT COUNT(*) FROM expected").fetchone()[0],
                      "shards": shards, "proofsVerified": False, "productionReady": False,
                      "verificationRequired": graph.VERIFICATION_REQUIRED if graph else "Native authenticated Burn-event history verification after reconstructing raw files"}
            (destination / index_name).write_bytes(canonical(report))
        finally:
            if current is not None:
                current.close()
            if inventory is not None:
                inventory.close()
    database.unlink()
    verify(destination / index_name, graph=graph)
    (destination / "ARCHIVE-COMPLETE").write_text(package.digest_file(destination / index_name)["sha256"] + "\n")
    return report


def verify(index_path, *, graph=None):
    manifest_name = graph.MANIFEST_NAME if graph else MANIFEST_NAME
    schema = graph.SCHEMA if graph else SCHEMA
    parse_manifest = graph.manifest_refs if graph else manifest_refs
    parse_metadata = graph.metadata_refs if graph else metadata_refs
    scale.require(index_path.stat().st_size <= MAX_ENTRY, "Archive index bound")
    index = json.loads(index_path.read_bytes())
    scale.require(index.get("schema") == schema and index.get("checkpointHash") == scale.CHECKPOINT_HASH and index.get("manifestPath") == manifest_name, "Wrong archive identity")
    scale.require(isinstance(index.get("shards"), list) and 0 < len(index["shards"]) <= MAX_SHARDS, "Archive shard count bound")
    references = []
    with tempfile.TemporaryDirectory(prefix="soranx-archive-verify-") as scratch, closing(db_new(Path(scratch) / "refs.sqlite")) as db:
        expected(db, manifest_name, index["sourceManifestSha256"], "manifest")
        first = True
        manifest = None
        for shard_index, shard in enumerate(index["shards"]):
            path = package.local_file(index_path.parent, shard["file"])
            actual = package.digest_file(path)
            scale.require(all(shard.get(key) == actual[key] for key in actual) and actual["bytes"] <= 1024 * 1024**2, "Evidence shard digest/size mismatch")
            references.append((shard["file"], actual))
            inventory_seen = False
            members = 0
            with tarfile.open(path, "r|") as archive_file:
                for member in archive_file:
                    scale.require(member.isreg() and member.mode == 0o444 and member.uid == member.gid == member.mtime == 0 and not member.uname and not member.gname, "Noncanonical evidence archive member")
                    scale.require(not inventory_seen and 0 <= member.size <= MAX_ENTRY, "Evidence member order/size bound")
                    stream = archive_file.extractfile(member)
                    if member.name == INVENTORY:
                        inventory_seen = True
                        for line in stream:
                            scale.require(len(line) <= 1024, "Evidence inventory line bound")
                            row = json.loads(line)
                            scale.require(set(row) == {"path", "sha256", "bytes"}, "Invalid evidence inventory row")
                            found = db.execute("SELECT sha,size,shard,checked FROM actual WHERE path=?", (row["path"],)).fetchone()
                            scale.require(found == (row["sha256"], row["bytes"], shard_index, 0), "Evidence inventory mismatch/duplicate")
                            db.execute("UPDATE actual SET checked=1 WHERE path=?", (row["path"],))
                        continue
                    if first:
                        scale.require(member.name == manifest_name, "Archive must begin with raw manifest")
                        first = False
                    # Path validation is independent of extraction: paths are
                    # reconstructed only by a separate, explicitly invoked tool.
                    parsed = Path(member.name)
                    scale.require(not parsed.is_absolute() and ".." not in parsed.parts and "\\" not in member.name and parsed.as_posix() == member.name and len(member.name.encode()) <= 100,
                                  "Unsafe evidence member name")
                    scale.require(not any("sydneycreds" in part.lower() or part.lower() in (".env", "keystore") for part in parsed.parts), "Credential material is not public evidence")
                    data = stream.read(MAX_ENTRY + 1)
                    scale.require(len(data) == member.size, "Truncated evidence member")
                    sha = hashlib.sha256(data).hexdigest()
                    try:
                        db.execute("INSERT INTO actual(path,sha,size,shard) VALUES(?,?,?,?)", (member.name, sha, len(data), shard_index))
                    except sqlite3.IntegrityError as error:
                        raise ValueError("Duplicate evidence archive member") from error
                    kind = db.execute("SELECT kind FROM expected WHERE path=?", (member.name,)).fetchone()
                    if member.name == manifest_name:
                        manifest = parse_manifest(db, data)
                    elif kind and kind[0] in ("initial", "page"):
                        parse_metadata(db, kind[0], data)
                    members += 1
            scale.require(inventory_seen and members == shard.get("entries"), "Missing evidence inventory/count mismatch")
            db.commit()
        scale.require(manifest and manifest["first"] == index["first"] and manifest["last"] == index["last"], "Archive range mismatch")
        scale.require(db.execute("SELECT COUNT(*) FROM actual").fetchone()[0] == index["files"], "Archive file count mismatch")
        mismatch = db.execute("SELECT expected.path FROM expected LEFT JOIN actual USING(path) WHERE actual.path IS NULL OR expected.sha<>actual.sha LIMIT 1").fetchone()
        extra = db.execute("SELECT actual.path FROM actual LEFT JOIN expected USING(path) WHERE expected.path IS NULL OR actual.checked<>1 LIMIT 1").fetchone()
        scale.require(mismatch is None and extra is None, "Missing, extra or mismatched referenced evidence")
    return references


def restore(index_path, destination, *, graph=None):
    references = verify(index_path, graph=graph)
    scale.require(not destination.exists(), "Restore destination must be new")
    destination.mkdir(parents=True)
    total = 0

    class DigestReader:
        def __init__(self, source):
            self.source, self.sha, self.size = source, hashlib.sha256(), 0
        def read(self, size=-1):
            data = self.source.read(size)
            self.sha.update(data)
            self.size += len(data)
            return data

    for relative, expected_digest in references:
        source_path = package.local_file(index_path.parent, relative)
        with source_path.open("rb") as raw:
            reader = DigestReader(raw)
            with tarfile.open(fileobj=reader, mode="r|") as archive_file:
                for member in archive_file:
                    if member.name == INVENTORY:
                        continue
                    parsed = Path(member.name)
                    scale.require(member.isreg() and not parsed.is_absolute() and ".." not in parsed.parts and "\\" not in member.name and parsed.as_posix() == member.name and len(member.name.encode()) <= 100,
                                  "Unsafe restore member")
                    scale.require(0 <= member.size <= MAX_ENTRY, "Restore member bound")
                    output = destination / parsed
                    output.parent.mkdir(parents=True, exist_ok=True)
                    stream = archive_file.extractfile(member)
                    written = 0
                    with output.open("xb") as target:
                        while chunk := stream.read(1024 * 1024):
                            written += len(chunk)
                            scale.require(written <= member.size, "Restore member length")
                            target.write(chunk)
                    scale.require(written == member.size, "Truncated restore member")
                    output.chmod(0o444)
                    total += 1
            while reader.read(1024 * 1024):
                pass
            scale.require(reader.sha.hexdigest() == expected_digest["sha256"] and reader.size == expected_digest["bytes"],
                          "Archive changed while reconstructing evidence")
    (destination / "RESTORE-COMPLETE").write_text(package.digest_file(index_path)["sha256"] + "\n")
    return {"manifest": str(destination / (graph.MANIFEST_NAME if graph else MANIFEST_NAME)), "files": total, "proofsVerified": False, "productionReady": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--destination", type=Path)
    parser.add_argument("--verify", type=Path)
    parser.add_argument("--restore", type=Path)
    parser.add_argument("--shard-bytes", type=int, default=DEFAULT_SHARD)
    args = parser.parse_args()
    if args.restore:
        scale.require(args.verify is None and args.manifest is None and args.destination is not None, "Choose archive, verify or restore")
        print(json.dumps(restore(args.restore, args.destination)))
    elif args.verify:
        scale.require(args.manifest is None and args.destination is None, "Choose archive or verify")
        refs = verify(args.verify)
        print(json.dumps({"shards": len(refs), "referenceGraphBound": True, "productionReady": False}))
    else:
        scale.require(args.manifest is not None and args.destination is not None, "Manifest and new destination required")
        result = archive(args.manifest, args.destination, args.shard_bytes)
        print(json.dumps({"files": result["files"], "shards": len(result["shards"]), "referenceGraphBound": True, "productionReady": False}))


if __name__ == "__main__":
    main()
