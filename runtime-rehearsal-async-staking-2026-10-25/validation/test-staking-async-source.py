#!/usr/bin/env python3
"""Positive and tamper-detection checks for the immutable staking source verifier."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("verify-staking-async-source.py").resolve()
spec = importlib.util.spec_from_file_location("staking_async_source_verifier", SCRIPT)
verifier = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verifier)
UPSTREAM_REPO: Path


class SourceVerifierTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="staking-source-verifier-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.fork = self.root / verifier.VENDOR_PATH
        shutil.copytree(SCRIPT.parents[2] / verifier.VENDOR_PATH, self.fork)
        self.provenance_path = self.fork / "stable2609-customizations.json"
        self.provenance = json.loads(self.provenance_path.read_text())
        self.args = argparse.Namespace(repo_root=self.root, upstream_repo=UPSTREAM_REPO)

    def save_provenance(self):
        self.provenance_path.write_text(json.dumps(self.provenance))

    def test_exact_recorded_source_passes(self):
        result = verifier.verify(self.args)
        self.assertEqual(result["status"], "passed")
        self.assertEqual(result["verifierSha256"], hashlib.sha256(SCRIPT.read_bytes()).hexdigest())
        self.assertTrue(result["verification"]["completeRustInventoryMatches"])

    def test_unchanged_source_mutation_is_rejected(self):
        changed = {entry["path"] for entry in self.provenance["changedSourceFiles"]}
        unchanged = next(path for path in sorted(self.fork.rglob("*.rs"))
                         if path.relative_to(self.fork).as_posix() not in changed)
        unchanged.write_bytes(unchanged.read_bytes() + b"\n// altered unchanged source\n")
        with self.assertRaisesRegex(ValueError, "Complete Rust source inventory differs"):
            verifier.verify(self.args)

    def test_manifest_hash_cannot_authorize_unlisted_policy_change(self):
        manifest = self.fork / "Cargo.toml"
        manifest.write_text(manifest.read_text() +
                            "\n[package.metadata.verifier_negative_fixture]\nforbidden = true\n")
        self.provenance["materializedManifestSha256"] = hashlib.sha256(manifest.read_bytes()).hexdigest()
        self.save_provenance()
        with self.assertRaisesRegex(ValueError, "adaptations beyond explicit"):
            verifier.verify(self.args)

    def test_extra_local_crate_source_cannot_hide_in_excluded_upstream_scope(self):
        extra = self.fork / "rc-client/src/unrecorded.rs"
        extra.parent.mkdir(parents=True)
        extra.write_text("// unrecorded local source\n")
        with self.assertRaisesRegex(ValueError, "Complete Rust source inventory differs"):
            verifier.verify(self.args)

    def test_directory_symlink_cannot_escape_source_inventory(self):
        external = self.root / "external-source"
        external.mkdir()
        (external / "mod.rs").write_text("// outside the recorded fork\n")
        (self.fork / "unrecorded-module").symlink_to(external, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "Symlink outside the source inventory"):
            verifier.inventory(self.fork)

    def test_source_patch_cannot_hide_non_rust_changes(self):
        patch = self.fork / "stable2609-customizations.patch"
        patch.write_bytes(patch.read_bytes() +
                          b"--- /dev/null\n+++ b/unrecorded.txt\n@@ -0,0 +1 @@\n+unexpected\n")
        self.provenance["sourcePatchSha256"] = hashlib.sha256(patch.read_bytes()).hexdigest()
        self.save_provenance()
        with self.assertRaisesRegex(ValueError, "outside the Rust inventory"):
            verifier.verify(self.args)


def main() -> int:
    global UPSTREAM_REPO
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream-repo", required=True, type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    UPSTREAM_REPO = args.upstream_repo.resolve()
    stream = io.StringIO()
    result = unittest.TextTestRunner(stream=stream, verbosity=2).run(
        unittest.defaultTestLoader.loadTestsFromTestCase(SourceVerifierTests))
    report = {"status": "passed" if result.wasSuccessful() else "failed",
              "testsRun": result.testsRun, "failures": len(result.failures),
              "errors": len(result.errors), "verifierSha256": hashlib.sha256(SCRIPT.read_bytes()).hexdigest(),
              "testScriptSha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "details": stream.getvalue()}
    encoded = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.write_text(encoded)
    else:
        sys.stdout.write(encoded)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
