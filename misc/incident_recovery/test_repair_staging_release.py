"""Pure byte/graph checks only; synthetic signatures are never runtime evidence."""
import copy
from pathlib import Path
import tempfile
import unittest

import artifact_identity_release as identity
import hashi_codec as scale
import release_package as package
import repair_staging_release as staging


class RepairStagingTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.domain = {'chainSpecId': 'sora-nexus-unit-test', 'releaseNonce': '0x' + '08' * 32}
        self.domain_raw = package.authorization_domain_scale(self.domain)
        self.original = {'commitment': '11' * 32, 'authorizationDomain': self.domain}
        self.council = ['01' * 32, '02' * 32, '03' * 32]
        self.paths, self.wasm, self.policies = {}, {}, {}
        for version in (135, 136, 137):
            code = bytes([version]) * (staging.CHUNK + 7 if version == 136 else 11)
            role = str(version); self.put(role, code)
            self.wasm[role] = {'role': role, 'blake2': identity.blake(code), 'sha256': identity.sha(code)}
            self.policies[role] = {'commitment': f'{version:02x}' * 32, 'workChunks': 3}

    def put(self, role, raw):
        path = self.root / role; path.write_bytes(raw); self.paths[role] = path

    def raw(self, version=135, destination=136, phase='Hold', counter=1, nonce=0, signers=None):
        code = self.paths[str(destination)].read_bytes()
        auth = bytes.fromhex(self.original['commitment']) + bytes([staging.PHASES.index(phase)]) + scale.u32(2) + counter.to_bytes(8, 'little')
        auth += bytes.fromhex(self.wasm[str(version)]['blake2']) * 2 + bytes.fromhex(self.wasm[str(destination)]['blake2'])
        auth += scale.u32(destination) + bytes.fromhex(self.policies[str(destination)]['commitment']) + self.domain_raw + b'\x09' * 32
        chunks = [code[i:i + staging.CHUNK] for i in range(0, len(code), staging.CHUNK)]
        manifest = auth + nonce.to_bytes(8, 'little') + scale.u32(len(code)) + scale.compact(len(chunks))
        manifest += b''.join(bytes.fromhex(identity.blake(chunk)) for chunk in chunks)
        accounts = self.council[:2] if signers is None else signers
        signatures = scale.compact(len(accounts)) + b''.join(bytes.fromhex(key) + b'\x00' + b'\x77' * 64 for key in accounts)
        return b'snxrec01\x01\x0c' + manifest + signatures, auth, chunks

    def detail(self, role, tag, version=135, phase='Hold', counter=1, destination=None):
        return {'action': tag, 'executingSpec': version, 'recoveryRole': role, 'counterBefore': counter,
            'policy': self.policies[str(version)], 'step': {'expectedSpec': destination or version, 'expectedPhase': phase}}

    def graph(self):
        raw, auth, chunks = self.raw()
        self.put('tick', b'snxrec01\x01\x00')
        self.put('auth', raw)
        details = [self.detail('tick', 0), self.detail('auth', 12)]
        for index, chunk in enumerate(chunks):
            role = f'chunk{index}'; self.put(role, b'snxrec01\x01\x03\x02' + scale.u32(index) + scale.vec(chunk))
            details.append(self.detail(role, 3))
        self.put('escape', b'snxrec01\x01\x04' + auth)
        details.append(self.detail('escape', 4, destination=136))
        return details

    def bind(self, details):
        return staging.bind(details, self.wasm, self.policies, self.original, self.council, self.paths)

    def test_two_chunk_repair_is_exact_and_bounded(self):
        rows = self.bind(self.graph())
        self.assertEqual(rows[0]['codeLength'], staging.CHUNK + 7)
        self.assertEqual(len(rows[0]['chunkHashes']), 2)
        self.assertEqual(rows[0]['stagingNonceAfter'], 1)

    def test_reauthorization_preserves_artifact_cursor_and_consumes_nonce(self):
        details = self.graph()
        raw, _, _ = self.raw(nonce=1); self.put('again', raw)
        details.insert(3, self.detail('again', 12))
        rows = self.bind(details)
        self.assertEqual([row['stagingNonceBefore'] for row in rows], [0, 1])
        details[3]['recoveryRole'] = 'auth'
        with self.assertRaisesRegex(ValueError, 'nonce'): self.bind(details)

    def test_second_repair_keeps_original_council_domain_and_advances_nonce(self):
        details = self.graph()
        raw, auth, chunks = self.raw(version=136, destination=137, phase='Failed', counter=2, nonce=1)
        self.put('auth137', raw)
        self.put('chunk137', b'snxrec01\x01\x03\x02' + scale.u32(0) + scale.vec(chunks[0]))
        self.put('escape137', b'snxrec01\x01\x04' + auth)
        details += [self.detail('auth137', 12, 136, 'Failed', 2), self.detail('chunk137', 3, 136, 'Failed', 2),
            self.detail('escape137', 4, 136, 'Failed', 2, 137)]
        self.assertEqual([row['destinationSpec'] for row in self.bind(details)], [136, 137])
        self.put('auth137', self.raw(version=136, destination=137, phase='Failed', counter=2, nonce=0)[0])
        with self.assertRaisesRegex(ValueError, 'nonce'): self.bind(details)

    def test_missing_approval_chunk_order_short_chunk_full_code_and_escape_mismatch(self):
        details = self.graph()
        for changed in (details[:1] + details[2:], details[:2] + [details[3], details[2]] + details[4:], details[:3] + details[4:]):
            with self.assertRaises(ValueError): self.bind(changed)
        original = self.paths['chunk0'].read_bytes()
        self.put('chunk0', b'snxrec01\x01\x03\x02' + scale.u32(0) + scale.vec(b'wrong'))
        with self.assertRaises(ValueError): self.bind(details)
        self.put('chunk0', original)
        escape = self.paths['escape'].read_bytes()
        self.put('escape', escape[:-1] + b'\x10')
        with self.assertRaisesRegex(ValueError, 'escape differs'): self.bind(details)
        self.put('escape', escape)
        self.put('136', self.paths['136'].read_bytes() + b'wrong')
        with self.assertRaisesRegex(ValueError, 'exact ordered repair Wasm'): self.bind(details)

    def test_raw_quorum_domain_and_canonical_size_are_enforced(self):
        for accounts in ([], self.council[:1], [self.council[0]] * 2, [self.council[0], 'ff' * 32]):
            with self.subTest(accounts=accounts), self.assertRaises(ValueError):
                staging.decode(self.raw(signers=accounts)[0], self.domain_raw, self.council)
        raw, auth, _ = self.raw()
        for candidate in (raw[:-1], raw + b'\0', raw.replace(self.domain_raw, self.domain_raw[:-1] + b'\xff', 1)):
            with self.assertRaises(ValueError): staging.decode(candidate, self.domain_raw, self.council)
        offset = 10 + len(auth)
        for nonce, length, count in ((2**64 - 1, 1, 1), (0, 0, 0), (0, staging.MAX_CODE + 1, 513), (0, 1, 2)):
            malformed = raw[:offset] + nonce.to_bytes(8, 'little') + scale.u32(length) + scale.compact(count)
            with self.assertRaises(ValueError): staging.decode(malformed, self.domain_raw, self.council)

    def test_unconsumed_authorization_and_stale_context_fail(self):
        details = self.graph()
        with self.assertRaises(ValueError): self.bind(details[:-1])
        for key, value in [('counterBefore', 2), ('executingSpec', 134)]:
            changed = copy.deepcopy(details); changed[1][key] = value
            with self.assertRaises(ValueError): self.bind(changed)


if __name__ == '__main__':
    unittest.main()
