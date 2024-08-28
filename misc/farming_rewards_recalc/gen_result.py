import json

def get_rewards():
    with open('rewards.json') as data_file:
        rewards_data = json.load(data_file)
        return rewards_data

def update_result_file(data):
    with open('result.json', 'w') as f:
        json.dump(data, f, indent=4)

rewards = get_rewards()
result = {}
for block_reward in rewards.values():
    for account,data in block_reward.items():
        if account in result:
            result[account] += data['diff']
        else:
            result[account] = data['diff'] 

update_result_file(result)

