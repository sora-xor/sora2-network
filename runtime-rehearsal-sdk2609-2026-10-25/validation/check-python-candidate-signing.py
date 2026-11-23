"""Offline SORA Nexus candidate signing compatibility; synthetic keys, no submission."""
import argparse
import hashlib
import inspect
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
from unittest.mock import Mock, patch

if not __debug__:
    raise RuntimeError('Signing verification requires assertions; do not use -O/PYTHONOPTIMIZE')

ROOT = Path(__file__).resolve().parents[2]
TOOL = ROOT / 'misc/runtime_upgrade'
sys.path.insert(0, str(TOOL))
import main as upgrade
from scalecodec.base import ScaleBytes
from scalecodec.types import GenericExtrinsicV4


def raw(value):
    return bytes(value.data)


def compact(value):
    if value < 1 << 6:
        return bytes([value << 2])
    if value < 1 << 14:
        return ((value << 2) | 1).to_bytes(2, 'little')
    if value < 1 << 30:
        return ((value << 2) | 2).to_bytes(4, 'little')
    size = (value.bit_length() + 7) // 8
    return bytes([((size - 4) << 2) | 3]) + value.to_bytes(size, 'little')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--metadata', type=Path, required=True)
    parser.add_argument('--runtime-report', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    runtime_report = json.loads(args.runtime_report.read_text())
    assert runtime_report['status'] == 'passed'
    runtime = runtime_report['inputs']['candidate']['info']['core_version']
    assert runtime['specVersion'] == 136
    assert runtime['transactionVersion'] == 132
    genesis = '0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5'
    metadata_bytes = args.metadata.read_bytes()
    assert hashlib.sha256(metadata_bytes).hexdigest() == runtime_report['inputs']['candidate']['metadataScaleSha256']
    report = {
        'script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'candidate_sha256': runtime_report['inputs']['candidate']['sha256'],
        'purpose': 'Offline candidate governance-signing compatibility',
        'metadata_sha256': hashlib.sha256(metadata_bytes).hexdigest(),
        'metadata_bytes': len(metadata_bytes),
        'runtime_report_sha256': hashlib.sha256(args.runtime_report.read_bytes()).hexdigest(),
        'runtime_version': runtime,
        'genesis_hash': genesis,
        'key_policy': 'Synthetic SR25519 key derived from 32 fixed 0x42 bytes; no operator key accessed',
        'network_policy': 'socket creation, RPC and submissions fail closed',
        'source_sha256': {},
        'cases': [],
    }
    for src in [TOOL/'main.py', TOOL/'vendor/substrateinterface/base.py',
                TOOL/'vendor/substrateinterface/keypair.py', Path(inspect.getfile(GenericExtrinsicV4))]:
        report['source_sha256'][str(src)] = hashlib.sha256(src.read_bytes()).hexdigest()
    with patch('socket.socket', side_effect=AssertionError('Network access forbidden')):
        substrate = upgrade.SubstrateInterface(websocket=Mock(), ss58_format=69, auto_discover=False)
        substrate.rpc_request = Mock(side_effect=AssertionError('RPC forbidden'))
        substrate.submit_extrinsic = Mock(side_effect=AssertionError('Submission forbidden'))
        metadata = substrate.runtime_config.create_scale_object('MetadataVersioned', data=ScaleBytes(metadata_bytes))
        metadata.decode()
        substrate.metadata = metadata
        substrate.runtime_config.add_portable_registry(metadata)
        substrate.runtime_version = runtime['specVersion']
        substrate.transaction_version = runtime['transactionVersion']
        substrate.runtime_config.set_active_spec_version_id(runtime['specVersion'])
        substrate.init_runtime = Mock()
        substrate.get_block_hash = Mock(return_value=genesis)
        keypair = upgrade.Keypair.create_from_seed('0x'+'42'*32, ss58_format=69)
        report['synthetic_public_key'] = keypair.public_key.hex()
        assert keypair.crypto_type == 1
        info = metadata[1][1]['extrinsic'].value
        assert info['version'] == 4
        extensions = metadata.get_signed_extensions()
        assert list(extensions) == ['AuthorizeCall', 'CheckSpecVersion', 'CheckTxVersion', 'CheckGenesis',
                                    'CheckMortality', 'CheckNonce', 'CheckWeight', 'ChargeTransactionPayment']
        report['signed_extensions'] = extensions
        registry = {t['id']: t['type'] for t in metadata.portable_registry.value['types']}
        params = {p['name']: p['type'] for p in registry[info['ty']]['params']}
        extra_ty = 'scale_info::' + str(params['Extra'])
        assert len(registry[params['Extra']]['def']['tuple']) == 8

        def encode(ty, value):
            return raw(substrate.runtime_config.create_scale_object(ty, metadata=metadata).encode(value))

        explicit_auth = encode(extensions['AuthorizeCall']['extrinsic'], None)
        implicit_auth = encode(extensions['AuthorizeCall']['additional_signed'], ())
        assert explicit_auth == implicit_auth == b''
        report['authorize_call'] = {'explicit_hex': explicit_auth.hex(), 'implicit_hex': implicit_auth.hex(),
                                    'whole_extra_type': extra_ty}
        proposal_hash = '0x' + '12'*32
        calls = {
            'council': upgrade.compose_council_motion(substrate, proposal_hash, 123, 3),
            'technical_committee': upgrade.compose_fast_track_motion(substrate, proposal_hash, 3, 10, 0),
        }
        for name, data in [('note_preimage', b'\x00asm\x01\x00\x00\x00'),
                           ('note_preimage_hashed_payload', bytes(range(256))*4)]:
            with tempfile.NamedTemporaryFile() as fixture:
                fixture.write(data); fixture.flush()
                calls[name] = upgrade.handle_note_preimage(substrate, SimpleNamespace(call_file_path=fixture.name))
        for name, call in calls.items():
            for nonce, tip in [(0, 0), (7, 19)]:
                explicit_values = {'CheckMortality': '00', 'CheckNonce': nonce, 'ChargeTransactionPayment': tip}
                implicit_values = {'CheckSpecVersion': runtime['specVersion'], 'CheckTxVersion': runtime['transactionVersion'],
                                   'CheckGenesis': genesis, 'CheckMortality': genesis}
                explicit = b''.join(encode(types['extrinsic'], explicit_values.get(identifier))
                                    for identifier, types in extensions.items())
                implicit = b''.join(encode(types['additional_signed'], implicit_values.get(identifier, ()))
                                    for identifier, types in extensions.items())
                whole_extra = encode(extra_ty, [None, None, None, None, '00', nonce, None, tip])
                assert explicit == whole_extra == b'\x00' + compact(nonce) + compact(tip)
                assert implicit == (runtime['specVersion'].to_bytes(4, 'little') +
                                    runtime['transactionVersion'].to_bytes(4, 'little') + bytes.fromhex(genesis[2:])*2)
                unsigned_payload = raw(call.data) + whole_extra + implicit
                expected_payload = hashlib.blake2b(unsigned_payload, digest_size=32).digest() if len(unsigned_payload) > 256 else unsigned_payload
                payload = substrate.generate_signature_payload(call=call, nonce=nonce, tip=tip)
                assert raw(payload) == expected_payload
                signature = keypair.sign(payload)
                assert keypair.verify(payload, signature)
                old_implicit = implicit[:4] + (131).to_bytes(4, 'little') + implicit[8:]
                old_unsigned = raw(call.data) + whole_extra + old_implicit
                old_payload = hashlib.blake2b(old_unsigned, digest_size=32).digest() if len(old_unsigned) > 256 else old_unsigned
                assert old_payload != expected_payload
                assert not keypair.verify(ScaleBytes(old_payload), signature)
                extrinsic = substrate.create_signed_extrinsic(call=call, keypair=keypair, nonce=nonce, tip=tip, signature=signature)
                address = encode('scale_info::'+str(params['Address']), '0x'+keypair.public_key.hex())
                assert address == keypair.public_key
                signature_field = encode('scale_info::'+str(params['Signature']), {'Sr25519':'0x'+signature.hex()})
                assert signature_field == b'\x01'+signature
                body = b'\x84'+address+signature_field+whole_extra+raw(call.data)
                expected_extrinsic = compact(len(body))+body
                assert raw(extrinsic.data) == expected_extrinsic
                decoded = substrate.runtime_config.create_scale_object('Extrinsic', data=ScaleBytes(expected_extrinsic), metadata=metadata)
                decoded.decode()
                assert decoded.data.offset == len(expected_extrinsic)
                assert decoded.value['nonce'] == nonce
                assert decoded.value['tip'] == {'tip': tip}
                report['cases'].append({'call':name, 'nonce':nonce, 'tip':tip, 'payload_is_blake2_256':len(unsigned_payload)>256,
                                        'call_hex':str(call.data), 'payload_hex':str(payload),
                                        'extrinsic_hex':'0x'+expected_extrinsic.hex(), 'extrinsic_bytes':len(expected_extrinsic),
                                        'full_extra_matches_all_metadata_extensions':True,
                                        'signature_verifies':True, 'signature_rejects_transaction_version_131':True,
                                        'extrinsic_roundtrip_consumes_all_bytes':True})
        substrate.rpc_request.assert_not_called()
        substrate.submit_extrinsic.assert_not_called()
        report['rpc_calls'] = substrate.rpc_request.call_count
        report['submission_calls'] = substrate.submit_extrinsic.call_count
        substrate.close()
    report['status'] = 'passed'
    report['production_helper_changed'] = False
    report['limits'] = 'Checks encoding and signatures offline; no governance authorization, chain execution or transaction submission is claimed.'
    args.out.write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps({'status':report['status'], 'cases':len(report['cases']), 'metadata_sha256':report['metadata_sha256'],
                      'specVersion':runtime['specVersion'], 'transactionVersion':runtime['transactionVersion'],
                      'rpc_calls':report['rpc_calls'], 'submission_calls':report['submission_calls']}))

if __name__ == '__main__':
    main()
