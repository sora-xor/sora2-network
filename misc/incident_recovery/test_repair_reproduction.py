"""Fresh repair reproduction with mocked owned processes; no native/Cargo run."""
import contextlib
import copy
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import production_build as build
import repair_reproduction as repair
import supporting_checks_release as support
from test_release_package import policy, wasm


class RepairReproductionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='sora-nexus-repair-reproduction-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.release = self.root / 'release'
        self.paths = {}
        self.pins = {}
        self.guards = []
        self.commands = []
        self.change = None
        self.verifier = '9' * 64
        self.original = policy()
        self.put_role('policyScale', self.original)
        self.put_role('finalWasm', wasm(138, 134))
        self.put_role('operatorsScale', b'public synthetic operator SCALE')
        self.base = {'policyScale': '0x' + self.original.hex(),
            'policyCommitment': '0x' + build.pin_bytes(self.original)['blake2_256'],
            'operatorsScale': '0x' + self.paths['operatorsScale'].read_bytes().hex(),
            'previousGrandpaSetId': 7, 'firstRecoverySlot': 900, 'finalWasm': 'finalWasm'}
        self.put_role('holdBinding', self.base)
        self.put_role('forwardRepairReplay', {'schema': 'synthetic not executed'})
        self.paths['recoveryParentReplay'] = self.paths['forwardRepairReplay']
        self.report = {'gates': {'automaticActivationEscapeAndForwardRepair':
            {'status': 'verified', 'verifierSha256': self.verifier}},
            'forwardRepairReplay': {'repairs': [], 'originalPolicy': build.release.policy_identity(self.original),
                                   'policies': {}, 'wasm': {}}}
        self.root_inventory = self.put(self.root / 'inputs/root-inventory.json', {'source': 'synthetic'})
        self.lock = self.put(self.root / 'inputs/legacy.lock', b'reviewed resolved legacy lock')
        self.context = {'refs': {'sourceInventory': self.root_inventory, 'legacyLock': self.lock}}
        self.output = self.root / 'repairs'
        self.target = self.root / 'warm-target'
        self.target.mkdir()

    def put(self, path, value):
        build.write(path, value)
        return path

    def put_role(self, role, value):
        self.paths[role] = self.put(self.release / role, value)
        return self.paths[role]

    def add(self, version):
        prefix = 'repair' + str(version)
        active = self.original[:-1] + bytes([version - 120])
        self.put_role(prefix + 'PolicyScale', active)
        binding = dict(self.base, policyScale='0x' + active.hex(),
            policyCommitment='0x' + build.pin_bytes(active)['blake2_256'],
            originalPolicyScale='0x' + self.original.hex(), originalPolicyCommitment=self.base['policyCommitment'])
        self.put_role(prefix + 'Binding', binding)
        self.put_role(prefix + 'Provenance', {'bound': 'not trusted by this fresh reproduction'})
        self.put_role(prefix + 'BuildReceipt', {'status': 'not trusted by this fresh reproduction'})
        role = prefix + 'Wasm'
        self.put_role(role, wasm(version, 132))
        context = self.report['forwardRepairReplay']
        context['repairs'].append(version)
        context['policies'][str(version)] = build.release.policy_identity(active)
        context['wasm'][str(version)] = {'role': role, 'sha256': build.digest(self.paths[role])['sha256'],
                                       'blake2': build.digest(self.paths[role])['blake2_256']}
        self.paths['incomingActivePolicyScale'] = self.paths[prefix + 'PolicyScale']

    def admitted(self):
        return repair.plan(self.report, self.paths, self.verifier)

    def retain(self, path, pin):
        self.assertTrue(str(path) not in self.pins or self.pins[str(path)] == pin)
        self.pins[str(path)] = pin

    def remember(self, path, value):
        pin = build.write(path, value)
        self.retain(path, pin)
        return pin

    def guard(self):
        build.guard_files(self.pins)
        for guard in self.guards:
            guard()

    def export(self, command):
        source = Path(command[command.index('--destination') + 1])
        version = int(command[command.index('--spec-version') + 1])
        binding_path = Path(command[command.index('--binding') + 1])
        binding = build.parse(build.read(binding_path))
        self.assertEqual(binding['originalPolicyScale'], '0x' + self.original.hex())
        generated, evidence, _ = build.bootstrap.bind_inputs(binding_path, version)
        self.put(source / 'Cargo.lock', b'pre-normalization export lock')
        self.put(source / 'runtime/src/incident_recovery/generated.rs', generated.encode())
        fields = build.fingerprint(source)
        provenance = {'fixtureOnly': False, 'bound': True, 'releaseReady': False,
            'specVersion': version, 'transactionVersion': 132, 'bindingEvidence': evidence,
            'sourceFiles': {name: {key: pin[key] for key in ('sha256', 'blake2_256')} for name, pin in fields.items()}}
        self.put(source / 'RECOVERY-PROVENANCE.json', provenance)
        return build.serialized({'destination': str(source), 'bound': True, 'releaseReady': False,
            'provenancePin': build.pin_bytes(build.serialized(provenance))})

    def engine(self, command):
        source = Path(command[command.index('--source') + 1])
        output = Path(command[command.index('--output') + 1])
        version = int(command[command.index('--spec-version') + 1])
        self.assertEqual(command[3], 'build-engine-wasm')
        self.assertEqual(Path(command[command.index('--target') + 1]), self.target)
        self.assertEqual(build.read(Path(command[command.index('--original-policy') + 1])), self.original)
        self.assertEqual(build.digest(Path(command[command.index('--final-wasm') + 1])), build.digest(self.paths['finalWasm']))
        (source / 'Cargo.lock').write_bytes(self.lock.read_bytes())
        amendment = self.put(source / 'PRODUCTION-LOCK-AMENDMENT.json', {'resolved': build.digest(self.lock)})
        wasm_path = self.put(output / 'wasm' / ('runtime-' + str(version) + '.compact.compressed.wasm'),
            self.paths['repair' + str(version) + 'Wasm'].read_bytes())
        # Model an old warm-lane export, which is retained as history only.
        previous = self.put(output / 'wasm/previous-exports/Cargo.toml', b'previous warm export manifest')
        actual_command = [str(repair.native.ROOT / 'scripts/with_llvm_env.sh'), 'cargo', 'check', '--offline',
            '--locked', '-p', 'framenode-runtime', '--lib', '--features', 'build-wasm-binary']
        launch = {'schema': 'sora-nexus-production-build-process-v1', 'command': actual_command,
            'cwd': str(source), 'startedAt': 'synthetic test', 'productionReady': False}
        execution = {**launch, 'pid': 731, 'exitCode': 0, 'stopReason': None, 'guardError': None, 'cleanupErrors': []}
        output_pins = {}
        for name, raw in [('stdout.log', b'synthetic actual compiler stdout'), ('stderr.log', b'')]:
            output_pins[name] = build.digest(self.put(output / 'wasm/cargo' / name, raw))
        execution['outputs'] = output_pins
        for name, value in [('launch.json', launch), ('started.json', {**launch, 'pid': 731}), ('execution.json', execution)]:
            self.put(output / 'wasm/cargo' / name, value)
        source_files = self.put(output / 'wasm/source-files.json', build.fingerprint(source))
        provenance = self.put(output / 'wasm/source-provenance.json', (source / 'RECOVERY-PROVENANCE.json').read_bytes())
        logs = build.copy_cargo_logs(output / 'wasm', execution)
        receipt = {'schema': 'sora-nexus-production-recovery-wasm-build-v1', 'fixtureOnly': False,
            'productionReady': False, 'source': str(source), 'specVersion': version, 'transactionVersion': 132,
            'status': 'passed', 'exitCode': 0, 'command': actual_command,
            'sourceFingerprint': build.digest(source_files), 'sourceProvenance': build.digest(provenance),
            'processReceipt': build.digest(output / 'wasm/cargo/execution.json'), 'logs': logs,
            'previousExports': [{'file': 'Cargo.toml', **build.digest(previous)}]}
        receipt_path = self.put(output / 'wasm/build.json', receipt)
        pins = {str(source / name): pin for name, pin in build.fingerprint(source).items()}
        for path in (amendment, source / 'RECOVERY-PROVENANCE.json', wasm_path, receipt_path):
            pins[str(path)] = build.digest(path)
        for path in (output / 'wasm').rglob('*'):
            if path.is_file() and not path.is_relative_to(output / 'wasm/previous-exports'):
                pins[str(path)] = build.digest(path)
        result = {'schema': 'sora-nexus-production-repair-engine-build-v1', 'specVersion': version,
            'transactionVersion': 132, 'status': 'passed', 'fixtureOnly': False, 'productionReady': False,
            'rootInventory': build.digest(self.root_inventory), 'wasm': {'path': str(wasm_path), **build.digest(wasm_path)},
            'buildReceipt': {'path': str(receipt_path), **build.digest(receipt_path)}, 'inputPins': pins}
        self.put(output / 'completion.json', result)
        return build.serialized(result)

    def process(self, command, cwd, output, guard, **kwargs):
        self.commands.append((command, kwargs['timeout']))
        guard()
        output.mkdir(parents=True)
        raw = self.export(command) if '--destination' in command else self.engine(command)
        for name, value in [('stdout.log', raw), ('execution.json', {'exitCode': 0, 'command': command})]:
            self.remember(output / name, value)
        if self.change:
            self.change(command, output)
        guard()
        return raw, {'exitCode': 0}

    def run_rebuild(self, rows=None):
        rows = self.admitted() if rows is None else rows
        with patch.object(build, 'process', side_effect=self.process):
            return repair.rebuild(rows, self.paths, self.output, self.paths['finalWasm'], self.context,
                self.target, self.guard, self.remember, self.retain, self.guards, {'PATH': '/unused'})

    def test_actual_context_plan_covers_direct136_direct137_and136_then137(self):
        for version in (136, 137):
            self.add(version)
            rows = self.admitted()
            self.assertEqual([row['version'] for row in rows], list(range(136, version + 1)))
        self.report['forwardRepairReplay']['repairs'] = [137]
        for role in list(self.paths):
            if role.startswith('repair136'):
                del self.paths[role]
        self.assertEqual([row['version'] for row in self.admitted()], [137])

    def test_missing_or_different_fresh_verifier_is_rejected(self):
        self.add(136)
        self.report['gates']['automaticActivationEscapeAndForwardRepair']['verifierSha256'] = '0' * 64
        with self.assertRaisesRegex(ValueError, 'fresh same-verifier'):
            self.admitted()

    def test_active_policy_cannot_change_mapping_council_or_original_lineage(self):
        self.add(136)
        path = self.paths['repair136PolicyScale']
        raw = bytearray(path.read_bytes());raw[101] ^= 1;path.write_bytes(raw)
        with self.assertRaises(ValueError):
            self.admitted()

    def test_binding_original_or_operator_change_is_rejected(self):
        self.add(136)
        path = self.paths['repair136Binding']
        original = build.parse(path.read_bytes())
        for key in ('originalPolicyCommitment', 'operatorsScale', 'firstRecoverySlot'):
            value = dict(original)
            value[key] = 901 if key == 'firstRecoverySlot' else '0x' + '00' * 32
            path.write_bytes(build.serialized(value))
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.admitted()

    def test_omitted_repair_or_wrong_terminal_policy_is_rejected(self):
        self.add(136);self.add(137)
        self.paths['incomingActivePolicyScale'] = self.paths['repair136PolicyScale']
        with self.assertRaisesRegex(ValueError, 'terminal active policy'):
            self.admitted()
        self.paths['incomingActivePolicyScale'] = self.paths['repair137PolicyScale']
        self.report['forwardRepairReplay']['repairs'] = [136]
        with self.assertRaisesRegex(ValueError, 'unused or unknown'):
            self.admitted()

    def test_fresh_export_engine_rebuilds_both_roles_and_preserves_original_final(self):
        self.add(136);self.add(137)
        final_before, original_before = build.digest(self.paths['finalWasm']), build.digest(self.paths['policyScale'])
        joins = self.run_rebuild()
        self.assertEqual(set(joins), {'repair136Wasm', 'repair137Wasm'})
        self.assertEqual(build.digest(self.paths['finalWasm']), final_before)
        self.assertEqual(build.digest(self.paths['policyScale']), original_before)
        self.assertEqual(len(self.commands), 4)
        self.assertTrue(all(timeout >= 24 * 3600 for _, timeout in self.commands))
        self.guard()
        self.assertEqual(support.retained_tree(self.output), self.pins)

    def test_full_supporting_hook_reproduces_all_five_versions_before_aggregation(self):
        from test_supporting_checks_release import SupportingChecksTests
        case = SupportingChecksTests('test_fresh_exact_rebuild_and_all_fresh_gates_clear_only_aggregate')
        case.setUp()
        self.addCleanup(case.doCleanups)
        self.add(136);self.add(137)
        self.put_role('bootstrapWasm', wasm(134, 131))
        self.put_role('holdWasm', wasm(135, 132))
        case.paths = self.paths
        case.report['policy'] = build.release.policy_identity(self.original)
        case.report['forwardRepairReplay'] = copy.deepcopy(self.report['forwardRepairReplay'])
        case.report['gates']['automaticActivationEscapeAndForwardRepair'] = {
            'status': 'verified', 'verifierSha256': build.digest(case.verifier)['sha256']}
        case.report['artifacts'] = [{'role': role, **build.digest(path)} for role, path in case.paths.items()]
        self.root_inventory = case.refs['sourceInventory']
        self.lock = case.refs['legacyLock']
        self.target = case.targets[2]
        original_process = case.process
        def dispatch(command, cwd, output, guard, **kwargs):
            if '--destination' not in command and 'build-engine-wasm' not in command:
                return original_process(command, cwd, output, guard, **kwargs)
            case.commands.append((command, cwd, kwargs['timeout'], kwargs.get('env')))
            guard()
            output.mkdir(parents=True)
            stdout = self.export(command) if '--destination' in command else self.engine(command)
            for name, value in [('stdout.log', stdout), ('execution.json', {'exitCode': 0, 'pid': 717})]:
                pin = build.write(output / name, value)
                kwargs['retain'](output / name, pin)
            guard()
            return stdout, {'exitCode': 0, 'pid': 717}
        with patch.object(case, 'process', side_effect=dispatch):
            result = case.run_hook()
        self.assertEqual(set(result['rebuiltArtifacts']), {'bootstrapWasm', 'holdWasm', 'finalWasm', 'repair136Wasm', 'repair137Wasm'})
        self.assertEqual(result['aggregate']['unreproducedRepairRoles'], [])
        self.assertTrue(result['aggregate']['verified'])
        self.assertEqual(len(case.commands), 13)
        self.assertEqual((case.output / 'rebuilt/policy.scale').read_bytes(), self.original)
        self.assertFalse(case.report['productionReady'])

    def test_export_manifest_change_after_process_is_rejected(self):
        self.add(136)
        def mutate(command, _):
            if '--destination' in command:
                (Path(command[command.index('--destination') + 1]) / 'RECOVERY-PROVENANCE.json').write_bytes(b'{}')
        self.change = mutate
        with self.assertRaisesRegex(ValueError, 'file pin differs'):
            self.run_rebuild()

    def test_native_source_mutation_is_rejected_before_receipt_adoption(self):
        self.add(136)
        def mutate(command, _):
            if '--source' in command:
                (Path(command[command.index('--source') + 1]) / 'runtime/src/incident_recovery/generated.rs').write_bytes(b'changed')
        self.change = mutate
        with self.assertRaisesRegex(ValueError, 'bound file changed'):
            self.run_rebuild()

    def test_changed_native_completion_is_rejected(self):
        self.add(136)
        def mutate(command, _):
            if '--source' in command:
                (Path(command[command.index('--output') + 1]) / 'completion.json').write_bytes(b'{}')
        self.change = mutate
        with self.assertRaisesRegex(ValueError, 'file pin differs'):
            self.run_rebuild()

    def test_rehashed_compiler_command_cannot_fake_actual_process_join(self):
        self.add(136)
        original = self.engine
        def wrong(command):
            raw = original(command)
            result = build.parse(raw)
            output = Path(command[command.index('--output') + 1])
            receipt_path = output / 'wasm/build.json'
            receipt = build.parse(receipt_path.read_bytes())
            receipt['command'] = ['echo', 'not the actual fixed compiler']
            receipt_path.write_bytes(build.serialized(receipt))
            result['buildReceipt'].update(build.digest(receipt_path))
            result['inputPins'][str(receipt_path)] = build.digest(receipt_path)
            (output / 'completion.json').write_bytes(build.serialized(result))
            return build.serialized(result)
        with patch.object(self, 'engine', side_effect=wrong):
            with self.assertRaisesRegex(ValueError, 'production Wasm command differs'):
                self.run_rebuild()

    def test_unapproved_lock_mutation_is_rejected(self):
        self.add(136)
        def mutate(command, _):
            if '--source' in command:
                (Path(command[command.index('--source') + 1]) / 'Cargo.lock').write_bytes(b'unapproved lock')
        self.change = mutate
        with self.assertRaisesRegex(ValueError, 'exported source changed'):
            self.run_rebuild()

    def test_post_validation_receipt_mutation_cannot_rebind_output_pins(self):
        self.add(136)
        original = repair.verify_engine
        def mutate(stdout, directory, *args):
            result = original(stdout, directory, *args)
            (directory / 'wasm/source-files.json').write_bytes(b'{}')
            return result
        with patch.object(repair, 'verify_engine', side_effect=mutate):
            with self.assertRaisesRegex(ValueError, 'bound file changed'):
                self.run_rebuild()

    def test_unreproduced_or_extra_repair_never_clears_aggregate(self):
        gates = {name: {'status': 'verified'} for name in build.release.GATES}
        self.assertFalse(support.aggregate(gates, True, ['repair136Wasm', 'repair137Wasm'], ['repair136Wasm'])['verified'])
        self.assertTrue(support.aggregate(gates, True, ['repair136Wasm'], ['repair136Wasm'])['verified'])
        with self.assertRaisesRegex(ValueError, 'unexpected reproduced'):
            support.aggregate(gates, True, ['repair136Wasm'], ['repair137Wasm'])

    def test_actual_log_copy_joins_raw_bytes_and_rejects_changed_receipt_input(self):
        output = self.root / 'log-copy'
        stdout = self.put(output / 'cargo/stdout.log', b'actual simulated cargo stdout')
        stderr = self.put(output / 'cargo/stderr.log', b'actual simulated cargo stderr')
        receipt = {'outputs': {'stdout.log': build.digest(stdout), 'stderr.log': build.digest(stderr)}}
        copied = build.copy_cargo_logs(output, receipt)
        self.assertEqual(set(copied), {'cargo.stdout.log', 'cargo.stderr.log'})
        self.assertEqual((output / 'cargo.stdout.log').read_bytes(), stdout.read_bytes())
        for name in copied:
            (output / name).unlink()
        stdout.write_bytes(b'changed after actual receipt')
        with self.assertRaisesRegex(ValueError, 'file pin differs'):
            build.copy_cargo_logs(output, receipt)

    def test_exporter_stdout_pins_in_process_provenance_value(self):
        value = {'bound': True, 'fixtureOnly': False, 'sourceFiles': {'exact': 'source'}}
        destination = self.root / 'never-created-source'
        stdout = io.StringIO()
        args = ['prepare.py', '--destination', str(destination), '--spec-version', '136', '--binding', '/unused-binding']
        with patch('sys.argv', args), patch.object(build.bootstrap, 'prepare', return_value=value), contextlib.redirect_stdout(stdout):
            build.bootstrap.main()
        summary = json.loads(stdout.getvalue())
        self.assertEqual(summary['provenancePin'], build.pin_bytes(build.serialized(value)))
        self.assertFalse(destination.exists())


if __name__ == '__main__':
    unittest.main()
