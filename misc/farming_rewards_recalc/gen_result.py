import json

def get_rewards(type):
    with open(f'./data/{type}_rewards.json') as data_file:
        rewards_data = json.load(data_file)
        return rewards_data

def update_result_file(data):
    with open('./data/result.json', 'w') as f:
        json.dump(data, f, indent=4)

actual_rewards = get_rewards('actual')
calculated_rewards = get_rewards('calculated')
result = {}
for blocknum, block_reward in actual_rewards.items():
    if blocknum not in calculated_rewards:
        print(f"Warning: Block {blocknum} not found in calculated rewards. Skipping...")
        continue
    for account in block_reward.keys():
        if account not in calculated_rewards[blocknum]:
            print(f"Warning: Account {account} not found in calculated rewards for block {blocknum}. Skipping...")
            continue
        if account in result:
            result[account] += (calculated_rewards[blocknum][account] - actual_rewards[blocknum][account])
        else:
            result[account] = (calculated_rewards[blocknum][account] - actual_rewards[blocknum][account])

# Add a check for the specific block number 17042400
if '17042400' not in calculated_rewards:
    print("Warning: Block 17042400 not found in calculated rewards. This block may need special handling.")

update_result_file(result)
