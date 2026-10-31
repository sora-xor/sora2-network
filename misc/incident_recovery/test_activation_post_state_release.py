"""Adversarial activation evidence joins; these tests do not execute Wasm."""
import copy
import unittest
import activation_post_state_release as activation
from test_activation_block_weight import limits


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
    last_epoch = slots[-1] // 600
    gap = max(b-a for a,b in zip(slots, slots[1:]))
    witness['maintenance'] = {'checkpointSlot': 0, 'checkpointEpoch': 0, 'checkpointSession': 100,
        'checkpointActiveEra': 6, 'checkpointCurrentEra': 7, 'firstRecoverySlot': slots[1],
        'firstRecoveryEpoch': slots[1] // 600, 'lastSlot': slots[-1], 'lastEpoch': last_epoch, 'blocks': first,
        'maximumSlotGap': gap, 'coverage': {'minimumSlotGap': 14_400, 'minimumEpochAdvance': 24,
            'observedEpochAdvance': last_epoch, 'complete': gap >= 14_400 and last_epoch >= 24}}
    transition_index = next((index for index in range(len(ordinary))
        if details[first+index]['step']['slot'] // 600 > last_epoch), None)
    observed = []
    for index, block in enumerate(ordinary):
        changed = transition_index is not None and index >= transition_index
        observed.append({'block': block['number'], 'blockHash': block['hash'], 'stateRoot': block['stateRoot'],
            'snapshotManifestSha256': block['diskSnapshotManifestSha256'], 'executingCodeBlake2': code,
            'slot': details[first+index]['step']['slot'], 'epoch': details[first+index]['step']['slot'] // 600,
            'timestamp': details[first+index]['step']['timestamp'],
            'session': 101 if changed else 100, 'currentEra': 7, 'activeEra': 7 if changed else 6,
            'activeEraStart': details[first+transition_index]['step']['timestamp'] if changed else 1,
            'asyncState': {'pendingValidators': 0, 'pendingEra': None, 'pruneUpTo': None,
                'queuedSession': None if changed else 101, 'queuedEra': None if changed else 7,
                'electionDeadline': None, 'forced': False, 'timestampEra': None},
            'lastValidatorEra': {'count': 1, 'sha256': 'ab'*32, 'active': [{'account': '0x'+'ee'*32, 'era': 7}],
                'queued': [] if changed else [{'account': '0x'+'ee'*32, 'era': 7}], 'pending': []},
            'readOnlyStateRootMatches': True})
    witness['rows'] = observed
    witness['blockWeightLimits'] = limits(code)
    from test_activation_block_weight import observed as weight_observation
    for row in witness['rows']:
        row['blockWeight'] = weight_observation()
    witness['transitions'] = {'firstSession': {'block': ordinary[transition_index]['number'], 'before': 100, 'after': 101} if transition_index is not None else None,
        'firstActiveEra': {'block': ordinary[transition_index]['number'], 'before': 6, 'after': 7} if transition_index is not None else None,
        'postFirstSessionBlocks': len(ordinary)-transition_index-1 if transition_index is not None else 0,
        'postFirstActiveEraBlocks': len(ordinary)-transition_index-1 if transition_index is not None else 0}
    return witness


def fixture(count=3, maintenance_gap=14_400):
    code = 'ab'*32
    rows = [{'number': 100 + index, 'hash': '0x' + f'{index+1:02x}' * 32,
             'stateRoot': '0x' + f'{index+11:02x}' * 32,
             'diskSnapshotManifestSha256': f'{index+21:02x}' * 32,
             'phase': 'Complete', 'resultingSpec': 138, 'executingCodeBlake2': code,
             'independentImportRootMatches': True, 'diskRestartRootMatches': True}
            for index in range(count+1)]
    first_ordinary = maintenance_gap + 1
    second_ordinary = max(first_ordinary + 1, (maintenance_gap // 600 + 1) * 600)
    slots = [maintenance_gap, first_ordinary] + [second_ordinary+i for i in range(max(count-1, 0))]
    steps = [{'executingSpec': 134 if i == 0 else 138,
              'step': {'slot': slot, 'timestamp': slot*6000}} for i, slot in enumerate(slots)]
    result = {'publicTestCouncilFixture': False, 'publicTestHashiFixture': False,
              'forwardRepairObserved': False, 'ordinaryRuntimeBlocks': count, 'blocks': rows,
              'activationValidation': activation_witness(rows, steps, 'cd'*32, code)}
    context = {'policyCommitment': 'cd'*32, 'wasm': {'138': {'blake2': code}},
               'scenarios': {'activation': {'steps': steps}}}
    return result, context


class ActivationTests(unittest.TestCase):
    def test_missing_or_overweight_accounting_cannot_pass_activation(self):
        from test_activation_block_weight import observed
        for change in (lambda w: w.pop('blockWeightLimits'),
                       lambda w: w['rows'][1].update(blockWeight=observed(((0, 0), (0, 0), (101, 1))))):
            result, context = fixture(); change(result['activationValidation'])
            self.assertEqual(activation.validate(result, context)['status'], 'unverified')

    def test_exact_24_hour_gap_passes_and_short_diagnostics_remain_unverified(self):
        for gap, status in ((14_399, 'unverified'), (14_400, 'verified'), (14_401, 'verified')):
            result, context = fixture(maintenance_gap=gap)
            with self.subTest(gap=gap):
                self.assertEqual(activation.validate(result, context)['status'], status)

    def test_old_observer_without_maintenance_coverage_cannot_pass(self):
        result, context = fixture()
        del result['activationValidation']['maintenance']['coverage']
        self.assertEqual(activation.validate(result, context)['status'], 'unverified')

    def test_actual_epoch_advance_is_required_independently_of_slot_gap(self):
        result, context = fixture()
        witness = result['activationValidation']
        witness['maintenance'].update(checkpointEpoch=1)
        witness['maintenance']['coverage'].update(observedEpochAdvance=23, complete=False)
        self.assertEqual(activation.validate(result, context)['status'], 'unverified')

    def test_maintenance_coverage_cannot_be_asserted_or_weakened(self):
        result, context = fixture(maintenance_gap=14_399)
        changes = [lambda w: w['maintenance']['coverage'].update(complete=True),
            lambda w: w['maintenance']['coverage'].update(minimumSlotGap=1),
            lambda w: w['maintenance']['coverage'].update(minimumEpochAdvance=1),
            lambda w: w['maintenance']['coverage'].update(observedEpochAdvance=25),
            lambda w: w['maintenance']['coverage'].update(complete=0),
            lambda w: w['maintenance'].update(lastEpoch=25),
            lambda w: w['maintenance'].update(maximumSlotGap=14_400)]
        for change in changes:
            bad = copy.deepcopy(result); change(bad['activationValidation'])
            with self.assertRaises(ValueError): activation.validate(bad, context)

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
