from contextlib import contextmanager
import gzip
import hashlib
import io
import json
from pathlib import Path
import sqlite3
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import archive_burn_evidence as archive
import hashi_codec as scale
import release_package as package


class ArchiveTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.raw = self.root / "raw"
        self.raw.mkdir()

    @contextmanager
    def connections_closed(self):
        """Assert closure directly; GC ResourceWarnings alone do not fail tests."""
        connections = []
        create = archive.db_new

        def track(path):
            db = create(path)
            connections.append(db)
            return db

        with patch.object(archive, "db_new", side_effect=track):
            try:
                yield
            finally:
                try:
                    self.assertTrue(connections)
                    for db in connections:
                        with self.assertRaisesRegex(sqlite3.ProgrammingError, "closed database"):
                            db.execute("SELECT 1")
                finally:
                    for db in connections:
                        db.close()

    def write(self, relative, data):
        path = self.raw / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        return {"file": relative, "sha256": hashlib.sha256(data).hexdigest()}

    def fixture(self, nodes):
        hashes = []
        for data in nodes:
            digest = hashlib.sha256(data).hexdigest()
            self.write("objects/" + digest, data)
            hashes.append(digest)
        initial = self.write("initial.json", archive.canonical({"proof": hashes}))
        page = self.write("pages/page.json.gz", gzip.compress(archive.canonical({"schema": "sora-nexus-burn-proof-page-v1", "records": [{"proof": hashes}]}), mtime=0))
        manifest = {"schema": "sora-nexus-burn-proof-stream-v1", "checkpointHash": scale.CHECKPOINT_HASH, "first": scale.CHECKPOINT, "last": scale.CHECKPOINT, "initial": initial, "pages": [page]}
        self.write(archive.MANIFEST_NAME, archive.canonical(manifest))
        return self.raw / archive.MANIFEST_NAME

    def test_shards_are_deterministic_bounded_and_bind_every_reference_once(self):
        manifest = self.fixture([b"x" * 600_000, b"y" * 600_000, b"z" * 600_000])
        a, b = self.root / "a", self.root / "b"
        left = archive.archive(manifest, a, 1024**2)
        right = archive.archive(manifest, b, 1024**2)
        self.assertEqual(left, right)
        self.assertGreater(len(left["shards"]), 1)
        self.assertEqual(left["files"], 6)
        self.assertFalse(left["proofsVerified"])
        self.assertFalse(left["productionReady"])
        self.assertEqual(len(archive.verify(a / "burn-evidence-archive.json")), len(left["shards"]))
        self.assertTrue(all(item["bytes"] <= 1024**2 for item in left["shards"]))
        with self.assertRaisesRegex(ValueError, "new"):
            archive.archive(manifest, a)

    def test_restore_reconstructs_exact_native_verifier_inputs_without_overwrite(self):
        manifest = self.fixture([b"node"])
        output = self.root / "archive"
        archive.archive(manifest, output)
        destination = self.root / "restored"
        result = archive.restore(output / "burn-evidence-archive.json", destination)
        self.assertFalse(result["proofsVerified"])
        for path in self.raw.rglob("*"):
            if path.is_file():
                self.assertEqual(path.read_bytes(), (destination / path.relative_to(self.raw)).read_bytes())
        self.assertFalse((destination / archive.INVENTORY).exists())
        self.assertTrue((destination / "RESTORE-COMPLETE").exists())
        with self.assertRaisesRegex(ValueError, "new"):
            archive.restore(output / "burn-evidence-archive.json", destination)

    def test_mutated_missing_nodes_and_shards_are_rejected(self):
        manifest = self.fixture([b"node"])
        node = next((self.raw / "objects").iterdir())
        node.write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "SHA256"):
            archive.archive(manifest, self.root / "bad")
        node.write_bytes(b"node")
        output = self.root / "good"
        result = archive.archive(manifest, output)
        shard = output / result["shards"][0]["file"]
        shard.write_bytes(shard.read_bytes() + b"changed")
        with self.connections_closed():
            with self.assertRaisesRegex(ValueError, "digest"):
                archive.verify(output / "burn-evidence-archive.json")

    def test_invalid_metadata_closes_database_before_shard_creation(self):
        manifest = self.fixture([b"node"])
        (self.raw / "initial.json").write_bytes(b"changed")
        with self.connections_closed():
            with self.assertRaisesRegex(ValueError, "SHA256"):
                archive.archive(manifest, self.root / "invalid-metadata")
        self.assertFalse(list((self.root / "invalid-metadata").glob("*.tar")))

    def test_packager_binds_archive_shards_without_promoting_evidence(self):
        manifest = self.fixture([b"node"])
        out = self.root / "archive"
        result = archive.archive(manifest, out)
        path = out / "burn-evidence-archive.json"
        digest = package.digest_file(path)
        candidate = {"format": package.FORMAT, "checkpoint": {"number": scale.CHECKPOINT, "hash": scale.CHECKPOINT_HASH}, "nodeVersion": "4.8.13",
                     "artifacts": [{"role": "burnHistoryManifest", "path": "archive/burn-evidence-archive.json", "sha256": digest["sha256"], "blake2_256": digest["blake2_256"]}], "evidence": []}
        candidate_path = self.root / "candidate.json"
        candidate_path.write_bytes(archive.canonical(candidate))
        report, _ = package.verify_candidate(candidate_path)
        self.assertEqual(len(report["artifacts"]), 1 + len(result["shards"]))
        self.assertFalse(report["productionReady"])
        self.assertEqual(report["gates"]["burnEntitlementsAndClaimHolds"]["status"], "unverified")

    def test_member_removal_is_rejected_even_if_outer_shard_digest_is_updated(self):
        manifest = self.fixture([b"node"])
        output = self.root / "archive"
        result = archive.archive(manifest, output)
        shard = output / result["shards"][0]["file"]
        modified = self.root / "modified.tar"
        with tarfile.open(shard) as source, tarfile.open(modified, "w", format=tarfile.USTAR_FORMAT) as destination:
            for member in source:
                if member.name.startswith("objects/"):
                    continue
                destination.addfile(member, source.extractfile(member))
        shard.write_bytes(modified.read_bytes())
        result["shards"][0].update(package.digest_file(shard))
        (output / "burn-evidence-archive.json").write_bytes(archive.canonical(result))
        with self.connections_closed():
            with self.assertRaisesRegex(ValueError, "inventory mismatch|count mismatch|referenced"):
                archive.verify(output / "burn-evidence-archive.json")


if __name__ == "__main__":
    unittest.main()
