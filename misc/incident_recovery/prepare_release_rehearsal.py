#!/usr/bin/env python3
"""Prepare an offline SORA Nexus replay from a hash-pinned completed production build.

Every committed work/audit/code chunk is retained. Ordinary blocks cover the
first active-era transition and its following block, with all retained authors
represented in intervening sessions. This prepares inputs; only the native
exact-artifact replay and activation gate establish execution success.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
from pathlib import Path

import hashi_codec as scale
import production_build as build_tools
import release_package as release

SLOT_MS = 6000
EPOCH_SLOTS = 600
SESSIONS_PER_ERA = 6
MAINTENANCE_GAP_SLOTS = 14_400
# These constants were decoded from this exact checkpoint metadata. A different
# runtime/metadata needs an explicit reviewed scheduling adapter, not guesses.
CHECKPOINT_METADATA_SHA256 = '70fb15b7f598a58fc71a174af66df68b9136a2e87ec51dcd56aa279165cd6ba4'
KEYS = {
    'timestamp': 'f0c365c3cf59d671eb72da0e7a4113c49f1f0515f462cdcf84e0f1d6045dfcbb',
    'genesis': '1cb6f36e027abb2091cfb5110ab5087f678711d15ebbceba5cd0cea158e6675a',
    'slot': '1cb6f36e027abb2091cfb5110ab5087f06155b3cd9a8c9e5e9a23fd5dc13a5ed',
    'epoch': '1cb6f36e027abb2091cfb5110ab5087f38316cbf8fa0da822a20ac1c55bf1be3',
    'session': 'cec5070d609dd3497f72bde07fc96ba072763800a36a99fdfc7c10f6415f6ee6',
    'validators': 'cec5070d609dd3497f72bde07fc96ba088dcde934c658227ee1dfafcd6e16903',
    'active': '5f3e4907f716ac89b6347d15ececedca487df464e44a534ba6b0cbb32407b587',
    'bonded': '5f3e4907f716ac89b6347d15ececedcaea07de2b8f010516dca3f7ef52f7ac5a',
    'current': '5f3e4907f716ac89b6347d15ececedca0b6a45321efae92aea15e0740ec7afe7',
    'forced': '5f3e4907f716ac89b6347d15ececedcaf7dad0317324aecae8744b87fc95f2f3',
}
need = scale.require


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


class BoundBuild:
    """Read only files bound by the supplied outer build-manifest digest."""
    def __init__(self, root, expected):
        self.root = root.resolve(strict=True)
        self.manifest_raw = build_tools.read(self.root / 'BUILD-MANIFEST.json', expected, 32 * 1024**2)
        self.manifest = build_tools.parse(self.manifest_raw)
        need(self.manifest.get('schema') == 'sora-nexus-production-recovery-build-v1'
             and self.manifest.get('status') == 'passed' and self.manifest.get('fixtureOnly') is False
             and self.manifest.get('productionReady') is False, 'completed production build required')
        self.files = self.manifest['files']
        need(isinstance(self.files, dict) and self.files, 'build file inventory missing')
        self.used = {}

    def read(self, name, maximum=512 * 1024**2):
        name = build_tools.relative(name)
        need(name in self.files, 'file absent from completed build: ' + name)
        raw = build_tools.read(self.root / name, self.files[name], maximum)
        self.used[name] = self.files[name]
        return raw

    def document(self, name):
        return build_tools.parse(self.read(name))

    def guard(self):
        need(build_tools.read(self.root / 'BUILD-MANIFEST.json') == self.manifest_raw, 'build manifest changed')
        for name, pin in self.used.items():
            need(build_tools.digest(self.root / name) == pin, 'build input changed: ' + name)


def checkpoint_facts(bound, capture_name):
    prefix = 'inputs/' + build_tools.relative(capture_name) + '/'
    capture = bound.document(prefix + 'capture-complete.json')
    need(capture['checkpoint']['hash'] == scale.CHECKPOINT_HASH
         and capture['checkpoint']['stateRoot'] == release.CHECKPOINT_ROOT, 'checkpoint identity differs')
    metadata = bound.read(prefix + 'runtime-metadata.scale')
    need(sha(metadata) == CHECKPOINT_METADATA_SHA256
         == capture['artifacts']['runtime-metadata.scale']['sha256'], 'unreviewed checkpoint scheduling metadata')
    wanted, found = {'0x' + key: name for name, key in KEYS.items()}, {}
    for page in capture['top']['pages']:
        # Range hints select a candidate page, never authenticate a value. The
        # exact row must occur in the pinned page; native replay rechecks the
        # complete checkpoint trie, including these scheduling inputs.
        keys = {key for key in wanted if page['keyCount'] and page['firstKey'] <= key <= page['lastKey']}
        if not keys:
            continue
        raw = bound.read(prefix + build_tools.relative(page['file']))
        need(sha(raw) == page['sha256'], 'checkpoint page hash differs')
        with gzip.GzipFile(fileobj=io.BytesIO(raw)) as stream:
            data = stream.read(8 * 1024**2 + 1)
        need(len(data) <= 8 * 1024**2, 'checkpoint page decompression bound')
        lines = data.splitlines()
        header = build_tools.parse(lines[0])
        need(header['checkpointHash'] == scale.CHECKPOINT_HASH and header['trie'] == 'top', 'checkpoint page header differs')
        rows = [build_tools.parse(line) for line in lines[1:]]
        need(len(rows) == page['keyCount'] and all(isinstance(row, list) and len(row) == 2 for row in rows), 'checkpoint page rows differ')
        order = [row[0] for row in rows]
        need(order == sorted(set(order)) and order[0] == page['firstKey'] and order[-1] == page['lastKey'], 'checkpoint page ordering/range differs')
        for key, value in rows:
            if key in keys:
                need(key not in found, 'duplicate checkpoint scheduling row')
                found[key] = scale.raw(value)
    need(set(found) == set(wanted), 'checkpoint scheduling rows missing')
    raw = {name: found[key] for key, name in wanted.items()}

    def integer(name, length):
        need(len(raw[name]) == length, 'checkpoint scalar width differs: ' + name)
        return int.from_bytes(raw[name], 'little')

    facts = {name: integer(name, 8 if name in ('timestamp', 'genesis', 'slot', 'epoch') else 4)
             for name in ('timestamp', 'genesis', 'slot', 'epoch', 'session', 'current')}
    need(len(raw['active']) == 13 and raw['active'][4] == 1 and raw['forced'] == b'\0', 'checkpoint active/forced era differs')
    facts['active'] = int.from_bytes(raw['active'][:4], 'little')
    reader = scale.Reader(raw['bonded']); count = reader.compact()
    need(0 < count <= 100, 'checkpoint bonded-era count')
    bonded = [(int.from_bytes(reader.take(4), 'little'), int.from_bytes(reader.take(4), 'little')) for _ in range(count)]
    reader.end()
    need(bonded == sorted(set(bonded)) and bonded[-1][0] == facts['active'], 'checkpoint bonded-era ordering differs')
    facts['eraStartSession'] = bonded[-1][1]
    reader = scale.Reader(raw['validators']); count = reader.compact()
    need(0 < count <= 1200 and count < EPOCH_SLOTS, 'checkpoint author count')
    authors = [reader.take(32) for _ in range(count)]; reader.end()
    need(len(set(authors)) == count, 'duplicate checkpoint author')
    facts['authors'] = count
    facts['rotations'] = facts['eraStartSession'] + SESSIONS_PER_ERA - facts['session']
    need(facts['current'] == facts['active'] and 1 < facts['rotations'] <= SESSIONS_PER_ERA
         and facts['slot'] >= facts['genesis'] and (facts['slot'] - facts['genesis']) // EPOCH_SLOTS == facts['epoch'],
         'unsupported checkpoint session/era schedule')
    return facts


def normal_slots(last, facts):
    """All authors per completed session; first new era plus one follow-up."""
    first = facts['genesis'] + ((last - facts['genesis']) // EPOCH_SLOTS + 1) * EPOCH_SLOTS
    result = [(first + epoch * EPOCH_SLOTS + author, author)
              for epoch in range(facts['rotations'] - 1) for author in range(facts['authors'])]
    result += [(first + (facts['rotations'] - 1) * EPOCH_SLOTS + offset, offset)
               for offset in range(2)]
    need(result[-1][0] * SLOT_MS + facts['timestamp'] % SLOT_MS < 2**64, 'schedule timestamp overflow')
    return result


def work(bound, inputs, policy):
    name = 'mixed/stdout.log' if inputs['mixedLayout'] is not None else 'compose/stdout.log'
    report = bound.document(name)
    need(release.bounded_hex(report['policyScale'], release.MAX_POLICY) == policy, 'final composed work policy differs')
    if inputs['mixedLayout'] is None:
        need(report.get('storageEngineRehearsed') is True and report.get('productionReady') is False, 'composed rehearsal scope differs')
        rows = report['applyEnvelopes']
    else:
        need(report.get('structuralValidationOnly') is True and report.get('productionReady') is False, 'mixed compiler scope differs')
        rows = report['workEnvelopes']
        mixed_input = bound.document('mixed-input.json')
        need(rows == mixed_input['workEnvelopes'] and report['terminalEnvelope'] == mixed_input['terminalEnvelope']
             and report['terminalPreludeEnvelopes'] == mixed_input['terminalPreludeEnvelopes'], 'mixed work order differs')
        build_tools.validate_native_balance_derivation(mixed_input, report)
    identity = release.policy_identity(policy)
    need(len(rows) == identity['workChunks'] and len(report['auditEnvelopes']) == identity['auditChunks'], 'committed work count differs')
    return {**report, 'applyEnvelopes': rows}, name


def prepare(build, destination, first_slot=None, *, build_manifest_sha256):
    bound = BoundBuild(build, build_manifest_sha256)
    inputs_raw = bound.read('input-manifest.json')
    need(build_tools.pin_bytes(inputs_raw) == bound.manifest['inputManifest'], 'input manifest differs')
    inputs = build_tools.parse(inputs_raw)
    need(inputs['schema'] == build_tools.SCHEMA, 'production input schema differs')
    selected, observed = build_tools.public_consensus(inputs, bound.root / 'inputs')
    need(bound.document('public-consensus.json') == observed, 'retained public slot selection differs')
    for name in ('started.json', 'headers.json', 'completion.json'):
        bound.read('inputs/' + inputs['consensus']['publicCapture'] + '/' + name)
    slot0 = selected['firstRecoverySlot']
    need(first_slot is None or (type(first_slot) is int and first_slot == slot0), 'first slot differs from compiled selection')
    policy = bound.read('policy.scale')
    need(build_tools.pin_bytes(policy) == bound.manifest['policy'], 'built policy hash differs')
    identity = release.policy_identity(policy)
    composition, work_name = work(bound, inputs, policy)
    consensus = bound.document('checkpoint-consensus/stdout.log')
    capture_name = inputs['compose']['capture']
    capture_raw = bound.read('inputs/' + capture_name + '/capture-complete.json')
    operators = build_tools.checkpoint_consensus_witness(consensus, selected, sha(capture_raw))
    need(bound.read('checkpoint-operators.scale') == operators, 'retained operator bytes differ')
    facts = checkpoint_facts(bound, capture_name)
    need(consensus['checkpointSlot'] == facts['slot'] and slot0 > facts['slot'], 'checkpoint/first slot differs')
    wasms, receipts = {}, {}
    for role, key, directory, version in (('bootstrap', '134', 'build-134', (134, 131)),
                                         ('hold', '135', 'build-135', (135, 132)),
                                         ('finalRuntime', 'final138', 'build-final138', (138, 134))):
        ref = bound.manifest['artifacts'][key]
        raw = bound.read(ref['path'], release.MAX_WASM)
        pin = build_tools.pin_bytes(raw)
        need(pin == {k: ref[k] for k in pin}, 'build artifact pin differs')
        receipt_raw = bound.read(directory + '/build.json')
        need(build_tools.pin_bytes(receipt_raw) == bound.manifest['buildReceipts'][directory], 'build receipt pin differs')
        receipt = build_tools.parse(receipt_raw)
        need(receipt.get('status') == 'passed' and receipt.get('exitCode') == 0 and receipt.get('fixtureOnly') is False
             and (receipt['specVersion'], receipt['transactionVersion']) == version
             and receipt['artifacts'].get(Path(ref['path']).name) == pin, 'passed artifact build receipt differs')
        actual = release.wasm_identity(raw)
        need((actual['specVersion'], actual['transactionVersion']) == version, 'wrong build Wasm identity')
        wasms[role], receipts[version[0]] = raw, receipt
    need(bound.read('bindings/final138.wasm') == wasms['finalRuntime']
         and bound.read('bindings/hold135.wasm') == wasms['hold']
         and bound.read('bindings/policy.scale') == policy
         and bound.read('bindings/operators.scale') == operators, 'bootstrap artifact bindings differ')
    for version in (134, 135):
        binding = bound.document(f'bindings/binding-{version}.json')
        expected = {'policyScale': '0x' + policy.hex(), 'policyCommitment': '0x' + identity['commitment'],
                    'operatorsScale': '0x' + operators.hex(), 'previousGrandpaSetId': selected['previousGrandpaSetId'],
                    'firstRecoverySlot': slot0, 'finalWasm': 'final138.wasm'}
        if version == 134:
            expected['holdWasm'] = 'hold135.wasm'
        need(binding == expected, 'compiled first-slot/policy binding differs')
        generated, evidence, _ = build_tools.bootstrap.bind_inputs(bound.root / f'bindings/binding-{version}.json', version)
        source = f'source-{version}'
        provenance_raw = bound.read(source + '/RECOVERY-PROVENANCE.json')
        provenance = build_tools.parse(provenance_raw)
        need(provenance.get('bound') is True and provenance.get('fixtureOnly') is False
             and provenance['bindingEvidence'] == evidence
             and build_tools.pin_bytes(provenance_raw) == receipts[version]['sourceProvenance'],
             'compiled bootstrap provenance differs')
        actual_generated = bound.read(source + '/runtime/src/incident_recovery/generated.rs')
        need(actual_generated == generated.encode(), 'compiled bootstrap constants differ')
        source_files_raw = bound.read(f'build-{version}/source-files.json')
        need(build_tools.pin_bytes(source_files_raw) == receipts[version]['sourceFingerprint']
             and build_tools.parse(source_files_raw)['runtime/src/incident_recovery/generated.rs'] == build_tools.pin_bytes(actual_generated),
             'compiled bootstrap source fingerprint differs')
    final_provenance_raw = bound.read('source-final138/RECOVERY-PROVENANCE.json')
    final_provenance = build_tools.parse(final_provenance_raw)
    need(build_tools.pin_bytes(final_provenance_raw) == receipts[138]['sourceProvenance']
         and final_provenance.get('fixtureOnly') is False and final_provenance.get('bound') is True
         and final_provenance['policy'] == build_tools.pin_bytes(policy), 'compiled final policy provenance differs')
    final_generated = bound.read('source-final138/runtime/src/incident_recovery/generated.rs')
    build_tools.validate_emitted(final_generated, policy)
    final_source_files = bound.read('build-final138/source-files.json')
    need(build_tools.pin_bytes(final_source_files) == receipts[138]['sourceFingerprint']
         and build_tools.parse(final_source_files)['runtime/src/incident_recovery/generated.rs'] == build_tools.pin_bytes(final_generated),
         'compiled final policy fingerprint differs')

    destination.mkdir(parents=False)
    def artifact(name, data):
        path = destination / name
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open('xb') as stream:
            stream.write(data)
        return {'file': name, 'sha256': sha(data)}

    capture_prefix = 'inputs/' + capture_name + '/'
    capture_manifest = build_tools.parse(capture_raw)
    closure = {name: pin['sha256'] for name, pin in capture_manifest['artifacts'].items()}
    for trie in [capture_manifest['top'], *capture_manifest['children']]:
        for page in trie['pages']:
            name = build_tools.relative(page['file'])
            need(name not in closure, 'duplicate checkpoint capture dependency')
            closure[name] = page['sha256']
    need(all(capture_prefix + name in bound.files
             and bound.files[capture_prefix + name]['sha256'] == digest for name, digest in closure.items()),
         'checkpoint dependency missing or differs from build inventory')
    for name in sorted(bound.files):
        if name.startswith(capture_prefix):
            artifact('checkpoint/' + name[len(capture_prefix):], bound.read(name))
    wasm_refs = {role: artifact(f'runtime-{version}.wasm', wasms[role])
                 for role, version in (('bootstrap', 134), ('hold', 135), ('finalRuntime', 138))}
    steps, slot = [], slot0 - 1
    malformed = artifact('work/malformed.scale', b'snxrec01\x01\xfe')

    def step(name, data, phase, expected_spec=134, rejected=(), gap=1, author=0):
        nonlocal slot
        slot += gap
        need(type(slot) is int and 0 < slot * SLOT_MS + facts['timestamp'] % SLOT_MS < 2**64, 'step timestamp overflow')
        payload = artifact(f'work/{name}.scale', data) if data is not None else None
        steps.append({'slot': slot, 'timestamp': slot * SLOT_MS + facts['timestamp'] % SLOT_MS, 'authorityIndex': author,
                      'recovery': payload, 'rejectedRecovery': list(rejected), 'expectedSpec': expected_spec, 'expectedPhase': phase})
        return payload

    step('tick', b'snxrec01\x01\x00', 'Applying')
    previous = None
    envelopes = composition['applyEnvelopes']
    for index, payload in enumerate(envelopes):
        previous = step(f'apply-{index:05}', release.bounded_hex(payload, 8 * 1024**2),
                        'Auditing' if index + 1 == len(envelopes) else 'Applying',
                        rejected=[malformed] + ([previous] if previous else []),
                        gap=MAINTENANCE_GAP_SLOTS if index == 0 else 1)
    for index, payload in enumerate(composition['auditEnvelopes']):
        step(f'audit-{index:05}', release.bounded_hex(payload, 8 * 1024**2), 'Auditing')
    final_bytes = wasms['finalRuntime']
    previous = None
    for index, offset in enumerate(range(0, len(final_bytes), 65536)):
        payload = b'snxrec01\x01\x03\x00' + scale.u32(index) + scale.vec(final_bytes[offset:offset + 65536])
        previous = step(f'final-code-{index:05}', payload, 'Auditing', rejected=[previous] if previous else [])
    for index, payload in enumerate(composition['terminalPreludeEnvelopes']):
        step(f'terminal-page-{index:05}', release.bounded_hex(payload, 8 * 1024**2), 'Auditing')
    step('terminal', release.bounded_hex(composition['terminalEnvelope'], 8 * 1024**2), 'Complete', expected_spec=138)
    ordinary = normal_slots(slot, facts)
    for index, (target, author) in enumerate(ordinary):
        step(f'ordinary-{index:03}', None, 'Complete', expected_spec=138, gap=target - slot, author=author)
    manifest = {'schema': 'sora-nexus-maintenance-replay-v1', 'checkpointHash': scale.CHECKPOINT_HASH,
                'checkpointCapture': 'checkpoint', 'policy': artifact('policy.scale', policy), **wasm_refs,
                'repairs': [], 'previousGrandpaSet': selected['previousGrandpaSetId'],
                'freshGrandpa': consensus['grandpaAuthorities'], 'steps': steps}
    bound.guard()
    manifest_ref = artifact('manifest.json', build_tools.serialized(manifest))
    evidence = {'schema': 'sora-nexus-production-replay-preparation-v2',
                'buildManifestSha256': sha(bound.manifest_raw), 'boundInputs': bound.used,
                'workReport': work_name, 'checkpointSchedule': facts, 'ordinaryBlocks': len(ordinary),
                'expectedFirstActiveEra': facts['active'] + 1, 'expectedSessionRotations': facts['rotations'],
                'maintenanceGapSlots': MAINTENANCE_GAP_SLOTS, 'nativeExecutionVerified': False,
                'activationGatePassed': False, 'productionReady': False}
    artifact('PREPARATION.json', build_tools.serialized(evidence))
    return {'manifest': str(destination / 'manifest.json'), 'manifestSha256': manifest_ref['sha256'],
            'steps': len(steps), 'applyEnvelopes': len(envelopes), 'auditEnvelopes': len(composition['auditEnvelopes']),
            'terminalPreludeEnvelopes': len(composition['terminalPreludeEnvelopes']),
            'finalCodeChunks': (len(final_bytes) + 65535) // 65536, 'ordinaryBlocks': len(ordinary),
            'firstSlot': slot0, 'lastSlot': slot, 'policyCommitment': '0x' + identity['commitment'], 'productionReady': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--build', type=Path, required=True)
    parser.add_argument('--build-manifest-sha256', required=True)
    parser.add_argument('--destination', type=Path, required=True)
    parser.add_argument('--first-slot', type=int, help='Optional assertion; must equal the compiled, public-capture-derived slot')
    args = parser.parse_args()
    print(json.dumps(prepare(args.build, args.destination, args.first_slot,
                             build_manifest_sha256=args.build_manifest_sha256), indent=2))
