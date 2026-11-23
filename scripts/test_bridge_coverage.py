"""Regression checks for the coverage gate and its narrow generated-code exclusion."""

import contextlib
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import bridge_coverage as coverage
import bridge_coverage_snapshot as snapshots
from bridge_coverage_exclusions import remove_empty_call_lines


class CoverageTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def lcov(self, filename, records):
        path = self.root / filename
        path.write_text("".join(
            f"SF:{source}\n" + "".join(f"DA:{line},{count}\n" for line, count in hits)
            + "end_of_record\n" for source, hits in records
        ))
        return path

    def test_union_counts_source_lines_once_and_preserves_uncovered_lines(self):
        source = "pallets/eth-bridge/src/recovery.rs"
        first = self.lcov("first.lcov", [(source, [(1, 0), (2, 1), (3, 0)])])
        second = self.lcov("second.lcov", [(source, [(1, 2), (2, 0), (4, 0)])])
        self.assertEqual(coverage.read_lcov([first, second])[Path(source)], {1: 2, 2: 1, 3: 0, 4: 0})

    def test_scope_excludes_harnesses_but_keeps_migrations_and_handwritten_weights(self):
        for path in ["pallets/eth-bridge/src/tests/ocw.rs", "pallets/bridge-proxy/src/coverage.rs", "pallets/bridge-proxy/src/mock.rs", "pallets/bridge-proxy/src/weights.rs", "common/src/lib.rs"]:
            self.assertIsNone(coverage.component(Path(path)), path)
        self.assertEqual(coverage.component(Path("pallets/bridge-proxy/src/migrations.rs")), "bridge-proxy")
        self.assertEqual(coverage.component(Path("vendor/sora2-common/pallets/beefy-light-client/src/weights.rs")), "beefy-light-client")

    def gate(self, covered, *, include_second=True):
        records = []
        components = {"first": "first/src", "second": "second/src"}
        for name, hits in [("first", covered), ("second", 100)]:
            source = self.root / f"{name}/src/lib.rs"
            source.parent.mkdir(parents=True, exist_ok=True)
            source.write_text("line\n" * 100)
            if name != "second" or include_second:
                records.append((str(source), [(n, int(n <= hits)) for n in range(1, 101)]))
        report = self.lcov("gate.lcov", records)
        output = self.root / "report"
        with patch.object(coverage, "ROOT", self.root), patch.object(coverage, "COMPONENTS", components), patch.object(sys, "argv", ["bridge_coverage.py", str(report), "--output-dir", str(output)]), contextlib.redirect_stdout(io.StringIO()):
            result = coverage.main()
        return result, json.loads((output / "summary.json").read_text())

    def test_exactly_ninety_nine_fails_even_when_aggregate_exceeds_ninety_nine(self):
        result, report = self.gate(99)
        self.assertEqual(result, 1)
        self.assertFalse(report["components"]["first"]["passed"])
        self.assertTrue(report["components"]["second"]["passed"])

    def test_every_component_must_exceed_threshold(self):
        result, _ = self.gate(100)
        self.assertEqual(result, 0)
        result, report = self.gate(100, include_second=False)
        self.assertEqual(result, 1)
        self.assertEqual(report["components"]["second"]["lines"], 0)

    def test_snapshot_detects_shared_runtime_fixture_changes(self):
        helper = self.root / "common/src/mock.rs"
        helper.parent.mkdir(parents=True)
        helper.write_text("type OperationFilter = ();\n")
        with patch.object(snapshots, "ROOT", self.root), patch.object(snapshots, "COMPONENTS", {}):
            before = snapshots.snapshot()
            helper.write_text("type OperationFilter = BridgePolicy;\n")
            after = snapshots.snapshot()
        self.assertIn("common/src/mock.rs", before)
        self.assertNotEqual(before["common/src/mock.rs"], after["common/src/mock.rs"])


class EmptyCallProvenanceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = Path("vendor/sora2-common/pallets/leaf-provider/src/lib.rs")
        path = self.root / self.source
        path.parent.mkdir(parents=True)
        path.write_text("#[frame_support::pallet]\npub mod pallet {}\n")

    def function(self, name, first, last, *, count=0, kind=0, file=None):
        return {"name": name, "count": count, "filenames": [str(self.root / (file or self.source))], "regions": [[first, 1, last, 2, count, 0, 0, kind]]}

    def test_only_synthetic_zero_lines_removed_real_uncovered_overlap_retained(self):
        files = {self.source: {1: 0, 2: 0, 3: 0, 4: 5, 5: 0}}
        functions = [self.function("_RNv4Call17get_dispatch_info", 1, 4), self.function("_RNv6Pallet10extra_data", 2, 3)]
        removed = remove_empty_call_lines(files, functions, self.root)
        self.assertEqual(files[self.source], {2: 0, 3: 0, 4: 5, 5: 0})
        self.assertEqual(removed[str(self.source)]["lines"], [1])

    def test_callable_static_metadata_and_nonallowlisted_files_remain(self):
        other = Path("pallets/bridge-proxy/src/lib.rs")
        files = {self.source: {1: 0, 2: 0, 3: 0}, other: {1: 0}}
        functions = [self.function("_RNv4Call13get_call_name", 1, 3), self.function("_RNv4Call14get_call_names", 1, 1), self.function("_RNv6Pallet14call_functions", 2, 2), self.function("_RNv4Call16get_call_indices", 3, 3), self.function("_RNv4Call13get_call_name", 1, 1, file=other)]
        self.assertEqual(remove_empty_call_lines(files, functions, self.root), {})
        self.assertEqual(files[other], {1: 0})

    def test_gap_and_skipped_regions_cannot_suppress_source(self):
        for kind in [1, 2, 3]:
            files = {self.source: {1: 0}}
            remove_empty_call_lines(files, [self.function("_RNv4Call17get_dispatch_info", 1, 1, kind=kind)], self.root)
            self.assertEqual(files[self.source], {1: 0})

    def test_executed_stub_or_new_dispatchable_fails_closed(self):
        with self.assertRaisesRegex(ValueError, "executed"):
            remove_empty_call_lines({self.source: {1: 0}}, [self.function("_RNv4Call17get_dispatch_info", 1, 1, count=1)], self.root)
        for attribute in ["#[pallet::call]", "#[pallet::call(weight = <T as Config>::WeightInfo)]", "#[pallet::call(weight(<T as Config>::WeightInfo))]"]:
            with self.subTest(attribute=attribute):
                (self.root / self.source).write_text(attribute + "\nimpl<T: Config> Pallet<T> {}\n")
                with self.assertRaisesRegex(ValueError, "dispatchables"):
                    remove_empty_call_lines({self.source: {1: 0}}, [], self.root)

    def test_handwritten_method_cannot_be_mistaken_for_generated_stub(self):
        (self.root / self.source).write_text("fn get_call_name() {}\n")
        with self.assertRaisesRegex(ValueError, "handwritten"):
            remove_empty_call_lines({self.source: {1: 0}}, [], self.root)


if __name__ == "__main__":
    unittest.main()
