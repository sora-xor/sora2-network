from scalecodec.type_registry import load_type_registry_file
from substrateinterface.utils import ss58
from substrateinterface.base import SubstrateInterface
import json

substrate = SubstrateInterface(
    url='ws://localhost:19944',
    ss58_format=69,
    type_registry_preset='core',
    type_registry=load_type_registry_file('../PythonBot/types_scalecodec_python.json'),
)

with open("pallets/vested-rewards/crowdloan_rewards.json", "r") as f:
    rewards = json.load(f)

res = []
for reward in rewards:
    account = ss58.ss58_encode(reward['Address'])
    for user_reward in substrate.query_map("VestedRewards", "CrowdloanClaimHistory", [account]):
        asset, block = user_reward
        res.append((account, asset.value['code'], block.value))

with open("claim_history.json", "w") as f:
    json.dump(res, f)
