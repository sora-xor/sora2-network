"""Join council-approved repair bytes to their fresh native staging execution.

This is a bounded raw-code/authorization checker, not a Python signature engine.
Cryptographic approval and storage transitions are checked by the pinned native
replay; these checks join its witnesses to the actual envelopes and block roots.
"""
from pathlib import Path

import artifact_identity_release as identity
import hashi_codec as scale

require = identity.require
CHUNK = 64 * 1024
MAX_CODE = 32 * 1024 * 1024
PHASES = ['Applying', 'Auditing', 'Ready', 'Hold', 'Complete', 'Failed']
FIELDS = {'schema', 'step', 'blockNumber', 'policyCommitment', 'recoveryAnchor',
    'authorizationDomainScale', 'phase', 'cursor', 'operationCounter',
    'stagingNonceBefore', 'stagingNonceAfter', 'storedCodeBlake2', 'executingCodeBlake2',
    'destinationCodeBlake2', 'destinationSpec', 'codeLength', 'chunkHashes', 'signers',
    'quorumVerified', 'productionReady', 'blockHash', 'stateRoot',
    'diskSnapshotManifestSha256', 'independentImportAndRestartVerified',
    'manifestSha256', 'signingMessageSha256', 'envelopeSha256'}


def bounded(path, maximum):
    with Path(path).open('rb') as stream:
        raw = stream.read(maximum + 1)
    require(len(raw) <= maximum, 'repair staging input exceeds bound')
    return raw


def authorization(reader, domain):
    start = reader.offset
    result = {'policyCommitment': '0x' + reader.take(32).hex()}
    phase = reader.take(1)[0]
    require(phase < len(PHASES), 'repair staging phase codec')
    result.update(phase=PHASES[phase], cursor=int.from_bytes(reader.take(4), 'little'),
        operationCounter=int.from_bytes(reader.take(8), 'little'))
    for field in ('storedCodeBlake2', 'executingCodeBlake2', 'destinationCodeBlake2'):
        result[field] = '0x' + reader.take(32).hex()
    result['destinationSpec'] = int.from_bytes(reader.take(4), 'little')
    result['revisedPolicyCommitment'] = '0x' + reader.take(32).hex()
    require(reader.take(len(domain)) == domain, 'repair staging authorization domain differs')
    result['authorizationDomainScale'] = '0x' + domain.hex()
    result['recoveryAnchor'] = '0x' + reader.take(32).hex()
    require(result['recoveryAnchor'] != '0x' + '00' * 32, 'repair staging anchor absent')
    return result, reader.data[start:reader.offset]


def signers(reader, council):
    count = reader.compact()
    require(len(council) // 2 < count <= len(council), 'repair staging strict council majority required')
    actual = []
    for _ in range(count):
        account = '0x' + reader.take(32).hex()
        require(account not in actual and account[2:] in council, 'repair staging signer duplicate or outside original council')
        actual.append(account)
        kind = reader.take(1)[0]
        require(kind in (0, 1, 2), 'repair staging signature codec')
        reader.take(65 if kind == 2 else 64)
    return actual


def decode(raw, domain, council):
    require(identity.action(raw)[0] == 12, 'expected repair staging authorization action')
    reader = scale.Reader(raw[10:])
    auth, auth_raw = authorization(reader, domain)
    nonce = int.from_bytes(reader.take(8), 'little')
    length = int.from_bytes(reader.take(4), 'little')
    count = reader.compact()
    require(nonce < 2**64 - 1 and 0 < length <= MAX_CODE and count == (length + CHUNK - 1) // CHUNK,
        'repair staging length/chunk count/nonce bound')
    chunks = ['0x' + reader.take(32).hex() for _ in range(count)]
    require(all(value != '0x' + '00' * 32 for value in chunks), 'zero repair staging chunk hash')
    manifest = reader.data[:reader.offset]
    actual = signers(reader, council)
    reader.end()
    return {**auth, 'stagingNonceBefore': nonce, 'stagingNonceAfter': nonce + 1,
        'codeLength': length, 'chunkHashes': chunks, 'signers': actual,
        'authorizationScale': auth_raw.hex(), 'manifestScale': manifest.hex(),
        'manifestSha256': identity.sha(manifest), 'envelopeSha256': identity.sha(raw)}


def message(manifest):
    return (scale.vec(b'sora-nexus/recovery-repair-staging/v1')
        + bytes.fromhex(scale.GENESIS_HASH[2:]) + bytes.fromhex(scale.CHECKPOINT_HASH[2:]) + manifest)


def bind(details, wasm, policies, original, council, paths):
    """Require actual action12 and exact ordered repair chunks before each escape.

    Reauthorization may continue an already staged artifact; chunk cursors remain
    keyed by code hash, as in the runtime. Every new approval consumes one nonce.
    """
    import release_package as package
    domain = package.authorization_domain_scale(original['authorizationDomain'])
    approved, active, cursors, nonce, consumed = [], None, {}, 0, set()
    for index, detail in enumerate(details):
        tag, version = detail['action'], detail['executingSpec']
        role = detail['recoveryRole']
        raw = bounded(paths[role], identity.MAX_ENVELOPE) if role else b''
        if tag == 12:
            item = decode(raw, domain, council)
            destination = item['destinationSpec']
            require(index > 0 and version in (135, 136) and destination in (136, 137) and destination > version
                and str(destination) in wasm, 'repair staging version corridor')
            expected = {'policyCommitment': '0x' + original['commitment'],
                'storedCodeBlake2': '0x' + wasm[str(version)]['blake2'],
                'executingCodeBlake2': '0x' + wasm[str(version)]['blake2'],
                'destinationCodeBlake2': '0x' + wasm[str(destination)]['blake2'],
                'revisedPolicyCommitment': '0x' + policies[str(destination)]['commitment'],
                'operationCounter': detail['counterBefore'], 'stagingNonceBefore': nonce,
                'phase': detail['step']['expectedPhase']}
            require(all(item[key] == value for key, value in expected.items()) and item['phase'] != 'Complete'
                and item['cursor'] <= detail['policy']['workChunks'], 'repair staging policy/code/status/nonce differs')
            code = bounded(paths[wasm[str(destination)]['role']], MAX_CODE)
            require(identity.blake(code) == wasm[str(destination)]['blake2']
                and identity.sha(code) == wasm[str(destination)]['sha256']
                and len(code) == item['codeLength']
                and ['0x' + identity.blake(code[offset:offset + CHUNK]) for offset in range(0, len(code), CHUNK)] == item['chunkHashes'],
                'repair staging manifest does not commit exact ordered repair Wasm')
            item.update(step=index, recoveryRole=role)
            approved.append(item); active = item; nonce += 1
        elif tag == 3:
            reader = scale.Reader(raw[10:])
            kind = reader.take(1)[0]
            require(kind in (0, 1, 2), 'repair staging code kind')
            chunk_index = int.from_bytes(reader.take(4), 'little')
            chunk = reader.vector(CHUNK); reader.end()
            if kind != 2:
                continue
            require(active is not None and version in (135, 136), 'repair bytes precede council staging authorization')
            key = active['destinationCodeBlake2']
            cursor = cursors.get(key, 0)
            require(chunk_index == cursor and cursor < len(active['chunkHashes']), 'repair staging cursor/order differs')
            length = CHUNK if cursor + 1 < len(active['chunkHashes']) else active['codeLength'] - cursor * CHUNK
            require(len(chunk) == length and '0x' + identity.blake(chunk) == active['chunkHashes'][cursor], 'repair staged bytes differ from council commitment')
            require(detail['counterBefore'] == active['operationCounter'] and detail['step']['expectedPhase'] == active['phase'], 'repair staging context changed')
            cursors[key] = cursor + 1
        elif tag == 4 and detail['step']['expectedSpec'] in (136, 137):
            require(active is not None and cursors.get(active['destinationCodeBlake2'], 0) == len(active['chunkHashes']),
                'repair escape requires council-authorized complete staged Wasm')
            _, auth_raw = authorization(scale.Reader(raw[10:]), domain)
            require(auth_raw.hex() == active['authorizationScale'], 'repair escape differs from staging authorization')
            consumed.add(active['step']); active = None
    require(approved and consumed, 'repair staging authorization missing')
    # Extra approvals may be replaced before completion, but never after the last
    # repair or for a code hash that is not installed in this release corridor.
    require(active is None, 'unused final repair staging authorization')
    return approved


def validate(witnesses, rows, plan, context, evidence, authorizations):
    expected = plan['repairStaging']
    require(isinstance(witnesses, list) and len(witnesses) == len(expected), 'executed repair staging witness missing or surplus')
    for witness, item in zip(witnesses, expected):
        identity.keys(witness, FIELDS)
        index = item['step']; row = rows[index]
        joined = {key: value for key, value in item.items() if key in FIELDS}
        joined.update(schema='sora-nexus-executed-repair-staging-authorization-v1',
            step=index, blockNumber=scale.CHECKPOINT + index + 1, recoveryAnchor=rows[0]['hash'],
            phase=row['phase'], cursor=row['cursor'], operationCounter=row['operationCounter'],
            blockHash=row['hash'], stateRoot=row['stateRoot'],
            diskSnapshotManifestSha256=row['diskSnapshotManifestSha256'],
            quorumVerified=True, productionReady=False, independentImportAndRestartVerified=True)
        manifest = bytes.fromhex(item['manifestScale'])
        signing_message = message(manifest)
        joined['signingMessageSha256'] = identity.sha(signing_message)
        require(all(witness[key] == value and type(witness[key]) is type(value) for key, value in joined.items()),
            'executed repair staging context/root/quorum differs')
        require(item['phase'] == row['phase'] and item['cursor'] == row['cursor']
            and item['operationCounter'] == row['operationCounter'] and item['recoveryAnchor'] == rows[0]['hash'],
            'repair staging signed status/anchor differs from actual execution')
        prefix = f'{row["number"]}.repair-staging.'
        require(evidence.document(prefix + 'authorization.json') == witness
            and evidence.pinned(evidence.local(prefix + 'manifest.scale'), item['manifestSha256']) == manifest
            and evidence.pinned(evidence.local(prefix + 'signing-message.scale'), witness['signingMessageSha256']) == signing_message,
            'raw repair staging manifest/message differs')
        raw = evidence.pinned(Path(context['inputPaths'][item['recoveryRole']]), item['envelopeSha256'], identity.MAX_ENVELOPE)
        require(evidence.pinned(evidence.local(prefix + 'envelope.scale'), item['envelopeSha256'], identity.MAX_ENVELOPE) == raw,
            'executed repair staging envelope differs')
    # Join every repair installation to the most recent staging admission, after
    # its witness has been tied to independently imported and restored blocks.
    for escape in authorizations:
        if escape['destinationSpec'] == 135:
            continue
        prior = [item for item in expected if item['step'] < escape['step']]
        require(prior, 'repair installation lacks preceding executed staging')
        item = prior[-1]
        require(escape['authorizationSha256'] == identity.sha(bytes.fromhex(item['authorizationScale']))
            and escape['operationCounter'] == item['operationCounter'], 'executed repair installation differs from staged authorization')
