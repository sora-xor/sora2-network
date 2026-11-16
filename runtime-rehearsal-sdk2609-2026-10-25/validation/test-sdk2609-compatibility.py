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


if __name__ == "__main__":
    unittest.main(verbosity=2)
