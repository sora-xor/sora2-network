from substrateinterface import SubstrateInterface, Keypair
from substrateinterface.exceptions import SubstrateRequestException
from scalecodec.types import GenericCall
import argparse
from time import sleep

ss58_format = 69

def parse_args():
    parser = argparse.ArgumentParser(prog='On-demand handler', description='SORA Parachain bridge on-demand handler to produce parachain blocks')
    parser.add_argument('--sora-node-url', help='URL of the node to connect to', dest='sora_node_url', required=True)
    parser.add_argument('--parachain-node-url', help='URL of the node to connect to', dest='parachain_node_url', required=True)
    parser.add_argument('--kusama-node-url', help='URL of the node to connect to', dest='kusama_node_url', required=True)
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
        print('✅ Success')
    else:
        raise Exception('⚠️ Extrinsic Failed: ', receipt.error_message)
    
def main():
    sora = None
    parachain = None
    kusama = None
    try:
        args = parse_args()
        keypair = get_keypair_using_args(args)
        print("Connecting SORA...")
        sora = SubstrateInterface(
            url=args.sora_node_url,
            ss58_format=ss58_format,
        )

        print("Connecting SORA Parachain...")
        parachain = SubstrateInterface(
            url=args.parachain_node_url,
            ss58_format=ss58_format,
        )

        print("Connecting Kusama...")
        kusama = SubstrateInterface(
            url=args.kusama_node_url,
            ss58_format=ss58_format,
        )
        
        balance = kusama.query('System', 'Account', [keypair.ss58_address]).value['data']['free'] / 1e12
        print(f'Current balance: {balance}')
        while True:
            new_balance = kusama.query('System', 'Account', [keypair.ss58_address]).value['data']['free'] / 1e12
            if new_balance > balance:
                print(f'Remaining balance: {new_balance}, deposited: {new_balance - balance}')
            elif new_balance < balance:
                print(f'Remaining balance: {new_balance}, spent: {balance - new_balance}')
            balance = new_balance

            if balance < 0.1:
                print('Not enough balance')
                break

            should_produce_block = False
            dmp_messages = kusama.query('Dmp', 'DownwardMessageQueues', [2011]).value
            if len(dmp_messages) != 0:
                should_produce_block = True
                print(f"Have {len(dmp_messages)} pending XCM messages")

            pending = parachain.rpc_request('author_pendingExtrinsics', [])['result']
            if len(pending) != 0:
                should_produce_block = True
                print(f'Have {len(pending)} pending extrinsics')
                
            parachain_nonce = parachain.query('SubstrateBridgeInboundChannel', 'ChannelNonces', ['Mainnet']).value
                
            latest_commitment = sora.query('SubstrateBridgeOutboundChannel', 'LatestCommitment', ['Kusama'], block_hash=sora.get_chain_finalised_head()).value
            sora_block = sora.get_block_number(sora.get_chain_finalised_head())
            commitment_block = latest_commitment['block_number']
            commitment_nonce = latest_commitment['commitment']['Sub']['nonce']
            if parachain_nonce < commitment_nonce:
                print('Have unsent commitment')
                if commitment_block + 4 > sora_block:
                    print('Wait commitment approval')
                else:
                    should_produce_block = True
            
            if should_produce_block:
                print('Producing block...')
                parachain_block = parachain.get_block_number(parachain.get_chain_finalised_head())
                call = kusama.compose_call('OnDemandAssignmentProvider', 'place_order_keep_alive', {
                    'max_amount': 100000000000,
                    'para_id': 2011
                })
                send_extrinsic(kusama, keypair, call)
                while True:
                    new_parachain_block = parachain.get_block_number(parachain.get_chain_finalised_head())
                    if new_parachain_block > parachain_block:
                        break
                    print(f'Waiting for new block, current {new_parachain_block}')
                    sleep(6)
                print('Block produced')
            sleep(6)
        
        

    except Exception as e:
        print(f'Error: {e}')
        raise e
    finally:
        if sora:
            sora.close()
        if parachain:
            parachain.close()
        if kusama:
            kusama.close()



if __name__ == '__main__':
    main()
