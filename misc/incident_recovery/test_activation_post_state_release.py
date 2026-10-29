"""Adversarial activation evidence joins; these tests do not execute Wasm."""
import copy
import unittest
import activation_post_state_release as activation


def activation_witness(rows, details, policy, code):
    """Synthetic storage observations for evidence-join tests only."""
    first = next(i for i, detail in enumerate(details) if detail['executingSpec'] == 138)
    parent = rows[first-1]
    ordinary = rows[first:]
    witness = {'schema': activation.SCHEMA, 'policyCommitment': policy,
        'lineagePolicyCommitment': policy,
        'executingCodeBlake2': code, 'firstBlock': ordinary[0]['number'],
        'maintenanceParentHash': parent['hash'], 'maintenanceStateRoot': parent['stateRoot'],
        'maintenanceSnapshotSha256': parent['diskSnapshotManifestSha256'],
        'initializedStateRoot': '0x' + 'ff'*32, 'previousStakingVersion': 16,
        'activatedStakingVersion': 18, 'ordinaryBlocks': len(ordinary),
        'lastBlockHash': rows[-1]['hash'], 'lastStateRoot': rows[-1]['stateRoot'],
        'cutoffEra': 7, 'firstExecutionEra': 6, 'weightedPointsStartEra': 7,
        'runtimeUpgradeExecutedAsWasm': True, 'independentImportAndRestartVerified': True,
        'tryRuntimeFeatureChecksExecuted': False, 'fullCustomStakingVerified': False, 'productionReady': False}
    slots = [0] + [detail['step']['slot'] for detail in details[:first]]
    witness['maintenance'] = {'checkpointSlot': 0, 'checkpointEpoch': 1, 'checkpointSession': 100,
        'checkpointActiveEra': 6, 'checkpointCurrentEra': 7, 'firstRecoverySlot': slots[1],
        'firstRecoveryEpoch': 2, 'lastSlot': slots[-1], 'lastEpoch': 2, 'blocks': first,
        'maximumSlotGap': max(b-a for a,b in zip(slots, slots[1:]))}
    observed = []
    for index, block in enumerate(ordinary):
        changed = index >= 1
        observed.append({'block': block['number'], 'blockHash': block['hash'], 'stateRoot': block['stateRoot'],
            'snapshotManifestSha256': block['diskSnapshotManifestSha256'], 'executingCodeBlake2': code,
            'slot': details[first+index]['step']['slot'], 'epoch': 3 if changed else 2,
            'timestamp': details[first+index]['step']['timestamp'],
            'session': 101 if changed else 100, 'currentEra': 7, 'activeEra': 7 if changed else 6,
            'activeEraStart': details[first+1]['step']['timestamp'] if changed else 1,
            'asyncState': {'pendingValidators': 0, 'pendingEra': None, 'pruneUpTo': None,
                'queuedSession': None if changed else 101, 'queuedEra': None if changed else 7,
                'electionDeadline': None, 'forced': False, 'timestampEra': None},
            'lastValidatorEra': {'count': 1, 'sha256': 'ab'*32, 'active': [{'account': '0x'+'ee'*32, 'era': 7}],
                'queued': [] if changed else [{'account': '0x'+'ee'*32, 'era': 7}], 'pending': []},
            'readOnlyStateRootMatches': True})
    witness['rows'] = observed
    witness['transitions'] = {'firstSession': {'block': ordinary[1]['number'], 'before': 100, 'after': 101} if len(ordinary)>1 else None,
        'firstActiveEra': {'block': ordinary[1]['number'], 'before': 6, 'after': 7} if len(ordinary)>1 else None,
        'postFirstSessionBlocks': max(len(ordinary)-2,0), 'postFirstActiveEraBlocks': max(len(ordinary)-2,0)}
    return witness


def fixture(count=3):
    code = 'ab'*32
    rows = [{'number': 100 + index, 'hash': '0x' + f'{index+1:02x}' * 32,
             'stateRoot': '0x' + f'{index+11:02x}' * 32,
             'diskSnapshotManifestSha256': f'{index+21:02x}' * 32,
             'phase': 'Complete', 'resultingSpec': 138, 'executingCodeBlake2': code,
             'independentImportRootMatches': True, 'diskRestartRootMatches': True}
            for index in range(count+1)]
    steps = [{'executingSpec': 134 if i == 0 else 138,
              'step': {'slot': 100+i, 'timestamp': (100+i)*6000}} for i in range(count+1)]
    result = {'publicTestCouncilFixture': False, 'publicTestHashiFixture': False,
              'forwardRepairObserved': False, 'ordinaryRuntimeBlocks': count, 'blocks': rows,
              'activationValidation': activation_witness(rows, steps, 'cd'*32, code)}
    context = {'policyCommitment': 'cd'*32, 'wasm': {'138': {'blake2': code}},
               'scenarios': {'activation': {'steps': steps}}}
    return result, context


class ActivationTests(unittest.TestCase):
    def test_both_boundaries_and_followup_are_required(self):
        result, context = fixture()
        self.assertEqual(activation.validate(result, context)['status'], 'verified')
        for count in (1, 2):
            result, context = fixture(count)
            self.assertEqual(activation.validate(result, context)['status'], 'unverified')

    def test_legacy_observer_cannot_satisfy_boundary_gate(self):
        result, context = fixture()
        result['activationValidation']['schema'] = 'sora-nexus-exact-post-maintenance-activation-v1'
        self.assertEqual(activation.validate(result, context)['status'], 'unverified')

    def test_state_observation_and_transition_tampering_rejected(self):
        result, context = fixture()
        changes = [lambda w: w['maintenance'].update(lastSlot=999),
            lambda w: w['maintenance'].update(maximumSlotGap=1),
            lambda w: w['transitions'].update(postFirstActiveEraBlocks=999),
            lambda w: w['rows'][1].update(stateRoot='0x'+'33'*32),
            lambda w: w['rows'][1].update(slot=999),
            lambda w: w['rows'][1].update(session=103),
            lambda w: w['rows'][1]['asyncState'].update(queuedSession=100, queuedEra=8),
            lambda w: w['rows'][1]['lastValidatorEra']['active'][0].update(era=5),
            lambda w: w['rows'][1].update(readOnlyStateRootMatches=False)]
        for change in changes:
            changed = copy.deepcopy(result); change(changed['activationValidation'])
            with self.assertRaises(ValueError): activation.validate(changed, context)

    def test_repaired_runtime_uses_final_policy_and_same_boundaries(self):
        result, context = fixture()
        result['forwardRepairObserved'] = True
        context['scenarios']['repair'] = context['scenarios'].pop('activation')
        context['policies'] = {'138': {'commitment': 'cd'*32}}
        context['policyCommitment'] = 'ee'*32
        result['activationValidation']['lineagePolicyCommitment'] = 'ee'*32
        self.assertEqual(activation.validate(result, context, 'repair')['status'], 'verified')
        result['activationValidation']['policyCommitment'] = 'ee'*32
        with self.assertRaises(ValueError): activation.validate(result, context, 'repair')

    def test_installation_without_observer_never_passes(self):
        result, context = fixture()
        del result['activationValidation']
        self.assertEqual(activation.validate(result, context)['status'], 'unverified')

    def test_detached_parent_policy_code_and_postconditions_rejected(self):
        result, context = fixture()
        changes = {'policyCommitment': 'ee'*32, 'executingCodeBlake2': 'ef'*32,
                   'maintenanceParentHash': '0x'+'11'*32,
                   'maintenanceStateRoot': '0x'+'12'*32,
                   'maintenanceSnapshotSha256': '13'*32,
                   'firstBlock': True, 'previousStakingVersion': 18,
                   'activatedStakingVersion': 16, 'weightedPointsStartEra': 9,
                   'ordinaryBlocks': 999, 'lastStateRoot': '0x'+'14'*32,
                   'runtimeUpgradeExecutedAsWasm': False, 'tryRuntimeFeatureChecksExecuted': True}
        for key, value in changes.items():
            with self.subTest(key=key):
                bad = copy.deepcopy(result); bad['activationValidation'][key] = value
                with self.assertRaises(ValueError): activation.validate(bad, context)

    def test_fixture_and_missing_independent_import_rejected(self):
        result, context = fixture()
        result['publicTestCouncilFixture'] = True
        with self.assertRaises(ValueError): activation.validate(result, context)
        result['publicTestCouncilFixture'] = False
        result['blocks'][1]['independentImportRootMatches'] = False
        with self.assertRaises(ValueError): activation.validate(result, context)


if __name__ == '__main__':
    unittest.main()
