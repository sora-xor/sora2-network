import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import prepare_release_snapshot as snapshot
from prepare_release_recovery_config import ss58


class SnapshotTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()

    def tearDown(self):
        self.temp.cleanup()

    def file(self, name, raw=b"public fixture"):
        path = self.root / name
        path.write_bytes(raw)
        return path

    def test_owned_inputs_are_independent_and_refuse_overwrite(self):
        source = self.file("source")
        owned = snapshot.OwnedInputs(self.root / "owned")
        target = owned.add("node", source, True)
        self.assertNotEqual(source.stat().st_ino, target.stat().st_ino)
        self.assertEqual(target.stat().st_mode & 0o777, 0o500)
        owned.guard()
        with self.assertRaises(FileExistsError):
            owned.add("node", source)

    def test_source_rebuild_after_execution_cannot_get_a_new_receipt_hash(self):
        source = self.file("source")
        owned = snapshot.OwnedInputs(self.root / "owned")
        owned.add("node", source)
        source.write_bytes(b"different replacement binary")
        with self.assertRaisesRegex(ValueError, "bound input changed"):
            owned.guard()

    def test_owned_input_mutation_is_rejected(self):
        source = self.file("source")
        owned = snapshot.OwnedInputs(self.root / "owned")
        target = owned.add("node", source)
        target.chmod(0o600)
        target.write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "bound input changed"):
            owned.guard()

    def test_symlink_and_hardlink_inputs_are_rejected(self):
        source = self.file("source")
        link = self.root / "link"
        link.symlink_to(source)
        with self.assertRaises(ValueError):
            snapshot.regular(link, 100)
        link.unlink()
        os.link(source, link)
        with self.assertRaises(ValueError):
            snapshot.regular(source, 100)

    def inventory(self):
        directory = self.root / "database"
        directory.mkdir()
        (directory / "000001.sst").write_bytes(b"historical body and auxiliary data")
        return directory, {"000001.sst": {"size": (directory / "000001.sst").stat().st_size,
                                        "sha256": snapshot.sha(directory / "000001.sst")}}

    def test_exact_reconstruction_inventory_is_required(self):
        directory, rows = self.inventory()
        snapshot.verify_database_inventory(directory, rows)
        (directory / "foreign").write_bytes(b"unlisted")
        with self.assertRaisesRegex(ValueError, "inventory differs"):
            snapshot.verify_database_inventory(directory, rows)
        (directory / "foreign").unlink()
        (directory / "000001.sst").unlink()
        with self.assertRaisesRegex(ValueError, "inventory differs"):
            snapshot.verify_database_inventory(directory, rows)

    def test_modified_historical_record_is_rejected_even_with_unchanged_tip(self):
        directory, rows = self.inventory()
        path = directory / "000001.sst"
        path.write_bytes(b"x" * rows[path.name]["size"])
        with self.assertRaisesRegex(ValueError, "completed reconstruction"):
            snapshot.verify_database_inventory(directory, rows)

    def test_post_import_inventory_binds_archive_source_and_detects_later_changes(self):
        directory, expected = self.inventory()
        self.assertEqual(snapshot.database_inventory(directory), expected)
        (directory / "000001.sst").write_bytes(b"replaced after inspection")
        with self.assertRaisesRegex(ValueError, "completed reconstruction"):
            snapshot.verify_database_inventory(directory, expected)

    def test_archive_layout_excludes_network_keys_and_other_chains(self):
        base = self.root / "base"
        full = base / "chains" / snapshot.CHAIN_ID / "db" / "full"
        full.mkdir(parents=True)
        self.assertEqual(snapshot.snapshot_layout(base), full)
        unexpected = base / "chains" / snapshot.CHAIN_ID / "keystore"
        unexpected.mkdir()
        with self.assertRaisesRegex(ValueError, "archive layout"):
            snapshot.snapshot_layout(base)
        unexpected.rmdir()
        (base / "chains" / "another-chain").mkdir()
        with self.assertRaisesRegex(ValueError, "archive layout"):
            snapshot.snapshot_layout(base)

    def test_reconstruction_is_bound_to_existing_native_completion(self):
        path = self.root / "RECONSTRUCTION.json"
        value = {"schema": "sora-nexus-retained-predecessor-reconstruction-v1",
                 "bestNumber": snapshot.CHECKPOINT_NUMBER - 1, "bestHash": snapshot.PARENT_HASH,
                 "grandpaCurrentSetId": 7956, **{k: True for k in (
                     "checkpointAbsent", "clientDatabaseReconstructed", "fullCanonicalHistoryPersisted",
                     "parentStateReopenedAndCompletelyCompared", "noOldGrandpaVoteStateCopied", "noOldKeysCopied")}}
        snapshot.write_json(path, value)
        with self.assertRaisesRegex(ValueError, "completed native evidence"):
            snapshot.reconstruction(self.root)
        with patch.object(snapshot, "RECONSTRUCTION_SHA256", snapshot.sha(path)):
            self.assertEqual(snapshot.reconstruction(self.root), value)

    def admission(self, journal=None):
        row = {"schema": "sora-nexus-passive-node-observation-v1", "pid": 77, "sequence": 0,
               "kind": "database-admitted", "bestHash": snapshot.scale.CHECKPOINT_HASH,
               "bestNumber": snapshot.CHECKPOINT_NUMBER, "finalizedHash": snapshot.PARENT_HASH,
               "finalizedNumber": snapshot.CHECKPOINT_NUMBER - 1,
               "checkpointNumberIndexHash": snapshot.scale.CHECKPOINT_HASH,
               "checkpointHeaderPresentByHash": True, "checkpointImportPolicy": "0x" + "11"*32}
        if journal:
            snapshot.write_json(journal / "00000000.json", row)
        return row

    def test_native_admission_reports_actual_finality_and_policy(self):
        journal = self.root / "journal"
        journal.mkdir()
        row = self.admission(journal)
        self.assertEqual(snapshot.admitted_client(journal, 77, row["checkpointImportPolicy"]), row)
        for key, bad in [("finalizedNumber", snapshot.CHECKPOINT_NUMBER), ("finalizedHash", "0x"+"00"*32),
                         ("checkpointImportPolicy", "0x"+"22"*32), ("pid", 78)]:
            changed = {**row, key: bad}
            (journal / "00000000.json").write_text(json.dumps(changed))
            with self.assertRaises(ValueError):
                snapshot.admitted_client(journal, 77, row["checkpointImportPolicy"])

    def test_construction_log_alone_is_never_admission(self):
        self.file("node.log", b"Starting new voter with set ID 7957\n")
        self.assertIsNone(snapshot.admitted_client(self.root / "missing-journal", 77, "0x" + "11"*32))

    def test_partial_journal_waits_and_sequence_gaps_fail(self):
        journal = self.root / "journal"
        journal.mkdir()
        (journal / "00000000.json").write_text('{"schema":')
        self.assertIsNone(snapshot.admitted_client(journal, 77, "0x" + "11"*32))
        (journal / "00000000.json").rename(journal / "00000001.json")
        with self.assertRaisesRegex(ValueError, "gap"):
            snapshot.admitted_client(journal, 77, "0x" + "11"*32)

    def inspection(self, output, database):
        output.mkdir()
        artifacts = {}
        for name in ["checkpoint-header.scale", "parent-header.scale", "grandpa-voters.scale",
                     "grandpa-rounds.scale", "babe-epochs.scale"]:
            path = output / name
            path.write_bytes(name.encode())
            artifacts[name] = {"file": name, "bytes": path.stat().st_size, "sha256": snapshot.sha(path)}
        return {"schema": "sora-nexus-checkpoint-snapshot-inspection-v1", "database": str(database / "full"),
                "engineReadOnly": True, "genesisHash": snapshot.GENESIS,
                "bestHash": snapshot.scale.CHECKPOINT_HASH, "bestNumber": snapshot.CHECKPOINT_NUMBER,
                "finalizedHash": snapshot.PARENT_HASH, "finalizedNumber": snapshot.CHECKPOINT_NUMBER-1,
                "checkpointImportPolicy": "0x" + "11"*32, "grandpaSetId": 7957,
                "grandpaAuthorities": [["0x" + "01"*32, 1]], "savedRoundSetId": 7957,
                "freshRoundStateVerified": True, "artifacts": artifacts,
                "voterHealthVerified": False, "productionReady": False}

    def test_inspection_binds_raw_outputs_and_never_claims_voter_health(self):
        database, output = self.root / "db", self.root / "inspection"
        report = self.inspection(output, database)
        recovery = {"policyCommitment": report["checkpointImportPolicy"],
                    "grandpaAuthorities": [[ss58("01"*32), 1]]}
        def execute(command, log):
            self.assertEqual(command[1], "--inspect-checkpoint-snapshot")
            snapshot.write_json(log, report)
        with patch.object(snapshot, "run", execute):
            result = snapshot.inspect_snapshot(self.root / "tool", database, output, recovery)
            self.assertFalse(result["voterHealthVerified"])
        (self.root / "inspection.stdout.json").unlink()
        (output / "grandpa-rounds.scale").write_bytes(b"wrong rounds")
        with patch.object(snapshot, "run", execute), self.assertRaisesRegex(ValueError, "artifact changed"):
            snapshot.inspect_snapshot(self.root / "tool", database, output, recovery)

    def test_wrong_saved_rounds_or_authorities_cannot_be_asserted_away(self):
        database, output = self.root / "db", self.root / "inspection"
        report = self.inspection(output, database)
        recovery = {"policyCommitment": report["checkpointImportPolicy"],
                    "grandpaAuthorities": [[ss58("01"*32), 1]]}
        for key, bad in [("savedRoundSetId", 7956), ("freshRoundStateVerified", False),
                         ("grandpaAuthorities", [["0x"+"02"*32, 1]]), ("voterHealthVerified", True)]:
            changed = {**report, key: bad}
            def execute(command, log):
                log.write_text(json.dumps(changed))
            with patch.object(snapshot, "run", execute), self.assertRaises(ValueError):
                snapshot.inspect_snapshot(self.root / "tool", database, output, recovery)

    def test_evidence_closure_detects_changed_raw_bytes(self):
        evidence = self.root / "evidence"
        evidence.mkdir()
        (evidence / "raw.scale").write_bytes(b"observed")
        before = snapshot.evidence_inventory(evidence)
        (evidence / "raw.scale").write_bytes(b"changed")
        self.assertNotEqual(before, snapshot.evidence_inventory(evidence))


if __name__ == "__main__":
    unittest.main()
