"""Offline incoming coverage binding tests; no source claim proves finality."""
import copy
import json
import unittest

import hashi_codec as scale
import incoming_release as incoming
import release_package as release
import test_release_package as fixtures


class IncomingReleaseTests(unittest.TestCase):
    setUp = fixtures.ReleaseTests.setUp
    artifact = fixtures.ReleaseTests.artifact
    digest = fixtures.ReleaseTests.digest
    write_candidate = fixtures.ReleaseTests.write_candidate
    full_fixture = fixtures.ReleaseTests.full_fixture

    def receipt(self, report):
        return {"schema": "sora-nexus-incoming-coverage-verification-v1",
                "checkpointHash": scale.CHECKPOINT_HASH, "checkpointStateRoot": release.CHECKPOINT_ROOT,
                "policyCommitment": "0x" + report["policy"]["commitment"],
                **{field: self.inventory[role]["sha256"] for field, role in (
                    ("captureManifestSha256", "checkpointCaptureManifest"), ("policySha256", "policyScale"),
                    ("incomingWorkSha256", "recoveryWorkIndex"), ("incomingInventorySha256", "incomingDeliveryInventory"),
                    ("incomingReconciliationSha256", "incomingReconciliation"), ("externalFinalizedHeadsSha256", "externalFinalizedHeads"))},
                **{field: True for field in ("localPendingCoverageVerified", "declaredExternalInventoryCoverageVerified", "workCoverageVerified")},
                **{field: False for field in ("externalArchiveCompletenessVerified", "externalFinalityVerified", "authenticationVerified", "exactArtifactExecutionVerified", "productionReady")},
                "evidenceHashes": ["0x" + self.inventory["externalFinalizedHeads"]["blake2_256"]],
                "checkpointMetadata": {"schema": "sora-nexus-checkpoint-metadata-witness-v1",
                    "checkpointHash": scale.CHECKPOINT_HASH, "stateRoot": release.CHECKPOINT_ROOT,
                    "wasmExecutionVerified": True, "nativeExecution": False}}

    def test_incoming_shards_are_bound_and_partial_policy_stream_is_rejected(self):
        self.full_fixture()
        report, _ = release.verify_candidate(self.candidate)
        self.assertEqual(sum(row["role"].startswith("incomingWorkShard-") for row in report["artifacts"]), 3)
        path = self.root / "work/index.json"
        index = json.loads(path.read_text())
        index["work"].append(index["work"][0])
        self.artifact("recoveryWorkIndex", index, "work/index.json")
        with self.assertRaisesRegex(ValueError, "Incomplete policy"):
            release.verify_candidate(self.write_candidate())

    def test_incoming_shard_mutation_missing_file_and_path_traversal_are_rejected(self):
        self.full_fixture()
        shard = self.root / "work/work.scale"
        old = shard.read_bytes()
        shard.write_bytes(b"wrong")
        with self.assertRaisesRegex(ValueError, "shard hash/size"):
            release.verify_candidate(self.candidate)
        shard.write_bytes(old)
        index = json.loads((self.root / "work/index.json").read_text())
        index["work"][0]["file"] = "../policyScale"
        self.artifact("recoveryWorkIndex", index, "work/index.json")
        with self.assertRaisesRegex(ValueError, "must stay under"):
            release.verify_candidate(self.write_candidate())

    def test_incoming_receipt_binds_every_raw_input_and_bundled_evidence(self):
        self.full_fixture()
        report, paths = release.verify_candidate(self.candidate)
        bindings = {row["role"]: row["sha256"] for row in report["artifacts"]}
        receipt = self.receipt(report)
        self.assertTrue(incoming.valid_receipt(receipt, report, bindings, release.CHECKPOINT_ROOT))
        for field in ("captureManifestSha256", "policySha256", "incomingWorkSha256", "incomingInventorySha256", "incomingReconciliationSha256", "externalFinalizedHeadsSha256"):
            altered = copy.deepcopy(receipt); altered[field] = "ff" * 32
            self.assertFalse(incoming.valid_receipt(altered, report, bindings, release.CHECKPOINT_ROOT), field)
        receipt["evidenceHashes"] = ["0x" + "ee" * 32]
        self.assertFalse(incoming.valid_receipt(receipt, report, bindings, release.CHECKPOINT_ROOT))
        paths.pop("incomingDeliveryInventory")
        self.assertIsNone(incoming.arguments(paths))

    def test_incoming_coverage_never_claims_bridge_finality_or_overall_readiness(self):
        self.full_fixture()
        report, _ = release.verify_candidate(self.candidate)
        bindings = {row["role"]: row["sha256"] for row in report["artifacts"]}
        for field in ("externalArchiveCompletenessVerified", "externalFinalityVerified", "authenticationVerified", "exactArtifactExecutionVerified", "productionReady"):
            receipt = self.receipt(report); receipt[field] = True
            self.assertFalse(incoming.valid_receipt(receipt, report, bindings, release.CHECKPOINT_ROOT), field)
        self.assertEqual(report["gates"]["bridgeReconciliation"]["status"], "unverified")
        self.assertFalse(report["productionReady"])


if __name__ == "__main__":
    unittest.main()
