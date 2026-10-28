"""Fixture preparation tests. No synthetic data is treated as release evidence."""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

import fixture_artifacts as fixture
import hashi_codec as scale
import release_package as release
from test_release_package import policy, wasm, hx


class FixtureTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

    def inventory(self):
        return {"schema": "sora-nexus-checkpoint-consensus-membership-v2", "checkpointHash": scale.CHECKPOINT_HASH,
                "stateRoot": release.CHECKPOINT_ROOT, "validators": [hx(2)], "observedSigningInventory": [
                    {"owner": hx(2), "retiredGrandpa": [hx(8)], "retainedBabe": [hx(9)], "retainedImOnline": [hx(10)], "operatorApproved": False}]}

    def test_operator_fixture_is_deterministic_complete_and_never_approval(self):
        encoded, rows = fixture.fixture_operators(self.inventory())
        second, _ = fixture.fixture_operators(self.inventory())
        self.assertEqual(encoded, second)
        self.assertEqual(release.operator_identity(encoded)["owners"], 1)
        self.assertFalse(rows[0]["operatorApproved"])
        self.assertTrue(rows[0]["fixtureOnly"])
        self.assertNotEqual(rows[0]["freshGrandpa"], hx(8))
        changed = self.inventory()
        changed["observedSigningInventory"][0]["retainedBabe"] = [hx(9), hx(9)]
        with self.assertRaisesRegex(ValueError, "union"):
            fixture.fixture_operators(changed)
        changed = self.inventory()
        changed["validators"] = [hx(3)]
        with self.assertRaisesRegex(ValueError, "reordered"):
            fixture.fixture_operators(changed)

    def test_policy_source_is_exact_and_destination_is_new(self):
        data = policy()
        source = fixture.final_source(data)
        self.assertIn("NEVER DEPLOY", source)
        self.assertIn("[" + ", ".join(str(b) for b in data) + "]", source)
        self.assertIn("[" + ", ".join(str(b) for b in bytes([1]) * 32) + "]", source)
        fixture.fresh_directory(self.root / "out")
        with self.assertRaisesRegex(ValueError, "new directory"):
            fixture.fresh_directory(self.root / "out")

    def test_final_export_preserves_lock_and_worktree_and_records_generated_binding(self):
        repo = self.root / "repo"
        repo.mkdir()
        subprocess.run(["git", "init", "-q", str(repo)], check=True)
        for name, data in {"Cargo.lock": "preserved lock", "runtime/src/lib.rs": "spec_version: 138, transaction_version: 134,",
                           "runtime/src/incident_recovery/generated.rs": "EMPTY", "rust-toolchain.toml": "pinned"}.items():
            path = repo / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(data)
        subprocess.run(["git", "add", "."], cwd=repo, check=True)
        subprocess.run(["git", "-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture"], cwd=repo, check=True)
        (repo / "runtime/untracked.rs").write_text("untracked recovery source")
        policy_path = self.root / "policy.scale"
        policy_path.write_bytes(policy())
        output = self.root / "export"
        report = fixture.export_final(policy_path, output, repo)
        self.assertTrue(report["fixtureOnly"])
        self.assertFalse(report["productionReady"])
        self.assertEqual((output / "Cargo.lock").read_text(), "preserved lock")
        self.assertEqual((repo / "runtime/src/incident_recovery/generated.rs").read_text(), "EMPTY")
        self.assertIn("untracked recovery", (output / "runtime/untracked.rs").read_text())
        self.assertEqual(report["generatedPolicy"], fixture.digest((output / "runtime/src/incident_recovery/generated.rs").read_bytes()))
        with self.assertRaisesRegex(ValueError, "new directory"):
            fixture.export_final(policy_path, output, repo)

    def test_bindings_pin_versions_and_mark_fixture(self):
        inputs = self.root / "inputs"
        inputs.mkdir()
        for name, data in {"policy.scale": policy(), "operators.scale": fixture.fixture_operators(self.inventory())[0],
                           "final.wasm": wasm(138, 134), "hold.wasm": wasm(135, 132)}.items():
            (inputs / name).write_bytes(data)
        output = self.root / "bound"
        report = fixture.write_bindings(inputs / "policy.scale", inputs / "operators.scale", inputs / "final.wasm", inputs / "hold.wasm", 7, 11, output)
        self.assertTrue(report["fixtureOnly"])
        self.assertFalse(report["signingStopVerified"])
        binding = json.loads((output / "bootstrap-134-binding.json").read_text())
        self.assertEqual(binding["policyCommitment"], "0x" + hashlib.blake2b(policy(), digest_size=32).hexdigest())
        self.assertEqual((output / binding["finalWasm"]).read_bytes(), wasm(138, 134))
        (inputs / "final.wasm").write_bytes(wasm(137, 133))
        with self.assertRaisesRegex(ValueError, "identity"):
            fixture.write_bindings(inputs / "policy.scale", inputs / "operators.scale", inputs / "final.wasm", None, 7, 11, self.root / "bad")


if __name__ == "__main__":
    unittest.main()
