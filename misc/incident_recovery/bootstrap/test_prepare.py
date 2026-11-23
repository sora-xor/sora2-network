import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("recovery_prepare", HERE / "prepare.py")
prepare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prepare)

class LegacyOverlayTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.legacy = subprocess.check_output(["git", "show", f"{prepare.LEGACY_TREE}:runtime/src/lib.rs"], cwd=prepare.ROOT, text=True)

    def test_checkpoint_runtime_and_metadata_layout_are_preserved(self):
        result = prepare.patch_runtime(self.legacy, 134)
        self.assertIn("spec_version: 134,", result)
        self.assertIn("transaction_version: 131,", result)
        # The runtime composition and signed extension aliases are untouched.
        start = self.legacy.index("\nconstruct_runtime!")
        end = self.legacy.index("impl_runtime_apis!", start)
        self.assertEqual(self.legacy[start:end], result[result.index("\nconstruct_runtime!"):result.index("impl_runtime_apis!")])
        self.assertIn("incident_recovery::execute_block(block)", result)
        self.assertNotIn("Executive::execute_block(block)", result)

    def test_shared_historical_bridge_resolver_is_wired_without_opening_ingress(self):
        result = prepare.patch_runtime(self.legacy, 134)
        self.assertIn("type IncomingAccountResolver = incident_recovery_incoming::PersistentAccountResolver;", result)
        self.assertIn("type RefundAccountResolver = incident_recovery_incoming::PersistentAccountResolver;", result)
        self.assertIn("pub mod incident_recovery_evm_source;", result)
        overlay = (prepare.HERE / "overlay/incident_recovery.rs").read_text()
        self.assertIn("pub fn replacement(", overlay)
        self.assertIn("Err(RecoveryError::WrongPhase)", overlay)

    def test_hashi_api_keeps_version_inventory_and_existing_methods(self):
        result = prepare.patch_runtime(self.legacy, 134)
        start = self.legacy.index("pub const VERSION:")
        end = self.legacy.index("\n};", start) + len("\n};")
        patched_start = result.index("pub const VERSION:")
        patched_end = result.index("\n};", patched_start) + len("\n};")
        self.assertEqual(self.legacy[start:end], result[patched_start:patched_end])
        self.assertEqual(result.count("eth_bridge_runtime_api::EthBridgeRuntimeApi<"), 1)
        self.assertEqual(result.count("fn recovery_hashi_prepare("), 1)
        self.assertEqual(result.count("fn recovery_hashi_prepare_source("), 1)
        self.assertEqual(result.count("fn recovery_hashi_admit("), 1)
        self.assertIn("mod incident_recovery_hashi_legacy_tests;", result)
        for spec in (135, 136, 137):
            self.assertNotIn("mod incident_recovery_hashi_legacy_tests;", prepare.patch_runtime(self.legacy, spec))
        api = (prepare.ROOT / "pallets/eth-bridge/runtime-api/src/lib.rs").read_text()
        self.assertNotIn("api_version", api)
        for method in ("get_requests", "get_approved_requests", "get_approvals", "get_account_requests", "get_registered_assets"):
            self.assertEqual(api.count(f"fn {method}("), 1)

    def test_hashi_api_manifest_retains_deployed_sdk_references(self):
        original = subprocess.check_output(["git", "show", f"{prepare.LEGACY_TREE}:pallets/eth-bridge/runtime-api/Cargo.toml"], cwd=prepare.ROOT, text=True)
        patched = prepare.patch_bridge_api_manifest(original)
        self.assertIn('scale-info = { version = "2",', patched)
        self.assertIn('"scale-info/std",', patched)
        self.assertEqual([row for row in original.splitlines() if "polkadot-sdk" in row],
                         [row for row in patched.splitlines() if "polkadot-sdk" in row])

    def test_only_repair_identity_changes_in_hold_export(self):
        result = prepare.patch_runtime(self.legacy, 135)
        self.assertIn("spec_version: 135,", result)
        self.assertIn("transaction_version: 132,", result)
        self.assertIn("incident_recovery_consensus::prove_grandpa(set_id, authority_id)", result)

    def test_all_maintenance_versions_use_the_abandoned_set_report_guard(self):
        for version in (134, 135, 136, 137):
            result = prepare.patch_runtime(self.legacy, version)
            self.assertIn("type EquivocationReportSystem = incident_recovery_consensus::RecoveryGrandpaReports;", result)
            self.assertNotIn("type EquivocationReportSystem = pallet_grandpa::EquivocationReportSystem<", result)
            self.assertIn("type EquivocationReportSystem = incident_recovery_babe_reports::RecoveryBabeReports;", result)
            self.assertNotIn("type EquivocationReportSystem = pallet_babe::EquivocationReportSystem<", result)

    def test_changed_or_duplicate_pinned_source_markers_fail_closed(self):
        with self.assertRaises(ValueError):
            prepare.patch_runtime(self.legacy.replace("spec_version: 134,", "spec_version: 138,"), 134)
        with self.assertRaises(ValueError):
            prepare.replace_once("marker marker", "marker", "new")

    def test_binding_rejects_policy_mismatch_before_reading_artifact_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "binding.json"
            path.write_text(json.dumps({"policyScale": "0x01", "policyCommitment": "0x" + "00" * 32,
                "operatorsScale": "0x01", "previousGrandpaSetId": 1, "firstRecoverySlot": 1,
                "finalWasm": "missing.wasm", "holdWasm": "missing.wasm"}))
            with self.assertRaisesRegex(ValueError, "mismatched"):
                prepare.bind_inputs(path, 134)

    def test_export_is_new_preserves_original_lock_and_is_never_release_ready(self):
        with tempfile.TemporaryDirectory() as tmp:
            destination = Path(tmp) / "legacy"
            result = prepare.prepare(destination, 134, None)
            lock = subprocess.check_output(["git", "show", f"{prepare.LEGACY_TREE}:Cargo.lock"], cwd=prepare.ROOT)
            self.assertEqual((destination / "Cargo.lock").read_bytes(), lock)
            self.assertEqual(result["baseCargoLock"]["sha256"], hashlib.sha256(lock).hexdigest())
            self.assertFalse(result["bound"])
            self.assertFalse(result["releaseReady"])
            self.assertIn("polkadot-stable2603-3", (destination / "runtime/Cargo.toml").read_text())
            self.assertNotIn("polkadot-stable2609", (destination / "runtime/Cargo.toml").read_text())
            # Permanent historical-message cancellations must survive every
            # maintenance version, not only the final runtime's bridge guard.
            guard = Path("vendor/sora2-common/pallets/types/src/recovery.rs")
            self.assertEqual((destination / guard).read_bytes(), (prepare.ROOT / guard).read_bytes())
            # Both maintenance and final runtimes must test the same authenticated
            # incoming path, including exact checkpoint-pending HASHI loads.
            for relative in ["runtime/src/incident_recovery_babe_reports.rs",
                             "pallets/eth-bridge/runtime-api/src/lib.rs",
                             "pallets/eth-bridge/runtime-api/src/recovery.rs",
                             "runtime/src/incident_recovery_hashi_witness.rs",
                             "runtime/src/incident_recovery_hashi_source.rs",
                             "runtime/src/incident_recovery_hashi_source_tests.rs",
                             "runtime/src/incident_recovery_hashi_witness_tests.rs",
                             "runtime/src/incident_recovery_hashi_legacy_tests.rs",
                             "runtime/src/incident_recovery_evm_source.rs",
                             "runtime/src/incident_recovery_evm_source_tests.rs",
                             "runtime/src/incident_recovery_incoming_hashi_source_tests.rs",
                             "runtime/src/incident_recovery_incoming_hashi_delayed_tests.rs",
                             "pallets/eth-bridge/src/recovery.rs"]:
                self.assertEqual((destination / relative).read_bytes(), (prepare.ROOT / relative).read_bytes())
                self.assertEqual(result["overlay"][relative], prepare.digest((prepare.ROOT / relative).read_bytes()))
            # Legacy calls retain their reviewed historical inventory. Importing
            # the current pallet wholesale would introduce runtime-138 retry calls.
            for relative in ("pallets/eth-bridge/src/lib.rs", "pallets/bridge-proxy/src/lib.rs"):
                exported = (destination / relative).read_text()
                self.assertNotIn("pub fn retry_refund(", exported)
                self.assertNotIn("pub fn finalize_approved_request(", exported)
            incoming = (destination / "pallets/eth-bridge/src/requests/incoming.rs").read_text()
            self.assertIn("crate::recovery::ensure_ordinary_transfer_source::<T>(&self.from)?;", incoming)
            self.assertIn("crate::recovery::ordinary_transfer_beneficiary::<T, R>(&self.from, &self.to)?", incoming)
            proxy = (destination / "pallets/bridge-proxy/src/lib.rs").read_text()
            self.assertIn("common::with_transaction(|| Self::refund_inner(", proxy)
            self.assertLess(proxy.index("T::RefundAccountResolver::resolve(beneficiary)?"),
                            proxy.index("T::HashiBridge::refund(network_id"))
            shared_tests = (destination / "runtime/src/incident_recovery_incoming.rs").read_text()
            self.assertIn("mod legacy_bridge_security_tests;", shared_tests)
            self.assertEqual(shared_tests.count("#[cfg(any())]"), 1)
            with self.assertRaisesRegex(ValueError, "existing"):
                prepare.prepare(destination, 134, None)

    def test_security_adapter_preserves_dispatch_inventory(self):
        # Check the exact retained files before/after applying the adapter rather
        # than assuming that the current runtime has the historical call layout.
        with tempfile.TemporaryDirectory() as tmp:
            destination = Path(tmp)
            paths = ["pallets/eth-bridge/src/recovery.rs", "pallets/eth-bridge/src/requests/incoming.rs",
                     "pallets/bridge-proxy/src/lib.rs", "pallets/bridge-proxy/src/mock.rs", "common/src/mock.rs"]
            # Export the reviewed base plus patch, without invoking prepare's adapter.
            archive = subprocess.check_output(["git", "archive", "--format=tar", prepare.LEGACY_TREE, *prepare.SOURCE_PATHS], cwd=prepare.ROOT)
            with prepare.tarfile.open(fileobj=prepare.io.BytesIO(archive)) as tar:
                tar.extractall(destination, filter="data")
            subprocess.run(["git", "apply", str(prepare.HERE / "bridge-recovery.patch")], cwd=destination, check=True)
            before = {path: (destination / path).read_text() for path in paths}
            prepare.patch_legacy_bridge_security(destination)
            for relative in ("pallets/eth-bridge/src/requests/incoming.rs", "pallets/bridge-proxy/src/lib.rs"):
                self.assertEqual(re.findall(r"#\[pallet::call_index\(\d+\)\]", before[relative]),
                                 re.findall(r"#\[pallet::call_index\(\d+\)\]", (destination / relative).read_text()))

if __name__ == "__main__":
    unittest.main()
