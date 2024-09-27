
from substrateinterface import SubstrateInterface
from typing import List, Any
import json


def get_substrate_connection():
    substrate = SubstrateInterface(url="wss://mof2.sora.org/", ss58_format=69)
    return substrate

def balance(value):
    return str(value) + '0' * 18


def retreive_data_from_storage(substrate: SubstrateInterface, module: str, storage_func: str, params: Any, block_num: int)-> List[Any]:
    block_hash = substrate.get_block_hash(block_id=block_num)
    data = substrate.query_map(module=module, storage_function=storage_func, params=params, block_hash=block_hash, page_size=1000)
    records = data.records
    while True:
        next_page = data.retrieve_next_page(data.last_key)
        if len(next_page) == 0:
            break
        records.extend(next_page)
    return records


def load_json_file(path: str, filename: str):
    try:
        with open(f'{path}/{filename}') as data_file:
            data = json.load(data_file)
            return True, data
    except FileNotFoundError:
        return False, {}
    except json.JSONDecodeError:
        print('Error: Invalid JSON format in rewards.json')
        return False, {}
    except Exception as e:
        print(f'Error: {str(e)}')
        return False, {}
