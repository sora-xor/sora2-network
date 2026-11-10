#!/usr/bin/env python3
"""Append an offline HASHI signature-witness bundle without changing its release.

This checks byte/context consistency, not signature validity, state proofs or
finality. It never signs, submits, executes native code or changes package files.
Only the original policy/bootstrap134 corridor is supported.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re

import hashi_codec as codec
import release_package as package

SCHEMA = 'sora-nexus-hashi-execution-witness-input-v1'
OUTPUT = 'sora-nexus-hashi-execution-witness-v1'
MAX_FILE = 64 * 1024**2
MAX_JSON = 16 * 1024**2
ROLES = {'policy': 'policyScale', 'wasm': 'bootstrapWasm'}
FILES = {'release', 'policy', 'wasm', 'metadata', 'template', 'plan', 'baseline',
         'context', 'signatures', 'stateEvidence', 'finalityEvidence'}
require = codec.require


def sha(data):
    return hashlib.sha256(data).hexdigest()


def blake(data):
    return '0x' + hashlib.blake2b(data, digest_size=32).hexdigest()


def fields(value, expected, label):
    require(isinstance(value, dict) and set(value) == set(expected), label + ' fields differ')


def parse(data):
    require(len(data) <= MAX_JSON, 'JSON exceeds bound')
    def unique(rows):
        result = {}
        for key, value in rows:
            require(key not in result, 'duplicate JSON field')
            result[key] = value
        return result
    return json.loads(data, object_pairs_hook=unique)


def read(path):
    require(not path.is_symlink() and path.is_file(), 'regular nonsymlink input required')
    with path.open('rb') as stream:
        data = stream.read(MAX_FILE + 1)
    require(len(data) <= MAX_FILE, 'input file exceeds bound')
    return data


def reference(root, row):
    fields(row, {'file', 'bytes', 'sha256'}, 'artifact reference')
    require(isinstance(row['file'], str) and '\\' not in row['file'], 'relative artifact path required')
    relative = PurePosixPath(row['file'])
    require(not relative.is_absolute() and relative.as_posix() == row['file']
            and relative.parts and all(p not in ('', '.', '..') for p in relative.parts),
            'artifact path escapes input directory')
    path = root
    for part in relative.parts:
        path /= part
        require(not path.is_symlink(), 'artifact path contains symlink')
    require(type(row['bytes']) is int and 0 < row['bytes'] <= MAX_FILE
            and isinstance(row['sha256'], str) and re.fullmatch(r'[0-9a-f]{64}', row['sha256']),
            'artifact size/hash differs')
    data = read(path)
    require(len(data) == row['bytes'] and sha(data) == row['sha256'], 'artifact bytes/hash differ')
    return path, data


def envelope(template, signatures):
    return b'snxrec01\x01\x06' + codec.vec(codec.encode_operation(template, signatures))


def work_commitment(policy, index):
    # Full canonical admission stays in the existing package policy decoder.
    package.policy_identity(policy)
    reader = codec.Reader(policy)
    reader.take(68)
    reader.take(reader.compact() * 64)
    reader.take(reader.compact() * 32)
    count = reader.compact()
    require(type(index) is int and 0 <= index < count, 'template work index outside policy')
    reader.take(index * 32)
    return '0x' + reader.take(32).hex()


def verify(data, roles):
    release, template, plan, context, witnesses = (parse(data[k]) for k in
                                                ('release', 'template', 'plan', 'context', 'signatures'))
    require(release.get('format') == package.MANIFEST
            and release.get('checkpoint') == {'number': codec.CHECKPOINT, 'hash': codec.CHECKPOINT_HASH},
            'release checkpoint/format differs')
    fields(roles, {'template', 'plan', 'baseline', 'metadata'}, 'release roles')
    require(all(isinstance(v, str) and v for v in roles.values())
            and len(set([*ROLES.values(), *roles.values()])) == 6, 'distinct release roles required')
    records = release.get('artifacts')
    require(isinstance(records, list) and len(records) <= 16384, 'release artifact inventory bound')
    by_role = {}
    for row in records:
        require(isinstance(row, dict) and isinstance(row.get('role'), str)
                and row['role'] not in by_role, 'duplicate or malformed release role')
        by_role[row['role']] = row
    for name, role in {**ROLES, **roles}.items():
        row = by_role.get(role)
        require(row and row.get('sha256') == sha(data[name]) and row.get('bytes') == len(data[name])
                and row.get('blake2_256') == blake(data[name])[2:], 'artifact differs from immutable release: ' + name)
    policy = package.policy_identity(data['policy'])
    require(release.get('policy') == json.loads(json.dumps(policy)) and 'incomingRecoveryReplay' not in release,
            'original release policy required; repaired policy corridor unsupported')
    commitment = codec.commitment(template)
    require(work_commitment(data['policy'], template['index']) == commitment, 'template is not committed policy work')
    old_accounts = {'0x' + old for old, _ in policy['mapping']}
    require(all(a['signer'] not in old_accounts for a in template['approvals']), 'retired approval signer')
    source = next((r for r in template['registries'] if r['network'] == {'evmLegacy': template['network']}), None)
    require(source is not None, 'HASHI network registry missing')
    require({'evm': codec.ATTACKER_EVM} in source['accounts'], 'published EVM attacker required on every HASHI network')
    fields(plan, {'schema', 'templateCommitment', 'network', 'firstExecutionBlock', 'lastExecutionBlock',
                  'specVersion', 'transactionVersion', 'executionCodeHash', 'metadataHash'}, 'execution plan')
    require(plan['schema'] == 'sora-nexus-hashi-execution-plan-v1' and plan['templateCommitment'] == commitment
            and codec.integer(plan['network']) == template['network'], 'planned template/network differs')
    first, last = (codec.integer(plan[k]) for k in ('firstExecutionBlock', 'lastExecutionBlock'))
    require(codec.CHECKPOINT + 1 < first <= last, 'invalid planned execution window')
    version = package.wasm_identity(data['wasm'])
    require((version['specVersion'], version['transactionVersion']) == (134, 131)
            and (plan['specVersion'], plan['transactionVersion']) == (134, 131)
            and plan['executionCodeHash'] == blake(data['wasm'])
            and plan['metadataHash'] == blake(data['metadata']), 'planned runtime/metadata identity differs')
    fields(context, {'schema', 'releaseManifestSha256', 'templateCommitment', 'network', 'parentHash',
                     'parentNumber', 'signingContext', 'stateObservation'}, 'execution context')
    require(context['schema'] == 'sora-nexus-hashi-execution-context-v1'
            and context['releaseManifestSha256'] == sha(data['release'])
            and context['templateCommitment'] == commitment and codec.integer(context['network']) == template['network'],
            'context release/template/network differs')
    signing = context['signingContext']
    payloads = codec.signing_payloads(template, signing)
    current = signing['executionBlock']
    require(first <= current <= last, 'execution outside immutable planned window or expired')
    for key in ('specVersion', 'transactionVersion', 'executionCodeHash', 'metadataHash'):
        require(signing[key] == plan[key], 'runtime context drift: ' + key)
    require(signing['lineage']['policyHash'] == '0x' + policy['commitment'], 'signing lineage differs from release')
    number, _, parent = codec.decode_header(codec.raw(signing['lineage']['headers'][-1]), '0x' + policy['commitment'])
    require(number == current - 1 == codec.integer(context['parentNumber'])
            and context['parentHash'] == '0x' + parent.hex(), 'exact execution parent required')
    require(signing['lineage']['finalityEvidenceSha256'] == '0x' + sha(data['finalityEvidence']),
            'finality evidence bytes differ; hashing does not verify them')
    state = context['stateObservation']
    fields(state, {'at', 'policyCommitment', 'phase', 'cursor', 'operationCounter', 'accountNonces', 'evidenceSha256'},
           'state observation')
    require(state['at'] == context['parentHash'] and state['policyCommitment'] == '0x' + policy['commitment']
            and state['phase'] == 'Applying' and type(state['cursor']) is int and state['cursor'] == template['index']
            and type(state['operationCounter']) is int and state['operationCounter'] == 0
            and state['evidenceSha256'] == sha(data['stateEvidence']), 'state/cursor/evidence context drift')
    nonces = state['accountNonces']
    require(isinstance(nonces, list) and len(nonces) == len(template['approvals']), 'nonce observation coverage differs')
    deaths = []
    for approval, observed, payload in zip(template['approvals'], nonces, payloads['payloads']):
        fields(observed, {'signer', 'nonce'}, 'nonce observation')
        period, phase, nonce = codec.decode_extra(codec.raw(approval['extra']))
        birth = lambda height: (max(height, phase) - phase) // period * period + phase
        require(birth(first) == birth(last) == payload['birthBlock'] and last < payload['birthBlock'] + period,
                'planned window crosses mortal era or expiry')
        require(observed['signer'] == approval['signer'] and type(observed['nonce']) is int and observed['nonce'] == nonce,
                'signer/nonce observation differs from committed template')
        deaths.append(payload['birthBlock'] + period)
    fields(witnesses, {'schema', 'contextSha256', 'templateCommitment', 'baselineSignatures', 'signatures'}, 'signature witness')
    require(witnesses['schema'] == 'sora-nexus-hashi-signature-witnesses-v1'
            and witnesses['contextSha256'] == sha(data['context']) and witnesses['templateCommitment'] == commitment,
            'signature witness signing context drift')
    require(envelope(template, witnesses['baselineSignatures']) == data['baseline'], 'baseline envelope changes business fields')
    assembled = envelope(template, witnesses['signatures'])
    require(all(any(codec.raw(s)[1:]) for s in witnesses['signatures']), 'placeholder signature is not an execution witness')
    identity = sha(b'sora-nexus/hashi-execution-witness/v1\0' + bytes.fromhex(sha(data['release'])) + codec.u32(template['index']))
    return assembled, {'schema': OUTPUT, 'bundleId': identity, 'releaseManifestSha256': sha(data['release']),
        'policyCommitment': '0x' + policy['commitment'], 'templateCommitment': commitment,
        'operationIndex': template['index'], 'network': template['network'], 'parentHash': context['parentHash'],
        'parentNumber': number, 'executionBlock': current, 'runtime': version,
        'executionCodeHash': plan['executionCodeHash'], 'metadataHash': plan['metadataHash'],
        'unverifiedSigningPayloads': payloads['payloads'], 'unverifiedEraDeaths': deaths,
        'envelopeSha256': sha(assembled), 'envelopeBlake2': blake(assembled), 'envelopeBytes': len(assembled),
        'onlySignatureWitnessesMayDiffer': True, 'contextConsistencyVerified': True,
        'originalPolicyOnly': True, 'signatureVerification': False, 'nativeAdmissionVerified': False,
        'stateProofVerified': False, 'finalityVerified': False, 'runtimeExecuted': False,
        'chainReplayProtectionVerified': False, 'productionReady': False,
        'remaining': 'Existing peers must sign; fixed native admission must verify signatures, membership, state, nonce and execution. Local append uniqueness is not chain replay protection.'}


def bundle(manifest_path, manifest_sha256, journal):
    manifest_path, journal = Path(manifest_path).absolute(), Path(journal).absolute()
    require(manifest_path == manifest_path.resolve() and journal == journal.resolve(), 'canonical input/journal paths required')
    original = read(manifest_path)
    require(sha(original) == manifest_sha256, 'input manifest pin differs')
    manifest = parse(original)
    fields(manifest, {'schema', 'files', 'releaseRoles'}, 'input manifest')
    require(manifest['schema'] == SCHEMA, 'unknown execution witness schema')
    fields(manifest['files'], FILES, 'input artifacts')
    loaded = {name: reference(manifest_path.parent, row) for name, row in manifest['files'].items()}
    require(len({path for path, _ in loaded.values()}) == len(FILES), 'artifact path alias')
    require(sum(len(data) for _, data in loaded.values()) <= 128 * 1024**2, 'aggregate input bound')
    data = {name: content for name, (_, content) in loaded.items()}
    assembled, report = verify(data, manifest['releaseRoles'])
    require(read(manifest_path) == original and all(read(path) == content for path, content in loaded.values()), 'input changed during verification')
    journal.mkdir(parents=False, exist_ok=True)
    require(not journal.is_symlink() and journal.is_dir(), 'canonical journal directory required')
    destination = journal / report['bundleId']
    # Atomic create arbitrates concurrent appenders. On partial-write failure this
    # directory stays reserved; neither this tool nor a retry deletes evidence.
    destination.mkdir()
    (destination / 'inputs').mkdir()
    for name, content in data.items():
        with (destination / 'inputs' / name).open('xb') as output:
            output.write(content)
    with (destination / 'input-manifest.json').open('xb') as output:
        output.write(original)
    with (destination / 'envelope.scale').open('xb') as output:
        output.write(assembled)
    report.update(inputManifestSha256=sha(original), inputs={k: {'sha256': sha(v), 'bytes': len(v)} for k, v in data.items()},
                  helperSha256=sha(read(Path(__file__).resolve())), codecSha256=sha(read(Path(codec.__file__).resolve())),
                  packageDecoderSha256=sha(read(Path(package.__file__).resolve())))
    encoded = (json.dumps(report, sort_keys=True, indent=2) + '\n').encode()
    with (destination / 'bundle.json').open('xb') as output:
        output.write(encoded)
    with (destination / 'BUNDLE-COMPLETE').open('xb') as output:
        output.write((sha(encoded) + '\n').encode())
    return destination, report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', required=True)
    parser.add_argument('--manifest-sha256', required=True)
    parser.add_argument('--journal', required=True)
    args = parser.parse_args()
    destination, report = bundle(args.manifest, args.manifest_sha256, args.journal)
    print(json.dumps({'bundle': str(destination), 'envelopeSha256': report['envelopeSha256'], 'nativeAdmissionVerified': False, 'productionReady': False}))


if __name__ == '__main__':
    main()
