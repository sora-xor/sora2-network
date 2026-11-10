#!/usr/bin/env python3
"""Fail-closed tests for the precise SDK2609 metadata exception."""
import copy
import importlib.util
from pathlib import Path
import unittest

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("metadata_comparison", HERE / "compare-metadata.py")
COMPARATOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(COMPARATOR)


class Sdk2609CompatibilityTests(unittest.TestCase):
    def setUp(self):
        self.old = COMPARATOR.load(HERE / "previous-candidate-metadata.json")
        self.new = copy.deepcopy(self.old)
        types = self.new["types"]["types"]
        self.authorize = max(entry["id"] for entry in types) + 1
        types.append({"id": self.authorize, "type": {
            "path": ["frame_system", "extensions", "authorize_call", "AuthorizeCall"],
            "def": {"composite": {"fields": []}},
        }})
        unit = next(entry["id"] for entry in types if entry["type"]["def"] == {"tuple": []})
        self.new["extrinsic"]["signed_extensions"].insert(0, {
            "identifier": "AuthorizeCall", "ty": self.authorize, "additional_signed": unit,
        })
        type_map = {entry["id"]: entry["type"] for entry in types}
        extra = next(param["type"] for param in type_map[self.new["extrinsic"]["ty"]]["params"]
                     if param["name"] == "Extra")
        type_map[extra]["def"]["tuple"].insert(0, self.authorize)
        self.types = type_map
        pallets = {p["name"]: p for p in self.new["pallets"]}
        def event(pallet, name):
            return next(v for v in type_map[pallets[pallet]["event"]["ty"]]["def"]["variant"]["variants"]
                        if v["name"] == name)
        self.code_updated = event("System", "CodeUpdated")
        hash_type = next(ext["additional_signed"] for ext in self.old["extrinsic"]["signed_extensions"]
                         if ext["identifier"] == "CheckGenesis")
        self.code_updated["fields"] = [{"name": "hash", "type": hash_type}]
        self.offence = event("Offences", "Offence")
        self.offence["fields"][1]["name"] = "slot"
        self.u32 = next(k for k, v in self.types.items() if v["def"] == {"primitive": "u32"})
        compact = next(k for k, v in self.types.items() if v["def"] == {"compact": {"type": self.u32}})
        self.identity_calls = type_map[pallets["Identity"]["calls"]["ty"]]["def"]["variant"]["variants"]
        self.identity_events = type_map[pallets["Identity"]["event"]["ty"]]["def"]["variant"]["variants"]
        self.remove_registrar = {"name": "remove_registrar", "index": 24,
                                 "fields": [{"name": "index", "type": compact}]}
        self.registrar_removed = {"name": "RegistrarRemoved", "index": 22,
                                  "fields": [{"name": "registrar_index", "type": self.u32}]}
        self.identity_calls.append(self.remove_registrar)
        self.identity_events.append(self.registrar_removed)

    def rejects(self, mutate):
        mutate()
        try:
            result = COMPARATOR.sdk2609_comparison(self.old, self.new)
        except (ValueError, KeyError):
            return
        self.assertFalse(result["reviewedSdkMigrationCompatible"])

    def test_only_zero_encoded_leading_authorization_is_projected(self):
        result = COMPARATOR.sdk2609_comparison(self.old, self.new)
        self.assertTrue(result["reviewedSdkMigrationCompatible"])
        self.assertFalse(result["compatibleExistingScaleEncoding"])
        self.assertTrue(result["rawBreakingChanges"])
        self.assertFalse(result["reviewedEventChanges"][0]["binaryCompatible"])
        self.assertEqual(result["additions"], [])
        self.assertEqual(len(result["reviewedAdditions"]), 2)
        self.assertTrue(result["rawAdditions"])
        self.assertEqual(result["constantValueChanges"], [])
        self.assertEqual(result["reviewedSignedExtensionChange"]["requiresTransactionVersion"], 132)
        self.assertFalse(COMPARATOR.Comparison(self.old, self.new).run()["compatibleExistingScaleEncoding"])

    def test_changed_authorization_shape_is_rejected(self):
        primitive = next(k for k, v in self.types.items() if v["def"] == {"primitive": "u32"})
        self.rejects(lambda: self.types[self.authorize]["def"]["composite"]["fields"].append({"type": primitive}))

    def test_implicit_signing_bytes_are_rejected(self):
        primitive = next(k for k, v in self.types.items() if v["def"] == {"primitive": "u32"})
        self.rejects(lambda: self.new["extrinsic"]["signed_extensions"][0].update(additional_signed=primitive))

    def test_wrong_identity_is_rejected(self):
        self.rejects(lambda: self.types[self.authorize].update(path=["unreviewed", "AuthorizeCall"]))

    def test_extension_reorder_is_rejected(self):
        self.rejects(lambda: self.new["extrinsic"]["signed_extensions"].reverse())

    def test_existing_nested_encoding_change_is_rejected(self):
        primitive = next(value for value in self.types.values() if value["def"] == {"primitive": "u32"})
        self.rejects(lambda: primitive.update({"def": {"primitive": "u64"}}))

    def test_existing_storage_change_is_rejected(self):
        system = next(p for p in self.new["pallets"] if p["name"] == "System")
        account = next(entry for entry in system["storage"]["entries"] if entry["name"] == "Account")
        self.rejects(lambda: account["ty"]["Map"].update(hashers=["Identity"]))

    def test_wrong_extra_tuple_is_rejected(self):
        extrinsic = self.types[self.new["extrinsic"]["ty"]]
        extra = self.types[next(param["type"] for param in extrinsic["params"] if param["name"] == "Extra")]
        self.rejects(lambda: extra["def"]["tuple"].reverse())

    def test_code_updated_hash_drift_is_rejected(self):
        primitive = next(k for k, v in self.types.items() if v["def"] == {"primitive": "u32"})
        self.rejects(lambda: self.code_updated["fields"][0].update(type=primitive))

    def test_code_updated_index_drift_is_rejected(self):
        self.rejects(lambda: self.code_updated.update(index=250))

    def test_offence_field_type_drift_is_rejected(self):
        primitive = next(k for k, v in self.types.items() if v["def"] == {"primitive": "u32"})
        self.rejects(lambda: self.offence["fields"][1].update(type=primitive))

    def test_identity_call_index_drift_is_rejected(self):
        self.rejects(lambda: self.remove_registrar.update(index=25))

    def test_identity_call_noncompact_index_is_rejected(self):
        self.rejects(lambda: self.remove_registrar["fields"][0].update(type=self.u32))

    def test_identity_event_insertion_is_rejected(self):
        def insert():
            for value in self.identity_events[:-1]:
                if value["index"] > 0:
                    value["index"] += 1
            self.registrar_removed["index"] = 1
        self.rejects(insert)

    def test_identity_event_type_drift_is_rejected(self):
        u64 = next(k for k, v in self.types.items() if v["def"] == {"primitive": "u64"})
        self.rejects(lambda: self.registrar_removed["fields"][0].update(type=u64))

    def test_unreviewed_additional_call_is_rejected(self):
        self.rejects(lambda: self.identity_calls.append({"name": "unexpected", "index": 25, "fields": []}))


class AsyncStakingCompatibilityTests(unittest.TestCase):
    def setUp(self):
        # Build the already-reviewed SDK changes, then add a minimal explicit
        # staking review fixture. Its hashes are local to this test; production
        # approval hashes are never learned or overwritten by the verifier.
        Sdk2609CompatibilityTests.setUp(self)
        self.saved_shapes = COMPARATOR.ASYNC_ADDITION_SHAPES
        pallets = {pallet["name"]: pallet for pallet in self.new["pallets"]}
        self.staking = pallets["Staking"]
        self.wrapper_types = []
        def wrapper(inner, module, name):
            type_id = max(self.types) + 1
            definition = {"path": ["bounded_collections", module, name], "def": {"composite": {"fields": [{"type": inner}]}}}
            self.types[type_id] = definition
            self.new["types"]["types"].append({"id": type_id, "type": definition})
            self.wrapper_types.append(definition)
            return type_id
        entries = {entry["name"]: entry for entry in self.staking["storage"]["entries"]}
        bonded = entries["BondedEras"]["ty"]
        bonded["Plain"] = wrapper(bonded["Plain"], "bounded_vec", "BoundedVec")
        claimed = entries["ClaimedRewards"]["ty"]["Map"]
        claimed["value"] = wrapper(claimed["value"], "weak_bounded_vec", "WeakBoundedVec")
        points = self.types[entries["ErasRewardPoints"]["ty"]["Map"]["value"]]["def"]["composite"]["fields"]
        points[1]["type"] = wrapper(points[1]["type"], "bounded_btree_map", "BoundedBTreeMap")
        self.calls = self.types[self.staking["calls"]["ty"]]["def"]["variant"]["variants"]
        self.added_call = {"index": 240, "name": "reviewed_test_call", "fields": [{"name": "value", "type": self.u32}]}
        self.calls.append(self.added_call)
        self.bridge = {"name": "StakingAsyncBridge", "index": 121, "calls": None, "event": None, "error": None,
            "constants": [], "storage": {"prefix": "StakingAsyncBridge", "entries": [
                {"name": "State", "modifier": "Default", "default": [0, 0, 0, 0], "ty": {"Plain": self.u32}}]}}
        self.new["pallets"].append(self.bridge)
        COMPARATOR.ASYNC_ADDITION_SHAPES = {
            "Staking.calls.reviewed_test_call": COMPARATOR.shape_digest(COMPARATOR.addition_shape(self.new, "calls", self.added_call)),
            "StakingAsyncBridge": COMPARATOR.shape_digest(COMPARATOR.addition_shape(self.new, "pallet", self.bridge)),
        }

    def tearDown(self):
        COMPARATOR.ASYNC_ADDITION_SHAPES = self.saved_shapes

    def rejects(self, mutate):
        mutate()
        try:
            result = COMPARATOR.async_staking_comparison(self.old, self.new)
        except (ValueError, KeyError):
            return
        self.assertFalse(result["reviewedAsyncStakingMigrationCompatible"])

    def test_reviewed_additions_leave_existing_contracts_strict(self):
        result = COMPARATOR.async_staking_comparison(self.old, self.new)
        self.assertTrue(result["reviewedAsyncStakingMigrationCompatible"])
        self.assertEqual(result["retainedLegacyStorageOutsideMetadata"], [])
        self.assertEqual(result["requiresStorageMigration"], {"pallet": "Staking", "from": 16, "to": 18, "rawStateWitnessRequired": True})
        self.assertEqual(result["reviewedSignedExtensionChange"]["requiresTransactionVersion"], 133)
        self.assertEqual(result["additions"], [])
        self.assertEqual([row["path"] for row in result["reviewedStorageMetadataWrappers"]], [
            "Staking.storage.BondedEras", "Staking.storage.ClaimedRewards", "Staking.storage.ErasRewardPoints.individual"])

    def test_named_bounded_wrapper_fields_are_rejected(self):
        self.rejects(lambda: self.wrapper_types[0]["def"]["composite"]["fields"][0].update(name="different"))

    def test_extra_bounded_wrapper_fields_are_rejected(self):
        self.rejects(lambda: self.wrapper_types[0]["def"]["composite"]["fields"].append({"type": self.u32}))

    def test_unexpected_bounded_wrapper_identity_is_rejected(self):
        self.rejects(lambda: self.wrapper_types[1].update(path=["other", "WeakBoundedVec"]))

    def test_bounded_wrapper_inner_drift_is_rejected(self):
        self.rejects(lambda: self.wrapper_types[2]["def"]["composite"]["fields"][0].update(type=self.u32))

    def test_bounded_storage_hashers_remain_strict(self):
        claimed = next(entry for entry in self.staking["storage"]["entries"] if entry["name"] == "ClaimedRewards")
        self.rejects(lambda: claimed["ty"]["Map"].update(hashers=["Identity", "Twox64Concat"]))

    def test_bounded_storage_defaults_remain_strict(self):
        claimed = next(entry for entry in self.staking["storage"]["entries"] if entry["name"] == "ClaimedRewards")
        self.rejects(lambda: claimed.update(default=[1]))

    def test_new_call_field_drift_is_rejected(self):
        self.rejects(lambda: self.added_call["fields"][0].update(name="different"))

    def test_new_call_index_drift_is_rejected(self):
        self.rejects(lambda: self.added_call.update(index=241))

    def test_existing_staking_call_drift_is_rejected(self):
        bond = next(call for call in self.calls if call["index"] == 0)
        self.rejects(lambda: bond["fields"][0].update(name="different"))

    def test_old_storage_removal_is_rejected(self):
        entries = self.staking["storage"]["entries"]
        self.rejects(lambda: entries.remove(next(entry for entry in entries if entry["name"] == "Ledger")))

    def test_old_storage_hasher_drift_is_rejected(self):
        ledger = next(entry for entry in self.staking["storage"]["entries"] if entry["name"] == "Ledger")
        self.rejects(lambda: ledger["ty"]["Map"].update(hashers=["Identity"]))

    def test_old_event_index_drift_is_rejected(self):
        events = self.types[self.staking["event"]["ty"]]["def"]["variant"]["variants"]
        self.rejects(lambda: events[0].update(index=250))

    def test_old_error_index_drift_is_rejected(self):
        errors = self.types[self.staking["error"]["ty"]]["def"]["variant"]["variants"]
        self.rejects(lambda: errors[0].update(index=250))

    def test_unreviewed_addition_is_rejected(self):
        self.rejects(lambda: self.calls.append({"index": 241, "name": "not_reviewed", "fields": []}))

    def test_bridge_shape_drift_is_rejected(self):
        self.rejects(lambda: self.bridge["storage"]["entries"][0].update(default=[1, 0, 0, 0]))

    def test_bridge_cannot_gain_calls(self):
        self.rejects(lambda: self.bridge.update(calls=self.staking["calls"]))

    def test_missing_reviewed_addition_is_rejected(self):
        self.rejects(lambda: self.calls.remove(self.added_call))


if __name__ == "__main__":
    unittest.main(verbosity=2)
