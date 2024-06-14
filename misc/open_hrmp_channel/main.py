from substrateinterface import SubstrateInterface, Keypair
from substrateinterface.exceptions import SubstrateRequestException
from scalecodec.types import GenericCall
import argparse

ss58_format = 69

def parse_args():
    parser = argparse.ArgumentParser(prog='Open HRMP Channel', description='Open HRMP channel between parachains')
    parser.add_argument('--node-url', help='URL of the relaychain node', dest='node_url', default='ws://127.0.0.1:9944', required=False)
    parser.add_argument('--first-para-id', help='First para ID', dest='first_para_id', type=int, required=True)
    parser.add_argument('--second-para-id', help='Second para ID', dest='second_para_id', type=int, required=True)
    parser.add_argument('--capacity', help='Channel capacity (default 4)', dest='capacity', type=int, default=4)
    parser.add_argument('--message-size', help='Channel max message size (default 524287)', dest='message_size', type=int, default=524287)
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
        receipt = substrate_provider.submit_extrinsic(extrinsic, wait_for_finalization=True)
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
        raise Exception('⚠️ Extrinsic Failed: ', receipt.error_message)

def open_hrmp_channel_call(sender, recipient, max_capacity, max_message_size):
    return {
        'call_module': 'Hrmp',
        'call_function': 'force_open_hrmp_channel',
        'call_args': {
            'sender': sender,
            'recipient': recipient,
            'max_capacity': max_capacity,
            'max_message_size': max_message_size
        }
    }
    

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
            call_function='sudo',
            call_params={
                'call': {
                    'call_module': 'Utility',
                    'call_function':'batch_all',
                    'call_args': {
                        'calls': [
                            open_hrmp_channel_call(args.first_para_id, args.second_para_id, args.capacity, args.message_size),
                            open_hrmp_channel_call(args.second_para_id, args.first_para_id, args.capacity, args.message_size),
                        ]
                    }
                },
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
