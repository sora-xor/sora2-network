"""Offline release consistency tests; synthetic Wasm is never execution evidence."""
import copy
import hashlib
import gzip
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import hashi_codec as scale
import release_package as release


def h(byte):
    return bytes([byte]) * 32


def hx(byte):
    return "0x" + h(byte).hex()


def leb(value):
    result = bytearray()
    while value >= 128:
        result.append((value & 127) | 128)
        value >>= 7
    return bytes(result) + bytes([value])


def custom(name, data):
    body = leb(len(name)) + name.encode() + data
    return b"\0" + leb(len(body)) + body


def wasm(spec, transaction, suffix=b""):
    version = scale.vec(b"sora-substrate") + scale.vec(b"sora-substrate") + scale.u32(1) + scale.u32(spec) + scale.u32(1) + b"\0" + scale.u32(transaction) + b"\x01"
    return b"\0asm\x01\0\0\0" + custom("runtime_version", version) + custom("runtime_apis", b"api-test" + scale.u32(1)) + (custom("test", suffix) if suffix else b"")


def policy():
    return scale.u32(2) + scale.raw(scale.CHECKPOINT_HASH) + scale.raw(release.CHECKPOINT_ROOT) + scale.compact(1) + h(1) + h(2) + scale.compact(1) + h(3) + scale.compact(1) + h(4) + scale.compact(1) + h(5) + release.authorization_domain_scale({"chainSpecId": "sora-nexus-recovery-28191817", "releaseNonce": hx(8)}) + h(6) + h(7)


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.inventory = {}
        self.candidate = self.root / "candidate.json"

    def artifact(self, role, data, name=None):
        path = self.root / (name or role)
        path.parent.mkdir(parents=True, exist_ok=True)
        if isinstance(data, dict):
            data = json.dumps(data, sort_keys=True).encode()
        elif isinstance(data, str):
            data = data.encode()
        path.write_bytes(data)
        digest = release.digest_file(path)
        self.inventory[role] = {"role": role, "path": path.relative_to(self.root).as_posix(), **{key: digest[key] for key in ("sha256", "blake2_256")}}
        return path

    def digest(self, role):
        return {key: self.inventory[role][key] for key in ("sha256", "blake2_256")}

    def write_candidate(self, **extra):
        self.data = {"format": release.FORMAT, "checkpoint": {"number": scale.CHECKPOINT, "hash": scale.CHECKPOINT_HASH}, "nodeVersion": "4.8.13", "artifacts": list(self.inventory.values()), "evidence": [], **extra}
        self.candidate.write_text(json.dumps(self.data))
        return self.candidate

    def full_fixture(self):
        for role, versions in release.RUNTIME_ROLES.items():
            self.artifact(role, wasm(*versions))
        self.artifact("policyScale", policy())
        self.artifact("operatorsScale", scale.compact(1) + h(2) + scale.compact(1) + h(8) + h(9) + scale.compact(1) + h(10) + scale.compact(1) + h(11))
        self.artifact("stakingRoleReview", {"evidenceHash": hx(12),
                      "operatorsHash": "0x" + self.inventory["operatorsScale"]["blake2_256"],
                      "records": [{"warning": "synthetic inventory; no native verification"}]})
        self.artifact("signedMappingCsv", f'"old account address","new account address"\n"{hx(1)}","{hx(2)}"\n')
        self.artifact("semanticPlan", {"derivedAccounts": []})
        self.artifact("derivedAccountInventory", b"[]")
        policy_hash = hashlib.blake2b(policy(), digest_size=32).hexdigest()
        config = {"schemaVersion": 2, "policyScale": "0x" + policy().hex(), "checkpointHash": scale.CHECKPOINT_HASH, "policyCommitment": "0x" + policy_hash,
                  "evidenceCommitment": hx(6), "previousGrandpaSetId": 44, "previousGrandpaAuthorities": [[hx(8), 1]],
                  "grandpaAuthorities": [[hx(9), 1]], "retiredGrandpaKeys": [hx(8)], "approvedGrandpaKeys": [hx(9)],
                  "lastAbandonedSignedSlot": 100, "firstRecoverySlot": 101}
        for role, field in (("bootstrapWasm", "bootstrapCodeHash"), ("holdWasm", "holdCodeHash"), ("finalWasm", "finalCodeHash")):
            config[field] = "0x" + self.inventory[role]["blake2_256"]
        self.spec = {"id": "sora-nexus-recovery-28191817", "protocolId": "sora-nexus-recovery-28191817-v1", "forkId": "recovery-28191817-v1", "recovery": config,
                     "codeSubstitutes": {str(scale.CHECKPOINT): "0x" + wasm(134, 131).hex()}}
        self.artifact("chainSpec", self.spec)
        for spec, role in ((134, "bootstrapBinding"), (135, "holdBinding")):
            binding = {"policyScale": "0x" + policy().hex(), "policyCommitment": "0x" + policy_hash,
                       "operatorsScale": "0x" + (self.root / "operatorsScale").read_bytes().hex(), "previousGrandpaSetId": 44, "firstRecoverySlot": 101, "finalWasm": "finalWasm"}
            if spec == 134:
                binding["holdWasm"] = "holdWasm"
            self.artifact(role, binding)
            provenance = {"formatVersion": 1, "deployedSourceTree": release.LEGACY_TREE, "sdk": "polkadot-stable2603-3", "specVersion": spec,
                          "transactionVersion": 131 if spec == 134 else 132, "bound": True,
                          "bindingEvidence": {"binding": self.digest(role), "policy": self.digest("policyScale"), "operators": self.digest("operatorsScale"), "finalWasm": self.digest("finalWasm"), "holdWasm": self.digest("holdWasm") if spec == 134 else None}}
            self.artifact("bootstrapProvenance" if spec == 134 else "holdProvenance", provenance)
        page = self.root / "capture" / "pages" / "top" / "00000000.jsonl.gz"
        page.parent.mkdir(parents=True)
        page.write_bytes(b"synthetic capture page; no trie proof")
        capture = {"schema": "sora-nexus-checkpoint-capture-v1", "checkpoint": {"hash": scale.CHECKPOINT_HASH}, "artifacts": {},
                   "top": {"pages": [{"file": "pages/top/00000000.jsonl.gz", "sha256": release.digest_file(page)["sha256"]}]}, "children": []}
        self.artifact("checkpointCaptureManifest", capture, "capture/capture-complete.json")
        proof_node = b"synthetic unverified burn trie node"
        proof_hash = hashlib.sha256(proof_node).hexdigest()
        proof_path = self.root / "burn" / "objects" / proof_hash
        proof_path.parent.mkdir(parents=True)
        proof_path.write_bytes(proof_node)
        initial = self.root / "burn" / "initial.json"
        initial.write_text(json.dumps({"proof": [proof_hash]}))
        page = self.root / "burn" / "pages" / "page.json.gz"
        page.parent.mkdir()
        page.write_bytes(gzip.compress(json.dumps({"schema": "sora-nexus-burn-proof-page-v1", "records": [{"proof": [proof_hash]}]}).encode(), mtime=0))
        self.artifact("burnHistoryManifest", {"schema": "sora-nexus-burn-proof-stream-v1", "first": scale.CHECKPOINT, "last": scale.CHECKPOINT,
                      "checkpointHash": scale.CHECKPOINT_HASH, "initial": {"file": "initial.json", "sha256": release.digest_file(initial)["sha256"]},
                      "pages": [{"file": "pages/page.json.gz", "sha256": release.digest_file(page)["sha256"]}]}, "burn/burn-history-manifest.json")
        work_files = []
        for name in ("work", "audit", "terminal"):
            shard = self.root / "work" / (name + ".scale")
            shard.parent.mkdir(exist_ok=True)
            shard.write_bytes(b"synthetic unverified envelope: " + name.encode())
            actual = release.digest_file(shard)
            work_files.append({"file": shard.name, "bytes": actual["bytes"], "sha256": actual["sha256"]})
        self.artifact("recoveryWorkIndex", {"schema": "sora-nexus-recovery-work-index-v1",
                      "policySha256": self.inventory["policyScale"]["sha256"],
                      "work": [work_files[0]], "audits": [work_files[1]], "terminal": work_files[2]}, "work/index.json")
        for role in sorted(release.REQUIRED - self.inventory.keys()):
            self.artifact(role, {"productionReady": True, "allChecksPass": True, "warning": "synthetic unverified report"})
        self.write_candidate(evidence=[{"gate": gate, "artifacts": ["finalProvenance"]} for gate in release.GATES])

    def test_full_bundle_is_bound_but_never_promoted_by_reports(self):
        self.full_fixture()
        output = self.root / "bundle"
        report = release.package(self.candidate, output)
        self.assertFalse(report["productionReady"])
        self.assertEqual(report["missingArtifacts"], [])
        self.assertTrue(all(row["status"] == "unverified" for row in report["gates"].values()))
        self.assertEqual(len(report["artifacts"]), len(release.REQUIRED) + 7)
        for row in report["artifacts"]:
            self.assertEqual(release.digest_file(output / row["object"])["sha256"], row["sha256"])
        manifest = output / "release-manifest.json"
        self.assertEqual((output / "PACKAGE-COMPLETE").read_text().strip(), release.digest_file(manifest)["sha256"])
        self.assertIn("NOT READY FOR PRODUCTION", (output / "README.txt").read_text())
        with self.assertRaisesRegex(ValueError, "new directory"):
            release.package(self.candidate, output)

    def test_soratopia_reports_cannot_replace_checkpoint_replay(self):
        self.full_fixture()
        # The fixture supplies success-looking JSON for every points role.
        report = release.package(self.candidate, self.root / "points-unverified")
        self.assertEqual(report["gates"]["soratopiaPointsOwnershipAndConservation"]["status"], "unverified")
        self.assertFalse(report["productionReady"])
        # An otherwise populated package must still identify absent points
        # evidence, including when on-chain account migration is configured.
        del self.inventory["soratopiaCheckpointEvidenceManifest"]
        self.write_candidate()
        report = release.package(self.candidate, self.root / "points-missing")
        self.assertIn("soratopiaCheckpointEvidenceManifest", report["missingArtifacts"])
        self.assertEqual(report["gates"]["soratopiaPointsOwnershipAndConservation"]["status"], "unverified")

    def test_raw_burn_proof_dependencies_are_bound_and_mutation_or_missing_nodes_rejected(self):
        self.full_fixture()
        report, _ = release.verify_candidate(self.candidate)
        bound = [row for row in report["artifacts"] if row["role"].startswith("burnHistoryFile-")]
        self.assertEqual(len(bound), 3)
        self.assertTrue(any("/objects/" in row["sourcePath"] for row in bound))
        node = next((self.root / "burn" / "objects").iterdir())
        original = node.read_bytes()
        node.write_bytes(b"mutated")
        with self.assertRaisesRegex(ValueError, "Burn evidence artifact hash"):
            release.verify_candidate(self.candidate)
        node.write_bytes(original)
        node.unlink()
        with self.assertRaisesRegex(ValueError, "regular file"):
            release.verify_candidate(self.candidate)

    def test_partial_candidate_lists_missing_inputs(self):
        self.artifact("exchangeNotice", "SORA Nexus recovery notice")
        report, _ = release.verify_candidate(self.write_candidate())
        self.assertIn("bootstrapWasm", report["missingArtifacts"])
        self.assertFalse(report["productionReady"])

    def test_staking_role_inventory_must_match_bootstrap_operators(self):
        self.full_fixture()
        self.artifact("stakingRoleReview", {"evidenceHash": hx(12), "operatorsHash": hx(99), "records": [{}]})
        with self.assertRaisesRegex(ValueError, "bootstrap operator inventory"):
            release.verify_candidate(self.write_candidate())

    def test_source_readiness_fields_rejected(self):
        self.artifact("exchangeNotice", "public notice")
        with self.assertRaisesRegex(ValueError, "readiness booleans"):
            release.verify_candidate(self.write_candidate(productionReady=True))
        with self.assertRaisesRegex(ValueError, "evidence gate"):
            release.verify_candidate(self.write_candidate(evidence=[{"gate": "checkpointData", "artifacts": ["exchangeNotice"], "passed": True}]))

    def test_digest_mutation_rejected(self):
        path = self.artifact("exchangeNotice", "before")
        self.write_candidate()
        path.write_text("after")
        with self.assertRaisesRegex(ValueError, "digest mismatch"):
            release.verify_candidate(self.candidate)

    def test_symlinks_paths_and_credential_inputs_rejected(self):
        path = self.artifact("exchangeNotice", "public")
        (self.root / "link").symlink_to(path)
        for name in ("../outside", "link", "sydneycreds.txt", ".env", "./exchangeNotice", "/tmp/file"):
            with self.subTest(name=name), self.assertRaises(ValueError):
                release.local_file(self.root, name)

    def test_duplicate_substitute_versions_and_wrong_genesis_identity_rejected(self):
        self.full_fixture()
        self.spec["codeSubstitutes"]["11271782"] = "0x" + wasm(57, 57).hex()
        self.spec["codeSubstitutes"]["23234812"] = "0x" + wasm(57, 57, b"other").hex()
        self.artifact("chainSpec", self.spec)
        with self.assertRaisesRegex(ValueError, "Duplicate substitute"):
            release.verify_candidate(self.write_candidate())
        self.spec["codeSubstitutes"].pop("11271782")
        self.spec["codeSubstitutes"].pop("23234812")
        self.spec["id"] = "sora-mainnet"
        self.artifact("chainSpec", self.spec)
        with self.assertRaisesRegex(ValueError, "network identity"):
            release.verify_candidate(self.write_candidate())

    def test_substitutes_cannot_overlap_recovery_or_exceed_node_height_bounds(self):
        self.full_fixture()
        original = copy.deepcopy(self.spec["codeSubstitutes"])
        for height, spec, message in (
            (scale.CHECKPOINT + 1, 133, "overlaps recovery"),
            (scale.CHECKPOINT + 1, 138, "overlaps recovery"),
            (scale.CHECKPOINT - 1, 135, "overlaps recovery"),
            (2**32, 57, "out-of-range"),
        ):
            with self.subTest(height=height, spec=spec):
                self.spec["codeSubstitutes"] = {**original, str(height): "0x" + wasm(spec, 131).hex()}
                self.artifact("chainSpec", self.spec)
                with self.assertRaisesRegex(ValueError, message):
                    release.verify_candidate(self.write_candidate())
        self.spec["codeSubstitutes"] = {**original,
            "11271782": "0x" + wasm(57, 57).hex(),
            "23234812": "0x" + wasm(115, 115).hex()}
        self.artifact("chainSpec", self.spec)
        report, _ = release.verify_candidate(self.write_candidate())
        self.assertFalse(report["productionReady"])

    def test_wrong_runtime_version_and_duplicate_embedded_identity_rejected(self):
        self.artifact("holdWasm", wasm(136, 132))
        with self.assertRaisesRegex(ValueError, "Wrong holdWasm version"):
            release.verify_candidate(self.write_candidate())
        with self.assertRaisesRegex(ValueError, "Duplicate runtime"):
            release.wasm_identity(wasm(134, 131) + custom("runtime_version", b"bad"))

    def test_compressed_runtime_identity(self):
        from compression import zstd
        code = wasm(135, 132)
        compressed = bytes([82, 188, 83, 118, 70, 219, 142, 5]) + zstd.compress(code)
        self.assertEqual(release.wasm_identity(compressed), release.wasm_identity(code))
        with patch.object(release, "MAX_WASM", 30), self.assertRaisesRegex(ValueError, "oversized"):
            release.wasm_identity(compressed)

    def test_immutable_bootstrap_rejects_replacement(self):
        self.full_fixture()
        report, _ = release.verify_candidate(self.candidate)
        prior = self.root / "prior.json"
        prior.write_text(json.dumps(report))
        report2, _ = release.verify_candidate(self.candidate, prior)
        self.assertEqual(report2["previousManifestSha256"], release.digest_file(prior)["sha256"])
        for row in report["artifacts"]:
            if row["role"] == "bootstrapWasm":
                row["blake2_256"] = "11" * 32
        prior.write_text(json.dumps(report))
        with self.assertRaisesRegex(ValueError, "cannot be replaced"):
            release.verify_candidate(self.candidate, prior)

    def test_mapping_must_match_compiled_policy_not_only_count(self):
        self.full_fixture()
        self.artifact("signedMappingCsv", f'old account address,new account address\n{hx(1)},{hx(4)}\n')
        with self.assertRaisesRegex(ValueError, "compiled policy mapping"):
            release.verify_candidate(self.write_candidate())

    def test_known_ss58_decoding_and_checksum(self):
        attacker = "cnTkvHKg2He1o5s515ody3BUNu2azPDi1xMaVxhg2eeB1b1t6"
        self.assertEqual(release.account(attacker), scale.raw(scale.ATTACKER_SORA))
        with self.assertRaisesRegex(ValueError, "checksum"):
            release.account(attacker[:-1] + "7")

    def test_unbound_or_inconsistent_build_provenance_rejected(self):
        self.full_fixture()
        path = self.root / "bootstrapProvenance"
        provenance = json.loads(path.read_text())
        provenance["bound"] = False
        self.artifact("bootstrapProvenance", provenance)
        with self.assertRaisesRegex(ValueError, "Unbound development"):
            release.verify_candidate(self.write_candidate())
        provenance["bound"] = True
        provenance["bindingEvidence"]["finalWasm"]["sha256"] = "00" * 32
        self.artifact("bootstrapProvenance", provenance)
        with self.assertRaisesRegex(ValueError, "Provenance binding"):
            release.verify_candidate(self.write_candidate())

    def test_consensus_inventory_and_signing_stop_are_exact(self):
        self.full_fixture()
        self.spec["recovery"]["grandpaAuthorities"][0][1] = 2
        self.artifact("chainSpec", self.spec)
        with self.assertRaisesRegex(ValueError, "weights/order"):
            release.verify_candidate(self.write_candidate())
        self.spec["recovery"]["grandpaAuthorities"][0][1] = 1
        self.spec["recovery"]["firstRecoverySlot"] = 100
        self.artifact("chainSpec", self.spec)
        with self.assertRaisesRegex(ValueError, "signing-stop"):
            release.verify_candidate(self.write_candidate())

    def test_active_grandpa_keys_preserve_operator_association_and_voting_weight(self):
        self.full_fixture()
        # Deliberately use unequal active weights and an approved inactive
        # operator. Set membership alone admits both invalid configurations.
        entries = [(2, [8, 10], 9, 20, 21), (3, [12], 13, 22, 23), (4, [16], 17, 24, 25)]
        encoded = scale.compact(len(entries)) + b"".join(
            h(owner) + scale.compact(len(retired)) + b"".join(h(key) for key in retired) + h(fresh) +
            scale.compact(1) + h(babe) + scale.compact(1) + h(online)
            for owner, retired, fresh, babe, online in entries)
        self.artifact("operatorsScale", encoded)
        config = self.spec["recovery"]
        config.update(previousGrandpaAuthorities=[[hx(8), 1], [hx(12), 3]],
                      grandpaAuthorities=[[hx(9), 1], [hx(13), 3]],
                      retiredGrandpaKeys=[hx(8), hx(10), hx(12), hx(16)],
                      approvedGrandpaKeys=[hx(9), hx(13), hx(17)])
        # These three raw artifacts intentionally form a partial candidate, so
        # this regression exercises the chain/operator relation independently
        # of source-provenance and omitted runtime execution gates.
        self.inventory = {role: self.inventory[role] for role in ("operatorsScale", "chainSpec", "bootstrapWasm")}
        self.artifact("chainSpec", self.spec)
        report, _ = release.verify_candidate(self.write_candidate())
        self.assertEqual(report["operators"]["grandpaTransitions"][0],
                         {"owner": h(2).hex(), "retired": [h(8).hex(), h(10).hex()], "fresh": h(9).hex()})
        for wrong in ([[hx(13), 1], [hx(9), 3]], [[hx(9), 1], [hx(17), 3]]):
            with self.subTest(fresh=wrong):
                config["grandpaAuthorities"] = wrong
                self.artifact("chainSpec", self.spec)
                with self.assertRaisesRegex(ValueError, "compiled operator owner"):
                    release.verify_candidate(self.write_candidate())
        # An active key may be any exact member of its owner's retired union.
        config["previousGrandpaAuthorities"] = [[hx(10), 1], [hx(12), 3]]
        config["grandpaAuthorities"] = [[hx(9), 1], [hx(13), 3]]
        self.artifact("chainSpec", self.spec)
        report, _ = release.verify_candidate(self.write_candidate())
        self.assertFalse(report["productionReady"])

    def test_capture_dependents_cannot_be_swapped(self):
        self.full_fixture()
        (self.root / "capture/pages/top/00000000.jsonl.gz").write_text("mutated")
        with self.assertRaisesRegex(ValueError, "capture artifact hash"):
            release.verify_candidate(self.candidate)

    def test_native_verifier_is_explicitly_pinned_and_cannot_promote_release(self):
        self.full_fixture()
        report, paths = release.verify_candidate(self.candidate)
        binary = self.root / "verifier"
        binary.write_text("synthetic local executable")
        digest = release.digest_file(binary)["sha256"]
        with self.assertRaisesRegex(ValueError, "binary hash mismatch"):
            release.run_evidence_verifier(report, paths, binary, "00" * 32)
        def execute(_, arguments):
            if arguments[0] == "verify-capture":
                result = {"schema": "sora-nexus-checkpoint-verification-v1", "checkpointHash": scale.CHECKPOINT_HASH, "stateRoot": release.CHECKPOINT_ROOT,
                          "headerHashVerified": True, "extrinsicsRootVerified": True, "stateRootVerified": True,
                          "captureManifestSha256": self.inventory["checkpointCaptureManifest"]["sha256"]}
            elif arguments[0] == "verify-metadata":
                result = {"schema": "sora-nexus-checkpoint-metadata-witness-v1", "checkpointHash": scale.CHECKPOINT_HASH, "stateRoot": release.CHECKPOINT_ROOT, "wasmExecutionVerified": True, "nativeExecution": False}
            elif arguments[0] == "verify-incoming-coverage":
                result = {}  # No synthetic report clears the incoming gate.
            else:
                self.assertEqual(arguments, ["verify-ownership", "--csv", str(paths["signedMappingCsv"]),
                    "--proofs", str(paths["ownershipProofs"]), "--capture", str(paths["checkpointCaptureManifest"].parent),
                    "--semantic", str(paths["semanticPlan"]), "--derived-inventory", str(paths["derivedAccountInventory"])])
                result = self.control_result()
            return subprocess.CompletedProcess(arguments, 0, json.dumps(result).encode(), b"")
        with patch.object(release, "execute_verifier", side_effect=execute) as runner:
            release.run_evidence_verifier(report, paths, binary, digest)
        self.assertEqual(runner.call_count, 4)
        self.assertEqual(report["gates"]["checkpointData"]["status"], "verified")
        self.assertEqual(report["gates"]["accountOwnership"]["status"], "verified")
        self.assertEqual(report["gates"]["derivedAccountControl"]["status"], "verified")
        self.assertEqual(report["gates"]["executingArtifactIdentity"]["status"], "unverified")
        self.assertFalse(report["productionReady"])

    def control_result(self):
        """Synthetic verifier output for binding tests, never actual crypto proof."""
        return {"schema": "sora-nexus-account-control-verification-v1", "ownershipVerified": True,
            "derivedControlVerified": True, "accounts": 1, "checkpointHash": scale.CHECKPOINT_HASH,
            "checkpointStateRoot": release.CHECKPOINT_ROOT,
            **{field: self.inventory[role]["sha256"] for field, role in (
                ("mappingCsvSha256", "signedMappingCsv"), ("ownershipProofsSha256", "ownershipProofs"),
                ("semanticPlanSha256", "semanticPlan"), ("derivedInventorySha256", "derivedAccountInventory"),
                ("captureManifestSha256", "checkpointCaptureManifest"))},
            "checkpointMetadata": {"schema": "sora-nexus-checkpoint-metadata-witness-v1",
                "checkpointHash": scale.CHECKPOINT_HASH, "stateRoot": release.CHECKPOINT_ROOT,
                "wasmExecutionVerified": True, "nativeExecution": False},
            "controls": {"schema": "sora-nexus-semantic-ownership-verification-v1", "checkpointHash": scale.CHECKPOINT_HASH,
                "mappedAccounts": 1, "directAccounts": [hx(1)], "derivedAccounts": [],
                "replacementOwnershipVerified": True, "derivedControlGraphVerified": True}}

    def test_control_verifier_rejects_wrong_inputs_omitted_owners_and_old_direct_receipts(self):
        self.full_fixture()
        original, paths = release.verify_candidate(self.candidate)
        binary = self.root / "verifier"
        binary.write_bytes(b"synthetic local verifier")
        digest = release.digest_file(binary)["sha256"]
        changed = []
        for field in ("mappingCsvSha256", "ownershipProofsSha256", "semanticPlanSha256", "derivedInventorySha256", "captureManifestSha256"):
            result = self.control_result()
            result[field] = "ff" * 32
            changed.append(result)
        result = self.control_result()
        result["controls"]["directAccounts"] = []
        changed.append(result)
        result = self.control_result()
        result["checkpointMetadata"]["wasmExecutionVerified"] = False
        changed.append(result)
        changed.append({"ownershipVerified": True, "accounts": 1})
        for result in changed:
            report = copy.deepcopy(original)
            with patch.object(release, "execute_verifier", side_effect=lambda _, args: subprocess.CompletedProcess(args, 0, json.dumps(result).encode(), b"")):
                release.run_evidence_verifier(report, paths, binary, digest)
            self.assertEqual(report["gates"]["accountOwnership"]["status"], "failed")
            self.assertEqual(report["gates"]["derivedAccountControl"]["status"], "failed")
            self.assertFalse(report["productionReady"])

    def test_missing_semantic_inputs_do_not_fall_back_to_direct_only_verification(self):
        self.full_fixture()
        report, paths = release.verify_candidate(self.candidate)
        paths.pop("semanticPlan")
        binary = self.root / "verifier"
        binary.write_bytes(b"synthetic local verifier")
        with patch.object(release, "execute_verifier", side_effect=lambda _, args: subprocess.CompletedProcess(args, 1, b"", b"synthetic")) as runner:
            release.run_evidence_verifier(report, paths, binary, release.digest_file(binary)["sha256"])
        self.assertEqual(runner.call_count, 3)
        self.assertTrue(all(call.args[1][0] != "verify-ownership" for call in runner.call_args_list))
        self.assertEqual(report["gates"]["accountOwnership"]["status"], "unverified")
        self.assertEqual(report["gates"]["derivedAccountControl"]["status"], "unverified")

    def test_separate_derived_inventory_must_match_compiler_semantic_plan(self):
        self.full_fixture()
        self.artifact("derivedAccountInventory", b'[{}]')
        with self.assertRaisesRegex(ValueError, "differs from semantic plan"):
            release.verify_candidate(self.write_candidate())

    def test_control_receipts_bind_original_candidate_not_mutated_live_files(self):
        self.full_fixture()
        report, paths = release.verify_candidate(self.candidate)
        self.artifact("semanticPlan", {"derivedAccounts": [], "controls": []})
        result = self.control_result()  # Uses the new file digest, not the admitted candidate.
        binary = self.root / "verifier"
        binary.write_bytes(b"synthetic local verifier")
        with patch.object(release, "execute_verifier", side_effect=lambda _, args: subprocess.CompletedProcess(args, 0, json.dumps(result).encode(), b"")):
            release.run_evidence_verifier(report, paths, binary, release.digest_file(binary)["sha256"])
        self.assertEqual(report["gates"]["accountOwnership"]["status"], "failed")

    def test_public_fixture_provenance_cannot_be_packaged(self):
        self.full_fixture()
        for role in ("bootstrapProvenance", "holdProvenance", "finalProvenance"):
            path = self.root / role
            data = json.loads(path.read_text())
            data["fixtureOnly"] = True
            self.artifact(role, data)
            self.write_candidate()
            with self.assertRaisesRegex(ValueError, "diagnostic fixture"):
                release.verify_candidate(self.candidate)
            data.pop("fixtureOnly")
            self.artifact(role, data)

    def test_retained_key_approval_inventory_is_mandatory_and_unique(self):
        old = h(2) + scale.compact(1) + h(8) + h(9)
        with self.assertRaises(ValueError):
            release.operator_identity(scale.compact(1) + old)
        retained = scale.compact(1) + h(10) + scale.compact(1) + h(11)
        identity = release.operator_identity(scale.compact(1) + old + retained)
        self.assertEqual(identity["approvedBabe"], [h(10).hex()])
        with self.assertRaisesRegex(ValueError, "multiple owners"):
            release.operator_identity(scale.compact(2) + old + retained +
                h(3) + scale.compact(1) + h(12) + h(13) + retained)
        with self.assertRaisesRegex(ValueError, "canonical"):
            release.operator_identity(scale.compact(1) + old +
                scale.compact(2) + h(10) + h(10) + scale.compact(1) + h(11))

    def test_policy_v2_has_explicit_network_nonce_and_no_legacy_default(self):
        raw = policy()
        identity = release.policy_identity(raw)
        self.assertEqual(identity["authorizationDomain"], {"chainSpecId": "sora-nexus-recovery-28191817", "releaseNonce": hx(8)})
        with self.assertRaisesRegex(ValueError, "Unknown policy format"):
            release.policy_identity(scale.u32(1) + raw[4:])
        encoded = release.authorization_domain_scale(identity["authorizationDomain"])
        for chain_id, nonce in [("other-chain", hx(8)), (identity["authorizationDomain"]["chainSpecId"], hx(9))]:
            changed = raw.replace(encoded, release.authorization_domain_scale({"chainSpecId": chain_id, "releaseNonce": nonce}), 1)
            self.assertNotEqual(release.policy_identity(changed)["commitment"], identity["commitment"])
        for domain in [
            {}, {"chainSpecId": "", "releaseNonce": hx(8)},
            {"chainSpecId": "x" * 129, "releaseNonce": hx(8)},
            {"chainSpecId": "bad chain", "releaseNonce": hx(8)},
            {"chainSpecId": "chain", "releaseNonce": hx(0)},
        ]:
            with self.assertRaises(ValueError):
                release.authorization_domain_scale(domain)
        empty_nonce = raw.replace(encoded, encoded[:-32] + bytes(32), 1)
        with self.assertRaisesRegex(ValueError, "nonzero"):
            release.policy_identity(empty_nonce)

    def test_chain_extension_must_carry_exact_domain_bound_policy(self):
        self.full_fixture()
        baseline = copy.deepcopy(self.spec)
        for mutate in [
            lambda spec: spec["recovery"].update(schemaVersion=1),
            lambda spec: spec["recovery"].pop("policyScale"),
            lambda spec: spec["recovery"].update(policyScale="0x" + (policy() + b"\0").hex()),
        ]:
            self.spec = copy.deepcopy(baseline)
            mutate(self.spec)
            self.artifact("chainSpec", self.spec)
            self.write_candidate()
            with self.assertRaises(ValueError):
                release.verify_candidate(self.candidate)

    def test_attacker_council_and_noncanonical_policy_rejected(self):
        data = policy().replace(h(3), scale.raw(scale.ATTACKER_SORA), 1)
        with self.assertRaisesRegex(ValueError, "council"):
            release.policy_identity(data)
        with self.assertRaisesRegex(ValueError, "Trailing"):
            release.policy_identity(policy() + b"\0")


if __name__ == "__main__":
    unittest.main()
