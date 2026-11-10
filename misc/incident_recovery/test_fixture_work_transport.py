"""Large fixture transport keeps every byte bound without widening review JSON."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import fixture_work_transport as transport
import hashi_codec as scale
import prepare_maintenance_fixture as maintenance
import release_package as release
from test_release_package import policy, hx, wasm


def report():
    return {"schema": "sora-nexus-storage-work-v1", "productionReady": False,
            "storageEngineRehearsed": True, "policyScale": "0x" + policy().hex(),
            "policyCommitment": "0x" + release.policy_identity(policy())["commitment"],
            "applyEnvelopes": ["0x01", "0x0203"], "auditEnvelopes": ["0x04"],
            "terminalEnvelope": "0x05", "changedKeys": 2,
            "evidence": {"complete": True}}


class TransportTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def bundle(self, document=None):
        source = self.root / "source.json"
        source.write_text(json.dumps(report() if document is None else document))
        summary = transport.pack(source, self.root / "bundle")
        path = Path(summary["index"])
        return path, scale.read_json(path), summary

    def test_exact_streamed_roundtrip_and_reiterable_envelopes(self):
        # Tiny reads deliberately split strings, numbers and JSON delimiters.
        with patch.object(transport, "READ_BYTES", 7):
            path, index, summary = self.bundle()
            loaded = transport.load(path, index)
        work, envelopes, expectations = maintenance.work_inputs(loaded)
        self.assertEqual(list(envelopes), report()["applyEnvelopes"])
        self.assertEqual(list(envelopes), report()["applyEnvelopes"])
        self.assertEqual(list(work["auditEnvelopes"]), report()["auditEnvelopes"])
        self.assertEqual(work["policyScale"], report()["policyScale"])
        self.assertIsNone(expectations)
        self.assertFalse(loaded["victimAuthorizationVerified"])
        self.assertFalse(summary["productionReady"])
        self.assertEqual(summary["nativeReport"]["sha256"], hashlib.sha256((self.root / "source.json").read_bytes()).hexdigest())
        with self.assertRaisesRegex(ValueError, "must be new"):
            transport.pack(self.root / "source.json", self.root / "bundle")

    def test_terminal_pages_roundtrip_and_omission_are_bound(self):
        document = report()
        document['terminalPreludeEnvelopes'] = ['0x0b0000', '0x0b0100']
        path, index, _ = self.bundle(document)
        loaded = transport.load(path, index)
        self.assertEqual(list(loaded['work']['terminalPreludeEnvelopes']), document['terminalPreludeEnvelopes'])
        wrong = copy.deepcopy(index)
        wrong['terminalPreludeEnvelopes'].pop()
        with self.assertRaisesRegex(ValueError, 'Missing indexed'):
            transport.load(path, wrong)
        wrong = copy.deepcopy(index)
        wrong['terminalPreludeEnvelopes'].reverse()
        with self.assertRaisesRegex(ValueError, 'differs from native report'):
            transport.load(path, wrong)

    def test_shard_tamper_is_rejected_even_after_initial_load(self):
        path, index, _ = self.bundle()
        loaded = transport.load(path, index)
        shard = path.parent / index["applyEnvelopes"][0]["file"]
        shard.write_bytes(b"\x09")
        with self.assertRaisesRegex(ValueError, "digest mismatch"):
            list(loaded["work"]["applyEnvelopes"])
        # Updating the index digest cannot change the retained native report.
        index["applyEnvelopes"][0]["sha256"] = hashlib.sha256(b"\x09").hexdigest()
        with self.assertRaisesRegex(ValueError, "differs from native report"):
            transport.load(path, index)

    def test_metadata_counts_paths_and_flags_cannot_be_changed(self):
        path, index, _ = self.bundle()
        for change, message in [
            (lambda d: d["metadata"].update(changedKeys=3), "metadata differs"),
            (lambda d: d["auditEnvelopes"].append(d["auditEnvelopes"][0]), "Extra indexed"),
            (lambda d: d["applyEnvelopes"].pop(), "Missing indexed"),
            (lambda d: d["applyEnvelopes"][0].update(file="../source.json"), "artifact path"),
            (lambda d: d.update(productionReady=True), "public fixture transport"),
            (lambda d: d.update(extra="unchecked"), "public fixture transport"),
        ]:
            changed = copy.deepcopy(index)
            change(changed)
            with self.assertRaisesRegex(ValueError, message):
                transport.load(path, changed)

    def test_complete_report_is_bound_and_duplicate_keys_are_rejected(self):
        path, index, _ = self.bundle()
        source = path.parent / index["nativeReport"]["file"]
        original = source.read_bytes()
        source.write_bytes(original + b" ")
        with self.assertRaisesRegex(ValueError, "size mismatch"):
            transport.load(path, index)
        for contents in [original[:-1] + b', "changedKeys": 3}',
                         original.replace(b'"complete": true', b'"complete": true, "complete": false'),
                         original + b"{}", original.replace(b'"changedKeys": 2', b'"changedKeys": NaN')]:
            source.write_bytes(contents)
            with self.assertRaises(ValueError):
                transport.parse_report(source, lambda *_: None)

    def test_bounds_empty_work_and_generic_json_limit(self):
        path, index, _ = self.bundle()
        with patch.object(transport, "MAX_REPORT_BYTES", 10):
            with self.assertRaisesRegex(ValueError, "size mismatch"):
                transport.load(path, index)
        source = self.root / "source.json"
        source.write_text('{"' + 'a' * 1025 + '": 1}')
        with self.assertRaisesRegex(ValueError, "value exceeds bound"):
            transport.parse_report(source, lambda *_: None)
        empty = report()
        empty["applyEnvelopes"] = []
        source.write_text(json.dumps(empty))
        with self.assertRaisesRegex(ValueError, "Incomplete fixture"):
            transport.parse_report(source, lambda *_: None)
        # Dedicated transport does not change the general-purpose reader.
        with patch.object(Path, "read_bytes", return_value=b" " * (64 * 1024 * 1024 + 1)):
            with self.assertRaisesRegex(ValueError, "JSON input exceeds bound"):
                scale.read_json(source)

    def test_array_item_and_total_counts_are_bounded(self):
        source = self.root / "source.json"
        source.write_text(json.dumps(report()))
        with patch.object(transport, "MAX_ENVELOPES", 1):
            with self.assertRaisesRegex(ValueError, "Too many"):
                transport.parse_report(source, lambda *_: None)
        with patch.object(transport, "MAX_ENVELOPE_BYTES", 1):
            with self.assertRaisesRegex(ValueError, "bound"):
                transport.parse_report(source, lambda *_: None)

    def test_maintenance_manifest_consumes_shards_and_preserves_native_hash(self):
        path, index, _ = self.bundle()
        (self.root / "checkpoint").mkdir()
        (self.root / "checkpoint/capture-complete.json").write_text("{}")
        consensus = self.root / "consensus.json"
        consensus.write_text(json.dumps({"checkpointHash": scale.CHECKPOINT_HASH, "stateRoot": release.CHECKPOINT_ROOT,
            "validators": [hx(2)], "records": [{"storage": "Grandpa.Authorities", "value": [[hx(8), 1]]}],
            "checkpointSlot": 298823260, "checkpointTimestamp": 1792939560002, "previousGrandpaSet": 7956}))
        operators = self.root / "operators.json"
        operators.write_text(json.dumps([{"owner": hx(2), "retainedGrandpa": [hx(8)], "fixtureOnly": True, "operatorApproved": False}]))
        paths = []
        for spec, tx in [(134, 131), (135, 132), (138, 134)]:
            runtime = self.root / f"runtime-{spec}.wasm"
            runtime.write_bytes(wasm(spec, tx))
            paths.append(runtime)
        result = maintenance.prepare(self.root, self.root / "rehearsal", path, consensus, operators, *paths)
        self.assertEqual(result["compiledWorkSha256"], index["nativeReport"]["sha256"])
        self.assertEqual(result["workTransportSchema"], transport.SCHEMA)
        manifest = scale.read_json(self.root / result["manifest"])
        payloads = [(self.root / step["recovery"]["file"]).read_bytes().hex() for step in manifest["steps"][:3]]
        self.assertEqual(payloads, ["01", "0203", "04"])
        self.assertEqual([s["expectedPhase"] for s in manifest["steps"][:3]], ["Applying", "Auditing", "Auditing"])
        self.assertFalse(result["productionReady"])


if __name__ == "__main__":
    unittest.main()
