"""Pure synthetic witness tests; no production signature/native admission claim."""
import copy
import json
from pathlib import Path
import tempfile
import unittest

import hashi_codec as codec
import hashi_execution_witness as witness
import release_package as package
from test_release_package import wasm


def encoded(value):
    return (json.dumps(value, sort_keys=True) + '\n').encode()


def hx(number):
    return '0x' + bytes([number]).hex() * 32


def fixture(source=False):
    golden = codec.read_json(Path(__file__).with_name('hashi_codec_fixture.json'))
    template = copy.deepcopy(golden['template'])
    if source:
        template.update(format=codec.SOURCE_FORMAT, rawTransaction='0x0102', expectedChainId=hx(1))
        request = codec.raw(template['request'])
        template['request'] = '0x' + (request[:1] + bytes(20) + request[21:]).hex()
    commitment = codec.commitment(template)
    policy = (codec.u32(2) + codec.raw(codec.CHECKPOINT_HASH) + codec.raw(package.CHECKPOINT_ROOT)
              + codec.compact(1) + bytes([1]) * 32 + bytes([2]) * 32
              + codec.compact(1) + bytes([3]) * 32 + codec.compact(1) + codec.raw(commitment)
              + codec.compact(1) + bytes([4]) * 32
              + package.authorization_domain_scale({'chainSpecId': 'sora-nexus-public-synthetic-witness', 'releaseNonce': hx(5)})
              + bytes([6]) * 32 + bytes([7]) * 32)
    policy_hash = witness.blake(policy)
    code, metadata = wasm(134, 131), b'synthetic metadata, not execution evidence'
    plan = {'schema': 'sora-nexus-hashi-execution-plan-v1', 'templateCommitment': commitment,
            'network': 0, 'firstExecutionBlock': codec.CHECKPOINT + 2, 'lastExecutionBlock': codec.CHECKPOINT + 3,
            'specVersion': 134, 'transactionVersion': 131,
            'executionCodeHash': witness.blake(code), 'metadataHash': witness.blake(metadata)}
    baseline = ['0x01' + '00' * 64] * len(template['approvals'])
    data = {'policy': policy, 'wasm': code, 'metadata': metadata, 'template': encoded(template),
            'plan': encoded(plan), 'baseline': witness.envelope(template, baseline),
            'stateEvidence': b'unverified synthetic state observation', 'finalityEvidence': b'unverified synthetic finality'}
    roles = {'template': 'hashiTemplate0', 'plan': 'hashiExecutionPlan0', 'baseline': 'incomingWorkShard-00000', 'metadata': 'recoveryMetadata'}
    release = {'format': package.MANIFEST, 'checkpoint': {'number': codec.CHECKPOINT, 'hash': codec.CHECKPOINT_HASH},
               'productionReady': False, 'policy': package.policy_identity(policy), 'artifacts': [
                   {'role': role, 'sha256': witness.sha(data[name]), 'bytes': len(data[name]), 'blake2_256': witness.blake(data[name])[2:]}
                   for name, role in {**witness.ROLES, **roles}.items()]}
    data['release'] = encoded(release)
    marker = codec.MAGIC + codec.raw(codec.CHECKPOINT_HASH) + codec.raw(policy_hash)
    header = codec.raw(codec.CHECKPOINT_HASH) + codec.compact(codec.CHECKPOINT + 1) + bytes(64) + codec.compact(1) + b'\0' + codec.vec(marker)
    parent = witness.blake(header)
    signing = {'specVersion': 134, 'transactionVersion': 131, 'genesisHash': codec.GENESIS_HASH,
               'executionCodeHash': plan['executionCodeHash'], 'metadataHash': plan['metadataHash'],
               'executionBlock': codec.CHECKPOINT + 2,
               'lineage': {'policyHash': policy_hash, 'headers': ['0x' + header.hex()],
                           'expectedFinalizedHeadHash': parent, 'finalityEvidenceSha256': '0x' + witness.sha(data['finalityEvidence'])}}
    context = {'schema': 'sora-nexus-hashi-execution-context-v1', 'releaseManifestSha256': witness.sha(data['release']),
               'templateCommitment': commitment, 'network': 0, 'parentHash': parent, 'parentNumber': codec.CHECKPOINT + 1,
               'signingContext': signing, 'stateObservation': {'at': parent, 'policyCommitment': policy_hash,
                   'phase': 'Applying', 'cursor': 0, 'operationCounter': 0,
                   'accountNonces': [{'signer': a['signer'], 'nonce': 0} for a in template['approvals']],
                   'evidenceSha256': witness.sha(data['stateEvidence'])}}
    data['context'] = encoded(context)
    data['signatures'] = encoded({'schema': 'sora-nexus-hashi-signature-witnesses-v1', 'contextSha256': witness.sha(data['context']),
                                'templateCommitment': commitment, 'baselineSignatures': baseline, 'signatures': golden['signatures']})
    return data, roles


def edit(data, name, change):
    value = json.loads(data[name])
    change(value)
    data[name] = encoded(value)


def rebind_artifact(data, roles, name):
    """A different synthetic release can bind invalid content; deeper guards
    must still reject it. This is not an immutable-release edit API."""
    role = {**witness.ROLES, **roles}[name]
    def update(release):
        for row in release['artifacts']:
            if row['role'] == role:
                row.update(sha256=witness.sha(data[name]), bytes=len(data[name]), blake2_256=witness.blake(data[name])[2:])
    edit(data, 'release', update)
    edit(data, 'context', lambda context: context.update(releaseManifestSha256=witness.sha(data['release'])))
    edit(data, 'signatures', lambda signatures: signatures.update(contextSha256=witness.sha(data['context'])))


class HashiExecutionWitnessTests(unittest.TestCase):
    def test_both_native_variants_bind_signature_only_replacement(self):
        for source in (False, True):
            data, roles = fixture(source)
            envelope, report = witness.verify(data, roles)
            template, signatures = json.loads(data['template']), json.loads(data['signatures'])['signatures']
            self.assertEqual(envelope, witness.envelope(template, signatures))
            self.assertNotEqual(envelope, data['baseline'])
            self.assertEqual(report['templateCommitment'], codec.commitment(template))
            self.assertEqual(report['parentNumber'] + 1, report['executionBlock'])
            self.assertNotIn('signingPayloads', report)
            self.assertIn('unverifiedSigningPayloads', report)
            for key in ('signatureVerification', 'nativeAdmissionVerified', 'stateProofVerified', 'finalityVerified',
                        'runtimeExecuted', 'chainReplayProtectionVerified', 'productionReady'):
                self.assertIs(report[key], False)

    def test_noncanonical_signature_width_or_variant_is_rejected(self):
        for signature in ('0x01' + '11' * 63, '0x02' + '11' * 66, '0x03' + '11' * 64):
            with self.subTest(signatureBytes=len(codec.raw(signature))):
                data, roles = fixture()
                edit(data, 'signatures', lambda value: value['signatures'].__setitem__(0, signature))
                with self.assertRaises(ValueError):
                    witness.verify(data, roles)

    def test_business_field_baseline_or_release_artifact_tampering_rejected(self):
        for name, change in [('template', lambda v: v.update(network=1)),
                             ('template', lambda v: v['approvals'][0].update(call='0x0000')),
                             ('template', lambda v: v['approvals'][0].update(extra='0xa5000400')),
                             ('plan', lambda v: v.update(lastExecutionBlock=codec.CHECKPOINT + 200)),
                             ('release', lambda v: v['artifacts'][0].update(sha256='00' * 32))]:
            data, roles = fixture(); edit(data, name, change)
            with self.assertRaises(ValueError): witness.verify(data, roles)
        data, roles = fixture(); data['baseline'] = data['baseline'][:-1] + b'\0'
        with self.assertRaises(ValueError): witness.verify(data, roles)

    def test_committed_operation_must_match_policy(self):
        data, roles = fixture()
        # Even a consistently rehashed release may not move the template to an
        # unrelated operation hash in its policy.
        bad = data['policy'].replace(codec.raw(codec.commitment(json.loads(data['template']))), bytes([8]) * 32, 1)
        data['policy'] = bad
        def update(value):
            value['policy'] = package.policy_identity(bad)
            for row in value['artifacts']:
                if row['role'] == 'policyScale': row.update(sha256=witness.sha(bad), bytes=len(bad), blake2_256=witness.blake(bad)[2:])
        edit(data, 'release', update)
        with self.assertRaisesRegex(ValueError, 'committed policy'): witness.verify(data, roles)

    def test_context_drift_nonce_replay_network_and_runtime_fail(self):
        for change in [lambda v: v.update(network=1), lambda v: v.update(parentHash=hx(8)),
                       lambda v: v.update(parentNumber=codec.CHECKPOINT),
                       lambda v: v['stateObservation'].update(cursor=1),
                       lambda v: v['stateObservation'].update(operationCounter=1),
                       lambda v: v['stateObservation'].update(phase='Complete'),
                       lambda v: v['stateObservation']['accountNonces'][0].update(nonce=1),
                       lambda v: v['signingContext'].update(executionCodeHash=hx(8)),
                       lambda v: v['signingContext'].update(metadataHash=hx(8)),
                       lambda v: v['signingContext'].update(transactionVersion=134),
                       lambda v: v['signingContext'].update(genesisHash=hx(8)),
                       lambda v: v['signingContext']['lineage'].update(policyHash=hx(8)),
                       lambda v: v['signingContext']['lineage'].update(finalityEvidenceSha256=hx(8))]:
            data, roles = fixture(); edit(data, 'context', change)
            with self.assertRaises(ValueError): witness.verify(data, roles)

    def test_expired_window_and_nonparent_head_fail(self):
        data, roles = fixture()
        edit(data, 'context', lambda v: v['signingContext'].update(executionBlock=codec.CHECKPOINT + 4))
        with self.assertRaisesRegex(ValueError, 'expired'): witness.verify(data, roles)
        data, roles = fixture()
        edit(data, 'context', lambda v: v['signingContext'].update(executionBlock=codec.CHECKPOINT + 3))
        with self.assertRaisesRegex(ValueError, 'exact execution parent'): witness.verify(data, roles)

    def test_release_bound_invalid_baseline_and_cross_era_window_fail(self):
        data, roles = fixture()
        data['baseline'] += b'changed'
        rebind_artifact(data, roles, 'baseline')
        with self.assertRaisesRegex(ValueError, 'baseline envelope changes business fields'):
            witness.verify(data, roles)
        data, roles = fixture()
        edit(data, 'plan', lambda plan: plan.update(lastExecutionBlock=codec.CHECKPOINT + 1024))
        rebind_artifact(data, roles, 'plan')
        with self.assertRaisesRegex(ValueError, 'planned window crosses mortal era or expiry'):
            witness.verify(data, roles)

    def test_boolean_network_aliases_are_rejected(self):
        data, roles = fixture()
        edit(data, 'context', lambda context: context.update(network=False))
        with self.assertRaisesRegex(ValueError, 'Invalid unsigned integer'):
            witness.verify(data, roles)
        data, roles = fixture()
        edit(data, 'plan', lambda plan: plan.update(network=False))
        rebind_artifact(data, roles, 'plan')
        with self.assertRaisesRegex(ValueError, 'Invalid unsigned integer'):
            witness.verify(data, roles)

    def test_missing_duplicate_or_forged_observation_is_not_admitted_as_authentication(self):
        for change in [lambda v: v['stateObservation']['accountNonces'].pop(),
                       lambda v: v['stateObservation']['accountNonces'][0].update(signer=hx(8)),
                       lambda v: v['stateObservation'].update(authenticated=True),
                       lambda v: v.update(finalityVerified=True)]:
            data, roles = fixture(); edit(data, 'context', change)
            with self.assertRaises(ValueError): witness.verify(data, roles)
        data, roles = fixture(); data['stateEvidence'] += b'changed'
        with self.assertRaises(ValueError): witness.verify(data, roles)

    def test_signature_context_binding_and_placeholder_rejection(self):
        for change in [lambda v: v.update(contextSha256='00' * 32),
                       lambda v: v.update(templateCommitment=hx(8)),
                       lambda v: v['signatures'].pop(),
                       lambda v: v.update(signatures=v['baselineSignatures']),
                       lambda v: v['baselineSignatures'].pop()]:
            data, roles = fixture(); edit(data, 'signatures', change)
            with self.assertRaises(ValueError): witness.verify(data, roles)

    def test_repaired_policy_duplicate_roles_and_unknown_fields_rejected(self):
        for incoming in ({'activePolicy': 'other'}, {}, None):
            data, roles = fixture(); edit(data, 'release', lambda v: v.update(incomingRecoveryReplay=incoming))
            with self.assertRaises(ValueError): witness.verify(data, roles)
        data, roles = fixture(); roles['plan'] = roles['template']
        with self.assertRaises(ValueError): witness.verify(data, roles)
        with self.assertRaises(ValueError): witness.parse(b'{"a":1,"a":2}')

    def write_inputs(self, root):
        data, roles = fixture()
        files = {}
        for name, content in data.items():
            (root / name).write_bytes(content)
            files[name] = {'file': name, 'bytes': len(content), 'sha256': witness.sha(content)}
        manifest = root / 'manifest.json'
        manifest.write_bytes(encoded({'schema': witness.SCHEMA, 'files': files, 'releaseRoles': roles}))
        return manifest, data

    def test_append_is_exclusive_and_preserves_release_and_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            manifest, before = self.write_inputs(root)
            destination, report = witness.bundle(manifest, witness.sha(manifest.read_bytes()), root / 'journal')
            self.assertEqual((destination / 'BUNDLE-COMPLETE').read_text().strip(), witness.sha((destination / 'bundle.json').read_bytes()))
            self.assertEqual((destination / 'envelope.scale').read_bytes(), witness.envelope(json.loads(before['template']), json.loads(before['signatures'])['signatures']))
            for name, original in before.items(): self.assertEqual((root / name).read_bytes(), original)
            with self.assertRaises(FileExistsError): witness.bundle(manifest, witness.sha(manifest.read_bytes()), root / 'journal')
            self.assertIs(report['nativeAdmissionVerified'], False)

    def test_input_pins_paths_and_aliases_fail_before_any_append(self):
        for mutation in ('pin', 'path', 'symlink', 'changed', 'extra'):
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve(); manifest, _ = self.write_inputs(root)
                value = json.loads(manifest.read_bytes())
                if mutation == 'path': value['files']['template']['file'] = '../template'
                if mutation == 'extra': value['unknown'] = True
                if mutation == 'symlink':
                    (root / 'template').rename(root / 'original'); (root / 'template').symlink_to('original')
                if mutation == 'changed': (root / 'template').write_bytes(b'changed')
                manifest.write_bytes(encoded(value))
                pin = '00' * 32 if mutation == 'pin' else witness.sha(manifest.read_bytes())
                with self.assertRaises(ValueError): witness.bundle(manifest, pin, root / 'journal')
                self.assertFalse((root / 'journal').exists())


if __name__ == '__main__':
    unittest.main()
