use crate::{AssetIdOf, Config, PoolData, Pools, UserInfo, UserInfos, Weight};
use common::{Balance, XOR};
use frame_support::log;
use frame_support::traits::Get;
use sp_std::vec::Vec;

pub fn migrate<T: Config>() -> Weight {
    sp_runtime::runtime_logger::RuntimeLogger::init();
    migrate_pool_and_user_data::<T>()
}

pub fn migrate_pool_and_user_data<T: Config>() -> Weight {
    let mut weight: u64 = 0;

    Pools::<T>::translate::<Vec<(u32, Balance, bool, bool, Balance, Balance, Balance, bool)>, _>(
        |k1, _, v| {
            Some(
                v.into_iter()
                    .map(
                        |(
                            multiplier,
                            deposit_fee,
                            is_core,
                            is_farm,
                            total_tokens_in_pool,
                            rewards,
                            rewards_to_be_distributed,
                            is_removed,
                        )| {
                            weight += 1;

                            let mut base_asset: AssetIdOf<T> = XOR.into();
                            if !is_farm {
                                base_asset = k1;
                            }

                            PoolData {
                                multiplier,
                                deposit_fee,
                                is_core,
                                is_farm,
                                total_tokens_in_pool,
                                rewards,
                                rewards_to_be_distributed,
                                is_removed,
                                base_asset: base_asset.into(),
                            }
                        },
                    )
                    .collect::<Vec<PoolData<AssetIdOf<T>>>>(),
            )
        },
    );

    UserInfos::<T>::translate::<Vec<(AssetIdOf<T>, AssetIdOf<T>, bool, Balance, Balance)>, _>(
        |_, v| {
            Some(
                v.into_iter()
                    .map(
                        |(pool_asset, reward_asset, is_farm, pooled_tokens, rewards)| {
                            weight += 1;

                            let mut base_asset: AssetIdOf<T> = XOR.into();
                            if !is_farm {
                                base_asset = pool_asset;
                            }

                            UserInfo {
                                base_asset,
                                pool_asset,
                                reward_asset,
                                is_farm,
                                pooled_tokens,
                                rewards,
                            }
                        },
                    )
                    .collect::<Vec<UserInfo<AssetIdOf<T>>>>(),
            )
        },
    );

    log::info!(
        target: "runtime",
        "PoolData and UserInfo migrated to new version with base_asset field"
    );

    T::DbWeight::get().reads_writes(weight, weight)
}
