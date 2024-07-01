use crate::{AssetIdOf, Config, PoolData, Pools, UserInfo, UserInfos};
use codec::{Decode, Encode};
use common::{Balance, XOR};
use frame_support::pallet_prelude::Weight;
use frame_support::traits::Get;
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

pub fn migrate<T: Config>() -> Weight {
    sp_runtime::runtime_logger::RuntimeLogger::init();
    migrate_pool_and_user_data::<T>()
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
                        is_removed: old_pool_data.is_removed,
                        base_asset: base_asset,
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
                    }
                })
                .collect::<Vec<UserInfo<AssetIdOf<T>>>>(),
        )
    });

    log::info!(
        target: "runtime",
        "PoolData and UserInfo migrated to new version with base_asset field"
    );

    T::DbWeight::get().reads_writes(weight, weight)
}
