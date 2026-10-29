"""Adversarial activation evidence joins; these tests do not execute Wasm."""
import copy
import unittest
import activation_post_state_release as activation


def fixture(count=2):
    code = 'ab' * 32
    rows = [{'number': 100 + index, 'hash': '0x' + f'{index+1:02x}' * 32,
             'stateRoot': '0x' + f'{index+11:02x}' * 32,
             'diskSnapshotManifestSha256': f'{index+21:02x}' * 32,
             'phase': 'Complete', 'resultingSpec': 138, 'executingCodeBlake2': code,
             'independentImportRootMatches': True, 'diskRestartRootMatches': True}
            for index in range(count+1)]
    witness = {'schema': activation.SCHEMA, 'policyCommitment': 'cd'*32,
        'executingCodeBlake2': code, 'firstBlock': 101,
        'maintenanceParentHash': rows[0]['hash'], 'maintenanceStateRoot': rows[0]['stateRoot'],
        'maintenanceSnapshotSha256': rows[0]['diskSnapshotManifestSha256'],
        'initializedStateRoot': '0x' + 'ff'*32, 'previousStakingVersion': 16,
        'activatedStakingVersion': 18, 'ordinaryBlocks': count,
        'lastBlockHash': rows[-1]['hash'], 'lastStateRoot': rows[-1]['stateRoot'],
        'cutoffEra': 7, 'firstExecutionEra': 6, 'weightedPointsStartEra': 7,
        'runtimeUpgradeExecutedAsWasm': True, 'independentImportAndRestartVerified': True,
        'tryRuntimeFeatureChecksExecuted': False, 'fullCustomStakingVerified': False, 'productionReady': False}
    result = {'publicTestCouncilFixture': False, 'publicTestHashiFixture': False,
              'forwardRepairObserved': False, 'ordinaryRuntimeBlocks': count,
              'blocks': rows, 'activationValidation': witness}
    context = {'policyCommitment': 'cd'*32, 'wasm': {'138': {'blake2': code}},
               'scenarios': {'activation': {'steps': [{'executingSpec': 134}]
                                             + [{'executingSpec': 138}]*count}}}
    return result, context


class ActivationTests(unittest.TestCase):
    def test_two_executed_blocks_and_migration_are_required(self):
        result, context = fixture()
        self.assertEqual(activation.validate(result, context)['status'], 'verified')
        result, context = fixture(1)
        self.assertEqual(activation.validate(result, context)['status'], 'unverified')

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
