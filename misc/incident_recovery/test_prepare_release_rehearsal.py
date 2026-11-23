"""Synthetic file/admission tests; no runtime execution, Cargo, RPC or state proof."""
import copy
import gzip
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import hashi_codec as scale
import prepare_release_rehearsal as replay
import production_build as build
import release_package as release
from test_release_package import h, hx, operators, wasm


def policy(work=1):
    return (scale.u32(2) + scale.raw(scale.CHECKPOINT_HASH) + scale.raw(release.CHECKPOINT_ROOT)
            + scale.compact(1) + h(1) + h(2) + scale.compact(1) + h(3)
            + scale.compact(work) + b''.join(h(4 + i) for i in range(work)) + scale.compact(1) + h(6)
            + release.authorization_domain_scale({'chainSpecId': 'sora-nexus-recovery-28191817', 'releaseNonce': hx(8)})
            + h(7) + h(9))


def envelope(tag, index=0):
    return '0x' + (b'snxrec01\x01' + bytes([tag]) + scale.u32(index) + b'placeholder').hex()


class PrepareReleaseRehearsalTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='sora-production-replay-unit-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.output_counter = 0
        self.selected = {'previousGrandpaSetId': 7956, 'lastObservedAbandonedSlot': 298871044, 'firstRecoverySlot': 298871045}
        self.observed = {'schema': 'synthetic-unexecuted-public-slot-selection', **self.selected}
        mock = patch.object(build, 'public_consensus', return_value=(self.selected, self.observed))
        mock.start(); self.addCleanup(mock.stop)
        mock = patch.object(replay, 'CHECKPOINT_METADATA_SHA256', replay.sha(b'synthetic checkpoint metadata'))
        mock.start(); self.addCleanup(mock.stop)

    def write(self, name, value):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(value if isinstance(value, bytes) else build.serialized(value))
        return build.digest(path)

    def reseal(self):
        self.outer['files'] = {p.relative_to(self.root).as_posix(): build.digest(p)
                               for p in self.root.rglob('*') if p.is_file() and p.name != 'BUILD-MANIFEST.json'}
        self.write('BUILD-MANIFEST.json', self.outer)
        self.pin = build.digest(self.root / 'BUILD-MANIFEST.json')['sha256']

    def fixture(self, mixed=False):
        final_policy = policy(2 if mixed else 1)
        self.write('policy.scale', final_policy)
        composition = {'policyScale': '0x' + policy().hex(), 'storageEngineRehearsed': True,
                       'productionReady': False, 'applyEnvelopes': [envelope(1)], 'auditEnvelopes': [envelope(2)],
                       'terminalPreludeEnvelopes': [], 'terminalEnvelope': envelope(5)}
        self.write('compose/stdout.log', composition)
        if mixed:
            mixed_input = {'workEnvelopes': [envelope(1), envelope(6, 1)], 'auditEnvelopes': [envelope(2)],
                           'terminalEnvelope': envelope(5), 'terminalPreludeEnvelopes': [], 'deriveNativeBalanceAudits': True}
            self.write('mixed-input.json', mixed_input)
            self.write('mixed/stdout.log', {**mixed_input, 'policyScale': '0x' + final_policy.hex(),
                       'structuralValidationOnly': True, 'productionReady': False,
                       'nativeBalanceAuditsDerived': True, 'nativeBalanceAuditIndices': [], 'baselineAuditEnvelopes': [envelope(2)]})
        self.write('public-consensus.json', self.observed)
        for name in ('started.json', 'headers.json', 'completion.json'):
            self.write('inputs/public/' + name, {})
        inputs = {'schema': build.SCHEMA, 'compose': {'capture': 'retained'}, 'mixedLayout': 'mixed.json' if mixed else None,
                  'consensus': {'publicCapture': 'public'}}
        input_pin = self.write('input-manifest.json', inputs)
        raw = {'timestamp': (298823260 * 6000 + 2).to_bytes(8, 'little'),
               'genesis': (269923710).to_bytes(8, 'little'), 'slot': (298823260).to_bytes(8, 'little'),
               'epoch': (48165).to_bytes(8, 'little'), 'session': scale.u32(48165), 'current': scale.u32(7810),
               'active': scale.u32(7810) + b'\x01' + (12).to_bytes(8, 'little'), 'forced': b'\0',
               'bonded': scale.compact(1) + scale.u32(7810) + scale.u32(48164),
               'validators': scale.compact(15) + b''.join(h(30 + i) for i in range(15))}
        rows = sorted([['0x' + replay.KEYS[name], '0x' + data.hex()] for name, data in raw.items()])
        page = gzip.compress(b'\n'.join(build.serialized(item).strip().replace(b'\n', b'') for item in
                             [{'checkpointHash': scale.CHECKPOINT_HASH, 'trie': 'top'}, *rows]), mtime=0)
        page_pin = self.write('inputs/retained/pages/top/0.jsonl.gz', page)
        metadata_pin = self.write('inputs/retained/runtime-metadata.scale', b'synthetic checkpoint metadata')
        capture_pin = self.write('inputs/retained/capture-complete.json', {
            'checkpoint': {'hash': scale.CHECKPOINT_HASH, 'stateRoot': release.CHECKPOINT_ROOT},
            'artifacts': {'runtime-metadata.scale': metadata_pin}, 'children': [],
            'top': {'pages': [{'file': 'pages/top/0.jsonl.gz', 'sha256': page_pin['sha256'],
                              'keyCount': len(rows), 'firstKey': rows[0][0], 'lastKey': rows[-1][0]}]}})
        op = operators((1, [2], [4], [5]))
        self.write('checkpoint-operators.scale', op)
        self.write('checkpoint-consensus/stdout.log', {
            'schema': 'sora-nexus-checkpoint-consensus-v2', 'checkpointHash': scale.CHECKPOINT_HASH,
            'checkpointNumber': scale.CHECKPOINT, 'checkpointStateRoot': release.CHECKPOINT_ROOT,
            'captureManifestSha256': capture_pin['sha256'], 'signingKeysPreserved': True,
            'manualOperatorApprovalRequired': False, 'productionReady': False, 'checkpointSlot': 298823260,
            'previousGrandpaSetId': 7956, 'grandpaAuthorities': [[hx(2), 1]], 'previousGrandpaAuthorities': [[hx(2), 1]],
            'approvedGrandpaKeys': [hx(2)], 'operatorsScale': '0x' + op.hex(),
            'operatorsBlake2': '0x' + build.pin_bytes(op)['blake2_256'],
            'checkpointMetadata': {'schema': 'sora-nexus-checkpoint-metadata-witness-v1',
                'checkpointHash': scale.CHECKPOINT_HASH, 'stateRoot': release.CHECKPOINT_ROOT,
                'wasmExecutionVerified': True, 'nativeExecution': False}})
        artifacts, receipts = {}, {}
        for key, directory, version in (('134', 'build-134', (134, 131)), ('135', 'build-135', (135, 132)),
                                         ('final138', 'build-final138', (138, 134))):
            name = f'runtime-{version[0]}.compact.compressed.wasm'
            pin = self.write(directory + '/' + name, wasm(*version))
            artifacts[key] = {'path': directory + '/' + name, **pin}
            receipts[directory] = self.write(directory + '/build.json', {
                'status': 'passed', 'exitCode': 0, 'fixtureOnly': False,
                'specVersion': version[0], 'transactionVersion': version[1], 'artifacts': {name: pin}})
        self.write('bindings/final138.wasm', wasm(138, 134))
        self.write('bindings/hold135.wasm', wasm(135, 132))
        self.write('bindings/policy.scale', final_policy)
        self.write('bindings/operators.scale', op)
        for version in (134, 135):
            binding = {'policyScale': '0x' + final_policy.hex(), 'policyCommitment': '0x' + release.policy_identity(final_policy)['commitment'],
                       'operatorsScale': '0x' + op.hex(), 'previousGrandpaSetId': 7956,
                       'firstRecoverySlot': self.selected['firstRecoverySlot'], 'finalWasm': 'final138.wasm'}
            if version == 134:
                binding['holdWasm'] = 'hold135.wasm'
            self.write(f'bindings/binding-{version}.json', binding)
        for version, directory, source in ((134, 'build-134', 'source-134'), (135, 'build-135', 'source-135'),
                                           (138, 'build-final138', 'source-final138')):
            if version == 138:
                generated = ('pub const POLICY_SCALE: &[u8] = &[' + ','.join(map(str, final_policy)) + '];\n').encode()
                provenance = {'bound': True, 'fixtureOnly': False, 'policy': build.pin_bytes(final_policy)}
            else:
                generated, evidence, _ = build.bootstrap.bind_inputs(self.root / f'bindings/binding-{version}.json', version)
                generated = generated.encode()
                provenance = {'bound': True, 'fixtureOnly': False, 'bindingEvidence': evidence}
            generated_pin = self.write(source + '/runtime/src/incident_recovery/generated.rs', generated)
            source_pin = self.write(directory + '/source-files.json', {'runtime/src/incident_recovery/generated.rs': generated_pin})
            provenance_pin = self.write(source + '/RECOVERY-PROVENANCE.json', provenance)
            receipt = json.loads((self.root / directory / 'build.json').read_bytes())
            receipts[directory] = self.write(directory + '/build.json', {**receipt,
                'sourceProvenance': provenance_pin, 'sourceFingerprint': source_pin})
        self.outer = {'schema': 'sora-nexus-production-recovery-build-v1', 'status': 'passed', 'fixtureOnly': False,
                      'productionReady': False, 'inputManifest': input_pin, 'policy': build.pin_bytes(final_policy),
                      'artifacts': artifacts, 'buildReceipts': receipts}
        self.reseal()

    def prepare(self, first=None):
        self.output_counter += 1
        destination = self.root.parent / (self.root.name + '-output-' + str(self.output_counter))
        self.addCleanup(lambda: __import__('shutil').rmtree(destination, ignore_errors=True))
        result = replay.prepare(self.root, destination, first, build_manifest_sha256=self.pin)
        return result, json.loads((destination / 'manifest.json').read_bytes()), destination

    def test_completed_artifacts_full_era_schedule_and_gap(self):
        self.fixture()
        result, manifest, destination = self.prepare()
        self.assertEqual(result['ordinaryBlocks'], 62)
        self.assertEqual(manifest['steps'][1]['slot'] - manifest['steps'][0]['slot'], 14400)
        ordinary = [row for row in manifest['steps'] if row['recovery'] is None]
        for session in range(4):
            rows = ordinary[session * 15:session * 15 + 15]
            self.assertEqual([r['authorityIndex'] for r in rows], list(range(15)))
            self.assertEqual((rows[0]['slot'] - 269923710) % 600, 0)
        self.assertEqual(ordinary[-1]['slot'], ordinary[-2]['slot'] + 1)
        self.assertEqual((ordinary[-2]['slot'] - 269923710) // 600 - (ordinary[0]['slot'] - 269923710) // 600, 4)
        self.assertTrue(all(row['timestamp'] == row['slot'] * 6000 + 2 for row in manifest['steps']))
        self.assertEqual((destination / 'runtime-138.wasm').read_bytes(), wasm(138, 134))
        self.assertFalse(json.loads((destination / 'PREPARATION.json').read_bytes())['nativeExecutionVerified'])

    def test_mixed_final_work_is_selected_instead_of_original_composition(self):
        self.fixture(mixed=True)
        result, manifest, destination = self.prepare()
        self.assertEqual(result['applyEnvelopes'], 2)
        self.assertEqual((destination / manifest['steps'][2]['recovery']['file']).read_bytes(), scale.raw(envelope(6, 1)))
        self.assertEqual(json.loads((destination / 'PREPARATION.json').read_bytes())['workReport'], 'mixed/stdout.log')

    def test_changed_same_version_artifact_is_rejected(self):
        self.fixture()
        self.write('build-final138/runtime-138.compact.compressed.wasm', wasm(138, 134, b'different'))
        with self.assertRaisesRegex(ValueError, 'pin differs'):
            self.prepare()

    def test_manifest_must_be_passed_and_externally_pinned(self):
        self.fixture()
        self.outer['status'] = 'running'; self.reseal()
        with self.assertRaisesRegex(ValueError, 'completed production'):
            self.prepare()
        self.outer['status'] = 'passed'; self.reseal(); self.pin = '00' * 32
        with self.assertRaisesRegex(ValueError, 'pin differs'):
            self.prepare()

    def test_first_slot_assertion_and_each_compiled_binding_are_exact(self):
        self.fixture()
        for first in (True, 298871044, 298871046):
            with self.subTest(first=first), self.assertRaisesRegex(ValueError, 'first slot'):
                self.prepare(first)
        for version in (134, 135):
            name = f'bindings/binding-{version}.json'
            original = json.loads((self.root / name).read_bytes())
            self.write(name, {**original, 'firstRecoverySlot': 298871046}); self.reseal()
            with self.subTest(version=version), self.assertRaisesRegex(ValueError, 'compiled first-slot'):
                self.prepare()
            self.write(name, original); self.reseal()

    def test_stale_mixed_policy_and_missing_report_are_rejected(self):
        self.fixture(mixed=True)
        name = 'mixed/stdout.log'; value = json.loads((self.root / name).read_bytes())
        self.write(name, {**value, 'policyScale': '0x' + policy().hex()}); self.reseal()
        with self.assertRaisesRegex(ValueError, 'final composed work policy'):
            self.prepare()
        (self.root / name).unlink(); self.reseal()
        with self.assertRaisesRegex(ValueError, 'absent from completed build'):
            self.prepare()

    def test_failed_build_receipt_and_mismatched_manifest_artifact_rejected(self):
        self.fixture()
        name = 'build-135/build.json'; value = json.loads((self.root / name).read_bytes())
        self.outer['buildReceipts']['build-135'] = self.write(name, {**value, 'status': 'failed'}); self.reseal()
        with self.assertRaisesRegex(ValueError, 'passed artifact build'):
            self.prepare()
        self.outer['buildReceipts']['build-135'] = self.write(name, value)
        self.outer['artifacts']['135']['sha256'] = '00' * 32; self.reseal()
        with self.assertRaisesRegex(ValueError, 'build artifact pin'):
            self.prepare()

    def test_schedule_overflow_and_author_bounds(self):
        facts = {'genesis': 269923710, 'rotations': 5, 'authors': 15, 'timestamp': 2}
        with self.assertRaisesRegex(ValueError, 'overflow'):
            replay.normal_slots(2**64 // 6000, facts)

    def test_checkpoint_page_mutation_and_metadata_mismatch_rejected(self):
        self.fixture()
        name = 'inputs/retained/pages/top/0.jsonl.gz'
        self.write(name, b'not original page'); self.reseal()
        with self.assertRaisesRegex(ValueError, 'checkpoint page hash'):
            self.prepare()

    def test_outer_path_traversal_rejected_before_read(self):
        self.fixture()
        self.outer['artifacts']['134']['path'] = '../runtime.wasm'; self.reseal()
        with self.assertRaisesRegex(ValueError, 'unsafe relative'):
            self.prepare()

    def test_compiled_first_slot_constants_must_match_bound_input(self):
        self.fixture()
        name = 'source-134/runtime/src/incident_recovery/generated.rs'
        data = (self.root / name).read_bytes().replace(b'FIRST_RECOVERY_SLOT: u64 = 298871045', b'FIRST_RECOVERY_SLOT: u64 = 298871046')
        self.write(name, data); self.reseal()
        with self.assertRaisesRegex(ValueError, 'compiled bootstrap constants'):
            self.prepare()

    def test_missing_child_capture_dependency_rejected(self):
        self.fixture()
        name = 'inputs/retained/capture-complete.json'
        value = json.loads((self.root / name).read_bytes())
        value['children'] = [{'pages': [{'file': 'pages/child/0.jsonl.gz', 'sha256': 'ab' * 32}]}]
        pin = self.write(name, value)
        witness_name = 'checkpoint-consensus/stdout.log'
        witness = json.loads((self.root / witness_name).read_bytes())
        witness['captureManifestSha256'] = pin['sha256']
        self.write(witness_name, witness); self.reseal()
        with self.assertRaisesRegex(ValueError, 'checkpoint dependency missing'):
            self.prepare()

    def test_mixed_derived_audit_baseline_cannot_be_replaced(self):
        self.fixture(mixed=True)
        name = 'mixed/stdout.log'; value = json.loads((self.root / name).read_bytes())
        self.write(name, {**value, 'auditEnvelopes': [envelope(2, 99)]}); self.reseal()
        with self.assertRaisesRegex(ValueError, 'unlisted audit changed'):
            self.prepare()


if __name__ == '__main__':
    unittest.main()
