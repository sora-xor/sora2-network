import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest

import build_fixture_wasm as build
import fixture_artifacts as fixture
import refresh_fixture_source as refresh
from test_release_package import policy


class RefreshTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        subprocess.run(["git", "init", "-q", str(self.repo)], check=True)
        for name, data in {"Cargo.lock": "lock", "runtime/src/lib.rs": "spec_version: 138, transaction_version: 134, // before",
                           "runtime/src/incident_recovery/generated.rs": "EMPTY", "rust-toolchain.toml": "pinned"}.items():
            path = self.repo / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(data)
        subprocess.run(["git", "add", "."], cwd=self.repo, check=True)
        subprocess.run(["git", "-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture"], cwd=self.repo, check=True)
        self.policy = self.root / "policy.scale"
        self.policy.write_bytes(policy())
        self.export = self.root / "export"
        fixture.export_final(self.policy, self.export, self.repo)
        (self.export / "target").mkdir()
        (self.export / "target/warm").write_text("warm target retained")

    def test_explicit_refresh_archives_previous_source_and_preserves_target(self):
        source = self.repo / "runtime/src/lib.rs"
        source.write_text(source.read_text().replace("before", "after"))
        evidence = self.root / "evidence"
        result = refresh.refresh(self.export, self.policy, evidence, self.repo)
        self.assertEqual(result["status"], "complete")
        self.assertTrue(result["warmTargetPreserved"])
        self.assertEqual((self.export / "target/warm").read_text(), "warm target retained")
        self.assertIn("after", (self.export / "runtime/src/lib.rs").read_text())
        with tarfile.open(evidence / "previous-source.tar.gz") as archive:
            self.assertIn(b"before", archive.extractfile("runtime/src/lib.rs").read())
            self.assertNotIn("target/warm", archive.getnames())
        self.assertFalse(result["productionReady"])

    def test_modified_fixture_refuses_refresh_without_losing_work(self):
        source = self.export / "runtime/src/lib.rs"
        source.write_text("local modification")
        with self.assertRaisesRegex(ValueError, "differs"):
            refresh.refresh(self.export, self.policy, self.root / "evidence", self.repo)
        self.assertEqual(source.read_text(), "local modification")
        self.assertFalse((self.root / "evidence").exists())

    def test_active_build_lock_prevents_refresh(self):
        with build.source_lock(self.export), self.assertRaisesRegex(ValueError, "Another fixture"):
            refresh.refresh(self.export, self.policy, self.root / "evidence", self.repo)

    def test_unrecorded_source_files_are_not_silently_deleted(self):
        (self.export / "runtime/local.rs").write_text("local work")
        with self.assertRaisesRegex(ValueError, "Unexpected files"):
            refresh.refresh(self.export, self.policy, self.root / "evidence", self.repo)
        self.assertTrue((self.export / "runtime/local.rs").exists())


if __name__ == "__main__":
    unittest.main()
