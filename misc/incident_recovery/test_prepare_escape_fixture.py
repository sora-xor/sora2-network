import json
from pathlib import Path
import tempfile
import unittest

import hashi_codec as scale
import prepare_escape_fixture as escape
import release_package as release
from test_release_package import h, hx, wasm


class EscapeFixtureTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.operators = self.root / "operators.scale"
        self.operators.write_bytes(b'snxops02' + scale.compact(1) + h(2) + scale.compact(1) + h(8) + scale.compact(1) + h(10) + scale.compact(1) + h(11))
        self.final = self.root / "unused-final.wasm"
        self.final.write_bytes(wasm(138, 134))
        self.inputs = self.root / "inputs"
        self.report = escape.inputs(self.inputs, self.operators, self.final, 7956, 298823261)

    def test_test_council_is_explicit_and_revised_work_keeps_same_authority(self):
        self.assertEqual(self.report["publicCouncil"], [
            "0x3d0bfb1e8c2639da9165a9b841eff0322a91e4726f5b3cfbbab33ae237248cb6",
            "0xa2384b2633793ee7124bd13b0dfaab6b5c820085401e80fed28b415381f99b7c",
            "0xcc75822e1a79475ca9b33bb1e9a6d331032e0b89733524d5881d4096c48649e3",
        ])
        self.assertFalse(self.report["automaticActivationAllowed"])
        self.assertFalse(self.report["governanceStateReplaced"])
        self.assertFalse(self.report["productionReady"])
        original = (self.inputs / "original-policy.scale").read_bytes()
        revised = (self.inputs / "revised-policy.scale").read_bytes()
        self.assertNotEqual(release.policy_identity(original)["commitment"], release.policy_identity(revised)["commitment"])
        binding = json.loads((self.inputs / "repair-136-binding.json").read_text())
        self.assertEqual(scale.raw(binding["originalPolicyScale"]), original)
        self.assertEqual(scale.raw(binding["policyScale"]), revised)
        self.assertNotIn("operationCounter", binding)
        with self.assertRaisesRegex(ValueError, "new directory"):
            escape.inputs(self.inputs, self.operators, self.final, 7956, 298823261)

    def test_manifest_preserves_timestamp_offset_and_leaves_signatures_dynamic(self):
        consensus = self.root / "consensus.json"
        consensus.write_text(json.dumps({"checkpointHash": scale.CHECKPOINT_HASH, "stateRoot": release.CHECKPOINT_ROOT,
            "validators": [hx(2)], "records": [{"storage": "Grandpa.Authorities", "value": [[hx(8), 1]]}],
            "checkpointSlot": 298823260, "checkpointTimestamp": 1792939560002, "previousGrandpaSet": 7956}))
        operators = self.root / "operators.json"
        operators.write_text(json.dumps([{"owner": hx(2), "retainedGrandpa": [hx(8)], "fixtureOnly": True, "operatorApproved": False}]))
        paths = []
        for spec, tx in ((134, 131), (135, 132), (136, 132)):
            path = self.root / f"runtime-{spec}.wasm"
            path.write_bytes(wasm(spec, tx, b"x" * 140000 if spec == 136 else b""))
            paths.append(path)
        report = escape.manifest(self.root, self.root / "diagnostic", self.inputs, consensus, operators, *paths, self.final)
        manifest = json.loads(Path(report["manifest"]).read_text())
        self.assertEqual(manifest["schema"], escape.MANIFEST_SCHEMA)
        self.assertTrue(all(step["timestamp"] == step["slot"] * 6000 + 2 for step in manifest["steps"]))
        dynamic = [step for step in manifest["steps"] if step["fixtureEscape"]]
        self.assertEqual([step["fixtureEscape"]["destinationSpec"] for step in dynamic], [135, 136])
        self.assertTrue(all(step["recovery"] is None for step in dynamic))
        self.assertEqual(manifest["steps"][-1]["expectedPhase"], "Auditing")
        self.assertTrue(all(step["expectedSpec"] != 138 for step in manifest["steps"]))
        self.assertGreater(len(manifest["steps"]), 7)
        self.assertNotIn("signatures", json.dumps(manifest))

    def test_modified_inputs_cannot_be_relabelled_as_escape_fixture(self):
        (self.inputs / "revised-policy.scale").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "Changed escape fixture input"):
            escape.manifest(self.root, self.root / "bad", self.inputs, self.root / "absent", self.root / "absent", self.final, self.final, self.final, self.final)
        self.assertFalse((self.root / "bad").exists())


if __name__ == "__main__":
    unittest.main()
