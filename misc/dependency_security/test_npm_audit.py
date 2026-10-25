import json
from pathlib import Path
import tempfile
import unittest

from prepare_npm_audit import HISTORICAL_PREFIX, REVIEWED_PACKAGES, canonical_lock, check_reviewed_native_packages, prepare


class AuditStagingTests(unittest.TestCase):
    def test_historical_relocation_preserves_every_package_record(self):
        record = {"version": "1.2.3", "integrity": "sha512-example", "dependencies": {"bar": "1"}}
        original = {"lockfileVersion": 3, "packages": {"": {}, HISTORICAL_PREFIX + "node_modules/foo": record}}
        result, count = canonical_lock(original)
        self.assertEqual(count, 1)
        self.assertEqual(result["packages"]["node_modules/foo"], record)
        self.assertIn(HISTORICAL_PREFIX + "node_modules/foo", original["packages"])

    def test_unresolved_mixed_or_empty_graphs_fail(self):
        for packages in ({"": {}}, {"": {}, "../../unknown/node_modules/foo": {"version": "1"}},
                         {"": {}, "node_modules/foo": {"link": True}}):
            with self.assertRaises(ValueError):
                canonical_lock({"lockfileVersion": 3, "packages": packages})

    def native_graph(self, chopsticks=False):
        names = list(REVIEWED_PACKAGES) if chopsticks else list(REVIEWED_PACKAGES)[:4]
        pins = ["@polkadot/wasm-crypto", "@polkadot/rpc-provider", "fetch-blob"]
        if chopsticks:
            pins.append("@acala-network/chopsticks-core")
        packages = {"": {"dependencies": {name: "file:../../" + REVIEWED_PACKAGES[name]
                                            for name in pins}}}
        for name in names:
            packages["node_modules/" + name] = {"resolved": "file:../../" + REVIEWED_PACKAGES[name]}
        return {"packages": packages}

    def check_native(self, lock):
        check_reviewed_native_packages(lock, Path("/repo"), Path("/repo/release/validation"))

    def test_reviewed_native_graph_rejects_duplicate_or_retired_binaries(self):
        lock = self.native_graph()
        self.check_native(lock)
        lock["packages"]["node_modules/smoldot"] = {"version": "2.0.26"}
        with self.assertRaisesRegex(ValueError, "retired embedded"):
            self.check_native(lock)
        del lock["packages"]["node_modules/smoldot"]
        lock["packages"]["node_modules/other/node_modules/@polkadot/wasm-crypto"] = {"resolved": "https://registry.npmjs.org/old"}
        with self.assertRaisesRegex(ValueError, "exactly one canonical reviewed"):
            self.check_native(lock)

    def test_removed_or_registry_direct_pin_does_not_disable_guard(self):
        for name in ("@polkadot/wasm-crypto", "@polkadot/rpc-provider", "fetch-blob", "@acala-network/chopsticks-core"):
            for replacement in (None, "7.5.4", "file:../../vendor/unreviewed"):
                with self.subTest(name=name, replacement=replacement):
                    lock = self.native_graph(chopsticks=True)
                    direct = lock["packages"][""]["dependencies"]
                    if replacement is None:
                        del direct[name]
                    else:
                        direct[name] = replacement
                    with self.assertRaisesRegex(ValueError, "required reviewed direct pin"):
                        self.check_native(lock)

    def test_known_operational_project_policy_cannot_be_removed_with_all_pins(self):
        for project in ("runtime-upgrade-4.8.12/validation", "misc/polkadotjs/bug-hunt/frontend-repro",
                        "runtime-rehearsal-dependency-migrations-2026-10-25/validation",
                        "runtime-rehearsal-sdk2609-2026-10-25/validation"):
            with self.subTest(project=project):
                # Even removing every reviewed record cannot disable the policy
                # for an operational project before npm checks graph completeness.
                lock = {"packages": {"": {"dependencies": {"other": "1.0.0"}},
                                     "node_modules/other": {"version": "1.0.0"}}}
                with self.assertRaisesRegex(ValueError, "required reviewed direct pin"):
                    check_reviewed_native_packages(lock, Path("/repo"), Path("/repo") / project)

    def test_wrong_local_endpoint_or_missing_transitive_package_is_rejected(self):
        self.check_native(self.native_graph(chopsticks=True))
        for name in REVIEWED_PACKAGES:
            for replacement in (None, "file:../../vendor/unreviewed", "https://registry.npmjs.org/old"):
                with self.subTest(name=name, replacement=replacement):
                    lock = self.native_graph(chopsticks=True)
                    location = "node_modules/" + name
                    if replacement is None:
                        del lock["packages"][location]
                    else:
                        lock["packages"][location]["resolved"] = replacement
                    with self.assertRaisesRegex(ValueError, "exactly one canonical reviewed"):
                        self.check_native(lock)

    def test_local_package_tree_is_staged_with_original_relative_paths(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            repo = base / "repo"
            project = repo / "release/validation"
            project.mkdir(parents=True)
            for name in ("core", "executor"):
                (repo / "vendor" / name).mkdir(parents=True)
            core = {"name": "core", "version": "1.0.0", "dependencies": {"executor": "file:../executor"}}
            (repo / "vendor/core/package.json").write_text(json.dumps(core))
            (repo / "vendor/executor/package.json").write_text(json.dumps({"name": "executor", "version": "1.0.0"}))
            (repo / "vendor/executor/runtime.wasm").write_bytes(b"wasm-fixture")
            package = {"dependencies": {"core": "file:../../vendor/core"}}
            manifest, lock = project / "rehearsal-package.json", project / "rehearsal-package-lock.json"
            manifest.write_text(json.dumps(package))
            lock.write_text(json.dumps({"lockfileVersion": 3, "packages": {"": package, "node_modules/core": core}}))
            original = manifest.read_bytes(), lock.read_bytes()
            staged = prepare(repo, manifest, lock, base / "audit")
            self.assertEqual((staged / "../../vendor/executor/runtime.wasm").read_bytes(), b"wasm-fixture")
            self.assertEqual((manifest.read_bytes(), lock.read_bytes()), original)


if __name__ == "__main__":
    unittest.main()
