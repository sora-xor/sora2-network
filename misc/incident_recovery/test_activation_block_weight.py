"""Synthetic SCALE/provenance joins only; never a runtime execution receipt."""
import hashlib
import tempfile
from pathlib import Path
import unittest

import activation_block_weight as weights

METADATA = b'meta\x0eSynthetic metadata retained by a mocked native process'
KEY = '0x26aa394eea5630e07c48ae0c9558cef734abf5cb34d6244378cddbf18e849d96'


def compact(value):
    if value < 64: return bytes([value << 2])
    if value < 16384: return (value << 2 | 1).to_bytes(2, 'little')
    if value < 2**30: return (value << 2 | 2).to_bytes(4, 'little')
    size = (value.bit_length()+7)//8
    return bytes([((size-4) << 2) | 3]) + value.to_bytes(size, 'little')


def pair(values):
    return dict(zip(('refTime', 'proofSize'), map(str, values)))


def limits(code='ab'*32, bound=(100, 100)):
    constant = b'\x00\x00' + b''.join(map(compact, bound)) + b'\x00'*15
    return {'schema': weights.LIMIT_SCHEMA, 'executingCodeBlake2': code,
        'metadataFile': weights.METADATA_FILE, 'metadataSha256': hashlib.sha256(METADATA).hexdigest(),
        'metadataBlake2': '0x'+hashlib.blake2b(METADATA, digest_size=32).hexdigest(),
        'constant': 'System.BlockWeights', 'constantScale': '0x'+constant.hex(),
        'maxBlock': pair(bound), 'storage': 'System.BlockWeight', 'storageKey': KEY,
        'metadataExecutedAsWasm': True, 'metadataNativeExecution': False, 'elapsedPerformanceMeasured': False}


def observed(parts=((10, 1), (20, 2), (70, 3))):
    return {'schema': weights.ROW_SCHEMA,
        'storageScale': '0x'+b''.join(compact(n) for row in parts for n in row).hex(),
        'classes': {key: pair(value) for key, value in zip(weights.CLASSES, parts)},
        'total': pair(tuple(sum(row[i] for row in parts) for i in range(2))),
        'accountingOnly': True, 'elapsedPerformanceMeasured': False}


def witness():
    return {'executingCodeBlake2': 'ab'*32, 'blockWeightLimits': limits(),
        'rows': [{'block': 1, 'blockWeight': observed()}]}


class BlockWeightTests(unittest.TestCase):
    def test_exact_limit_and_both_overweight_dimensions(self):
        good = witness()
        self.assertEqual(weights.validate(good, 'ab'*32)['status'], 'verified')
        for parts in (((10, 1), (20, 2), (71, 3)), ((10, 1), (20, 2), (70, 98))):
            bad = witness(); bad['rows'][0]['blockWeight'] = observed(parts)
            result = weights.validate(bad, 'ab'*32)
            self.assertEqual(result['status'], 'unverified')
            self.assertEqual(len(result['overweightBlocks']), 1)

    def test_sum_is_not_saturating(self):
        bad = witness(); bad['rows'][0]['blockWeight'] = observed(((weights.U64_MAX, weights.U64_MAX),)*3)
        self.assertEqual(weights.validate(bad, 'ab'*32)['status'], 'unverified')
        bad['rows'][0]['blockWeight']['total'] = pair((weights.U64_MAX, weights.U64_MAX))
        with self.assertRaisesRegex(ValueError, 'overflow'): weights.validate(bad, 'ab'*32)

    def test_missing_old_observers_stay_unverified(self):
        for field in ('blockWeightLimits', 'rows'):
            bad = witness(); del bad[field]
            self.assertEqual(weights.validate(bad, 'ab'*32)['status'], 'unverified')
        bad = witness(); del bad['rows'][0]['blockWeight']
        self.assertEqual(weights.validate(bad, 'ab'*32)['status'], 'unverified')

    def test_class_total_code_metadata_and_limit_tampering(self):
        changes = [lambda w: w['blockWeightLimits'].update(executingCodeBlake2='cd'*32),
            lambda w: w['blockWeightLimits']['maxBlock'].update(refTime='101'),
            lambda w: w['blockWeightLimits'].update(metadataNativeExecution=True),
            lambda w: w['blockWeightLimits'].update(metadataFile='../metadata.scale'),
            lambda w: w['blockWeightLimits'].update(storageKey='0x'+'00'*32),
            lambda w: w['rows'][0]['blockWeight']['classes']['mandatory'].update(refTime='69'),
            lambda w: w['rows'][0]['blockWeight']['total'].update(proofSize='0'),
            lambda w: w['rows'][0]['blockWeight'].update(elapsedPerformanceMeasured=True),
            lambda w: w['rows'][0]['blockWeight']['classes']['normal'].update(refTime=10)]
        for change in changes:
            bad = witness(); change(bad)
            with self.assertRaises(ValueError): weights.validate(bad, 'ab'*32)

    def test_noncanonical_truncated_trailing_and_oversized_scale(self):
        for raw in ('0x', '0x0100'+'00'*5, '0x'+'00'*7, '0x'+'ff'*55, '0x03'+'00'*4+'00'*5):
            bad = witness(); bad['rows'][0]['blockWeight']['storageScale'] = raw
            with self.assertRaises(ValueError): weights.validate(bad, 'ab'*32)
        bad = witness(); bad['blockWeightLimits']['constantScale'] += '00'
        with self.assertRaises(ValueError): weights.validate(bad, 'ab'*32)

    def test_all_compact_widths(self):
        for number in (0, 63, 64, 16383, 16384, 2**30-1, 2**30, 2**32, weights.U64_MAX):
            reader = weights.Scale(compact(number)); self.assertEqual(reader.compact(), number); reader.end()

    def test_metadata_is_bound_to_owned_output_bytes(self):
        from artifact_identity_release import OutputEvidence
        from release_package import digest_file
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); path = root / weights.METADATA_FILE
            path.write_bytes(METADATA); pins = {str(path): digest_file(path)}
            evidence = OutputEvidence(root, pins)
            weights.join_metadata(witness(), evidence)
            bad = witness(); bad['blockWeightLimits']['metadataBlake2'] = '0x'+'aa'*32
            with self.assertRaises(ValueError): weights.join_metadata(bad, evidence)
            path.write_bytes(METADATA+b'changed')
            with self.assertRaises(ValueError): weights.join_metadata(witness(), evidence)

    def test_every_row_is_checked(self):
        good = witness(); good['rows'].append({'block': 2, 'blockWeight': observed(((0, 0), (0, 0), (101, 0)))})
        self.assertEqual(weights.validate(good, 'ab'*32)['overweightBlocks'][0]['block'], 2)


if __name__ == '__main__': unittest.main()
