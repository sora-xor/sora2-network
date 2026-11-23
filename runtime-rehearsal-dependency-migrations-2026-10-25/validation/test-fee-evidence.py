#!/usr/bin/env python3
"""Offline negative checks for fee evidence identities and stale-success handling."""
import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("fee_verifier", HERE / "verify-fee-events.py")
verifier = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verifier)
sha = lambda value: hashlib.sha256(value).hexdigest()


class FeeEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "validation"
        self.root.mkdir()
        (self.root.parent / "candidate.compact.compressed.wasm").write_bytes(b"candidate")
        (self.root / "live-runtime.compact.compressed.wasm").write_bytes(b"deployed")
        self.write("snapshot.json", {"status": "passed", "code": {"sha256": sha(b"deployed")}})
        self.write("candidate-metadata.json", {"V14": {}})
        self.write("wasm-compatibility.json", {"status": "passed", "snapshotSha256": sha((self.root / "snapshot.json").read_bytes()), "inputs": {"baseline": {"sha256": sha(b"deployed")}, "candidate": {"sha256": sha(b"candidate"), "metadataSha256": sha((self.root / "candidate-metadata.json").read_bytes())}}})
        self.write("equivocation-fee-wasm-rehearsal.json", {"status": "passed", "candidate": {"sha256": sha(b"candidate")}, "inputs": {"snapshotSha256": sha((self.root / "snapshot.json").read_bytes())}})

    def write(self, name, value):
        (self.root / name).write_text(json.dumps(value))

    def test_matching_hash_chain_is_accepted(self):
        _, _, bindings = verifier.validate_bindings(self.root)
        self.assertEqual(bindings["candidateSha256"], sha(b"candidate"))

    def test_changed_candidate_metadata_snapshot_or_deployed_code_is_rejected(self):
        for target in [self.root.parent / "candidate.compact.compressed.wasm", self.root / "candidate-metadata.json", self.root / "snapshot.json", self.root / "live-runtime.compact.compressed.wasm"]:
            original = target.read_bytes()
            with self.subTest(file=target.name):
                target.write_bytes(original + b" ")
                with self.assertRaises(AssertionError):
                    verifier.validate_bindings(self.root)
                target.write_bytes(original)

    def test_stale_rehearsal_hash_and_failed_compatibility_are_rejected(self):
        for name, field, value in [("equivocation-fee-wasm-rehearsal.json", "candidate", {"sha256": "0" * 64}), ("wasm-compatibility.json", "status", "failed")]:
            path = self.root / name
            original = path.read_bytes()
            data = json.loads(original)
            data[field] = value
            self.write(name, data)
            with self.assertRaises(AssertionError):
                verifier.validate_bindings(self.root)
            path.write_bytes(original)

    def test_failure_replaces_old_success(self):
        self.write("fee-event-check.json", {"status": "passed", "stale": True})
        (self.root / "candidate-metadata.json").write_text("{}")
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(verifier.main(self.root), 1)
        report = json.loads((self.root / "fee-event-check.json").read_text())
        self.assertEqual(report["status"], "failed")
        self.assertNotIn("stale", report)

    def test_optimized_python_cannot_disable_verification(self):
        code = "import importlib.util, pathlib, sys; s=importlib.util.spec_from_file_location('v',sys.argv[1]);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);sys.exit(m.main(pathlib.Path(sys.argv[2])))"
        result = subprocess.run([sys.executable, "-O", "-c", code, str(HERE / "verify-fee-events.py"), str(self.root)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 1)
        self.assertIn("requires assertions", json.loads((self.root / "fee-event-check.json").read_text())["error"]["message"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
