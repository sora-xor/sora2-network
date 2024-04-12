from substrateinterface import SubstrateInterface, Keypair
from substrateinterface.exceptions import SubstrateRequestException
from scalecodec.types import GenericCall
import argparse

ss58_format = 69

def parse_args():
    parser = argparse.ArgumentParser(prog='Runtime Upgrade', description='Upgrade Runtime of a Substrate node')
    parser.add_argument('--node-url', help='URL of the node to connect to', dest='node_url', default='ws://127.0.0.1:9944', required=False)
    parser.add_argument('--wasm-file-path', help='Path to Compressed Wasm File', dest='wasm_file_path', required=True)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--uri', help='URI of the keypair to use', dest='uri_keypair', type=str)
    group.add_argument('--seed', help='Seed of the keypair to use', dest='seed', type=str)
    group.add_argument('--mnemonic', help='Seed phrase of the keypair to use', dest='mnemonic', type=str)
    args = parser.parse_args()
    return args

def get_keypair_using_args(args):
    if args.uri_keypair:
        return Keypair.create_from_uri(args.uri_keypair, ss58_format=ss58_format)
    elif args.seed:
        return Keypair.create_from_seed(args.seed, ss58_format=ss58_format)
    elif args.mnemonic:
        return Keypair.create_from_mnemonic(args.mnemonic, ss58_format=ss58_format)
    else:
        raise Exception("No keypair provided")
    

def send_extrinsic(substrate_provider: SubstrateInterface, keypair: Keypair, call: GenericCall):
    extrinsic = substrate_provider.create_signed_extrinsic(call=call, keypair=keypair)
    try:
        receipt = substrate_provider.submit_extrinsic(extrinsic, wait_for_inclusion=True)
        log_receipt(receipt)
    except SubstrateRequestException as e:
        print(f'Error in send_extrinsic: {e}')
        raise e

def log_receipt(receipt):
    print('Extrinsic "{}" included in block "{}"'.format(
        receipt.extrinsic_hash, receipt.block_hash
    ))

    if receipt.is_success:
        print('✅ Success, triggered events:')
        for event in receipt.triggered_events:
            print(f'* {event.value}')
    else:
        print('⚠️ Extrinsic Failed: ', receipt.error_message)
    

def get_new_code_from_wasm_file(wasm_file_path):
    with open(wasm_file_path, 'rb') as f:
        code = '0x' + f.read().hex()
    return code

def main():
    substrate = None
    try:
        args = parse_args()
        keypair = get_keypair_using_args(args)
        substrate = SubstrateInterface(
            url=args.node_url,
            ss58_format=ss58_format,
        )
          
        call = substrate.compose_call(
            call_module='Sudo',
            call_function='sudo_unchecked_weight',
            call_params={
                'call': {
                    'call_module': 'System',
                    'call_function':'set_code',
                    'call_args': {
                        'code': get_new_code_from_wasm_file(args.wasm_file_path)
                    }
                },
                'weight': {'ref_time': 0, 'proof_size': 0}
            }
        )
        
        send_extrinsic(substrate, keypair, call)
    except Exception as e:
        print(f'Error: {e}')
        raise e
    finally:
        if substrate:
            substrate.close()
        
        

if __name__ == '__main__':
    main()