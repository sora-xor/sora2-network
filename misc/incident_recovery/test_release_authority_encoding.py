"""Focused authority representation tests; no native runtime/node execution."""
import copy
import unittest
import release_package as release
import production_build
import test_release_package as package_tests
import test_artifact_identity_release as identity_tests
import test_forward_repair_release as repair_tests
import test_node_identity as node_tests


class AuthorityEncodingTests(unittest.TestCase):
    def fixture(self, cls):
        case = cls(unittest.defaultTestLoader.getTestCaseNames(cls)[0])
        case.setUp()
        self.addCleanup(case.doCleanups)
        return case

    def ss58(self, key):
        return node_tests.ss58_fixture(key.removeprefix('0x'))

    def convert(self, config):
        for name in ('retiredGrandpaKeys', 'approvedGrandpaKeys'):
            if name in config:
                config[name] = [self.ss58(key) for key in config[name]]
        for name in ('previousGrandpaAuthorities', 'grandpaAuthorities'):
            if name in config:
                config[name] = [[self.ss58(key), weight] for key, weight in config[name]]

    def test_decoder_accepts_only_canonical_nonzero_keys(self):
        key = '0x' + 'ab' * 32
        self.assertEqual(release.grandpa_key(key), bytes.fromhex(key[2:]))
        self.assertEqual(release.grandpa_key(self.ss58(key)), bytes.fromhex(key[2:]))
        encoded = self.ss58(key)
        for bad in [None, 1, key.upper(), '0x' + 'ab' * 31, '0x' + 'ab' * 33,
                    '0x' + '00' * 32, self.ss58('00' * 32), '1' + encoded,
                    node_tests.ss58_fixture('ab' * 32, prefix=0), encoded[:-1] + ('1' if encoded[-1] != '1' else '2')]:
            with self.subTest(value=bad), self.assertRaises(ValueError):
                release.grandpa_key(bad)

    def test_package_ss58_keeps_owner_weight_order_and_decoded_uniqueness(self):
        case = self.fixture(package_tests.ReleaseTests)
        case.test_active_grandpa_keys_preserve_operator_association_and_voting_weight()
        self.convert(case.spec['recovery'])
        valid = copy.deepcopy(case.spec)
        case.artifact('chainSpec', valid)
        report, _ = release.verify_candidate(case.write_candidate())
        self.assertFalse(report['productionReady'])
        for change in ('swapped', 'duplicate', 'unsorted', 'wrong-owner'):
            bad = copy.deepcopy(valid)
            config = bad['recovery']
            if change == 'swapped':
                config['grandpaAuthorities'].reverse()
            elif change == 'duplicate':
                config['approvedGrandpaKeys'].append(package_tests.hx(9))
            elif change == 'unsorted':
                config['approvedGrandpaKeys'].reverse()
            else:
                config['grandpaAuthorities'][1][0] = self.ss58(package_tests.hx(17))
            case.artifact('chainSpec', bad)
            with self.subTest(change=change), self.assertRaises(ValueError):
                release.verify_candidate(case.write_candidate())

    def test_identity_joins_hex_manifest_to_ss58_chain_and_rejects_public_keys(self):
        case = self.fixture(identity_tests.ArtifactIdentityTests)
        self.convert(case.chain['recovery'])
        case.put('chainSpec', case.chain)
        self.assertFalse(case.bind()['productionReady'])
        for key in production_build.PUBLIC_SIGNING_KEYS:
            bad = copy.deepcopy(case.chain)
            bad['recovery']['approvedGrandpaKeys'] = [self.ss58(key)]
            case.put('chainSpec', bad)
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, 'known public signing'):
                case.bind()
        case.put('chainSpec', case.chain)
        case.activation['freshGrandpa'][0][0] = self.ss58(package_tests.hx(9))
        case.save_manifests()
        with self.assertRaisesRegex(ValueError, 'canonical lowercase hex'):
            case.bind()

    def test_forward_repair_retains_decoded_exact_consensus_join(self):
        case = self.fixture(repair_tests.ForwardRepairTests)
        case.prepare()
        self.convert(case.chain['recovery'])
        case.put('chainSpec', case.chain)
        self.assertFalse(case.bind_repair()['productionReady'])
        case.repair_manifest['freshGrandpa'][0][1] = 2
        case.save_repair()
        with self.assertRaisesRegex(ValueError, 'repair consensus differs'):
            case.bind_repair()

    def test_rows_reject_duplicate_decoded_keys_and_reordered_weights(self):
        key = package_tests.hx(9)
        with self.assertRaisesRegex(ValueError, 'Duplicate decoded'):
            release.grandpa_rows([[key, 1], [self.ss58(key), 2]])
        for weight in (False, 0, -1, 2**64):
            with self.subTest(weight=weight), self.assertRaises(ValueError):
                release.grandpa_rows([[key, weight]])
        first = [[key, 1], [package_tests.hx(10), 3]]
        self.assertNotEqual(release.grandpa_rows(first), release.grandpa_rows(list(reversed(first))))


if __name__ == '__main__':
    unittest.main()
