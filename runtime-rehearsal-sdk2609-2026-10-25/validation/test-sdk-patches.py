#!/usr/bin/env python3
"""Negative checks for SDK source/provenance reconstruction."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location('verify_sdk_patches', Path(__file__).with_name('verify-sdk-patches.py'))
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)


class ProvenanceTests(unittest.TestCase):
    def test_modified_missing_or_extra_source_is_rejected(self):
        expected = {'src/lib.rs': 'a', 'tests/regression.rs': 'b'}
        for actual in ({'src/lib.rs': 'c', 'tests/regression.rs': 'b'},
                       {'src/lib.rs': 'a'}, {**expected, 'src/extra.rs': 'c'}):
            with self.subTest(actual=actual), self.assertRaises(ValueError):
                VERIFY.compare_sources(expected, actual)

    def test_override_cannot_apply_to_different_upstream_version(self):
        with self.assertRaises(ValueError):
            VERIFY.overrides({'dependencies': {'dependency': '2'}}, [
                {'path': ['dependencies', 'dependency'], 'upstream': '1', 'local': '3'}])

    def test_override_changes_only_named_value(self):
        before = {'dependencies': {'reviewed': '1', 'unrelated': '2'}}
        after = VERIFY.overrides(before, [
            {'path': ['dependencies', 'reviewed'], 'upstream': '1', 'local': '3'}])
        self.assertEqual(after, {'dependencies': {'reviewed': '3', 'unrelated': '2'}})
        self.assertEqual(before['dependencies']['reviewed'], '1')

    def test_inheritance_preserves_features_and_default_features(self):
        manifest = {'package': {'version': {'workspace': True}},
                    'dependencies': {'dependency': {'workspace': True, 'features': ['local']}}}
        workspace = {'package': {'version': '9'}, 'dependencies': {
            'dependency': {'path': 'some/path', 'version': '1', 'default-features': False,
                           'features': ['inherited']}}}
        result = VERIFY.normalize(manifest, workspace)
        self.assertEqual(result['package']['version'], '9')
        self.assertEqual(result['dependencies']['dependency'], {
            'git': 'https://github.com/paritytech/polkadot-sdk.git', 'tag': 'polkadot-stable2609',
            'default-features': False, 'features': ['inherited', 'local']})
        self.assertTrue(manifest['dependencies']['dependency']['workspace'])

    def test_materialized_docs_equal_upstream_symlink(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'upstream').mkdir()
            (root / 'docs').mkdir()
            (root / 'docs/example.rs').write_text('fn documented() {}\n')
            (root / 'upstream/docs').symlink_to('../docs', target_is_directory=True)
            (root / 'local/docs').mkdir(parents=True)
            (root / 'local/docs/example.rs').write_text('fn documented() {}\n')
            VERIFY.compare_sources(VERIFY.inventory(root / 'upstream'), VERIFY.inventory(root / 'local'))


if __name__ == '__main__':
    unittest.main()
