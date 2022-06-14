use crate::{AssetIdOf, Config, LockInfo, LockerData, Timestamp, Weight};
use common::Balance;
use frame_support::debug;
use frame_support::traits::Get;
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::vec::Vec;

pub fn migrate<T: Config>() -> Weight {
    debug::RuntimeLogger::init();
    migrate_locker_data::<T>()
}

pub fn migrate_locker_data<T: Config>() -> Weight {
    let mut weight: u64 = 0;

    let current_timestamp = Timestamp::<T>::get();
    let current_block = frame_system::Pallet::<T>::block_number();
    LockerData::<T>::translate_values::<
        Vec<(Balance, T::BlockNumber, AssetIdOf<T>, AssetIdOf<T>)>,
        _,
    >(|v| {
        Some(
            v.into_iter()
                .map(|(pool_tokens, unlocking_block, asset_a, asset_b)| {
                    weight += 1;
                    let unlocking_timestamp: T::Moment;
                    if unlocking_block > current_block {
                        let num_of_seconds: u32 = ((unlocking_block - current_block) * 6u32.into())
                            .unique_saturated_into();
                        unlocking_timestamp = current_timestamp + num_of_seconds.into();
                    } else {
                        let num_of_seconds: u32 = ((current_block - unlocking_block) * 6u32.into())
                            .unique_saturated_into();
                        unlocking_timestamp = current_timestamp - num_of_seconds.into();
                    }

                    LockInfo {
                        pool_tokens,
                        unlocking_timestamp,
                        asset_a,
                        asset_b,
                    }
                })
                .collect::<Vec<LockInfo<Balance, T::Moment, AssetIdOf<T>>>>(),
        )
    });

    debug::info!(
        target: "runtime",
        "LockInfo migrated to new version with unlocking_timestamp field"
    );

    T::DbWeight::get().reads_writes(weight, weight)
}
