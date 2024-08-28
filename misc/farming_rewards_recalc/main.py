from substrateinterface import SubstrateInterface
from decimal import Decimal
import json
import time

substrate = SubstrateInterface(url="wss://mof2.sora.org/", ss58_format=69)


dex_id = 0 # Polkaswap DEX Id

def balance(value):
    return str(value) + '0' * 18

# These time units are defined in number of blocks.
SECS_PER_BLOCK = 6
MINUTES = 60 / SECS_PER_BLOCK
HOURS = MINUTES * 60


PSWAP_PER_DAY = balance(2500000)  
VESTING_FREQUENCY = 6 * HOURS  
BLOCKS_PER_DAY = 14400  
VESTING_COEFF = 3  
REFRESH_FREQUENCY = 2 * HOURS

cached_pending_rewards = {}

def get_pending_rewards(block_num):
    if block_num in cached_pending_rewards:
        return cached_pending_rewards[block_num]
    block_hash = substrate.get_block_hash(block_id=block_num)
    data = substrate.query_map('VestedRewards', 'Rewards', None, block_hash,page_size=1000)
    records = data.records
    while True:
        next_page = data.retrieve_next_page(data.last_key)
        if len(next_page) == 0:
            break
        records.extend(next_page)
    pending_rewards = {}
    print(len(records))
    for account, reward_info in records:
        rewards = reward_info['rewards']
        farming_rewards = next((amount for reward_type, amount in rewards if reward_type == 'LiquidityProvisionFarming'), None)
        if farming_rewards is not None:
            pending_rewards[account.serialize()] = farming_rewards.serialize()
            
    cached_pending_rewards[block_num] = pending_rewards
    return pending_rewards

# def get_multiplier(asset_id, block_hash):
#     base_asset = substrate.query('Assets', 'BaseAssetId')
#     if asset_id == base_asset:
#         return balance(1)
#     else:
#         params = [
#             dex_id,
#             base_asset,
#             asset_id,
#             "1000000000000000000",
#             "WithDesiredOutput",
#             [],
#             "Disabled",
#             block_hash,
#         ]
#         result = substrate.rpc_request("liquidityProxy_quote", params)
#         return result['result']['amount_without_impact']

# def get_account_weight(trading_pair, multiplier, base_reserves, total_liquidity, pool_tokens):
#     pass

def get_farmer_weight_amplified_by_time(farmer_weight, farmer_block, now):
    farmer_farming_time = now - farmer_block
    coeff = (Decimal('1') + Decimal(farmer_farming_time) / Decimal(now)) ** VESTING_COEFF
    return coeff * Decimal(farmer_weight)

def prepare_pool_accounts_for_vesting(farmers, now, accounts):
    for farmer in farmers:
        weight = get_farmer_weight_amplified_by_time(farmer['weight'].serialize(), farmer['block'].serialize(), now)
        farmer_account = farmer['account'].serialize()
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

def calculate_rewards(block_number):
    block_hash = substrate.get_block_hash(block_id=block_number)
    pool_farmers = substrate.query_map('Farming', 'PoolFarmers', None, block_hash, page_size=1000)
    records = pool_farmers.records
    while True:
        next_page = pool_farmers.retrieve_next_page(pool_farmers.last_key)
        if len(next_page) == 0:
            break
        records.extend(next_page)
    all_accounts = {}
    pool_count, farmers_count = 0, 0 
    for pool, farmers in records:
        pool_count += 1
        farmers_count += len(farmers)
        prepare_pool_accounts_for_vesting(farmers, block_number, all_accounts) 
    rewards = prepare_account_rewards(all_accounts)
    return rewards

def is_calculated_with_data(block_num, rewards):
    if str(block_num) in rewards:
        return True, rewards
    try:
        with open('rewards.json') as data_file:
            rewards_data = json.load(data_file)
            return str(block_num) in rewards_data, rewards_data  
    except FileNotFoundError:
        return False, {}
    except json.JSONDecodeError:
        print('Error: Invalid JSON format in rewards.json')
        return False, {}
    except Exception as e:
        print(f'Error: {str(e)}')
        return False, {}

def update_rewards_file(data):
    with open('rewards.json', 'w') as f:
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
            block_rewards = calculate_rewards(block_num)
            print(f'from {int(block_num - VESTING_FREQUENCY)} to {block_num}')
            now_pending_rewards = get_pending_rewards(block_num)
            prev_pending_rewards = get_pending_rewards(int(block_num - VESTING_FREQUENCY))
            actual_rewards = {}
            for account in now_pending_rewards:
                calculated_actual_reward = max(now_pending_rewards.get(account, 0) - prev_pending_rewards.get(account, 0), 0)
                if calculated_actual_reward > 0:
                    actual_rewards[account] = calculated_actual_reward
            
            all_accounts = set(block_rewards.keys()) | set(actual_rewards.keys())
            rewards_per_account = {}
            for account in all_accounts:
                rewards_per_account[account] = {
                    'actual_reward': actual_rewards.get(account, 0),
                    'calculated_reward': block_rewards.get(account, 0),
                    'diff': block_rewards.get(account, 0) - actual_rewards.get(account, 0)
                    
                }
            
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