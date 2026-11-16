"""Offline trust-boundary tests. Synthetic receipts/binaries are never execution evidence."""
import hashlib
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import production_native as native
from test_release_package import policy, wasm, h
import hashi_codec as scale


class ProductionNativeTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        tool_root = self.root / "tools"
        self.put(tool_root / "scripts/with_llvm_env.sh", b"synthetic wrapper; never executed\n")
        override = patch.object(native, "ROOT", tool_root)
        override.start()
        self.addCleanup(override.stop)
        environment_path = tool_root / "build_environment.py"
        self.put(environment_path, b"# synthetic; never imported\n")
        environment_override = patch.object(native.build.build_helpers, "ENVIRONMENT_HELPER", environment_path)
        environment_override.start()
        self.addCleanup(environment_override.stop)
        helper = patch.object(native.build, "helper_pins", return_value={str(path): native.digest(path) for path in (tool_root / "scripts/with_llvm_env.sh", environment_path)})
        helper.start()
        self.addCleanup(helper.stop)

    def put(self, path, data):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(native.build.serialized(data) if isinstance(data, dict) else data)
        return native.digest(path)

    def fixture(self, version=134, fresh=None):
        base = self.root / str(version)
        source = base / 'wasm-source'
        output = base / 'wasm-build'
        original = policy()
        active = original if version == 134 else original[:-32] + bytes([version]) * 32
        self.put(base / 'original.scale', original)
        self.put(base / 'active.scale', active)
        self.put(source / 'Cargo.lock', native.read(native.ASSETS / 'repaired/lock/original.lock'))
        self.put(source / 'Cargo.toml', b'[workspace]\nmembers = ["runtime"]\n')
        self.put(source / 'runtime/Cargo.toml', b'[features]\ndefault = ["std"]\nbuild-wasm-binary = []\n')
        self.put(source / 'runtime/src/lib.rs', b'#[cfg(all(feature = "std", feature = "build-wasm-binary"))]\nconst EXAMPLE: () = ();\n')
        operators = b'snxops02' + scale.compact(1) + h(2) + scale.compact(1) + (fresh or h(9)) + scale.compact(1) + h(10) + scale.compact(1) + h(11)
        self.put(base / 'final.wasm', wasm(138, 134))
        generated = f'pub const OPERATORS_SCALE: &[u8] = &[{",".join(map(str, operators))}];\n'
        generated += f'pub const FINAL_CODE_HASH: &[u8] = &[{",".join(map(str, hashlib.blake2b(wasm(138, 134), digest_size=32).digest()))}];\n'
        for prefix, raw in (('', active), ('ORIGINAL_', original)):
            for suffix, value in (('SCALE', raw), ('COMMITMENT', hashlib.blake2b(raw, digest_size=32).digest())):
                generated += f'pub const {prefix}POLICY_{suffix}: &[u8] = &[{",".join(map(str,value))}];\n'
        self.put(source / 'runtime/src/incident_recovery/generated.rs', generated.encode())
        inventory = native.build.fingerprint(source)
        provenance = {'fixtureOnly': False, 'releaseReady': False, 'bound': True,
            'specVersion': version, 'transactionVersion': 131 if version == 134 else 132,
            'deployedSourceTree': '36c466398e3f2760e8ab44fdb89de796f5b0dd99', 'sdk': 'polkadot-stable2603-3',
            'sourceFiles': inventory, 'baseCargoLock': inventory['Cargo.lock'], 'overlay': {},
            'bindingEvidence': {'policy': native.digest(base / 'active.scale'), 'operators': native.build.pin_bytes(operators),
                'finalWasm': native.digest(base / 'final.wasm')}}
        self.put(source / 'RECOVERY-PROVENANCE.json', provenance)
        self.put(source / 'PRODUCTION-LOCK-AMENDMENT.json', {'kind': 'legacy-reviewed-direct-dependencies',
            'original': inventory['Cargo.lock'], 'resolved': inventory['Cargo.lock']})
        command = [str(native.ROOT / 'scripts/with_llvm_env.sh'), 'cargo', 'check', '--offline', '--locked',
            '-p', 'framenode-runtime', '--lib', '--features', 'build-wasm-binary']
        initial = {'schema': 'sora-nexus-production-build-process-v1', 'command': command,
            'cwd': str(source), 'startedAt': 'synthetic-no-process', 'productionReady': False}
        self.put(output / 'cargo/launch.json', initial)
        self.put(output / 'cargo/started.json', {**initial, 'pid': 123})
        logs = {name: self.put(output / 'cargo' / name, b'synthetic test log\n') for name in ('stdout.log', 'stderr.log')}
        process = {**initial, 'pid': 123, 'exitCode': 0, 'stopReason': None, 'guardError': None,
            'cleanupErrors': [], 'outputs': logs}
        receipt = self.put(output / 'cargo/execution.json', process)
        wasm_path = output / 'runtime.wasm'
        wasm_pin = self.put(wasm_path, wasm(version, provenance['transactionVersion']))
        report = {'schema': 'sora-nexus-production-recovery-wasm-build-v1', 'fixtureOnly': False,
            'productionReady': False, 'status': 'passed', 'exitCode': 0, 'source': str(source),
            'specVersion': version, 'transactionVersion': provenance['transactionVersion'],
            'sourceProvenance': self.put(output / 'source-provenance.json', provenance),
            'sourceFingerprint': self.put(output / 'source-files.json', inventory),
            'lockBefore': inventory['Cargo.lock'], 'lockAfter': inventory['Cargo.lock'],
            'command': command, 'processReceipt': receipt, 'artifacts': {wasm_path.name: wasm_pin},
            'wasmProjectDependency': {'default-features': False, 'features': ['build-wasm-binary'],
                'package': 'framenode-runtime', 'path': str(source / 'runtime')}}
        self.put(output / 'build.json', report)
        self.put(base / 'root-inventory.json', {})
        args = SimpleNamespace(source=source, wasm_build=output / 'build.json', wasm=wasm_path,
            policy=base / 'active.scale', original_policy=base / 'original.scale',
            root_inventory=base / 'root-inventory.json', destination=base / 'native-source')
        for name in ('wasm_build', 'wasm', 'policy', 'original_policy', 'root_inventory'):
            setattr(args, name + '_sha256', native.digest(getattr(args, name))['sha256'])
        return args

    def refresh_report(self, args, change):
        report = native.parse(native.read(args.wasm_build))
        change(report)
        args.wasm_build_sha256 = self.put(args.wasm_build, report)['sha256']

    def prepare(self, args):
        with patch.object(native.build, 'verify_inventory', return_value={}), patch.object(native.build, 'source_inventory', return_value={}):
            return native.prepare(args)

    def run_args(self, args):
        return SimpleNamespace(source=args.destination,
            configuration_sha256=native.digest(args.destination / 'RECORDER-PREPARATION.json')['sha256'],
            source_inventory_sha256=native.digest(args.destination / 'RECORDER-SOURCE-FILES.json')['sha256'],
            output=args.destination.parent / 'native-build')

    def test_all_supported_versions_prepare_exact_business_and_reviewed_overlays(self):
        for version in (134, 136, 137):
            with self.subTest(version=version):
                args = self.fixture(version)
                result = self.prepare(args)
                self.assertFalse(result['nativeBuildExecuted'])
                config = native.parse(native.read(args.destination / 'RECORDER-PREPARATION.json'))
                self.assertFalse(config['binding']['fixtureOnly'])
                self.assertTrue(config['businessPreservation']['runtimeBusinessSourcesIdentical'])
                self.assertEqual(config['binding']['specVersion'], version)
                self.assertEqual(native.read(args.destination / 'RECORDER-ORIGINAL-source-provenance.json'),
                    native.read(args.source / 'RECOVERY-PROVENANCE.json'))
                binding, generated = native.bound_overlay(config['binding'], config['originalSourceInventory'], config['variant'])
                native.validate_overlay(args.destination, binding, generated, config['variant'])

    def test_fixture_markers_and_public_control_keys_rejected(self):
        args = self.fixture()
        self.refresh_report(args, lambda row: row.update(fixtureOnly=True))
        with self.assertRaisesRegex(ValueError, 'nonfixture'): native.validate_inputs(args)
        for marker in ('NEVER-DEPLOY.txt', 'FIXTURE-PROVENANCE.json'):
            self.refresh_report(args, lambda row: row.update(fixtureOnly=False))
            self.put(args.source / marker, b'public fixture')
            with self.assertRaisesRegex(ValueError, 'nonfixture'): native.validate_inputs(args)
            (args.source / marker).unlink()
        raw = policy()
        bad_council = raw[:134] + next(iter(native.incoming.PUBLIC_COUNCIL)) + raw[166:]
        with self.assertRaisesRegex(ValueError, 'public test council'): native.nonfixture_policy(bad_council)
        bad_mapping = raw[:101] + bytes.fromhex(next(iter(native.build.PUBLIC_REPLACEMENTS))) + raw[133:]
        with self.assertRaisesRegex(ValueError, 'public replacement'): native.nonfixture_policy(bad_mapping)

    def test_retained_process_requires_real_shape_and_exact_pid_command_logs(self):
        args = self.fixture()
        report = native.parse(native.read(args.wasm_build))
        execution = args.wasm_build.parent / 'cargo/execution.json'
        original = native.parse(native.read(execution))
        changes = [{'pid': None}, {'exitCode': True}, {'stopReason': 'timeout'}, {'guardError': 'drift'},
            {'cleanupErrors': ['kill failed']}, {'cwd': str(self.root)}, {'command': ['cargo', 'check']}]
        for change in changes:
            with self.subTest(change=change):
                altered = {**original, **change}
                report['processReceipt'] = self.put(execution, altered)
                with self.assertRaises(ValueError): native.validate_process(report, args.wasm_build, args.source)
        report['processReceipt'] = self.put(execution, original)
        self.put(args.wasm_build.parent / 'cargo/started.json', {**original, 'pid': 456})
        with self.assertRaisesRegex(ValueError, 'PID'): native.validate_process(report, args.wasm_build, args.source)
        self.put(args.wasm_build.parent / 'cargo/started.json', original)
        self.put(args.wasm_build.parent / 'cargo/stdout.log', b'changed')
        with self.assertRaisesRegex(ValueError, 'pin differs'): native.validate_process(report, args.wasm_build, args.source)

    def test_public_test_keys_cannot_move_between_control_roles(self):
        raw = policy()
        for key in native.build.PUBLIC_SIGNING_KEYS:
            public = bytes.fromhex(key)
            with self.subTest(role='replacement', key=key), self.assertRaises(ValueError):
                native.nonfixture_policy(raw[:101] + public + raw[133:])
            with self.subTest(role='council', key=key), self.assertRaises(ValueError):
                native.nonfixture_policy(raw[:134] + public + raw[166:])
        args = self.fixture(fresh=next(iter(native.incoming.PUBLIC_COUNCIL)))
        with self.assertRaisesRegex(ValueError, 'public GRANDPA'): native.validate_inputs(args)

    def test_changed_mapping_compiled_policy_wasm_and_source_rejected(self):
        args = self.fixture(136)
        original_policy = native.read(args.original_policy)
        self.put(args.original_policy, original_policy[:101] + bytes([99]) * 32 + original_policy[133:])
        args.original_policy_sha256 = native.digest(args.original_policy)['sha256']
        with self.assertRaisesRegex(ValueError, 'checkpoint/mapping/council'): native.validate_inputs(args)
        self.put(args.original_policy, original_policy)
        args.original_policy_sha256 = native.digest(args.original_policy)['sha256']
        self.put(args.wasm, wasm(137, 132)); args.wasm_sha256 = native.digest(args.wasm)['sha256']
        self.refresh_report(args, lambda row: row['artifacts'].update({args.wasm.name: native.digest(args.wasm)}))
        with self.assertRaisesRegex(ValueError, 'actual Wasm version'): native.validate_inputs(args)
        self.put(args.wasm, wasm(136, 132)); args.wasm_sha256 = native.digest(args.wasm)['sha256']
        self.refresh_report(args, lambda row: row['artifacts'].update({args.wasm.name: native.digest(args.wasm)}))
        self.put(args.source / 'runtime/src/new.rs', b'// unauthorized business code')
        with self.assertRaisesRegex(ValueError, 'source file set'): native.validate_inputs(args)

    def test_destination_cannot_overlap_guarded_sources(self):
        args = self.fixture()
        args.destination = args.source / 'native'
        with self.assertRaisesRegex(ValueError, 'overlaps'):
            self.prepare(args)
        self.assertFalse(args.destination.exists())

    def test_build_rejects_tampered_recorder_even_with_rehashed_inventory(self):
        args = self.fixture()
        self.prepare(args)
        target = args.destination / native.PACKAGE_PATH / 'src/main.rs'
        target.write_bytes(native.read(target) + b'\n// forged recorder source\n')
        self.put(args.destination / 'RECORDER-SOURCE-FILES.json', native.source_inventory(args.destination))
        with patch.object(native.build, 'process') as process:
            with self.assertRaisesRegex(ValueError, 'reviewed recorder source differs'):
                native.run(self.run_args(args))
            process.assert_not_called()

    def test_build_rejects_fabricated_binding_even_with_matching_json_and_inventory(self):
        args = self.fixture(136)
        self.prepare(args)
        config_path = args.destination / 'RECORDER-PREPARATION.json'
        config = native.parse(native.read(config_path)); config['binding']['engineSha256'] = 'ab' * 32
        self.put(config_path, config)
        self.put(args.destination / 'RECORDER-ENGINE-BINDING.json', config['binding'])
        self.put(args.destination / 'RECORDER-SOURCE-FILES.json', native.source_inventory(args.destination))
        with patch.object(native.build, 'process') as process:
            with self.assertRaisesRegex(ValueError, 'original admission'): native.run(self.run_args(args))
            process.assert_not_called()

    def test_business_projection_rejects_changed_runtime_and_lock(self):
        args = self.fixture(); self.prepare(args)
        config = native.parse(native.read(args.destination / 'RECORDER-PREPARATION.json'))
        for name in ('runtime/src/lib.rs', 'Cargo.lock'):
            target = args.destination / name; saved = native.read(target)
            self.put(target, saved + b'\n// drift')
            with self.assertRaises(ValueError):
                native.business_projection(args.destination, args.source, config['originalSourceInventory'],
                    config['variant'], native.read(native.ASSETS / 'repaired/lock/amended.lock'))
            self.put(target, saved)

    def cargo_output(self, source, binding, *, wrong_path=False, duplicate=False, success=True):
        path = source / ('other' if wrong_path else 'target/debug') / native.PACKAGE
        marker = json.dumps(binding, sort_keys=True, separators=(',', ':')).encode()
        self.put(path, b'\x7fELFsynthetic-not-executable\0' + marker)
        row = {'reason': 'compiler-artifact', 'executable': str(path), 'target': {'name': native.PACKAGE, 'kind': ['bin']},
            'profile': {'test': False}}
        rows = [row] * (2 if duplicate else 1) + [{'reason': 'build-finished', 'success': success}]
        return b'\n'.join(json.dumps(item).encode() for item in rows)

    def test_executable_requires_successful_unique_cargo_result_owned_target_and_embedded_binding(self):
        source = self.root / 'native'; binding = {'engineSha256': 'ab' * 32, 'fixtureOnly': False}
        for options in ({'wrong_path': True}, {'duplicate': True}, {'success': False}):
            with self.subTest(options=options), self.assertRaises(ValueError):
                native.executable_from_cargo(self.cargo_output(source, binding, **options), source, binding)
        stdout = self.cargo_output(source, binding)
        executable, _ = native.executable_from_cargo(stdout, source, binding)
        self.put(executable, b'\x7fELFwrong-engine')
        with self.assertRaisesRegex(ValueError, 'embedded engine binding'):
            native.executable_from_cargo(stdout, source, binding)

    def test_mocked_build_pins_artifacts_without_claiming_execution_readiness(self):
        args = self.fixture(137); self.prepare(args)
        binding = native.parse(native.read(args.destination / 'RECORDER-ENGINE-BINDING.json'))
        calls = []
        def process(command, source, output, guard, env, timeout, retain=None):
            guard(); calls.append((command, env['CARGO_TARGET_DIR'], timeout))
            stdout = self.cargo_output(source, binding) if 'build' in command else b'test result: ok. 2 passed; 0 failed;\n'
            expected = self.put(output / 'execution.json', {'synthetic': True})
            if retain is not None: retain(output / 'execution.json', expected)
            return stdout, {'synthetic': True}
        environment = SimpleNamespace(prepare_build_environment=lambda *args, **kwargs: ({}, {'controlled': {}}))
        original_loader = native.load_module
        def load(name, path):
            return environment if name == 'production_native_environment' else original_loader(name, path)
        # All subprocesses are replaced. This verifies orchestration, not compiler evidence.
        with patch.object(native.build, 'verify_inventory', return_value={}), patch.object(native.build, 'source_inventory', return_value={}), \
             patch.object(native.build, 'process', side_effect=process), patch.object(native, 'load_module', side_effect=load):
            result = native.run(self.run_args(args))
        self.assertEqual(len(calls), len(native.commands()))
        self.assertEqual({target for _, target, _ in calls}, {str(args.destination / 'target')})
        self.assertTrue(all('--offline' in command and '--locked' in command for command, _, _ in calls))
        self.assertFalse(result['productionReady'])
        self.assertFalse(result['productionNativeReplayExecuted'])
        self.assertEqual(set(result['processes']), {name for name, _ in native.commands()})
        self.assertEqual(native.read(self.run_args(args).output / result['engineBinding']['file']),
            native.read(args.destination / 'RECORDER-ENGINE-BINDING.json'))
        self.assertTrue(result['helperCopies'])
        self.assertIn(str(self.run_args(args).output / 'tests/execution.json'), result['retainedArtifacts'])

    def test_prior_process_receipt_mutation_aborts_native_build(self):
        args = self.fixture(); self.prepare(args)
        environment = SimpleNamespace(prepare_build_environment=lambda *args, **kwargs: ({}, {'controlled': {}}))
        original_loader = native.load_module
        def load(name, path):
            return environment if name == 'production_native_environment' else original_loader(name, path)
        def process(command, source, output, guard, env, timeout, retain):
            guard()
            if output.name == 'tests':
                self.put(output.parent / 'metadata/execution.json', {'forged': True})
                guard()
            retain(output / 'execution.json', self.put(output / 'execution.json', {'synthetic': True}))
            return b'test result: ok. 2 passed; 0 failed;\n', {}
        run = self.run_args(args)
        with patch.object(native.build, 'verify_inventory', return_value={}), patch.object(native.build, 'source_inventory', return_value={}), \
             patch.object(native.build, 'process', side_effect=process), patch.object(native, 'load_module', side_effect=load):
            with self.assertRaisesRegex(ValueError, 'bound file changed'): native.run(run)
        self.assertTrue((run.output / 'failure.json').exists())
        self.assertFalse((run.output / 'completion.json').exists())

    def engine_args(self, version=136):
        args = self.fixture(version)
        args.spec_version = version
        args.final_wasm = args.source.parent / 'final.wasm'
        args.final_wasm_sha256 = native.digest(args.final_wasm)['sha256']
        args.legacy_lock = native.ASSETS / 'repaired/lock/original.lock'
        args.legacy_lock_sha256 = native.digest(args.legacy_lock)['sha256']
        args.target = args.source.parent / 'warm-target'; args.target.mkdir()
        args.output = args.source.parent / 'repair-build'
        return args

    def test_repair_engine_cli_uses_retained_guarded_existing_builder_for_both_versions(self):
        for version in (136, 137):
            args = self.engine_args(version)
            def build_wasm(source, output, target, spec, guard, retain, retain_source):
                self.assertEqual((source, target, spec), (args.source, args.target, version))
                guard(); retain_source(source, native.build.fingerprint(source))
                result = output / f'runtime-{spec}.compact.compressed.wasm'
                retain(result, self.put(result, wasm(spec, 132)))
                retain(output / 'build.json', self.put(output / 'build.json', {'synthetic': True}))
                guard()
                return result
            with patch.object(native.build, 'verify_inventory', return_value={}), patch.object(native.build, 'source_inventory', return_value={}), \
                 patch.object(native.build, 'build_wasm', side_effect=build_wasm):
                result = native.build_engine_wasm(args)
            self.assertEqual(result['specVersion'], version)
            self.assertFalse(result['approvalVerified'])
            self.assertFalse(result['operatorIdentityVerified'])
            self.assertFalse(result['productionReady'])

    def test_repair_engine_rejects_wrong_version_final_policy_fixture_and_lock(self):
        args = self.engine_args()
        with patch.object(native.build, 'verify_inventory', return_value={}), patch.object(native.build, 'source_inventory', return_value={}), \
             patch.object(native.build, 'build_wasm') as builder:
            args.spec_version = 134
            with self.assertRaisesRegex(ValueError, 'only forward'): native.build_engine_wasm(args)
            args.spec_version = 136
            original = native.read(args.original_policy)
            self.put(args.original_policy, original[:101] + h(99) + original[133:])
            args.original_policy_sha256 = native.digest(args.original_policy)['sha256']
            with self.assertRaisesRegex(ValueError, 'original controls'): native.build_engine_wasm(args)
            self.put(args.original_policy, original); args.original_policy_sha256 = native.digest(args.original_policy)['sha256']
            self.put(args.final_wasm, wasm(137, 132)); args.final_wasm_sha256 = native.digest(args.final_wasm)['sha256']
            with self.assertRaisesRegex(ValueError, 'pinned final138'): native.build_engine_wasm(args)
            self.put(args.final_wasm, wasm(138, 134)); args.final_wasm_sha256 = native.digest(args.final_wasm)['sha256']
            self.put(args.source / 'FIXTURE-PROVENANCE.json', b'{}')
            with self.assertRaisesRegex(ValueError, 'public fixture'): native.build_engine_wasm(args)
            (args.source / 'FIXTURE-PROVENANCE.json').unlink()
            args.legacy_lock = args.source.parent / 'wrong.lock'; self.put(args.legacy_lock, b'version = 4\n')
            args.legacy_lock_sha256 = native.digest(args.legacy_lock)['sha256']
            with self.assertRaisesRegex(ValueError, 'unsupported original runtime lock'): native.build_engine_wasm(args)
            builder.assert_not_called()

    def test_repair_engine_source_mutation_during_builder_is_rejected(self):
        args = self.engine_args()
        def altered(source, output, target, spec, guard, retain, retain_source):
            self.put(source / 'runtime/src/lib.rs', b'altered runtime business code')
            guard()
        with patch.object(native.build, 'verify_inventory', return_value={}), patch.object(native.build, 'source_inventory', return_value={}), \
             patch.object(native.build, 'build_wasm', side_effect=altered):
            with self.assertRaisesRegex(ValueError, 'source drift'): native.build_engine_wasm(args)
        self.assertTrue((args.output / 'failure.json').exists())
        self.assertFalse((args.output / 'completion.json').exists())

    def test_repair_engine_final_registration_cannot_rebind_changed_receipt(self):
        args = self.engine_args()
        original_digest = native.digest
        receipt = args.output / 'wasm/build.json'
        def builder(source, output, target, spec, guard, retain, retain_source):
            result = output / 'runtime.wasm'
            retain(result, self.put(result, wasm(spec, 132)))
            receipt.write_bytes(b'{"synthetic":true}')
            retain(receipt, original_digest(receipt))
            return result
        def digest(path):
            if path == receipt:
                # Completion-field computation occurs after the first final guard.
                receipt.write_bytes(b'{"changed-after-guard":true}')
            return original_digest(path)
        with patch.object(native.build, 'verify_inventory', return_value={}), patch.object(native.build, 'source_inventory', return_value={}), \
             patch.object(native.build, 'build_wasm', side_effect=builder), patch.object(native, 'digest', side_effect=digest):
            with self.assertRaisesRegex(ValueError, 'bound file changed'): native.build_engine_wasm(args)
        self.assertTrue((args.output / 'failure.json').exists())
        self.assertFalse((args.output / 'completion.json').exists())


if __name__ == '__main__': unittest.main()
