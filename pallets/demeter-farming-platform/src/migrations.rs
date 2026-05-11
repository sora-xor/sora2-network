use crate::{AssetIdOf, Config, PoolData, Pools, TokenInfos, UserInfo, UserInfos};
use codec::{Decode, Encode};
use common::{Balance, XOR};
use frame_support::__private::log;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use sp_std::vec::Vec;

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
struct OldPoolData {
    pub multiplier: u32,
    pub deposit_fee: Balance,
    pub is_core: bool,
    pub is_farm: bool,
    pub total_tokens_in_pool: Balance,
    pub rewards: Balance,
    pub rewards_to_be_distributed: Balance,
    pub is_removed: bool,
}

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
struct OldUserInfo<AssetId> {
    pub pool_asset: AssetId,
    pub reward_asset: AssetId,
    pub is_farm: bool,
    pub pooled_tokens: Balance,
    pub rewards: Balance,
}

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
struct V2PoolData<AssetId> {
    pub multiplier: u32,
    pub deposit_fee: Balance,
    pub is_core: bool,
    pub is_farm: bool,
    pub total_tokens_in_pool: Balance,
    pub rewards: Balance,
    pub rewards_to_be_distributed: Balance,
    pub is_removed: bool,
    pub base_asset: AssetId,
}

#[derive(Encode, Decode, Default, PartialEq, Eq, scale_info::TypeInfo)]
struct V2UserInfo<AssetId> {
    pub base_asset: AssetId,
    pub pool_asset: AssetId,
    pub reward_asset: AssetId,
    pub is_farm: bool,
    pub pooled_tokens: Balance,
    pub rewards: Balance,
}

pub fn migrate<T: Config>() -> Weight {
    sp_runtime::runtime_logger::RuntimeLogger::init();
    migrate_pool_and_user_data::<T>()
        .saturating_add(dedupe_active_pools::<T>())
        .saturating_add(recompute_token_multipliers::<T>())
}

pub fn migrate_v2_to_v3<T: Config>() -> Weight {
    sp_runtime::runtime_logger::RuntimeLogger::init();
    migrate_reward_checkpoints::<T>()
        .saturating_add(dedupe_active_pools::<T>())
        .saturating_add(recompute_token_multipliers::<T>())
}

pub fn migrate_pool_and_user_data<T: Config>() -> Weight {
    let mut weight: u64 = 0;

    Pools::<T>::translate::<Vec<OldPoolData>, _>(|k1, _, v| {
        Some(
            v.into_iter()
                .map(|old_pool_data: OldPoolData| {
                    weight += 1;

                    let mut base_asset: AssetIdOf<T> = XOR.into();
                    if !old_pool_data.is_farm {
                        base_asset = k1;
                    }

                    PoolData {
                        multiplier: old_pool_data.multiplier,
                        deposit_fee: old_pool_data.deposit_fee,
                        is_core: old_pool_data.is_core,
                        is_farm: old_pool_data.is_farm,
                        total_tokens_in_pool: old_pool_data.total_tokens_in_pool,
                        rewards: old_pool_data.rewards,
                        rewards_to_be_distributed: old_pool_data.rewards_to_be_distributed,
                        reward_per_token: 0,
                        is_removed: old_pool_data.is_removed,
                        base_asset,
                    }
                })
                .collect::<Vec<PoolData<AssetIdOf<T>>>>(),
        )
    });

    UserInfos::<T>::translate::<Vec<OldUserInfo<AssetIdOf<T>>>, _>(|_, v| {
        Some(
            v.into_iter()
                .map(|old_user_info: OldUserInfo<AssetIdOf<T>>| {
                    weight += 1;

                    let mut base_asset: AssetIdOf<T> = XOR.into();
                    if !old_user_info.is_farm {
                        base_asset = old_user_info.pool_asset;
                    }

                    UserInfo {
                        base_asset,
                        pool_asset: old_user_info.pool_asset,
                        reward_asset: old_user_info.reward_asset,
                        is_farm: old_user_info.is_farm,
                        pooled_tokens: old_user_info.pooled_tokens,
                        rewards: old_user_info.rewards,
                        reward_per_token_paid: 0,
                    }
                })
                .collect::<Vec<UserInfo<AssetIdOf<T>>>>(),
        )
    });

    log::info!(
        target: "runtime",
        "PoolData and UserInfo migrated to new version with base_asset and reward checkpoint fields"
    );

    T::DbWeight::get().reads_writes(weight, weight)
}

pub fn migrate_reward_checkpoints<T: Config>() -> Weight {
    let mut weight: u64 = 0;

    Pools::<T>::translate::<Vec<V2PoolData<AssetIdOf<T>>>, _>(|_, _, v| {
        Some(
            v.into_iter()
                .map(|pool_data| {
                    weight += 1;

                    PoolData {
                        multiplier: pool_data.multiplier,
                        deposit_fee: pool_data.deposit_fee,
                        is_core: pool_data.is_core,
                        is_farm: pool_data.is_farm,
                        total_tokens_in_pool: pool_data.total_tokens_in_pool,
                        rewards: pool_data.rewards,
                        rewards_to_be_distributed: pool_data.rewards_to_be_distributed,
                        reward_per_token: 0,
                        is_removed: pool_data.is_removed,
                        base_asset: pool_data.base_asset,
                    }
                })
                .collect::<Vec<PoolData<AssetIdOf<T>>>>(),
        )
    });

    UserInfos::<T>::translate::<Vec<V2UserInfo<AssetIdOf<T>>>, _>(|_, v| {
        Some(
            v.into_iter()
                .map(|user_info| {
                    weight += 1;

                    UserInfo {
                        base_asset: user_info.base_asset,
                        pool_asset: user_info.pool_asset,
                        reward_asset: user_info.reward_asset,
                        is_farm: user_info.is_farm,
                        pooled_tokens: user_info.pooled_tokens,
                        rewards: user_info.rewards,
                        reward_per_token_paid: 0,
                    }
                })
                .collect::<Vec<UserInfo<AssetIdOf<T>>>>(),
        )
    });

    log::info!(
        target: "runtime",
        "PoolData and UserInfo migrated to reward checkpoint fields"
    );

    T::DbWeight::get().reads_writes(weight, weight)
}

pub fn dedupe_active_pools<T: Config>() -> Weight {
    let mut reads: u64 = 0;
    let mut writes: u64 = 0;

    for (pool_asset, reward_asset, mut pool_infos) in Pools::<T>::iter() {
        reads += 1;
        let mut seen: Vec<(AssetIdOf<T>, bool)> = Vec::new();
        let mut changed = false;

        for pool_info in pool_infos.iter_mut() {
            if pool_info.is_removed {
                continue;
            }

            let duplicate = seen.iter().any(|(base_asset, is_farm)| {
                *base_asset == pool_info.base_asset && *is_farm == pool_info.is_farm
            });
            if duplicate {
                pool_info.is_removed = true;
                changed = true;
            } else {
                seen.push((pool_info.base_asset, pool_info.is_farm));
            }
        }

        if changed {
            Pools::<T>::insert(pool_asset, reward_asset, pool_infos);
            writes += 1;
        }
    }

    log::info!(
        target: "runtime",
        "Demeter duplicate active pools deduplicated"
    );

    T::DbWeight::get().reads_writes(reads, writes)
}

pub fn recompute_token_multipliers<T: Config>() -> Weight {
    let mut reads: u64 = 0;
    let mut writes: u64 = 0;

    for (reward_asset, mut token_info) in TokenInfos::<T>::iter() {
        reads += 1;
        token_info.farms_total_multiplier = 0;
        token_info.staking_total_multiplier = 0;
        TokenInfos::<T>::insert(reward_asset, token_info);
        writes += 1;
    }

    for (_, reward_asset, pool_infos) in Pools::<T>::iter() {
        reads += 1;
        if !TokenInfos::<T>::contains_key(&reward_asset) {
            continue;
        }
        reads += 1;
        let mut token_info = TokenInfos::<T>::get(&reward_asset).expect("checked above");
        let mut changed = false;

        for pool_info in pool_infos {
            if pool_info.is_removed {
                continue;
            }
            if pool_info.is_farm {
                token_info.farms_total_multiplier = token_info
                    .farms_total_multiplier
                    .saturating_add(pool_info.multiplier);
            } else {
                token_info.staking_total_multiplier = token_info
                    .staking_total_multiplier
                    .saturating_add(pool_info.multiplier);
            }
            changed = true;
        }

        if changed {
            TokenInfos::<T>::insert(reward_asset, token_info);
            writes += 1;
        }
    }

    log::info!(
        target: "runtime",
        "Demeter token multipliers recomputed from active pools"
    );

    T::DbWeight::get().reads_writes(reads, writes)
}
