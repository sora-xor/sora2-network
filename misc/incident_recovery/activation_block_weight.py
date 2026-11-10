"""Join fresh native weight accounting to its exact-Wasm metadata and replay.

The owned native process decodes the metadata type registry. This module checks
its retained metadata bytes and independently decodes the bounded FRAME SCALE
accounting values. It never substitutes a caller limit or a performance claim.
"""
import hashlib
import re

LIMIT_SCHEMA = 'sora-nexus-activation-block-weight-limits-v1'
ROW_SCHEMA = 'sora-nexus-recorded-block-weight-v1'
METADATA_FILE = 'activation-metadata.scale'
CLASSES = ('normal', 'operational', 'mandatory')
U64_MAX = 2**64 - 1


def need(ok, message):
    if not ok:
        raise ValueError('activation block weight: ' + message)


def raw_hex(value, maximum):
    need(isinstance(value, str) and re.fullmatch(r'0x(?:[0-9a-f]{2})*', value)
         and len(value) <= 2 + 2*maximum, 'invalid bounded SCALE hex')
    return bytes.fromhex(value[2:])


class Scale:
    def __init__(self, raw):
        self.raw, self.pos = raw, 0

    def take(self, size):
        need(size >= 0 and self.pos + size <= len(self.raw), 'truncated SCALE value')
        value = self.raw[self.pos:self.pos+size]
        self.pos += size
        return value

    def compact(self):
        first = self.take(1)[0]
        mode = first & 3
        if mode == 0:
            return first >> 2
        if mode in (1, 2):
            count = 2 if mode == 1 else 4
            value = int.from_bytes(bytes([first]) + self.take(count-1), 'little') >> 2
            need(value >= (64 if mode == 1 else 16384), 'noncanonical compact value')
        else:
            count = (first >> 2) + 4
            need(count <= 8, 'compact weight exceeds u64')
            raw = self.take(count)
            value = int.from_bytes(raw, 'little')
            need(value >= 2**30 and raw[-1] != 0, 'noncanonical large compact value')
        need(value <= U64_MAX, 'weight exceeds u64')
        return value

    def weight(self):
        return (self.compact(), self.compact())

    def end(self):
        need(self.pos == len(self.raw), 'trailing SCALE bytes')


def pair(value, maximum=U64_MAX):
    need(isinstance(value, dict) and set(value) == {'refTime', 'proofSize'}, 'weight fields differ')
    result = []
    for key in ('refTime', 'proofSize'):
        text = value[key]
        need(isinstance(text, str) and re.fullmatch(r'0|[1-9][0-9]{0,20}', text), 'noncanonical weight integer')
        number = int(text)
        need(number <= maximum, 'weight integer exceeds bound')
        result.append(number)
    return tuple(result)


def maximum(limits, code):
    from artifact_identity_release import digest_hex
    expected = {'schema', 'executingCodeBlake2', 'metadataFile', 'metadataSha256', 'metadataBlake2',
        'constant', 'constantScale', 'maxBlock', 'storage', 'storageKey',
        'metadataExecutedAsWasm', 'metadataNativeExecution', 'elapsedPerformanceMeasured'}
    need(isinstance(limits, dict) and set(limits) == expected
         and limits['schema'] == LIMIT_SCHEMA and limits['executingCodeBlake2'] == code
         and limits['metadataFile'] == METADATA_FILE
         and limits['constant'] == 'System.BlockWeights' and limits['storage'] == 'System.BlockWeight'
         and limits['metadataExecutedAsWasm'] is True and limits['metadataNativeExecution'] is False
         and limits['elapsedPerformanceMeasured'] is False, 'metadata/limit provenance differs')
    digest_hex(limits['metadataSha256']); digest_hex(limits['metadataBlake2'], True)
    # twox128(System) ++ twox128(BlockWeight), fixed FRAME storage namespace.
    need(limits['storageKey'] == '0x26aa394eea5630e07c48ae0c9558cef7' + '34abf5cb34d6244378cddbf18e849d96',
         'storage key differs')
    reader = Scale(raw_hex(limits['constantScale'], 1024))
    reader.weight()  # base_block
    bound = reader.weight()  # max_block
    for _ in CLASSES:
        reader.weight()  # base_extrinsic
        for _ in range(3):  # max_extrinsic, max_total, reserved
            tag = reader.take(1)[0]
            need(tag in (0, 1), 'weight limit option differs')
            if tag == 1:
                reader.weight()
    reader.end()
    need(all(bound) and pair(limits['maxBlock']) == bound, 'maxBlock differs from metadata constant')
    return bound


def join_metadata(witness, evidence):
    """Only called with the owned process's OutputEvidence, never candidate files."""
    if not isinstance(witness, dict) or witness.get('blockWeightLimits') is None:
        return
    limits = witness['blockWeightLimits']
    maximum(limits, witness.get('executingCodeBlake2'))
    raw = evidence.pinned(evidence.local(METADATA_FILE), limits['metadataSha256'], 16*1024**2)
    need(raw[:4] == b'meta' and len(raw) > 5 and raw[4] in (14, 15)
         and hashlib.sha256(raw).hexdigest() == limits['metadataSha256']
         and '0x' + hashlib.blake2b(raw, digest_size=32).hexdigest() == limits['metadataBlake2'],
         'retained exact-Wasm metadata bytes differ')


def validate(witness, code):
    limits = witness.get('blockWeightLimits')
    rows = witness.get('rows')
    if limits is None or not isinstance(rows, list) or not rows:
        return {'status': 'unverified', 'reason': 'Recorded weight and exact-Wasm metadata limits are missing'}
    bound = maximum(limits, code)
    excess = []
    for row in rows:
        observed = row.get('blockWeight')
        if observed is None:
            return {'status': 'unverified', 'reason': 'An ordinary block lacks recorded weight'}
        need(isinstance(observed, dict) and set(observed) == {'schema', 'storageScale', 'classes', 'total',
            'accountingOnly', 'elapsedPerformanceMeasured'} and observed['schema'] == ROW_SCHEMA
            and observed['accountingOnly'] is True and observed['elapsedPerformanceMeasured'] is False,
            'recorded weight schema/scope differs')
        reader = Scale(raw_hex(observed['storageScale'], 54))
        classes = [reader.weight() for _ in CLASSES]; reader.end()
        need(isinstance(observed['classes'], dict) and set(observed['classes']) == set(CLASSES)
             and all(pair(observed['classes'][name]) == actual for name, actual in zip(CLASSES, classes)),
             'recorded class weights differ from persisted SCALE')
        total = tuple(sum(parts[index] for parts in classes) for index in range(2))
        need(pair(observed['total'], 3*U64_MAX) == total, 'recorded total differs or overflow was hidden')
        if any(actual > maximum for actual, maximum in zip(total, bound)):
            excess.append({'block': row['block'], 'total': observed['total']})
    return {'status': 'unverified' if excess else 'verified', 'maxBlock': limits['maxBlock'],
        'ordinaryBlocks': len(rows), 'overweightBlocks': excess, 'elapsedPerformanceMeasured': False,
        'reason': ('Recorded total weight exceeds the exact runtime metadata maxBlock' if excess else
                   'All ordinary blocks fit the exact runtime metadata maxBlock in both weight dimensions')}
