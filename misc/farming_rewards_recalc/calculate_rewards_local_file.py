
from decimal import Decimal
import json
import time
from constants import *
from utils import get_substrate_connection, retreive_data_from_storage, load_json_file


start_block = 16755767


def get_farmer_weight_amplified_by_time(farmer_weight, farmer_block, now):
    farmer_farming_time = now - farmer_block
    coeff = (Decimal('1') + Decimal(farmer_farming_time) / Decimal(now)) ** VESTING_COEFF
    return coeff * Decimal(farmer_weight)

def prepare_pool_accounts_for_vesting(farmers, now, accounts):
    for farmer in farmers:
        weight = get_farmer_weight_amplified_by_time(farmer['weight'], farmer['block'], now)
        farmer_account = farmer['account']
        if farmer_account in accounts:
            accounts[farmer_account] += weight
        else:
            accounts[farmer_account] = weight
    return accounts

def prepare_account_rewards(accounts):
    total_weight = sum(accounts.values())
    reward_per_day = Decimal(PSWAP_PER_DAY)
    reward_vesting_part = Decimal(VESTING_FREQUENCY) / Decimal(BLOCKS_PER_DAY)
    reward = reward_per_day * reward_vesting_part

    rewards = {}
    for account, weight in accounts.items():
        account_reward = reward * weight / total_weight
        rewards[account] = int(account_reward)
    return rewards



def get_pool_farmers(end_block):
    _, pool_farmers = load_json_file('./data', 'pool_farmers.json')
    _, _pool_farmers = load_json_file('./data', 'initial_pool_farmers.json')
    for blocknum in range(start_block, end_block + 1):
        tmp_pool_farmer = pool_farmers.get(str(blocknum), {})
        for pool_account, farmers in tmp_pool_farmer.items():
            _pool_farmers[pool_account] = farmers
    return _pool_farmers

def calculate_rewards(block_number):
    records = get_pool_farmers(block_number)
    all_accounts = {}
    pool_count, farmers_count = 0, 0 
    for pool, farmers in records.items():
        pool_count += 1
        farmers_count += len(farmers)
        prepare_pool_accounts_for_vesting(farmers, block_number, all_accounts) 
    rewards = prepare_account_rewards(all_accounts)
    return rewards

def is_calculated_with_data(block_num, cached_data):
    if str(block_num) in cached_data:
        return True, cached_data
    _, data = load_json_file('./data', 'calculated_rewards.json')
    return str(block_num) in data, data
    

def update_rewards_file(data):
    with open('./data/calculated_rewards.json', 'w') as f:
        json.dump(data, f, indent=4)

def main():
    # start_block = 16755767
    end_block = 17053065
    last_update_time = 0
    
    _, rewards = is_calculated_with_data(start_block, {})
    
    def process_block(block_num):
        nonlocal last_update_time, rewards
        current_time = time.time()
        is_calculated, _ = is_calculated_with_data(block_num, rewards)
        if not is_calculated:
            block_rewards = calculate_rewards(block_num)
            print(f'from {int(block_num - VESTING_FREQUENCY)} to {block_num}')
            all_accounts = set(block_rewards.keys())
            rewards_per_account = {}
            for account in all_accounts:
                rewards_per_account[account] = block_rewards.get(account, 0)
            rewards[str(block_num)] = rewards_per_account
            if current_time - last_update_time >= 60:  # Update only once per minute
                update_rewards_file(rewards)
                last_update_time = current_time
            print(f"Processed block {block_num} out of {end_block}. we have {int((end_block - block_num)/ VESTING_FREQUENCY)} ...")
            print(f'block took {time.time() - current_time} sec')
    
    
    for block_num in range(start_block, end_block + 1):
        if block_num % VESTING_FREQUENCY == 0:
            process_block(block_num)
        

if __name__ == '__main__':
    main()
    