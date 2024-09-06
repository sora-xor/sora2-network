from utils import load_json_file, get_substrate_connection, retreive_data_from_storage
from constants import FARMING_MODULE, POOL_FARMERS_STORAGE
import json

substrate = get_substrate_connection()

def dump_pool_farmers_previous_state_if_not_exist(blocknum):
    is_created, data = load_json_file('./data', 'pool_famers')
    if is_created and str(blocknum) in data:
        return
    records = retreive_data_from_storage(substrate, FARMING_MODULE, POOL_FARMERS_STORAGE, None, blocknum)
    pool_farmers_at_block = {}
    for record in records:
        pool_farmers_at_block[record[0].value] = record[1].value
    with open('./data/initial_pool_farmers.json', 'w') as f:
        _, data  = load_json_file('./data', 'initial_pool_farmers.json')
        data = pool_farmers_at_block
        json.dump(data, f, indent=4)
        
        
def main():
    start_block = 16755767
    end_block = 17053065
    dump_pool_farmers_previous_state_if_not_exist(start_block - 1)
    
    
if __name__ == '__main__':
    main()