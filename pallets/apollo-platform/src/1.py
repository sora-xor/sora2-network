def get_reward(blocks): return reward - rate * blocks
def get_rate(): return reward / K


reward = 1000000000
rate = 0  # profit rate
K = 5256000

for i in range(365):
    print(f'{i}: rate: {rate}, reward: {reward}')
    reward = get_reward(14400)
    rate = get_rate()
