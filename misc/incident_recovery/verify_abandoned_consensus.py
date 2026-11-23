#!/usr/bin/env python3
"""Derive the restart slot floor from hash-linked public abandoned-chain headers.

This verifies header encoding/ancestry and scheduled-change placement. It does
not certify BABE signatures, GRANDPA finality or unseen private signing history;
the retained client/finality rehearsal and stopped-old-node instruction remain.
"""
import argparse
import hashlib
import json
from pathlib import Path

CHECKPOINT = 28_191_817
CHECKPOINT_HASH = '0x7f7c253fa72f32e7b03f32bdc9ae456fd7174fb131b689be6d876006124129f0'
CHECKPOINT_ROOT = '0x79bda41a1db53470463653f358890f2d1fca8ad622c33a1d7095294b0b140452'
LAST_FINALIZED_SET = 7956
MIN_OBSERVED_SLOT = 298_871_044
ENDPOINT = 'wss://mof2.sora.org'


def need(value, message):
    if not value:
        raise ValueError(message)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result, 'duplicate JSON field')
        result[key] = value
    return result


def raw(value, size=None):
    need(isinstance(value, str) and value.startswith('0x') and len(value) % 2 == 0,
         'invalid hex encoding')
    result = bytes.fromhex(value[2:])
    need(size is None or len(result) == size, 'invalid encoded length')
    return result


def compact(value):
    need(type(value) is int and 0 <= value < 2**64, 'invalid compact integer')
    if value < 64:
        return bytes([value << 2])
    if value < 16384:
        return ((value << 2) | 1).to_bytes(2, 'little')
    if value < 2**30:
        return ((value << 2) | 2).to_bytes(4, 'little')
    data = value.to_bytes((value.bit_length() + 7) // 8, 'little')
    return bytes([((len(data) - 4) << 2) | 3]) + data


def read_compact(data, offset):
    need(offset < len(data), 'truncated compact integer')
    mode = data[offset] & 3
    size = (1 << mode) if mode < 3 else (data[offset] >> 2) + 5
    need(offset + size <= len(data), 'truncated compact integer')
    if mode < 3:
        value = int.from_bytes(data[offset:offset+size], 'little') >> 2
    else:
        value = int.from_bytes(data[offset+1:offset+size], 'little')
    need(data[offset:offset+size] == compact(value), 'noncanonical compact integer')
    return value, offset + size


def header_hash(header):
    number = int(header['number'], 16)
    need(0 <= number < 2**32, 'header number exceeds u32')
    logs = header['digest']['logs']
    need(isinstance(logs, list) and len(logs) <= 64, 'header digest bounds')
    encoded = [raw(value) for value in logs]
    need(all(0 < len(value) <= 65536 for value in encoded), 'digest item bounds')
    for value in encoded:
        digest_item(value)
    data = (raw(header['parentHash'], 32) + compact(number)
            + raw(header['stateRoot'], 32) + raw(header['extrinsicsRoot'], 32)
            + compact(len(encoded)) + b''.join(encoded))
    return '0x' + hashlib.blake2b(data, digest_size=32).hexdigest()


def digest_item(data):
    need(bool(data), 'empty digest')
    kind = data[0]
    if kind == 8:
        need(len(data) == 1, 'runtime-environment digest has trailing bytes')
        return kind, None, b''
    need(kind in (0, 4, 5, 6), 'unsupported digest item tag')
    start = 1 if kind == 0 else 5
    need(len(data) > start, 'truncated digest item')
    length, offset = read_compact(data, start)
    payload = data[offset:]
    need(length == len(payload), 'consensus digest length differs')
    return kind, None if kind == 0 else data[1:5], payload


def inspect(header):
    slots, changes = [], []
    for item in header['digest']['logs']:
        data = raw(item)
        kind, engine, payload = digest_item(data)
        if kind not in (4, 5, 6):
            continue
        if kind == 6 and engine == b'BABE':
            need(len(payload) >= 13 and payload[0] in (1, 2, 3), 'invalid BABE pre-digest')
            need(len(payload) == (13 if payload[0] == 2 else 109), 'BABE pre-digest length differs')
            slots.append(int.from_bytes(payload[5:13], 'little'))
        if kind == 4 and engine == b'FRNK':
            need(payload and payload[0] == 1, 'forced or unsupported GRANDPA change needs review')
            count, offset = read_compact(payload, 1)
            need(0 < count <= 1000 and len(payload) == offset + count * 40 + 4,
                 'GRANDPA authority encoding differs')
            authorities = [(payload[offset+i*40:offset+i*40+32].hex(),
                            int.from_bytes(payload[offset+i*40+32:offset+(i+1)*40], 'little'))
                           for i in range(count)]
            need(len({key for key, _ in authorities}) == count
                 and all(weight > 0 for _, weight in authorities), 'invalid GRANDPA authorities')
            need(int.from_bytes(payload[-4:], 'little') == 0,
                 'delayed GRANDPA change needs explicit enactment analysis')
            changes.append({'number': int(header['number'], 16), 'kind': 'ScheduledChange',
                            'delay': int.from_bytes(payload[-4:], 'little'),
                            'authorities': [['0x' + key, weight] for key, weight in authorities]})
    need(len(slots) == 1, 'exactly one BABE pre-digest required')
    return slots[0], changes


def verify(start, rows, completed):
    need(start['endpoint'] == completed['endpoint'] == ENDPOINT, 'unexpected public endpoint')
    need(start['checkpointHash'] == CHECKPOINT_HASH
         and header_hash(start['checkpointHeader']) == CHECKPOINT_HASH
         and start['checkpointHeader']['stateRoot'] == CHECKPOINT_ROOT
         and int(start['checkpointHeader']['number'], 16) == CHECKPOINT, 'checkpoint header differs')
    need(completed['schema'] == 'sora-nexus-public-abandoned-consensus-capture-v1'
         and completed['headsUnchanged'] is True and completed['productionReady'] is False,
         'capture incomplete or heads changed')
    need(isinstance(rows, list) and 0 < len(rows) <= 100000
         and type(completed['headers']) is int and len(rows) == completed['headers'], 'header count differs')
    need(start['headHash'] == completed['headHash'] and start['finalized'] == completed['finalized'],
         'capture heads detached')
    previous = CHECKPOINT_HASH
    prior_slot, _ = inspect(start['checkpointHeader'])
    changes, finalized = [], None
    for number, row in enumerate(rows, CHECKPOINT + 1):
        header = row['header']
        need(type(row['number']) is int and row['number'] == number
             and int(header['number'], 16) == number and header['parentHash'] == previous
             and header_hash(header) == row['hash'], 'header hash or ancestry differs')
        previous = row['hash']
        if previous == completed['finalized']:
            finalized = number
        slot, logs = inspect(header)
        need(slot > prior_slot, 'BABE slot does not advance')
        prior_slot = slot
        changes.extend(logs)
    need(previous == completed['headHash'] and rows[-1]['header'] == start['head']
         and finalized is not None, 'best/finalized head missing from verified ancestry')
    ids = {name: int.from_bytes(raw(start['setIdCalls'][name], 8), 'little')
           for name in ('checkpoint', 'finalized', 'best')}
    need(ids['checkpoint'] == ids['finalized'] == LAST_FINALIZED_SET
         and ids['best'] == LAST_FINALIZED_SET + len(changes), 'reported authority-set state differs')
    need(all(change['number'] > finalized for change in changes),
         'scheduled change is at or before reported finality')
    need(MIN_OBSERVED_SLOT <= prior_slot < 2**64 - 1, 'stale or exhausted slot bound')
    return {'schema': 'sora-nexus-public-abandoned-consensus-v2',
            'checkpointHash': CHECKPOINT_HASH, 'checkpointNumber': CHECKPOINT,
            'finalizedNumber': finalized, 'finalizedHash': completed['finalized'],
            'bestNumber': rows[-1]['number'], 'bestHash': previous, 'setIds': ids,
            'scheduledChanges': changes, 'lastObservedAbandonedSlot': prior_slot,
            'firstRecoverySlot': prior_slot + 1, 'headersHashedAndLinked': len(rows) + 1,
            'allChangesScheduledBeyondReportedFinality': True,
            'finalityProofVerified': False, 'clientVotingHistoryProven': False,
            'operatorSigningStopVerified': False, 'productionReady': False}


def verify_directory(directory):
    values, pins = {}, {}
    for name in ('started.json', 'headers.json', 'completion.json'):
        path = directory / name
        need(path.is_file() and not path.is_symlink() and path.stat().st_size <= 128 * 1024**2,
             'capture input is absent, aliased or oversized')
        data = path.read_bytes()
        pins[name] = hashlib.sha256(data).hexdigest()
        values[name] = json.loads(data, object_pairs_hook=unique_object,
                                 parse_constant=lambda _: need(False, 'non-finite JSON value'))
    result = verify(values['started.json'], values['headers.json'], values['completion.json'])
    result['captureSha256'] = pins
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--capture', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = verify_directory(args.capture)
    with args.output.open('x') as output:
        json.dump(result, output, indent=2, sort_keys=True)
        output.write('\n')
    print(json.dumps({'output': str(args.output), 'firstRecoverySlot': result['firstRecoverySlot'],
                      'finalityProofVerified': False, 'productionReady': False}))


if __name__ == '__main__':
    main()
