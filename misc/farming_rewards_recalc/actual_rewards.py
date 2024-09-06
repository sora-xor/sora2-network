
from decimal import Decimal
import json
import time
from .constants import *
from .utils import get_substrate_connection, retreive_data_from_storage, load_json_file

substrate = get_substrate_connection()

cached_pending_rewards = {}

def get_pending_rewards(block_num):
    if block_num in cached_pending_rewards:
        return cached_pending_rewards[block_num]
    records = retreive_data_from_storage(substrate, VESTED_REWARDS_MODULE, REWARDS_STORAGE, None, block_num)
    pending_rewards = {}
    for account, reward_info in records:
        rewards = reward_info['rewards']
        farming_rewards = next((amount for reward_type, amount in rewards if reward_type == 'LiquidityProvisionFarming'), None)
        if farming_rewards is not None:
            pending_rewards[account.serialize()] = farming_rewards.serialize()          
    cached_pending_rewards[block_num] = pending_rewards
    return pending_rewards


def is_calculated_with_data(block_num, cached_data):
    if str(block_num) in cached_data:
        return True, cached_data
    _, data = load_json_file('./data', 'actual_rewards.json')
    return str(block_num) in data, data
    

def update_rewards_file(data):
    with open('/data/actual_rewards.json', 'w') as f:
        json.dump(data, f, indent=4)

def main():
    start_block = 16755767
    end_block = 17053065
    last_update_time = 0
    _, rewards = is_calculated_with_data(start_block, {})
    
    def process_block(block_num):
        nonlocal last_update_time, rewards
        current_time = time.time()
        is_calculated, _ = is_calculated_with_data(block_num, rewards)
        if not is_calculated:
            now_pending_rewards = get_pending_rewards(block_num)
            prev_pending_rewards = get_pending_rewards(int(block_num - VESTING_FREQUENCY))
            actual_rewards = {}
            for account in now_pending_rewards:
                calculated_actual_reward = max(now_pending_rewards.get(account, 0) - prev_pending_rewards.get(account, 0), 0)
                if calculated_actual_reward > 0:
                    actual_rewards[account] = calculated_actual_reward
            
            all_accounts =  set(actual_rewards.keys())
            rewards_per_account = {}
            for account in all_accounts:
                rewards_per_account[account] = actual_rewards.get(account, 0),
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