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
        Vec<LockInfo<Balance, T::BlockNumber, T::Moment, AssetIdOf<T>>>,
        _,
    >(|mut v| {
        for lockups in v.iter_mut() {
            if lockups.unlocking_block > current_block {
                let num_of_seconds: u32 = ((lockups.unlocking_block - current_block) * 6u32.into())
                    .unique_saturated_into();
                lockups.unlocking_timestamp = current_timestamp + num_of_seconds.into();
            } else {
                let num_of_seconds: u32 = ((current_block - lockups.unlocking_block) * 6u32.into())
                    .unique_saturated_into();
                lockups.unlocking_timestamp = current_timestamp - num_of_seconds.into();
            }
        }
        weight += 1;
        Some(v)
    });

    debug::info!(
        target: "runtime",
        "LockInfo migrated to new version with unlocking_timestamp field"
    );

    T::DbWeight::get().reads_writes(weight, weight)
}
