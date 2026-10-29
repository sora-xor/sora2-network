"""Synthetic admission/process tests. No Cargo, native recovery binary or Wasm."""
import copy
import json
import os
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import hashi_codec as scale
import production_build as build
from test_release_package import policy, h, wasm


def env(tag, index=0, payload=b'x'):
    return '0x' + (b'snxrec01\x01' + bytes([tag]) + scale.u32(index) + payload).hex()


def final_source(raw):
    return ('pub const POLICY_SCALE: &[u8] = &[' + ', '.join(map(str, raw)) + '];\n').encode()


class ProductionBuildTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='production-build-unit-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()

    def artifact(self, name, data):
        path = self.root / name
        build.write(path, data)
        return path

    def bundle(self):
        files = {}
        values = {'capture/capture-complete.json': {'schema': 'sora-nexus-checkpoint-capture-v1'},
            'mapping.csv': ('old account address,new account address\n0x' + h(1).hex() + ',0x' + h(2).hex() + '\n').encode(), 'proofs.json': [],
            'inventory.json': {}, 'economic.json': {}, 'semantic.json': {'derivedAccounts': []},
            'council.json': [], 'derived.json': [], 'staking.json': {},
            'operators.scale': scale.compact(1) + h(1) + scale.compact(1) + h(2) + h(3)
                + scale.compact(1) + h(4) + scale.compact(1) + h(5)}
        for name, value in values.items():
            files[name] = build.digest(self.artifact(name, value))
        value = {'schema': build.SCHEMA, 'files': files, 'authorizationDomain': build.release.policy_identity(policy())['authorizationDomain'], 'compose': {
            'capture': 'capture', 'csv': 'mapping.csv', 'proofs': 'proofs.json', 'inventory': 'inventory.json',
            'economic': 'economic.json', 'semantic': 'semantic.json', 'council': 'council.json', 'staking-roles': 'staking.json'},
            'operatorsScale': 'operators.scale', 'derivedInventory': 'derived.json', 'mixedLayout': None,
            'consensus': {'previousGrandpaSetId': 1, 'lastAbandonedSignedSlot': 7, 'firstRecoverySlot': 8}}
        path = self.artifact('inputs.json', value)
        return value, path

    def load(self, value, path):
        path.write_bytes(build.serialized(value))
        return build.load_inputs(path, build.digest(path)['sha256'])

    def composed(self):
        return {'policyScale': '0x' + policy().hex(), 'applyEnvelopes': [env(1, 0), env(1, 1, b'y')],
            'auditEnvelopes': [env(2, 0), env(8, 1)], 'terminalEnvelope': '0x' + b'snxrec01\x01\x05terminal'.hex()}

    def layout(self):
        return {'schema': 'sora-nexus-production-mixed-layout-v1',
            'work': [{'composed': 0}, {'composed': 1}, {'envelope': env(6, 2)}],
            'audits': [{'composed': 0}, {'composed': 1}, {'envelope': env(9, 2)}], 'terminalEnvelope': None}

    def test_bounded_complete_input_graph_and_data_commitment(self):
        value, path = self.bundle()
        loaded, paths, _, commitment = self.load(value, path)
        self.assertEqual(loaded, value)
        self.assertEqual(set(paths), set(value['files']))
        self.assertEqual(commitment, build.pin_bytes(build.serialized(value))['blake2_256'])
        changed = copy.deepcopy(value)
        changed['consensus']['firstRecoverySlot'] += 1
        self.assertNotEqual(self.load(changed, path)[3], commitment)

    def test_fixture_json_bytes_and_parent_markers_rejected(self):
        value, path = self.bundle()
        for content in ({'fixtureOnly': True}, {'schema': 'sora-nexus-public-fixture-v1'}, {'nested': [{'fixtureOnly': True}]}):
            source = self.root / 'economic.json'
            source.write_bytes(build.serialized(content))
            value['files']['economic.json'] = build.digest(source)
            with self.assertRaisesRegex(ValueError, 'fixture'):
                self.load(value, path)
        source.write_bytes(b'{}')
        value['files']['economic.json'] = build.digest(source)
        self.artifact('NEVER-DEPLOY.txt', b'NEVER DEPLOY')
        with self.assertRaisesRegex(ValueError, 'fixture marker'):
            self.load(value, path)

    def test_relocated_unlabeled_public_keys_rejected(self):
        value, path = self.bundle()
        council = self.root / 'council.json'
        council.write_bytes(build.serialized(['0x' + next(iter(build.PUBLIC_COUNCIL)).hex()]))
        value['files']['council.json'] = build.digest(council)
        with self.assertRaisesRegex(ValueError, 'public test council'):
            self.load(value, path)
        council.write_bytes(b'[]')
        value['files']['council.json'] = build.digest(council)
        operators = self.root / 'operators.scale'
        operators.write_bytes(scale.compact(1) + h(1) + scale.compact(1) + h(2)
            + bytes.fromhex(next(iter(build.PUBLIC_GRANDPA))) + scale.compact(1) + h(4) + scale.compact(1) + h(5))
        value['files']['operators.scale'] = build.digest(operators)
        with self.assertRaisesRegex(ValueError, 'public test GRANDPA'):
            self.load(value, path)

    def test_public_signing_keys_cannot_cross_roles(self):
        value, path = self.bundle()
        council = self.root / 'council.json'
        council.write_bytes(build.serialized(['0x' + next(iter(build.PUBLIC_REPLACEMENTS))]))
        value['files']['council.json'] = build.digest(council)
        with self.assertRaisesRegex(ValueError, 'public test council'):
            self.load(value, path)
        council.write_bytes(b'[]')
        value['files']['council.json'] = build.digest(council)
        mapping = self.root / 'mapping.csv'
        mapping.write_text('old account address,new account address\n0x' + h(1).hex() + ',0x' + next(iter(build.PUBLIC_COUNCIL)).hex() + '\n')
        value['files']['mapping.csv'] = build.digest(mapping)
        with self.assertRaisesRegex(ValueError, 'public test replacement'):
            self.load(value, path)

    def test_inventory_mutation_unsupported_flags_and_missing_control_inputs(self):
        value, path = self.bundle()
        for update in (lambda v: v['compose'].update({'shell': 'economic.json'}),
            lambda v: v['compose'].pop('staking-roles'),
            lambda v: v.update(derivedInventory='missing.json'),
            lambda v: v['files']['economic.json'].update(sha256='0'*64)):
            bad = copy.deepcopy(value)
            update(bad)
            with self.assertRaises(ValueError):
                self.load(bad, path)

    def test_slot_and_set_boundaries(self):
        value, path = self.bundle()
        for consensus in ({'previousGrandpaSetId': 2**64-1, 'firstRecoverySlot': 8, 'lastAbandonedSignedSlot': 7},
            {'previousGrandpaSetId': 1, 'firstRecoverySlot': 7, 'lastAbandonedSignedSlot': 7},
            {'previousGrandpaSetId': True, 'firstRecoverySlot': 8, 'lastAbandonedSignedSlot': 7}):
            bad = copy.deepcopy(value)
            bad['consensus'] = consensus
            with self.assertRaises(ValueError):
                self.load(bad, path)

    def test_safe_paths_symlinks_duplicate_json(self):
        for name in ('../x', '/x', 'x/../z', 'x//z', 'fixtures/proof.json', 'private-key.json'):
            with self.assertRaises(ValueError):
                build.relative(name)
        target = self.artifact('real', b'valid')
        (self.root / 'link').symlink_to(target)
        with self.assertRaises(ValueError):
            build.local(self.root, 'link')
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            build.parse(b'{"x":1,"x":2}')

    def test_production_input_requires_explicit_nonfixture_authorization_domain(self):
        value, path = self.bundle()
        for change in [
            lambda v: v.pop('authorizationDomain'),
            lambda v: v['authorizationDomain'].update(chainSpecId='sora-nexus-PUBLIC-FIXTURE-ONLY'),
            lambda v: v['authorizationDomain'].update(releaseNonce='0x' + '00' * 32),
            lambda v: v['authorizationDomain'].update(releaseNonce='0x' + '46' * 32),
        ]:
            candidate = copy.deepcopy(value)
            change(candidate)
            with self.assertRaises(ValueError):
                self.load(candidate, path)

    def test_mixed_layout_preserves_composed_bodies_and_order(self):
        composed = self.composed()
        result = build.mixed_input(composed, self.layout())
        self.assertEqual(bytes.fromhex(result['workEnvelopes'][1][2:])[14:], bytes.fromhex(composed['applyEnvelopes'][1][2:])[14:])
        self.assertEqual(bytes.fromhex(result['workEnvelopes'][1][2:])[10:14], scale.u32(1))
        self.assertEqual(result['auditEnvelopes'][:2], composed['auditEnvelopes'])
        self.assertEqual(result['terminalEnvelope'], composed['terminalEnvelope'])
        cleared = build.clear_work_policy(policy())
        self.assertEqual(cleared[:134], policy()[:134])
        self.assertEqual(cleared[-64:-32], h(6))
        self.assertEqual(cleared[-32:], bytes(32))

    def test_composed_edits_cannot_follow_incoming_or_outgoing(self):
        for tag in (6, 7):
            for work in ([{'envelope': env(tag, 0)}, {'composed': 0}, {'composed': 1}],
                         [{'composed': 0}, {'envelope': env(tag, 1)}, {'composed': 1}]):
                with self.subTest(tag=tag, work=work):
                    value = self.layout()
                    value['work'] = work
                    with self.assertRaisesRegex(ValueError, 'all composed storage edits must precede'):
                        build.mixed_input(self.composed(), value)

    def test_mixed_layout_omission_duplicate_reordering_and_arbitrary_storage_rejected(self):
        layouts = []
        for work in ([{'composed': 0}], [{'composed': 0}, {'composed': 0}, {'composed': 1}],
            [{'composed': 1}, {'composed': 0}], [{'composed': 0}, {'composed': 1}, {'envelope': env(1, 2)}],
            [{'composed': 0}, {'composed': 1}, {'envelope': env(4, 2)}]):
            value = self.layout()
            value['work'] = work
            layouts.append(value)
        for value in layouts:
            with self.assertRaises(ValueError):
                build.mixed_input(self.composed(), value)
        value = self.layout()
        value['audits'].append({'envelope': env(2, 3)})
        with self.assertRaises(ValueError):
            build.mixed_input(self.composed(), value)

    def test_terminal_pages_keep_exact_order_and_reject_single_final(self):
        composed = self.composed()
        composed['terminalPreludeEnvelopes'] = [env(11, 0), env(11, 1)]
        composed['terminalEnvelope'] = env(11, 2)
        result = build.mixed_input(composed, self.layout())
        self.assertEqual(result['terminalPreludeEnvelopes'], composed['terminalPreludeEnvelopes'])
        for prelude, final in [([env(11, 1), env(11, 0)], env(11, 2)),
                               ([env(11, 0)], env(5)), ([env(11, 0)], env(11, 2)),
                               ([], env(11, 0))]:
            layout = self.layout()
            layout['terminalPreludeEnvelopes'] = prelude
            layout['terminalEnvelope'] = final
            with self.assertRaises(ValueError):
                build.mixed_input(composed, layout)

    def test_terminal_must_be_terminal_and_policy_emission_exact(self):
        value = self.layout()
        value['terminalEnvelope'] = env(4)
        with self.assertRaises(ValueError):
            build.mixed_input(self.composed(), value)
        build.validate_emitted(final_source(policy()), policy())
        with self.assertRaises(ValueError):
            build.validate_emitted(final_source(policy()[:-1] + b'\0'), policy())
        with self.assertRaises(ValueError):
            build.validate_emitted(b'// public fixture\n' + final_source(policy()), policy())

    def test_final_lock_only_toml_equivalent_changes(self):
        source = self.root / 'source'
        source.mkdir()
        (source / 'Cargo.lock').write_text('version = 4\n\n[[package]]\nname="x"\nversion="1"\n')
        reference = self.artifact('lock', b'version = 4\n[[package]]\nname="x"\nversion="1"\n')
        before = build.digest(source / 'Cargo.lock')
        report = build.normalize_lock(source, reference, False)
        self.assertEqual(report['original'], before)
        self.assertTrue(report['parsedTomlIdentical'])
        self.assertFalse(report['rootLockChanged'])
        reference.write_text('version=4\n[[package]]\nname="y"\nversion="1"\n')
        with self.assertRaisesRegex(ValueError, 'dependencies'):
            build.normalize_lock(source, reference, False)

    def test_legacy_lock_never_allows_transitive_changes(self):
        source = self.root / 'source'
        source.mkdir()
        data = b'version=4\n[[package]]\nname="framenode-runtime"\nversion="1"\ndependencies=["x"]\n[[package]]\nname="x"\nversion="1"\n'
        (source / 'Cargo.lock').write_bytes(data)
        reference = self.artifact('lock', data.replace(b'["x"]', b'["x","sp-trie"]'))
        build.normalize_lock(source, reference, True)
        reference.write_bytes(reference.read_bytes().replace(b'name="x"', b'name="y"'))
        with self.assertRaises(ValueError):
            build.normalize_lock(source, reference, True)

    def test_fresh_python_process_receipts_and_retention_only(self):
        executable = str(Path(os.sys.executable).resolve())
        retained = {}
        raw, receipt = build.process([executable, '-I', '-c', 'print("synthetic child")'], self.root,
            self.root / 'process', lambda: None, retain=lambda path, pin: retained.update({str(path): pin}))
        self.assertEqual(raw, b'synthetic child\n')
        self.assertGreater(receipt['pid'], 0)
        self.assertEqual(receipt['exitCode'], 0)
        self.assertFalse(receipt['productionReady'])
        build.guard_files(retained)
        (self.root / 'process/stdout.log').write_bytes(b'changed')
        with self.assertRaises(ValueError):
            build.guard_files(retained)

    def test_launch_failure_has_no_invented_pid(self):
        with patch.object(build.subprocess, 'Popen', side_effect=OSError('missing')):
            with self.assertRaises(ValueError):
                build.process(['/missing'], self.root, self.root / 'failed', lambda: None)
        receipt = build.parse((self.root / 'failed/execution.json').read_bytes())
        self.assertIsNone(receipt['pid'])
        self.assertIsNone(receipt['exitCode'])
        self.assertIn('missing', receipt['stopReason'])

    def test_output_bound_and_nonzero_process_never_succeed(self):
        executable = str(Path(os.sys.executable).resolve())
        for index, code in enumerate(('print("x"*1000)', 'raise SystemExit(7)')):
            with self.assertRaises(ValueError):
                build.process([executable, '-I', '-c', code], self.root, self.root / str(index), lambda: None, maximum=100)
            receipt = build.parse((self.root / str(index) / 'execution.json').read_bytes())
            self.assertFalse(receipt['productionReady'])

    def test_guard_failure_retained_and_no_ready_claim(self):
        calls = 0
        def guard():
            nonlocal calls
            calls += 1
            if calls == 2:
                raise ValueError('changed')
        with self.assertRaises(ValueError):
            build.process([str(Path(os.sys.executable).resolve()), '-I', '-c', 'pass'], self.root, self.root / 'guard', guard)
        receipt = build.parse((self.root / 'guard/execution.json').read_bytes())
        self.assertEqual(receipt['guardError'], 'changed')
        self.assertFalse(receipt['productionReady'])


    def test_isolated_final_export_keeps_root_source_and_binds_generated_policy(self):
        source = self.root / 'root'
        source.mkdir()
        names = {'Cargo.lock': b'version = 4\n', 'runtime/src/lib.rs': b'spec_version: 138, transaction_version: 134,',
            'runtime/src/incident_recovery/generated.rs': b'unbound'}
        for name, raw in names.items():
            build.write(source / name, raw)
        inventory = {name: build.digest(source / name) for name in names}
        generated = final_source(policy())
        destination = self.root / 'export'
        with patch.object(build, 'ROOT', source):
            result = build.export_final(destination, inventory, generated, policy(), 'ab'*32, lambda: None)
        self.assertEqual((source / 'runtime/src/incident_recovery/generated.rs').read_bytes(), b'unbound')
        self.assertEqual((destination / 'runtime/src/incident_recovery/generated.rs').read_bytes(), generated)
        self.assertEqual(result['sourceFiles'], inventory)
        self.assertFalse(result['fixtureOnly'])
        self.assertFalse(result['productionReady'])
        self.assertEqual(result['sourceFingerprint']['runtime/src/incident_recovery/generated.rs'], build.pin_bytes(generated))

    def test_fixture_source_rejected_before_any_build_process(self):
        source, target = self.root / 'source', self.root / 'target'
        source.mkdir(); target.mkdir()
        build.write(source / 'FIXTURE-PROVENANCE.json', {'fixtureOnly': True})
        with patch.object(build, 'process') as child:
            with self.assertRaisesRegex(ValueError, 'fixture source'):
                build.build_wasm(source, self.root / 'output', target, 138, lambda: None)
            child.assert_not_called()

    def test_no_archived_build_receipt_is_an_input_field(self):
        value, path = self.bundle()
        value['actualBuildReceipt'] = {'status': 'passed', 'exitCode': 0}
        with self.assertRaisesRegex(ValueError, 'unexpected build input'):
            self.load(value, path)

    def test_full_mocked_pipeline_is_acyclic_and_never_promotes_readiness(self):
        # These substitutes exercise Python orchestration only, never represent
        # actual native/Cargo receipts, and disappear with the temporary tree.
        value, input_path = self.bundle()
        source = self.root / 'working-source'
        source.mkdir()
        lock = b'version=4\n[[package]]\nname="framenode-runtime"\nversion="1"\ndependencies=[]\n'
        for name, raw in {'Cargo.lock': lock, 'runtime/src/lib.rs': b'spec_version: 138, transaction_version: 134,',
                'runtime/src/incident_recovery/generated.rs': b'unbound'}.items():
            build.write(source / name, raw)
        inventory = {p.relative_to(source).as_posix(): build.digest(p) for p in source.rglob('*') if p.is_file()}
        inventory_path = self.artifact('inventory-source.json', inventory)
        legacy_lock = self.artifact('legacy.lock', lock)
        tool = self.artifact('synthetic-compiler', b'not executable; mock only')
        final_target, legacy_target = self.root / 'lane-final', self.root / 'lane-legacy'
        final_target.mkdir(); legacy_target.mkdir()
        args = SimpleNamespace(inputs=input_path, inputs_sha256=build.digest(input_path)['sha256'],
            source_inventory=inventory_path, source_inventory_sha256=build.digest(inventory_path)['sha256'],
            tool=tool, tool_sha256=build.digest(tool)['sha256'], legacy_lock=legacy_lock,
            legacy_lock_sha256=build.digest(legacy_lock)['sha256'], final_lock=None, final_lock_sha256=None,
            output=self.root / 'run', final_target=final_target, legacy_target=legacy_target)
        order = []
        def compose(value, inputs, output, binary, guard, commitment, remember, retain):
            guard()
            raw = policy()[:-64] + bytes.fromhex(commitment) + policy()[-32:]
            generated = final_source(raw)
            remember(output / 'policy.scale', raw)
            remember(output / 'generated.rs', generated)
            return raw, generated
        def fake_build(source, output, target, version, guard, retain, retain_source):
            guard(); order.append(version); output.mkdir()
            retain_source(source, build.fingerprint(source))
            result = output / f'runtime-{version}.compact.compressed.wasm'
            raw = wasm(version, 134 if version == 138 else 131 if version == 134 else 132)
            retain(result, build.write(result, raw))
            retain(output / 'build.json', build.write(output / 'build.json', {'schema': 'synthetic-not-executed', 'version': version}))
            return result
        def export(command, cwd, output, guard, **kwargs):
            guard()
            destination = Path(command[command.index('--destination') + 1])
            version = int(command[command.index('--spec-version') + 1])
            binding_path = Path(command[command.index('--binding') + 1])
            binding = build.parse(binding_path.read_bytes())
            self.assertEqual(set(binding), {'policyScale', 'policyCommitment', 'operatorsScale', 'previousGrandpaSetId', 'firstRecoverySlot', 'finalWasm'} | ({'holdWasm'} if version == 134 else set()))
            self.assertEqual(build.release.wasm_identity((binding_path.parent / binding['finalWasm']).read_bytes())['specVersion'], 138)
            if version == 134:
                self.assertEqual(build.release.wasm_identity((binding_path.parent / binding['holdWasm']).read_bytes())['specVersion'], 135)
            for name, raw in {'Cargo.lock': lock, 'runtime/src/incident_recovery/generated.rs': b'synthetic'}.items():
                build.write(destination / name, raw)
            build.write(destination / 'RECOVERY-PROVENANCE.json', {'bound': True, 'specVersion': version,
                'bindingEvidence': {'binding': build.bootstrap.digest(binding_path.read_bytes())}})
            output.mkdir()
            if version == 135 and getattr(self, 'mutate_completed_source', None):
                complete = cwd / 'source-final138'
                if self.mutate_completed_source == 'lock':
                    (complete / 'Cargo.lock').write_bytes(b'changed')
                else:
                    build.write(complete / 'runtime/unexpected.rs', b'unexpected')
            return b'{}', {'syntheticOnly': True}
        with patch.object(build, 'ROOT', source), patch.object(build, 'source_inventory', return_value=inventory), \
            patch.object(build, 'compose_inputs', side_effect=compose), patch.object(build, 'build_wasm', side_effect=fake_build), \
            patch.object(build, 'process', side_effect=export), patch.object(build, 'helper_pins', return_value={str(tool): build.digest(tool)}):
            result = build.build(args)
        self.assertEqual(order, [138, 135, 134])
        self.assertEqual((source / 'runtime/src/incident_recovery/generated.rs').read_bytes(), b'unbound')
        self.assertEqual(result['status'], 'passed')
        for key in ('productionReady', 'executingArtifactIdentityVerified', 'signingStopVerified', 'operatorApprovalsVerified'):
            self.assertFalse(result[key])
        self.assertIn('build-helpers/index.json', result['files'])
        self.assertIn('resolved-legacy-Cargo.lock', result['files'])
        self.assertTrue((args.output / 'BUILD-MANIFEST.json').is_file())

    def test_completed_source_mutation_during_later_stage_never_rebinds(self):
        for mutation in ('lock', 'extra-file'):
            case = ProductionBuildTests()
            case.setUp()
            case.mutate_completed_source = mutation
            try:
                with self.assertRaisesRegex(ValueError, 'completed isolated source'):
                    case.test_full_mocked_pipeline_is_acyclic_and_never_promotes_readiness()
                self.assertFalse((case.root / 'run/BUILD-MANIFEST.json').exists())
                self.assertTrue((case.root / 'run/failure.json').exists())
            finally:
                case.doCleanups()

    def test_source_provenance_rejects_prebuild_changes_and_legacy_omissions(self):
        actual = {'Cargo.lock': build.pin_bytes(b'lock'), 'runtime/code.rs': build.pin_bytes(b'code')}
        value = {'specVersion': 138, 'sourceFingerprint': copy.deepcopy(actual)}
        build.validate_source_provenance(self.root, value, actual)
        changed = copy.deepcopy(actual)
        changed['runtime/code.rs'] = build.pin_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'changed before build'):
            build.validate_source_provenance(self.root, value, changed)
        legacy = {'specVersion': 134, 'deployedSourceTree': build.bootstrap.LEGACY_TREE, 'sdk': 'polkadot-stable2603-3', 'overlay': {}}
        with self.assertRaisesRegex(ValueError, 'complete legacy'):
            build.validate_source_provenance(self.root, legacy, actual)

    def test_export_inventory_excludes_provenance_and_warm_target(self):
        build.write(self.root / 'Cargo.lock', b'lock')
        build.write(self.root / 'runtime/code.rs', b'code')
        build.write(self.root / 'RECOVERY-PROVENANCE.json', {})
        build.write(self.root / 'target/cached', b'cache')
        inventory = build.bootstrap.source_inventory(self.root)
        self.assertEqual(set(inventory), {'Cargo.lock', 'runtime/code.rs'})

    def test_unbound_refresh_inventory_tracks_preserved_actual_lock(self):
        destination = self.root / 'unbound'
        build.write(destination / 'Cargo.lock', b'preserved-local-lock')
        build.write(destination / 'runtime/code.rs', b'old')
        previous = {'bound': False, 'specVersion': 134, 'deployedSourceTree': build.bootstrap.LEGACY_TREE,
            'overlay': {'runtime/code.rs': build.bootstrap.digest(b'old')}}
        build.write(destination / 'RECOVERY-PROVENANCE.json', previous)
        def prepare(fresh, version, binding):
            build.write(fresh / 'Cargo.lock', b'base-lock')
            build.write(fresh / 'runtime/code.rs', b'new')
            return {**previous, 'overlay': {'runtime/code.rs': build.bootstrap.digest(b'new')}, 'sourceFiles': {'stale': {}}}
        with patch.object(build.bootstrap, 'prepare', side_effect=prepare):
            result = build.bootstrap.refresh(destination)
        self.assertEqual(result['sourceFiles'], build.bootstrap.source_inventory(destination))
        self.assertEqual(result['sourceFiles']['Cargo.lock'], build.bootstrap.digest(b'preserved-local-lock'))
        self.assertFalse(result['bound'])

    def test_timeout_kills_only_owned_mock_group(self):
        class Child:
            pid = 987654321
            returncode = None
            def poll(self): return None
            def wait(self, **kwargs): self.returncode = -9
        ticks = iter([0, 2, 3])
        with patch.object(build.subprocess, 'Popen', return_value=Child()), patch.object(build.time, 'monotonic', side_effect=lambda: next(ticks)), patch.object(build.os, 'killpg') as kill:
            with self.assertRaises(ValueError):
                build.process(['/synthetic'], self.root, self.root / 'timeout', lambda: None, timeout=1)
        kill.assert_called_once_with(987654321, build.signal.SIGKILL)
        receipt = build.parse((self.root / 'timeout/execution.json').read_bytes())
        self.assertEqual(receipt['stopReason'], 'timeout')
        self.assertEqual(receipt['exitCode'], -9)


if __name__ == '__main__':
    unittest.main()
