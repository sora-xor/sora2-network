"""Synthetic supporting-check tests. Never run Cargo, native recovery or Docker."""
import copy
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import supporting_checks_release as support
import production_build as build


class SupportingChecksTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='sora-nexus-support-check-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.source = self.root / 'source'
        self.source.mkdir()
        for name, raw in {'Cargo.lock': b'version = 4\n',
            'rust-toolchain.toml': b'[toolchain]\nchannel="nightly-2026-10-25"\n',
            'housekeeping/clippy.sh': b'fixed reviewed test shell fixture\n',
            'scripts/with_llvm_env.sh': b'fixed reviewed test wrapper fixture\n'}.items():
            self.put(self.source / name, raw)
        self.complete = {str(path.relative_to(self.source)): build.digest(path)
                         for path in self.source.rglob('*') if path.is_file()}
        self.production = {name: value for name, value in self.complete.items()
                           if name in ('Cargo.lock', 'rust-toolchain.toml')}
        self.paths = {}
        for role, raw in {'bootstrapWasm': b'exact134', 'holdWasm': b'exact135',
                          'finalWasm': b'exact138', 'policyScale': b'exact-policy'}.items():
            self.paths[role] = self.put(self.root / 'release' / role, raw)
        self.report = {'policy': {'commitment': 'a' * 64, 'evidenceCommitment': 'b' * 64},
            'artifacts': [{'role': role, **build.digest(path)} for role, path in self.paths.items()], 'gates': {key: {'status': 'verified', 'reason': 'synthetic fresh hook'}
                                      for key in build.release.GATES}, 'productionReady': False}
        self.report['gates'][support.GATE] = {'status': 'unverified'}
        self.original_artifacts = copy.deepcopy(self.report['artifacts'])
        self.manifest = self.put(self.root / 'inputs/manifest.json', {'schema': support.SCHEMA})
        self.refs = {}
        for key in ('sourceInventory', 'fullSourceInventory', 'buildInputs', 'legacyLock'):
            self.refs[key] = self.put(self.root / 'inputs' / (key + '.json'),
                self.production if key == 'sourceInventory' else self.complete if key == 'fullSourceInventory' else {})
        self.value = {key: {'file': path.name, 'sha256': build.digest(path)['sha256']}
                      for key, path in self.refs.items()}
        self.context = {'refs': self.refs, 'value': self.value, 'production': self.production,
            'complete': self.complete, 'inputPaths': {}, 'dataCommitment': 'b' * 64}
        self.verifier = self.put(self.root / 'native-verifier', b'synthetic executable, not run')
        self.tools = self.root / 'tool-launchers'
        for name in ('cargo', 'rustc'):
            self.put(self.tools / name, b'synthetic launcher, not run').chmod(0o555)
        self.helper = self.put(self.root / 'helper.py', b'synthetic pinned helper')
        self.targets = [self.root / name for name in ('checks-target', 'final-target', 'legacy-target')]
        for target in self.targets:
            target.mkdir()
        self.output = self.root / 'output'
        self.commands = []
        self.change = None

    def put(self, path, value):
        build.write(path, value)
        return path

    def fake_build(self, directory):
        self.put(directory / 'source-inventory.json', self.production)
        self.put(directory / 'policy.scale', self.paths['policyScale'].read_bytes())
        artifacts = {}
        for name, folder, version, role in [('134', 'build-134', 134, 'bootstrapWasm'),
            ('135', 'build-135', 135, 'holdWasm'), ('final138', 'build-final138', 138, 'finalWasm')]:
            relative = folder + '/runtime-' + str(version) + '.compact.compressed.wasm'
            path = self.put(directory / relative, self.paths[role].read_bytes())
            artifacts[name] = {'path': relative, **build.digest(path)}
        result = {'schema': 'sora-nexus-production-recovery-build-v1', 'fixtureOnly': False,
            'productionReady': False, 'status': 'passed', 'artifacts': artifacts,
            'files': {str(Path(name).relative_to(directory)): pin
                      for name, pin in support.retained_tree(directory).items()}}
        self.put(directory / 'BUILD-MANIFEST.json', result)
        return result

    def process(self, command, cwd, output, guard, env=None, timeout=3600, maximum=None, retain=None):
        self.commands.append((command, cwd, timeout, env))
        guard()
        output.mkdir(parents=True)
        raw = b'synthetic check output\n'
        if command[1:4] == ['-B', str(Path(build.__file__).resolve()), 'build']:
            directory = Path(command[command.index('--output') + 1])
            result = self.fake_build(directory)
            raw = build.serialized({'status': 'passed', 'manifest': str(directory / 'BUILD-MANIFEST.json'),
                'manifestPin': build.pin_bytes(build.serialized(result)), 'productionReady': False})
        receipt = {'exitCode': 0, 'pid': 71, 'command': command, 'guardError': None}
        for name, data in [('launch.json', {'command': command}), ('started.json', {'pid': 71}),
                           ('stdout.log', raw), ('stderr.log', b''), ('execution.json', receipt)]:
            pin = build.write(output / name, data)
            if retain:
                retain(output / name, pin)
        if self.change:
            self.change(output, receipt)
        guard()
        return raw, receipt

    def run_hook(self):
        with patch.object(build, 'ROOT', self.source), \
             patch.object(support, 'bind', return_value=self.context), \
             patch.object(support, 'inventory', side_effect=lambda root:
                {str(p.relative_to(root)): build.digest(p) for p in root.rglob('*') if p.is_file()}), \
             patch.object(support, 'helper_pins', side_effect=lambda: {str(self.helper): build.digest(self.helper)}), \
             patch.object(support, 'source_binding', return_value='c' * 64), \
             patch.object(support, 'environment', return_value=({'PATH': str(self.tools)}, {'controlled': {}})), \
             patch.object(build, 'process', side_effect=self.process):
            return support.package_hook(self.report, self.paths, self.manifest, 'c' * 64,
                self.output, *self.targets, self.verifier, build.digest(self.verifier)['sha256'])

    def test_fresh_exact_rebuild_and_all_fresh_gates_clear_only_aggregate(self):
        result = self.run_hook()
        self.assertEqual(len(self.commands), 9)
        self.assertEqual(set(result['checks']), {*support.CHECKS, 'cargo-version', 'rustc-version'})
        self.assertTrue(result['aggregate']['verified'])
        self.assertEqual(self.report['gates'][support.GATE]['status'], 'verified')
        self.assertFalse(self.report['productionReady'])
        self.assertEqual(set(result['rebuiltArtifacts']), {'bootstrapWasm', 'holdWasm', 'finalWasm'})
        self.assertTrue(all(timeout >= 24*3600 for _, _, timeout, _ in self.commands))
        self.assertEqual(self.commands[-1][2], 4 * 24 * 3600)
        self.assertTrue((self.output / 'checked-source/housekeeping/clippy.sh').is_file())

    def test_incomplete_scenarios_remain_unverified_despite_passed_checks_and_rebuild(self):
        for name in ('maintenanceEpochLivenessAndStaking', 'chunksCrashesAndRestarts'):
            self.report['gates'][name]['status'] = 'unverified'
        result = self.run_hook()
        self.assertTrue(result['aggregate']['freshBuildJoinVerified'])
        self.assertFalse(result['aggregate']['verified'])
        self.assertEqual(self.report['gates'][support.GATE]['status'], 'unverified')
        self.assertEqual(result['aggregate']['missingPrerequisites'], ['chunksCrashesAndRestarts', 'maintenanceEpochLivenessAndStaking'])

    def test_repair_requires_fresh_forward_context_before_any_command(self):
        self.paths['repair137PolicyScale'] = self.put(self.root / 'release/repair', b'revised')
        self.report['artifacts'].append({'role': 'repair137PolicyScale', **build.digest(self.paths['repair137PolicyScale'])})
        with self.assertRaisesRegex(ValueError, 'exact forward-replay context'):
            self.run_hook()
        self.assertEqual(self.commands, [])


    def test_failed_process_does_not_register_or_promote_partial_outputs(self):
        def fail(output, receipt):
            if output.name == 'runtime-tests':
                raise ValueError('synthetic actual check failure')
        self.change = fail
        with self.assertRaisesRegex(ValueError, 'actual check failure'):
            self.run_hook()
        self.assertEqual(self.report['artifacts'], self.original_artifacts)
        self.assertNotIn('supportingChecks', self.report)

    def test_boolean_exit_is_not_success(self):
        self.change = lambda output, receipt: receipt.update(exitCode=False)
        with self.assertRaisesRegex(ValueError, 'supporting command failed'):
            self.run_hook()
        self.assertEqual(self.report['artifacts'], self.original_artifacts)

    def test_source_change_during_check_is_rejected(self):
        self.change = lambda output, receipt: (self.source / 'Cargo.lock').write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'bound file changed'):
            self.run_hook()

    def test_new_source_file_is_rejected(self):
        self.change = lambda output, receipt: (self.source / 'new.rs').write_bytes(b'new code')
        with self.assertRaisesRegex(ValueError, 'source set changed'):
            self.run_hook()

    def test_prior_receipt_mutation_is_rejected(self):
        def change(output, receipt):
            if output.name == 'clippy-mainnet':
                (self.output / 'checks/rustc-version/execution.json').write_bytes(b'{}')
        self.change = change
        with self.assertRaisesRegex(ValueError, 'bound file changed'):
            self.run_hook()
        self.assertEqual(self.report['artifacts'], self.original_artifacts)

    def test_prerequisite_result_mutation_is_rejected(self):
        self.change = lambda output, receipt: self.report['gates']['checkpointData'].update(reason='rebound')
        with self.assertRaisesRegex(ValueError, 'prerequisite gate results changed'):
            self.run_hook()

    def test_fresh_build_manifest_mutation_after_child_exit_is_rejected(self):
        def change(output, receipt):
            if output.name == 'rebuild-process':
                manifest = self.output / 'rebuilt/BUILD-MANIFEST.json'
                value = build.parse(manifest.read_bytes())
                value['status'] = 'forged'
                manifest.write_bytes(build.serialized(value))
        self.change = change
        with self.assertRaisesRegex(ValueError, 'file pin differs'):
            self.run_hook()
        self.assertEqual(self.report['artifacts'], self.original_artifacts)

    def test_rebuild_evidence_change_after_validation_cannot_rebind_retained_pin(self):
        original = support.verify_rebuild
        def mutate(directory, *args):
            result = original(directory, *args)
            (directory / 'source-inventory.json').write_bytes(b'{}')
            return result
        with patch.object(support, 'verify_rebuild', side_effect=mutate):
            with self.assertRaisesRegex(ValueError, 'bound file changed'):
                self.run_hook()
        self.assertEqual(self.report['artifacts'], self.original_artifacts)

    def test_release_artifact_mutation_before_admission_is_rejected(self):
        self.paths['holdWasm'].write_bytes(b'changed before supporting hook')
        with self.assertRaisesRegex(ValueError, 'artifact changed before supporting'):
            self.run_hook()
        self.assertEqual(self.commands, [])

    def test_unknown_generated_file_blocks_registration(self):
        def change(output, receipt):
            if output.name == 'tool-clippy':
                self.put(self.output / 'unregistered.json', {})
        self.change = change
        with self.assertRaisesRegex(ValueError, 'unexpected or changed supporting output'):
            self.run_hook()
        self.assertEqual(self.report['artifacts'], self.original_artifacts)

    def test_exact_wasm_rebuild_mismatch_is_rejected(self):
        original = self.fake_build
        def changed(directory):
            result = original(directory)
            file = directory / result['artifacts']['135']['path']
            file.write_bytes(b'other hold runtime')
            result['artifacts']['135'].update(build.digest(file))
            result['files'][str(file.relative_to(directory))] = build.digest(file)
            (directory / 'BUILD-MANIFEST.json').write_bytes(build.serialized(result))
            return result
        with patch.object(self, 'fake_build', side_effect=changed):
            with self.assertRaisesRegex(ValueError, 'rebuilt bytes differ'):
                self.run_hook()

    def test_rebuild_source_inventory_mismatch_is_rejected(self):
        directory = self.root / 'wrong-build'
        result = self.fake_build(directory)
        (directory / 'source-inventory.json').write_bytes(build.serialized({}))
        with self.assertRaisesRegex(ValueError, 'another source inventory'):
            support.verify_rebuild(directory, result, self.paths, self.production)

    def test_fixed_commands_cover_all_modes_and_never_clean_or_change_jobs(self):
        commands = support.commands(self.source)
        self.assertEqual(set(commands), set(support.CHECKS))
        for mode in ('mainnet', 'try-runtime', 'extended'):
            self.assertEqual(commands['clippy-' + mode][-1], mode)
        flattened = [item for command in commands.values() for item in command]
        self.assertNotIn('clean', flattened)
        self.assertNotIn('--jobs', flattened)
        self.assertEqual(commands['tool-clippy'][-3:], ['--', '-D', 'warnings'])

    def test_missing_extra_or_boolean_prerequisites_do_not_promote(self):
        gates = copy.deepcopy(self.report['gates'])
        gates['fullAndWarpSynchronization']['status'] = True
        self.assertFalse(support.aggregate(gates, True, [])['verified'])
        self.assertFalse(support.aggregate(self.report['gates'], 1, [])['verified'])
        del gates['checkpointData']
        with self.assertRaisesRegex(ValueError, 'gate inventory'):
            support.aggregate(gates, True, [])

    def test_target_alias_or_reuse_is_rejected_before_any_process(self):
        self.targets[2] = self.targets[1]
        with self.assertRaisesRegex(ValueError, 'separate persistent'):
            self.run_hook()
        self.assertEqual(self.commands, [])

    def test_nested_targets_are_rejected_before_any_process(self):
        self.targets[2] = self.targets[1] / 'nested'
        self.targets[2].mkdir()
        with self.assertRaisesRegex(ValueError, 'must not contain'):
            self.run_hook()
        self.assertEqual(self.commands, [])

    def test_unreviewed_home_and_ancestor_cargo_config_is_rejected_without_reading(self):
        home = self.root / 'personal'
        config = self.put(home / '.cargo/config.toml', b'must not be read or copied')
        with self.assertRaisesRegex(ValueError, 'ambient Cargo configuration'):
            support.reject_ambient_cargo_config({'HOME': str(home)}, self.source)
        config.unlink()
        support.reject_ambient_cargo_config({'HOME': str(home)}, self.source)
        ancestor = self.put(self.root / '.cargo/config', b'unreviewed parent config')
        with self.assertRaisesRegex(ValueError, 'ambient Cargo configuration'):
            support.reject_ambient_cargo_config({}, self.source)
        ancestor.unlink()

    def test_helper_source_pin_mismatch_rejected_before_binding(self):
        with patch.object(support, 'source_binding', return_value='d' * 64):
            with self.assertRaisesRegex(ValueError, 'verifier source differs'):
                support.package_hook(self.report, self.paths, self.manifest, 'c' * 64,
                    self.output, *self.targets, self.verifier, build.digest(self.verifier)['sha256'])

    def test_no_serialized_success_can_clear_aggregate_without_fresh_join(self):
        gates = {name: {'status': 'verified', 'productionReady': True} for name in build.release.GATES}
        self.assertFalse(support.aggregate(gates, False, [])['verified'])

    def real_binding(self):
        from test_production_build import ProductionBuildTests
        fixture = ProductionBuildTests('test_bounded_complete_input_graph_and_data_commitment')
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        inputs, path = fixture.bundle()
        _, input_paths, _, commitment = build.load_inputs(path, build.digest(path)['sha256'])
        self.report['policy']['evidenceCommitment'] = commitment
        for key, role in ((inputs['compose']['csv'], 'signedMappingCsv'),
            (inputs['operatorsScale'], 'operatorsScale'), (inputs['compose']['proofs'], 'ownershipProofs'),
            (inputs['derivedInventory'], 'derivedAccountInventory')):
            self.paths[role] = input_paths[key]
        self.refs['buildInputs'] = path
        # References are canonical paths relative to this new input root.
        config = {'schema': support.SCHEMA, 'finalLock': None}
        for name in ('sourceInventory', 'fullSourceInventory', 'legacyLock'):
            source = self.refs[name]
            destination = fixture.root / source.name
            destination.write_bytes(source.read_bytes())
            config[name] = {'file': destination.name, 'sha256': build.digest(destination)['sha256']}
        config['buildInputs'] = {'file': path.name, 'sha256': build.digest(path)['sha256']}
        manifest = fixture.artifact('supporting.json', config)
        return manifest, config

    def bound(self, manifest):
        with patch.object(build, 'ROOT', self.source), \
             patch.object(build, 'source_inventory', return_value=self.production), \
             patch.object(support, 'inventory', return_value=self.complete):
            return support.bind(manifest, self.report, self.paths)

    def test_real_input_loader_binds_complete_sources_mapping_and_data_commitment(self):
        manifest, _ = self.real_binding()
        result = self.bound(manifest)
        self.assertEqual(result['complete'], self.complete)
        self.assertEqual(result['dataCommitment'], self.report['policy']['evidenceCommitment'])
        self.report['policy']['evidenceCommitment'] = 'd' * 64
        with self.assertRaisesRegex(ValueError, 'data commitment differs'):
            self.bound(manifest)

    def test_binding_rejects_mismatched_release_mapping_or_incomplete_source_set(self):
        manifest, config = self.real_binding()
        self.paths['signedMappingCsv'] = self.put(self.root / 'wrong.csv', b'other mapping')
        with self.assertRaisesRegex(ValueError, 'production input differs'):
            self.bound(manifest)
        self.paths['signedMappingCsv'] = manifest.parent / 'mapping.csv'
        source_manifest = manifest.parent / config['fullSourceInventory']['file']
        source_manifest.write_bytes(build.serialized(self.production))
        config['fullSourceInventory']['sha256'] = build.digest(source_manifest)['sha256']
        manifest.write_bytes(build.serialized(config))
        with self.assertRaisesRegex(ValueError, 'supporting source differs'):
            self.bound(manifest)

    def test_binding_rejects_unknown_commands_and_public_fixture_input(self):
        manifest, config = self.real_binding()
        config['command'] = ['echo', 'this is never run']
        manifest.write_bytes(build.serialized(config))
        with self.assertRaisesRegex(ValueError, 'unknown supporting-check'):
            self.bound(manifest)
        del config['command']
        data = manifest.parent / config['buildInputs']['file']
        inputs = build.parse(data.read_bytes())
        inputs['fixtureOnly'] = True
        data.write_bytes(build.serialized(inputs))
        config['buildInputs']['sha256'] = build.digest(data)['sha256']
        manifest.write_bytes(build.serialized(config))
        with self.assertRaises(ValueError):
            self.bound(manifest)

    def test_producer_stdout_pins_in_process_manifest_bytes(self):
        result = {'schema': 'synthetic-test-only', 'status': 'passed', 'files': {'exact': 'bytes'}}
        args = ['production_build.py', 'build']
        for name in ('inputs', 'source-inventory', 'tool', 'legacy-lock'):
            args += ['--' + name, '/unused', '--' + name + '-sha256', 'a' * 64]
        for name in ('output', 'final-target', 'legacy-target'):
            args += ['--' + name, '/unused-' + name]
        stdout = io.StringIO()
        with patch('sys.argv', args), patch.object(build, 'build', return_value=result), contextlib.redirect_stdout(stdout):
            build.main()
        value = json.loads(stdout.getvalue())
        self.assertEqual(value['manifestPin'], build.pin_bytes(build.serialized(result)))
        self.assertEqual(value['manifest'], '/unused-output/BUILD-MANIFEST.json')


if __name__ == '__main__':
    unittest.main()
