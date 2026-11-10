"""Manifest binding regressions; synthetic Wasm is never execution evidence."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

import hashi_codec as scale
import prepare_maintenance_fixture as maintenance
import release_package as release
from test_release_package import hx, policy, wasm


def mixed():
    commitment = "0x" + release.policy_identity(policy())["commitment"]
    return {
        "schema": maintenance.MIXED_SCHEMA,
        "fixtureOnly": True, "productionReady": False, "nativePreviewCompleted": True,
        "exactWasmRehearsed": False, "nativePreviewEvidence": [{}, {}],
        "retainedConfigurationKeys": 1,
        "retainedPublicConfiguration": [{"key": "0x01", "valueHash": hx(4)}],
        "nativeBridgeHaltVerified": True, "publicConfigurationRetained": True,
        "cleanupStorageEditsEmitted": False,
        "work": {
            "schema": "sora-nexus-structural-mixed-work-v1", "structuralValidationOnly": True,
            "authenticationVerified": False, "economicCompletionVerified": False,
            "storageEngineRehearsed": False, "exactArtifactRehearsed": False, "productionReady": False,
            "policyScale": "0x" + policy().hex(), "policyCommitment": commitment,
            "workEnvelopes": ["0x01", "0x02", "0x03"],
            "auditEnvelopes": ["0x05"], "terminalEnvelope": "0x06",
            "work": [{"index": i, "kind": kind} for i, kind in enumerate(
                ["storage", "incoming", "outgoing"])],
        },
        "fixtureExpectations": {
            "schema": "sora-nexus-public-fixture-storage-expectations-v1", "fixtureOnly": True,
            "policyCommitment": commitment, "records": [{"key": "0x01", "valueHash": hx(4)},
                {"key": maintenance.BRIDGE_HALT_KEY, "valueHash": maintenance.BRIDGE_HALT_VALUE_HASH}],
            "operations": [{"index": i, "records": [{"key": "0x01", "valueHash": hx(i)}]} for i in [1, 2]],
        },
    }


def hashi():
    document = mixed()
    document.update(schema=maintenance.HASHI_SCHEMA,
        nativeZeroSenderPreserved=True, signatureChainDomain="0x01", minted="2",
        sourceActor="0x9d8a62f656a8d1615c1294fd71e9cfb3e4855a4f",
        excludedSourceActor="0xb595b18c88b1f651ca387489067f855b5c8e6720",
        identicalSourceRegistryAcrossOperations=True,
        incomingScenarios=["retained-pending-credit", "fresh-credit", "fresh-source-actor-exclusion"])
    document["work"]["work"] = [{"index": i, "kind": kind} for i, kind in enumerate(
        ["storage", "incoming", "incoming", "incoming"])]
    document["work"]["workEnvelopes"] = ["0x01", "0x02", "0x03", "0x04"]
    document["fixtureExpectations"]["operations"] = [
        {"index": i, "records": [{"key": "0x01", "valueHash": hx(i)}]} for i in [1, 2, 3]]
    document["retainedConfigurationKeys"] = 6
    document["retainedPublicConfiguration"] = [
        {"key": f"0x{i:02x}", "valueHash": hx(i)} for i in range(1, 7)]
    document["fixtureExpectations"]["records"] = copy.deepcopy(document["retainedPublicConfiguration"]) + [
        {"key": maintenance.BRIDGE_HALT_KEY, "valueHash": maintenance.BRIDGE_HALT_VALUE_HASH}]
    document["fixtureHashiProvenance"] = {"schema": "sora-nexus-public-fixture-hashi-inputs-v1",
        "fixtureOnly": True, "productionReady": False, "checkpointHash": scale.CHECKPOINT_HASH,
        "policyCommitment": document["work"]["policyCommitment"],
        "peerScheme": "sora-nexus/PUBLIC-FIXTURE-ONLY/hashi-peer/v1/", "publicPeers": [hx(i) for i in range(4)]}
    return document


class MaintenanceFixtureTests(unittest.TestCase):
    def test_old_bridge_schemas_and_cleanup_schedules_are_rejected(self):
        for schema in ["sora-nexus-PUBLIC-FIXTURE-ONLY-mixed-bridge-v1",
                       "sora-nexus-PUBLIC-FIXTURE-ONLY-hashi-bridge-v1",
                       "sora-nexus-PUBLIC-FIXTURE-ONLY-hashi-bridge-v2"]:
            document = mixed()
            document["schema"] = schema
            with self.assertRaisesRegex(ValueError, "Unsupported diagnostic work schema"):
                maintenance.work_inputs(document)
        for fixture in [mixed, hashi]:
            document = fixture()
            document["work"]["work"].append({"index": len(document["work"]["work"]), "kind": "storage"})
            document["work"]["workEnvelopes"].append("0xff")
            with self.assertRaisesRegex(ValueError, "work schedule"):
                maintenance.work_inputs(document)

    def test_retained_configuration_flags_count_and_final_hashes_are_bound(self):
        for fixture in [mixed, hashi]:
            for field, value in [("nativeBridgeHaltVerified", False),
                                 ("publicConfigurationRetained", False),
                                 ("cleanupStorageEditsEmitted", True)]:
                document = fixture()
                document[field] = value
                with self.assertRaisesRegex(ValueError, "halted retained"):
                    maintenance.work_inputs(document)
            for count in [0, True, fixture()["retainedConfigurationKeys"] + 1]:
                document = fixture()
                document["retainedConfigurationKeys"] = count
                with self.assertRaisesRegex(ValueError, "match native final records"):
                    maintenance.work_inputs(document)
            document = fixture()
            document["retainedPublicConfiguration"][0]["valueHash"] = hx(99)
            with self.assertRaisesRegex(ValueError, "match native final records"):
                maintenance.work_inputs(document)
            document = fixture()
            document["fixtureExpectations"]["records"].pop()
            with self.assertRaisesRegex(ValueError, "persistent bridge halt"):
                maintenance.work_inputs(document)
            document = fixture()
            document["fixtureExpectations"]["records"][-1]["valueHash"] = hx(99)
            with self.assertRaisesRegex(ValueError, "persistent bridge halt"):
                maintenance.work_inputs(document)

    def test_native_record_duplicates_unordered_and_bad_hashes_are_rejected(self):
        for rows in [[], [{"key": "0x01", "valueHash": hx(1)}] * 2,
                     [{"key": "0x02", "valueHash": None}, {"key": "0x01", "valueHash": None}],
                     [{"key": "0x01", "valueHash": "0x01"}],
                     [{"key": "0x", "valueHash": None}]]:
            document = mixed()
            document["retainedPublicConfiguration"] = rows
            with self.assertRaises(ValueError):
                maintenance.work_inputs(document)
        document = hashi()
        document["retainedPublicConfiguration"].pop()
        document["retainedConfigurationKeys"] = 5
        with self.assertRaisesRegex(ValueError, "match native final records"):
            maintenance.work_inputs(document)

    def test_hashi_fixture_requires_exact_schedule_peer_provenance_and_write_expectations(self):
        document = hashi()
        _, envelopes, expectations = maintenance.work_inputs(document)
        self.assertEqual(envelopes, ["0x01", "0x02", "0x03", "0x04"])
        self.assertEqual([row["index"] for row in expectations["operations"]], [1, 2, 3])
        for field, value in [("productionReady", True), ("policyCommitment", hx(9)), ("publicPeers", [])]:
            changed = copy.deepcopy(document)
            changed["fixtureHashiProvenance"][field] = value
            with self.assertRaisesRegex(ValueError, "peer provenance"):
                maintenance.work_inputs(changed)
        document["fixtureExpectations"]["operations"] = []
        with self.assertRaisesRegex(ValueError, "dynamic storage expectations"):
            maintenance.work_inputs(document)

    def test_hashi_manifest_enables_only_incoming_public_witness_step(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "checkpoint").mkdir()
            (root / "checkpoint/capture-complete.json").write_text("{}")
            work = root / "work.json"
            work.write_text(json.dumps(hashi()))
            consensus = root / "consensus.json"
            consensus.write_text(json.dumps({"checkpointHash": scale.CHECKPOINT_HASH, "stateRoot": release.CHECKPOINT_ROOT,
                "validators": [hx(2)], "records": [{"storage": "Grandpa.Authorities", "value": [[hx(8), 1]]}],
                "checkpointSlot": 298823260, "checkpointTimestamp": 1792939560002, "previousGrandpaSet": 7956}))
            operators = root / "operators.json"
            operators.write_text(json.dumps([{"owner": hx(2), "retainedGrandpa": [hx(8)], "fixtureOnly": True, "operatorApproved": False}]))
            paths = []
            for spec, tx in [(134, 131), (135, 132), (138, 134)]:
                path = root / f"runtime-{spec}.wasm"
                path.write_bytes(wasm(spec, tx))
                paths.append(path)
            result = maintenance.prepare(root, root / "diagnostic", work, consensus, operators, *paths)
            manifest = json.loads((root / result["manifest"]).read_text())
            self.assertEqual(manifest["schema"], "sora-nexus-public-fixture-hashi-replay-v1")
            self.assertEqual([index for index, step in enumerate(manifest["steps"]) if step.get("fixtureHashi")], [1, 2, 3])
            reference = manifest["fixtureHashiPeers"]["provenance"]
            data = (root / reference["file"]).read_bytes()
            self.assertEqual(hashlib.sha256(data).hexdigest(), reference["sha256"])
            self.assertEqual(json.loads(data), hashi()["fixtureHashiProvenance"])
            self.assertFalse(result["productionReady"])
            # Every prelude page executes before the final activation envelope.
            paged = hashi()
            paged['work']['terminalPreludeEnvelopes'] = ['0x0b00', '0x0b01']
            work.write_text(json.dumps(paged))
            page_result = maintenance.prepare(root, root / 'paged-diagnostic', work, consensus, operators, *paths)
            page_manifest = json.loads((root / page_result['manifest']).read_text())
            page_steps = [step for step in page_manifest['steps'] if step.get('recovery')
                          and 'terminal-page' in step['recovery']['file']]
            self.assertEqual([step['expectedPhase'] for step in page_steps], ['Auditing', 'Auditing'])
            self.assertEqual([step['expectedPhase'] for step in page_manifest['steps'][-3:]], ['Complete'] * 3)

    def test_source_hashi_scope_and_three_incoming_steps_are_bound(self):
        document=hashi()
        _,envelopes,expectations=maintenance.work_inputs(document)
        self.assertEqual(len(envelopes),4)
        self.assertEqual([r["index"] for r in expectations["operations"]],[1,2,3])
        for key,value in [("nativeZeroSenderPreserved",False),("signatureChainDomain","0x02"),("minted","3"),("sourceActor",hx(1)),("identicalSourceRegistryAcrossOperations",False),("excludedSourceActor",hx(2)),("incomingScenarios",["fresh-credit"])]:
            wrong=copy.deepcopy(document);wrong[key]=value
            with self.assertRaisesRegex(ValueError,"source-actor fixture scope"):
                maintenance.work_inputs(wrong)
        wrong=copy.deepcopy(document);wrong["work"]["work"][2]["kind"]="storage"
        with self.assertRaisesRegex(ValueError,"work schedule"):
            maintenance.work_inputs(wrong)
        wrong=copy.deepcopy(document);wrong["fixtureExpectations"]["operations"].pop()
        with self.assertRaisesRegex(ValueError,"dynamic storage expectations"):
            maintenance.work_inputs(wrong)

    def test_source_hashi_manifest_signs_all_three_and_no_storage_steps(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);(root/"checkpoint").mkdir();(root/"checkpoint/capture-complete.json").write_text("{}")
            work=root/"work.json";work.write_text(json.dumps(hashi()))
            consensus=root/"consensus.json";consensus.write_text(json.dumps({"checkpointHash":scale.CHECKPOINT_HASH,"stateRoot":release.CHECKPOINT_ROOT,
                "validators":[hx(2)],"records":[{"storage":"Grandpa.Authorities","value":[[hx(8),1]]}],
                "checkpointSlot":298823260,"checkpointTimestamp":1792939560002,"previousGrandpaSet":7956}))
            operators=root/"operators.json";operators.write_text(json.dumps([{"owner":hx(2),"retainedGrandpa":[hx(8)],"fixtureOnly":True,"operatorApproved":False}]))
            paths=[]
            for spec,tx in [(134,131),(135,132),(138,134)]:
                path=root/f"runtime-{spec}.wasm";path.write_bytes(wasm(spec,tx));paths.append(path)
            report=maintenance.prepare(root,root/"source-diagnostic",work,consensus,operators,*paths)
            manifest=json.loads((root/report["manifest"]).read_text())
            self.assertEqual([i for i,s in enumerate(manifest["steps"]) if s.get("fixtureHashi")],[1,2,3])
            self.assertEqual([s["expectedPhase"] for s in manifest["steps"][:4]],["Applying"]*3+["Auditing"])
            self.assertFalse(report["productionReady"])

    def test_structural_flags_remain_false_and_preview_is_required(self):
        document = mixed()
        before = copy.deepcopy(document)
        maintenance.work_inputs(document)
        self.assertEqual(document, before)
        for field in ["authenticationVerified", "economicCompletionVerified", "storageEngineRehearsed",
                      "exactArtifactRehearsed", "productionReady"]:
            changed = copy.deepcopy(document)
            changed["work"][field] = True
            with self.assertRaisesRegex(ValueError, "Structural assembly"):
                maintenance.work_inputs(changed)
        document["nativePreviewCompleted"] = False
        with self.assertRaisesRegex(ValueError, "native public fixture previews"):
            maintenance.work_inputs(document)

    def test_reordered_work_or_wrong_policy_expectations_are_rejected(self):
        document = mixed()
        document["work"]["work"].reverse()
        with self.assertRaisesRegex(ValueError, "work schedule"):
            maintenance.work_inputs(document)
        document = mixed()
        document["fixtureExpectations"]["policyCommitment"] = hx(99)
        with self.assertRaisesRegex(ValueError, "policy-bound"):
            maintenance.work_inputs(document)
        document = mixed()
        document["fixtureExpectations"]["operations"].pop()
        with self.assertRaisesRegex(ValueError, "policy-bound"):
            maintenance.work_inputs(document)

    def test_manifest_binds_expectations_and_preserves_three_consecutive_work_blocks(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "checkpoint").mkdir()
            (root / "checkpoint/capture-complete.json").write_text("{}")
            document = mixed()
            work = root / "work.json"
            work.write_text(json.dumps(document))
            consensus = root / "consensus.json"
            consensus.write_text(json.dumps({"checkpointHash": scale.CHECKPOINT_HASH, "stateRoot": release.CHECKPOINT_ROOT,
                "validators": [hx(2)], "records": [{"storage": "Grandpa.Authorities", "value": [[hx(8), 1]]}],
                "checkpointSlot": 298823260, "checkpointTimestamp": 1792939560002, "previousGrandpaSet": 7956}))
            operators = root / "operators.json"
            operators.write_text(json.dumps([{"owner": hx(2), "retainedGrandpa": [hx(8)], "fixtureOnly": True, "operatorApproved": False}]))
            paths = []
            for spec, tx in [(134, 131), (135, 132), (138, 134)]:
                path = root / f"runtime-{spec}.wasm"
                path.write_bytes(wasm(spec, tx))
                paths.append(path)
            report = maintenance.prepare(root, root / "diagnostic", work, consensus, operators, *paths)
            manifest = json.loads((root / report["manifest"]).read_text())
            reference = manifest["fixtureExpectations"]
            data = (root / reference["file"]).read_bytes()
            self.assertEqual(hashlib.sha256(data).hexdigest(), reference["sha256"])
            self.assertEqual(json.loads(data), document["fixtureExpectations"])
            self.assertEqual([s["expectedPhase"] for s in manifest["steps"][:3]],
                             ["Applying", "Applying", "Auditing"])
            actual = [(root / s["recovery"]["file"]).read_bytes().hex() for s in manifest["steps"][:3]]
            self.assertEqual(actual, ["01", "02", "03"])
            self.assertTrue(all(s["timestamp"] == s["slot"] * 6000 + 2 for s in manifest["steps"]))
            self.assertFalse(report["exactWasmRehearsed"])
            self.assertFalse(report["productionReady"])

    def test_original_cancellation_schema_still_requires_storage_execution(self):
        document = {"schema": "sora-nexus-bridge-cancellation-rehearsal-v1", "productionReady": False,
                    "historicalCallsExecuted": False,
                    "work": {"storageEngineRehearsed": True, "applyEnvelopes": ["0x01"], "auditEnvelopes": ["0x02"]}}
        _, envelopes, expectations = maintenance.work_inputs(document)
        self.assertEqual(envelopes, ["0x01"])
        self.assertIsNone(expectations)
        document["work"]["storageEngineRehearsed"] = False
        with self.assertRaisesRegex(ValueError, "Verified diagnostic cancellation"):
            maintenance.work_inputs(document)

    def test_account_fixture_requires_native_composition_without_victim_authorization(self):
        document = {"schema": "sora-nexus-PUBLIC-FIXTURE-ONLY-storage-v1", "fixtureOnly": True,
                    "productionReady": False, "victimAuthorizationVerified": False, "exactWasmRehearsed": False,
                    "work": {"schema": "sora-nexus-storage-work-v1", "productionReady": False,
                             "storageEngineRehearsed": True, "applyEnvelopes": ["0x01"], "auditEnvelopes": ["0x02"]}}
        _, envelopes, expectations = maintenance.work_inputs(document)
        self.assertEqual(envelopes, ["0x01"])
        self.assertIsNone(expectations)
        document["victimAuthorizationVerified"] = True
        with self.assertRaisesRegex(ValueError, "Explicit public storage fixture"):
            maintenance.work_inputs(document)
        document["victimAuthorizationVerified"] = False
        document["work"]["storageEngineRehearsed"] = False
        with self.assertRaisesRegex(ValueError, "Explicit public storage fixture"):
            maintenance.work_inputs(document)


if __name__ == "__main__":
    unittest.main()
