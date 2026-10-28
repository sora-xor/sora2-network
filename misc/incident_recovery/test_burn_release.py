"""Synthetic orchestration tests; mocked verifier output is never release evidence."""
import copy
import hashlib
import gzip
import json
from pathlib import Path
import subprocess
import shutil
import tempfile
import unittest
from unittest.mock import patch

import burn_release as burn
import hashi_codec as scale
import release_package as release
from test_release_package import policy, hx


class BurnReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.paths = {}
        self.report = {"productionReady": False, "artifacts": [],
                       "gates": {gate: {"status": "unverified"} for gate in release.GATES}}
        self.put("policyScale", policy())
        self.put("signedMappingCsv", f'"old account address","new account address"\n"{hx(1)}","{hx(2)}"\n'.encode())
        self.allocation = {"version": 1, "policyHash": "0x" + hashlib.blake2b(policy(), digest_size=32).hexdigest(),
                           "genesisHash": scale.GENESIS_HASH, "checkpoint": {"block": scale.CHECKPOINT, "hash": scale.CHECKPOINT_HASH},
                           "substitutions": [{"old": hx(1), "new": hx(2)}], "attackerAccounts": [scale.ATTACKER_SORA]}
        self.put("burnAllocationPolicy", self.allocation)
        self.put("attackerCoverageEvidence", {"schema": "sora-nexus-attacker-typed-coverage-v1",
                                             "attackerRegistry": [scale.ATTACKER_SORA, scale.ATTACKER_EVM]})
        for role in ["burnNodeBinary", "burnNativeVerifier", "burnRecoveryVerifier"]:
            self.put(role, ("synthetic unexecuted fixture " + role).encode())
        self.put("burnRecoveryVerifierProvenance", {"schema": "sora-nexus-bundled-burn-verifier-v1",
            "artifact": {"sha256": self.sha("burnRecoveryVerifier"), "bytes": self.paths["burnRecoveryVerifier"].stat().st_size},
            "inputs": [{"file": "fixture", "sha256": "1" * 64, "bytes": 1}]})
        self.put("burnHistoryManifest", {"schema": "sora-nexus-burn-proof-stream-v1", "first": burn.FIRST,
            "last": scale.CHECKPOINT, "checkpointHash": scale.CHECKPOINT_HASH, "initial": {}, "pages": []})
        self.put("burnEntitlements", b'{"syntheticLedger":true}\n')

    def put(self, role, data):
        if not isinstance(data, bytes):
            data = json.dumps(data, sort_keys=True).encode()
        path = self.root / role
        path.write_bytes(data)
        self.paths[role] = path
        self.report["artifacts"] = [row for row in self.report["artifacts"] if row["role"] != role]
        actual = release.digest_file(path)
        self.report["artifacts"].append({"role": role, **actual, "object": "objects/" + actual["sha256"],
                                         "sourcePath": role, "executable": False})

    def sha(self, role):
        return release.digest_file(self.paths[role])["sha256"]

    def invoke(self, **override):
        args = dict(native=self.paths["burnNativeVerifier"], native_sha256=self.sha("burnNativeVerifier"),
                    node=self.paths["burnNodeBinary"], node_sha256=self.sha("burnNodeBinary"),
                    bundle_sha256=self.sha("burnRecoveryVerifier"), output=self.root / "verification")
        args.update(override)
        return burn.verify(self.report, self.paths, **args)

    def fresh_mock(self, binary, arguments, timeout):
        self.assertEqual(binary, self.paths["burnNodeBinary"].resolve())
        self.assertEqual(arguments[0], str(self.paths["burnRecoveryVerifier"]))
        flags = dict(zip(arguments[1::2], arguments[2::2]))
        self.assertEqual(set(flags), {"--verifier", "--verifier-sha256", "--manifest", "--policy", "--policy-sha256", "--expected-ledger", "--output"})
        self.assertEqual(flags["--verifier-sha256"], self.sha("burnNativeVerifier"))
        output = Path(flags["--output"])
        output.mkdir()
        (output / "verified-history").mkdir()
        (output / "canonical-receipts.json").write_bytes(b"synthetic captured events")
        page = gzip.compress(json.dumps({"schema": "sora-nexus-verified-burn-page-v1", "first": burn.FIRST,
                                         "records": [{"synthetic": True}]}).encode(), mtime=0)
        (output / "verified-history/page.json.gz").write_bytes(page)
        (output / "verified-history/metadata.scale").write_bytes(b"synthetic metadata")
        native = {"schema": "sora-nexus-verified-burn-stream-v1", "first": burn.FIRST,
                  "last": scale.CHECKPOINT, "checkpointHash": scale.CHECKPOINT_HASH,
                  "pages": [{"file": "page.json.gz", "sha256": hashlib.sha256(page).hexdigest()}],
                  "metadata": [{"codeHash": hx(3), "specVersion": 134,
                                "metadata": {"file": "metadata.scale",
                                "sha256": hashlib.sha256(b"synthetic metadata").hexdigest()}}]}
        (output / "verified-history/verification-complete.json").write_text(json.dumps(native))
        (output / "rederived-ledger.json").write_bytes(self.paths["burnEntitlements"].read_bytes())
        result = {"schema": "sora-nexus-burn-release-verification-v1",
                  "checkpoint": self.allocation["checkpoint"], "sourceManifestSha256": self.sha("burnHistoryManifest"),
                  "allocationPolicySha256": self.sha("burnAllocationPolicy"), "expectedLedgerSha256": self.sha("burnEntitlements"),
                  "rederivedLedgerSha256": self.sha("burnEntitlements"), "nativeVerifierSha256": self.sha("burnNativeVerifier"),
                  "canonicalReceiptsSha256": release.digest_file(output / "canonical-receipts.json")["sha256"],
                  "nativeVerificationSha256": release.digest_file(output / "verified-history/verification-complete.json")["sha256"],
                  **{field: True for field in ("completeHistoryFreshlyVerified", "sharedEventExtractorExecuted", "exactAllocationRederived",
                       "ledgerBytesIdentical", "replacementOwnershipAppliedAfterAllocation", "attackerCurvePreserved", "mappedNexusRecipientsHeld")},
                  "consensusFinalityVerified": False, "claimServiceDeploymentVerified": False, "productionReady": False}
        encoded = json.dumps(result).encode()
        (output / "burn-verification-complete.json").write_bytes(encoded)
        return subprocess.CompletedProcess(arguments, 0, encoded, b"")

    def test_fixed_fresh_workflow_clears_only_burn_gate(self):
        with patch.object(burn, "execute", side_effect=self.fresh_mock) as execute:
            receipt = self.invoke()
        self.assertEqual(execute.call_count, 1)
        self.assertEqual(receipt["inputSha256"]["policyScale"], self.sha("policyScale"))
        self.assertEqual(self.report["gates"][burn.GATE]["status"], "verified")
        self.assertTrue(all(row["status"] == "unverified" for gate, row in self.report["gates"].items() if gate != burn.GATE))
        self.assertFalse(self.report["productionReady"])
        generated = [row for row in self.report["artifacts"] if row["role"].startswith(burn.GENERATED_PREFIX)]
        self.assertGreaterEqual(len(generated), 10)
        self.assertTrue(all(row["sourcePath"].startswith(burn.GENERATED_PATH) for row in generated))
        for row in generated:
            self.assertEqual(release.digest_file(self.paths[row["role"]])["sha256"], row["sha256"])
        import archive_verified_burn_evidence as archive
        restored = self.root / "restored-fresh-evidence"
        archive.restore(self.paths["burnVerifiedHistoryArchive"], restored)
        self.assertEqual((restored / "metadata.scale").read_bytes(), b"synthetic metadata")
        self.assertEqual((restored / "page.json.gz").read_bytes(), (self.root / "verification/fresh-execution/verified-history/page.json.gz").read_bytes())

    def test_mutated_or_omitted_mapping_and_attackers_reject_before_execution(self):
        for field, value in [("substitutions", []), ("attackerAccounts", []), ("policyHash", hx(9))]:
            with self.subTest(field=field):
                data = copy.deepcopy(self.allocation); data[field] = value
                self.put("burnAllocationPolicy", data)
                with patch.object(burn, "execute") as execute, self.assertRaises(ValueError):
                    self.invoke()
                execute.assert_not_called()

    def test_live_policy_cannot_introduce_circular_release_hash(self):
        self.put("burnAllocationPolicy", {**self.allocation, "releaseManifestHash": hx(7)})
        with self.assertRaisesRegex(ValueError, "circular"):
            self.invoke()

    def test_supplied_pass_flags_and_existing_reports_cannot_clear_gate(self):
        self.put("burnEntitlements", {"allChecksPass": True, "productionReady": True})
        def fail(binary, args, timeout):
            return subprocess.CompletedProcess(args, 1, b"", b"fresh rederivation rejected ledger")
        with patch.object(burn, "execute", side_effect=fail), self.assertRaisesRegex(ValueError, "Fresh complete"):
            self.invoke()
        self.assertEqual(self.report["gates"][burn.GATE]["status"], "unverified")
        with self.assertRaisesRegex(ValueError, "new directory"):
            self.invoke()

    def test_wrong_executable_pin_and_incomplete_history_rejected(self):
        with self.assertRaisesRegex(ValueError, "pin differs"):
            self.invoke(bundle_sha256="0" * 64)
        history = json.loads(self.paths["burnHistoryManifest"].read_text())
        history["first"] = scale.CHECKPOINT
        self.put("burnHistoryManifest", history)
        with patch.object(burn, "execute") as execute, self.assertRaisesRegex(ValueError, "diagnostic"):
            self.invoke()
        execute.assert_not_called()

    def test_fresh_receipt_and_rederived_ledger_must_match_admitted_inputs(self):
        def corrupt(binary, arguments, timeout):
            result = self.fresh_mock(binary, arguments, timeout)
            output = Path(dict(zip(arguments[1::2], arguments[2::2]))["--output"])
            (output / "rederived-ledger.json").write_bytes(b"different allocations")
            return result
        with patch.object(burn, "execute", side_effect=corrupt), self.assertRaisesRegex(ValueError, "digest mismatch"):
            self.invoke()

    def test_candidate_mutation_during_fresh_execution_is_rejected(self):
        def mutate(binary, arguments, timeout):
            result = self.fresh_mock(binary, arguments, timeout)
            self.paths["signedMappingCsv"].write_text("changed during verification")
            return result
        with patch.object(burn, "execute", side_effect=mutate), self.assertRaisesRegex(ValueError, "changed after candidate"):
            self.invoke()

    def test_native_derived_graph_cannot_omit_or_mutate_metadata(self):
        def mutate(binary, arguments, timeout):
            result = self.fresh_mock(binary, arguments, timeout)
            output = Path(dict(zip(arguments[1::2], arguments[2::2]))["--output"])
            (output / "verified-history/metadata.scale").write_bytes(b"changed metadata")
            return result
        with patch.object(burn, "execute", side_effect=mutate), self.assertRaisesRegex(ValueError, "SHA256 mismatch"):
            self.invoke()
        self.assertEqual(self.report["gates"][burn.GATE]["status"], "unverified")

    def test_generated_evidence_cannot_overwrite_user_roles(self):
        self.report["artifacts"].append({"role": "burnVerifiedNativeReceipt", "sha256": "1" * 64})
        with patch.object(burn, "execute", side_effect=self.fresh_mock), self.assertRaisesRegex(ValueError, "Reserved generated"):
            self.invoke()

    def package(self):
        return release.package(self.root / "candidate.json", self.root / "bundle",
            verifier=self.paths["burnNativeVerifier"], verifier_sha256=self.sha("burnNativeVerifier"),
            burn_node=self.paths["burnNodeBinary"], burn_node_sha256=self.sha("burnNodeBinary"),
            burn_verifier_sha256=self.sha("burnRecoveryVerifier"), burn_verification_output=self.root / "verification")

    def test_outer_bundle_contains_all_fresh_evidence_objects(self):
        with patch.object(release, "verify_candidate", return_value=(self.report, self.paths)), \
             patch.object(release, "run_evidence_verifier"), patch.object(burn, "execute", side_effect=self.fresh_mock):
            report = self.package()
        for row in report["artifacts"]:
            self.assertEqual(release.digest_file(self.root / "bundle" / row["object"])["sha256"], row["sha256"])
        self.assertTrue((self.root / "bundle/PACKAGE-COMPLETE").is_file())
        self.assertFalse(report["productionReady"])

    def test_fresh_evidence_tampering_during_outer_copy_rejects_package(self):
        copyfile = shutil.copyfile
        def mutate(source, target):
            if source == self.paths.get("burnVerifiedHistoryShard00000"):
                source.write_bytes(source.read_bytes() + b"changed after fresh verification")
            return copyfile(source, target)
        with patch.object(release, "verify_candidate", return_value=(self.report, self.paths)), \
             patch.object(release, "run_evidence_verifier"), patch.object(burn, "execute", side_effect=self.fresh_mock), \
             patch.object(release.shutil, "copyfile", side_effect=mutate), self.assertRaisesRegex(ValueError, "changed while packaging"):
            self.package()
        self.assertFalse((self.root / "bundle/PACKAGE-COMPLETE").exists())


if __name__ == "__main__":
    unittest.main()
