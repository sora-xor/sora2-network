#!/usr/bin/env python3
"""Offline parser regressions; no Cargo or RPC invocation."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('native_runner', Path(__file__).with_name('run-native-remote.py'))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class FeatureGuardTests(unittest.TestCase):
    def test_clean_graph_with_dev_dependencies(self):
        result = runner.validate_feature_graph('framenode-runtime v4.8.13 (/repo/runtime)|default,std,try-runtime\n[dev-dependencies]\nframenode-chain-spec v4.8.13 (/repo/node/chain_spec)|test\nframenode-runtime v4.8.13 (/repo/runtime)|default,std,try-runtime (*)\n')
        self.assertEqual(result['runtimeFeatures'], ['default', 'std', 'try-runtime'])

    def test_each_forbidden_root_feature_is_rejected(self):
        for feature in ['private-net', 'stage', 'wip', 'runtime-benchmarks', 'reduced-pswap-reward-periods']:
            with self.subTest(feature=feature), self.assertRaisesRegex(RuntimeError, 'Nonproduction features'):
                runner.validate_feature_graph(f'framenode-runtime v4.8.13|std,try-runtime,{feature}\n')

    def test_dependency_only_feature_contamination_is_rejected(self):
        with self.assertRaisesRegex(RuntimeError, 'Nonproduction features'):
            runner.validate_feature_graph('framenode-runtime v4.8.13|std,try-runtime\nfarming v0.1.0|std,private-net\n')

    def test_missing_runtime_or_requested_features_is_rejected(self):
        for text in ['', 'framenode-runtime v4.8.13|std\n', 'other-runtime v1.0.0|std,try-runtime\n']:
            with self.subTest(text=text), self.assertRaisesRegex(RuntimeError, 'exactly one'):
                runner.validate_feature_graph(text)

    def test_unrecognized_graph_format_fails_closed(self):
        with self.assertRaisesRegex(RuntimeError, 'Unrecognized Cargo'):
            runner.validate_feature_graph('framenode-runtime v4.8.13 (unexpected format)\n')


if __name__ == '__main__':
    unittest.main()
