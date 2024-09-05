from constants import REFRESH_FREQUENCY, FARMING_MODULE, POOLS_STORAGE, \
    DEX_ID, TECHNICAL_MODULE, TECHNICAL_ACCOUNTS_STORAGE, XOR_ID, ASSET_IDS, POOL_XYK_MODULE, \
    KXOR_ID, ETH_ID
from utils import get_substrate_connection, load_json_file, balance
from typing import NewType
from decimal import Decimal
import json

BlockNumber = NewType('BlockNumber', int)


substrate = get_substrate_connection()
cache = {}


def GetChameleonPools(base):
    if base == XOR_ID:
        return (KXOR_ID, [ETH_ID])
    return None  

def refresh_pools(now: BlockNumber):
    _, data = load_json_file('./data', 'pool_farmers.json')
    if str(now) in data:
        return
    
    pools_index = int(now % REFRESH_FREQUENCY)
    pools = substrate.query(FARMING_MODULE, POOLS_STORAGE, [pools_index], block_hash=substrate.get_block_hash(block_id=now))
    new_farmer_pools = {}
    for pool in pools:
        update, pool_farmers = refresh_pool(pool.value, now, now)
        if update:
            new_farmer_pools[pool.value] = pool_farmers
            print(f'\n\n---New Farmers for Pool {pool.value}---')
            print(pool_farmers, end='\n\n')
    
    
    with open('./data/pool_farmers.json',  'w') as f:
        data[str(now)] = new_farmer_pools
        json.dump(data, f, indent=4)

def refresh_pool(pool, now: BlockNumber, block_num: BlockNumber):
    block_hash = substrate.get_block_hash(block_num)
    
    # 1. Query pool_xyk::get_pool_trading_pair -> trading_pair
    trading_pair = get_pool_trading_pair(pool, block_hash) #Done
    
    print(f'Trading Pair: {trading_pair}\n')

    # 2. Get multiplier of trading_pair.base_asset_id -> multiplier
    multiplier = get_multiplier(trading_pair['base_asset_id'], block_hash)

    print(f'Multiplier {multiplier} \n')
    
    # 3. Retrieve old farmers in the pool -> old_farmers
    old_farmers = substrate.query(FARMING_MODULE, 'PoolFarmers', [pool], block_hash)

    print(f'Old Farmers: {old_farmers}\n')
    
    # 4. Init empty list for new farmers -> new_farmers
    new_farmers = []

    # 5. Query pool_xyk::TotalIssuances(pool) -> pool_total_liquidity
    pool_total_liquidity = substrate.query('PoolXYK', 'TotalIssuances', [pool], block_hash)
    
    print(f'Pool Total Liquidity: {pool_total_liquidity}\n')

    # 6. pool_xyk::Pallet::<T>::get_actual_reserves(pool, trading_pair.base_asset_id, 
    #    trading_pair.base_asset_id, trading_pair.target_asset_id) -> pool_base_reserves
    pool_base_reserves, _, _ = get_actual_reserves(pool, trading_pair['base_asset_id'], trading_pair['base_asset_id'], trading_pair['target_asset_id'], block_hash)


    print(f'\nPool Base Reserve: {pool_base_reserves}\n')

    # 7. Query Farming::PoolProviders(pool) which retrieves list of (account, pool_tokens: Balance)
    pool_providers = substrate.query_map(POOL_XYK_MODULE, 'PoolProviders', [pool], block_hash)

    print(f'Pool Provider: {pool_providers}\n')
    
    # 8. Iterate over List[(account, pool_tokens: Balance),...]
    for account, pool_tokens in pool_providers:
        print(f"Calculating weight...{account} -> {pool_tokens}")
        # 8-1. get_account_weight(...), continue if weight == 0
        weight = get_account_weight(
            trading_pair,
            multiplier,
            pool_base_reserves,
            pool_total_liquidity.value,
            pool_tokens.value,
            block_hash
        )
        if weight == 0:
            continue

        # 8-2. Calculate the block
        block = next((farmer['block'].value for farmer in old_farmers if farmer['account'] == account), now - (now % REFRESH_FREQUENCY))

        # 8-3. Add new PoolFarmer object to new_farmers as {account, block, weight}
        new_farmers.append({'account': account.value, 'block': int(block), 'weight': int(weight)})

    if len(new_farmers) > 0 or len(old_farmers) > 0:
        return True, new_farmers
    else:
        return False, []
    
def get_multiplier(asset_id: str, block_hash: str):
    base_asset = XOR_ID
    if asset_id == base_asset:
        return balance(1)
    else:
        params = [
            DEX_ID,
            base_asset,
            asset_id,
            "1000000000000000000",
            "WithDesiredOutput",
            [],
            "Disabled",
            block_hash,
        ]
        result = substrate.rpc_request("liquidityProxy_quote", params)
        return result['result']['amount_without_impact']

def get_actual_reserves(pool, base_asset, input_asset, output_asset, block_hash):
    tpair, base_chameleon_asset, is_chameleon_pool = get_pair_info(base_asset, input_asset, output_asset)
    
    reserve_base = free_balance(tpair['base_asset_id'], pool, block_hash)
    reserve_target = free_balance(tpair['target_asset_id'], pool, block_hash)
    
    reserve_chameleon = 0
    if base_chameleon_asset is not None and is_chameleon_pool:
        reserve_chameleon = free_balance(base_chameleon_asset, pool, block_hash)
    
    reserve_base_chameleon = Decimal(reserve_base) + Decimal(reserve_chameleon)
    max_output = reserve_chameleon # default
    if output_asset == tpair['target_asset_id']:
        max_output = reserve_target
    elif output_asset == tpair['base_asset_id']:
        max_output = reserve_base
    
    
    if tpair['target_asset_id'] == input_asset:
        return reserve_target, reserve_base_chameleon, max_output
    else:
        return reserve_base_chameleon, reserve_target, max_output


def get_pair_info(base_asset, asset_a, asset_b):
    if asset_a == asset_b:
        raise ValueError("Cannot get pair info for identical assets")
    target_asset, base_chameleon_asset_id, chameleon_targets, is_allowed = None, None, None, False
    chameleon_pool = GetChameleonPools(base_asset)
    if asset_a == base_asset:
        target_asset = asset_b
    elif asset_b == base_asset:
        target_asset = asset_a
    elif chameleon_pool is not None:
        base_chameleon_asset_id, chameleon_targets = chameleon_pool[0], chameleon_pool[1]
        if asset_a == base_chameleon_asset_id:
            assert asset_b in chameleon_targets
            target_asset = asset_b
            is_allowed = True
        elif asset_b == base_chameleon_asset_id:
            assert asset_a in chameleon_targets
            target_asset = asset_a
            is_allowed = True
        else:
            raise RuntimeError("Base Asset Cannot be matched with any asset args")    
    else:
        raise RuntimeError("Base Asset Cannot be matched with any asset args")
    
    tpair = {
        "base_asset_id": base_asset,
        "target_asset_id": target_asset
    }
    return (tpair, base_chameleon_asset_id, is_allowed)

def get_base_asset_part(base_reserves: int, total_liquidity: int, liq_amount: int) -> int:
    print(f'get_base_asset_part( {base_reserves}, {total_liquidity}, {liq_amount})')
    try:
        fxw_liq_in_pool = Decimal(total_liquidity)
        fxw_piece = fxw_liq_in_pool / Decimal(liq_amount)
        fxw_value = Decimal(base_reserves) / fxw_piece
        value = int(fxw_value)
        return value
    except Exception as e:
        return 0
    
    
def get_account_weight(
    trading_pair,
    multiplier,
    base_reserves,
    total_liquidity,
    pool_tokens,
    block_hash
):
    base_asset_amt = get_base_asset_part(base_reserves, total_liquidity, pool_tokens)
    
    base_asset_amt = Decimal(base_asset_amt) * Decimal(multiplier)
    
    if 'lp_min_xor_for_bonus_reward' not in cache:
        cache['lp_min_xor_for_bonus_reward'] = substrate.query(FARMING_MODULE, 'LpMinXorForBonusReward', None, block_hash)
    lp_min_xor_for_bonus_reward = cache['lp_min_xor_for_bonus_reward']
    
    
    print(f"Account weight calculation:\n     base_asset_amt ({base_asset_amt}) <-> lp_min_xor_for_bonus_reward ({lp_min_xor_for_bonus_reward})")
    
    if base_asset_amt < lp_min_xor_for_bonus_reward:
        
        print("Returning 0 as account weight")
        return 0
    
    # params = [
    #     block_hash,
    # ]
    # result = substrate.rpc_request("farming_rewardDoublingAssets", params)
    result = ['0x0200050000000000000000000000000000000000000000000000000000000000', '0x0200040000000000000000000000000000000000000000000000000000000000', '0x0200060000000000000000000000000000000000000000000000000000000000', '0x0200070000000000000000000000000000000000000000000000000000000000', '0x0200090000000000000000000000000000000000000000000000000000000000', '0x02000a0000000000000000000000000000000000000000000000000000000000', '0x0003b1dbee890acfb1b3bc12d1bb3b4295f52755423f84d1751b2545cebf000b']
    pool_doubles_reward = any(asset_id in trading_pair for asset_id in result)

    if pool_doubles_reward:
        return base_asset_amt * 2
    else:
        return base_asset_amt
    
    
def get_pool_trading_pair(pool_account: str, block_hash: str):
    res = substrate.query(TECHNICAL_MODULE, TECHNICAL_ACCOUNTS_STORAGE, params=[pool_account], block_hash=block_hash)
    data = res.value_serialized['Pure'][1]['XykLiquidityKeeper']
    tp = {}
    for k, v in data.items():
        if 'Wrapped' in v:
            tp[k] = ASSET_IDS.get(v['Wrapped'], None)
        elif 'Escaped' in v:
            tp[k] = v['Escaped']
    print(data)
    print(tp)
    return tp



def free_balance(asset, account, block_hash):
    params = [
        account,
        asset,
        block_hash
    ]
    print(params)
    result = substrate.rpc_request('assets_freeBalance', params)
    return result['result']['balance']


def main():
    # start_block = 16755767
    start_block = 16755787
    end_block = 17053065
    
    for block_num in range(start_block, end_block + 1):
        print(f'Processing Block: {block_num}\n')
        refresh_pools(block_num)
        print('\n\n\n---------------------------------------------\n\n')

if __name__ == '__main__':
    try:
        main()
    except KeyboardInterrupt:
        pass
    except ValueError as e:
        print(e)
        pass
        