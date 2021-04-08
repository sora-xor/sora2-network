from eth_abi import decode_single
from web3 import Web3
import json
import requests

from nft_owners import NFT_OWNERS

SUPPORTED_TOKENS = [12277, 30297, 6929, 24403, 77235, 88849, 6895, 112043]
ATTEMPT_COUNT = 3
CONTRACT = '0xd07dc4262bcdbf85190c01c996b4c06a461d2430'
TRANSFER_SINGLE_TOPIC = Web3.keccak(
    text='TransferSingle(address,address,address,uint256,uint256)')
TRANSFER_BATCH_TOPIC = Web3.keccak(
    text='TransferBatch(address,address,address,uint256[],uint256[])')


def create_w3():
    return Web3(Web3.WebsocketProvider('ws://127.0.0.1:8546', websocket_timeout=600))


def get_transaction_receipt(w3, tx_hash):
    attempt = 1
    while True:
        try:
            return (w3, w3.eth.getTransactionReceipt(tx_hash))
        except:
            if attempt != ATTEMPT_COUNT:
                print(
                    f'attempt {attempt} failed. connection is closed when trying to get transaction receipt {tx_hash}. sleeping')
                time.sleep(60)
                attempt += 1
                w3 = create_w3()
            else:
                raise


def handle_creating_transaction(w3, tx_hash):
    w3, receipt = get_transaction_receipt(w3, tx_hash)
    assert receipt is not None
    assert receipt.status == 1
    created_tokens = {}
    for log in receipt.logs:
        if log.address.lower() != CONTRACT:
            continue
        event_topic = log.topics[0]
        if event_topic == TRANSFER_SINGLE_TOPIC or event_topic == TRANSFER_BATCH_TOPIC:
            giver = log.topics[2].hex()
            assert giver == '0x0000000000000000000000000000000000000000000000000000000000000000', 'unexpected giver %s' % giver
            if log.topics[0] == TRANSFER_SINGLE_TOPIC:
                token, qty = decode_single(
                    '(uint256,uint256)', Web3.toBytes(hexstr=log.data))
                tokens = [token]
                qtys = [qty]
            else:
                tokens, qtys = decode_single(
                    '(uint256[],uint256[])', Web3.toBytes(hexstr=log.data))
            for token, qty in zip(tokens, qtys):
                if token in SUPPORTED_TOKENS:
                    created_tokens[token] = qty
            return (w3, created_tokens)


def handle_transferring_transaction(w3, tx_hash):
    w3, receipt = get_transaction_receipt(w3, tx_hash)
    assert receipt is not None
    assert receipt.status == 1
    transferred_tokens = {}
    for log in receipt.logs:
        if log.address.lower() != CONTRACT:
            continue
        event_topic = log.topics[0]
        if event_topic == TRANSFER_SINGLE_TOPIC or event_topic == TRANSFER_BATCH_TOPIC:
            if log.topics[0] == TRANSFER_SINGLE_TOPIC:
                token, qty = decode_single(
                    '(uint256,uint256)', Web3.toBytes(hexstr=log.data))
                tokens = [token]
                qtys = [qty]
            else:
                tokens, qtys = decode_single(
                    '(uint256[],uint256[])', Web3.toBytes(hexstr=log.data))
            for token, qty in zip(tokens, qtys):
                if token in SUPPORTED_TOKENS:
                    transferred_tokens[token] = qty
            return (w3, transferred_tokens)


def verify(w3, owner, tokens, expected_tokens):
    uri = 'https://api.etherscan.io/api?module=logs&action=getLogs&fromBlock=10147630'
    uri += '&address=0xd07dc4262bcdbf85190c01c996b4c06a461d2430'
    uri += '&topic0=0xc3d58168c5ae7397731d063d5bbf3d657854427343f4c083240f7aacaa2d0f62'
    uri += '&topic2=%s' % owner
    uri += '&apikey=<>'
    data = json.loads(requests.get(uri).text)
    transactions = data['result']
    if len(transactions) > 0:
        assert len(transactions) < 1000, owner
        for transaction in transactions:
            tx_hash = transaction['transactionHash']
            w3, transferred_tokens = handle_transferring_transaction(w3, tx_hash)
            for token, qty in transferred_tokens.items():
                if token in tokens:
                    tokens[token] -= qty
    for token, qty in tokens.items():
        assert token in expected_tokens, '%s: %s not in %s' % (owner, token, expected_tokens)
        assert qty == expected_tokens[token], '%s: %s: %s != %s' % (owner, token, qty, expected_tokens[token])


def merge_tokens(source, target):
    for token, qty in source.items():
        assert token not in target
        target[token] = qty


def normalize_owners(owners):
    owner_keys = list(owners.keys())
    for owner in owner_keys:
        tokens = owners[owner]
        token_keys = list(tokens.keys())
        for token in token_keys:
            if tokens[token] == 0:
                tokens.pop(token)
        if len(tokens) == 0:
            owners.pop(owner)


w3 = create_w3()
normalize_owners(NFT_OWNERS)
for owner, owned_tokens in NFT_OWNERS.items():
    uri = 'https://api.etherscan.io/api?module=logs&action=getLogs&fromBlock=10147630'
    uri += '&address=0xd07dc4262bcdbf85190c01c996b4c06a461d2430'
    uri += '&topic0=0xc3d58168c5ae7397731d063d5bbf3d657854427343f4c083240f7aacaa2d0f62'
    uri += '&topic2=0x0000000000000000000000000000000000000000000000000000000000000000'
    uri += '&topic3=%s' % owner
    uri += '&apikey=<>'
    data = json.loads(requests.get(uri).text)
    transactions = data['result']
    if len(transactions) > 0:
        assert len(transactions) < 1000, owner
        tokens = {}
        for transaction in transactions:
            tx_hash = transaction['transactionHash']
            w3, created_tokens = handle_creating_transaction(w3, tx_hash)
            for token in created_tokens.keys():
                print('{} is owner of {}'.format(owner, token))
                if token in NFT_OWNERS[owner]:
                    NFT_OWNERS[owner].pop(token)
normalize_owners(NFT_OWNERS)
print(NFT_OWNERS)