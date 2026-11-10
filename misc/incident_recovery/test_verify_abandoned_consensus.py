import copy
import unittest

import verify_abandoned_consensus as verifier


# Public checkpoint header, not a generated rehearsal result.
CHECKPOINT_HEADER = {
    'parentHash': '0x60b63e40ce4de9b9c1408bf7455a6f5ced796825f2437a0ee8496476d80c56d6',
    'number': '0x1ae2c49', 'stateRoot': verifier.CHECKPOINT_ROOT,
    'extrinsicsRoot': '0x4a24fdf92274ca78ef10c57956d642b664287780b241a06fe45b68d2d11faa7d',
    'digest': {'logs': [
        '0x06424142453402020000005caecf1100000000',
        '0x0542414245010180149e1b922dc6fd0377e1e29b8f8277f0247ac10de60ab8339758df73fd141fa4b96671583c11a1355f072ef79e783eb2adc2242a9799eb80ee24d257defa8f',
    ]},
}


def digest(kind, engine, payload):
    return '0x' + (bytes([kind]) + engine + verifier.compact(len(payload)) + payload).hex()


def fixture(change_kind=1, finalized_change=False, head_slot=verifier.MIN_OBSERVED_SLOT, delay=0):
    rows, parent = [], verifier.CHECKPOINT_HASH
    for offset in (1, 2):
        header = copy.deepcopy(CHECKPOINT_HEADER)
        header['parentHash'] = parent
        header['number'] = hex(verifier.CHECKPOINT + offset)
        slot = head_slot - 2 + offset
        header['digest']['logs'] = [digest(6, b'BABE', b'\x02' + bytes(4) + slot.to_bytes(8, 'little'))]
        if offset == (1 if finalized_change else 2):
            payload = bytes([change_kind]) + verifier.compact(1) + b'\x11'*32 + (1).to_bytes(8, 'little') + delay.to_bytes(4, 'little')
            header['digest']['logs'].append(digest(4, b'FRNK', payload))
        parent = verifier.header_hash(header)
        rows.append({'number': verifier.CHECKPOINT+offset, 'hash': parent, 'header': header})
    start = {'endpoint': verifier.ENDPOINT, 'checkpointHash': verifier.CHECKPOINT_HASH,
             'checkpointHeader': CHECKPOINT_HEADER, 'headHash': parent, 'head': rows[-1]['header'],
             'finalized': rows[0]['hash'], 'setIdCalls': {
                 name: '0x' + value.to_bytes(8, 'little').hex()
                 for name, value in [('checkpoint', 7956), ('finalized', 7956), ('best', 7957)]}}
    completion = {'schema': 'sora-nexus-public-abandoned-consensus-capture-v1',
                  'endpoint': verifier.ENDPOINT, 'headsUnchanged': True, 'headers': 2,
                  'headHash': parent, 'finalized': rows[0]['hash'], 'productionReady': False}
    return start, rows, completion


class PublicAbandonedConsensusTests(unittest.TestCase):
    def test_derives_slot_without_claiming_finality_or_signing_stop(self):
        result = verifier.verify(*fixture())
        self.assertEqual(result['firstRecoverySlot'], verifier.MIN_OBSERVED_SLOT + 1)
        self.assertEqual(result['headersHashedAndLinked'], 3)
        self.assertFalse(result['finalityProofVerified'])
        self.assertFalse(result['clientVotingHistoryProven'])
        self.assertFalse(result['operatorSigningStopVerified'])
        self.assertFalse(result['productionReady'])

    def test_forced_or_finalized_change_cannot_authorize_same_key_restart(self):
        for data in (fixture(change_kind=2), fixture(finalized_change=True), fixture(delay=200000)):
            with self.assertRaises(ValueError):
                verifier.verify(*data)

    def test_hash_ancestry_state_set_and_slot_mismatches_fail(self):
        for mutation in ('hash', 'parent', 'state', 'set', 'slot', 'finality', 'head'):
            start, rows, completion = copy.deepcopy(fixture())
            if mutation == 'hash': rows[0]['hash'] = '0x' + '11'*32
            elif mutation == 'parent': rows[1]['header']['parentHash'] = '0x' + '11'*32
            elif mutation == 'state': start['checkpointHeader']['stateRoot'] = '0x' + '11'*32
            elif mutation == 'set': start['setIdCalls']['finalized'] = '0x151f000000000000'
            elif mutation == 'slot': start, rows, completion = fixture(head_slot=verifier.MIN_OBSERVED_SLOT-1)
            elif mutation == 'finality': completion['finalized'] = '0x' + '11'*32
            else: completion['headsUnchanged'] = False
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                verifier.verify(start, rows, completion)

    def test_noncanonical_or_truncated_digest_encoding_is_rejected(self):
        for data in (b'\x05\x00', b'\x01', b'\x03'):
            with self.assertRaises(ValueError): verifier.read_compact(data, 0)
        header = copy.deepcopy(CHECKPOINT_HEADER)
        header['digest']['logs'][0] = '0x06424142453802020000005caecf1100000000'
        with self.assertRaises(ValueError): verifier.inspect(header)

    def test_the_minimum_is_derived_from_a_newer_observed_head(self):
        result = verifier.verify(*fixture(head_slot=verifier.MIN_OBSERVED_SLOT+1000))
        self.assertEqual(result['firstRecoverySlot'], verifier.MIN_OBSERVED_SLOT+1001)

    def test_unknown_or_trailing_digest_items_are_not_valid_headers(self):
        for item in ('0xff', '0x0800', '0x000800', '0x0446524e4b0800'):
            header = copy.deepcopy(CHECKPOINT_HEADER)
            header['digest']['logs'].append(item)
            with self.subTest(item=item), self.assertRaises(ValueError):
                verifier.header_hash(header)
        self.assertEqual(verifier.digest_item(b'\x08'), (8, None, b''))
        self.assertEqual(verifier.digest_item(b'\x00\x04x'), (0, None, b'x'))


if __name__ == '__main__':
    unittest.main()
