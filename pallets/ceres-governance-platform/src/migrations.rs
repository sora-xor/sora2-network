use crate::{Config, PollInfo, Timestamp, Weight};
use frame_support::debug;
use frame_support::traits::Get;
use sp_runtime::traits::UniqueSaturatedInto;

pub fn migrate<T: Config>() -> Weight {
    debug::RuntimeLogger::init();

    (0 as Weight).saturating_add(migrate_locker_data::<T>())
}

pub fn migrate_locker_data<T: Config>() -> Weight {
    let mut weight: Weight = 0;

    let current_timestamp = Timestamp::<T>::get();
    let current_block = frame_system::Pallet::<T>::block_number();
    crate::PollData::<T>::translate_values::<PollInfo<T::BlockNumber, T::Moment>, _>(|mut p| {
        if p.poll_start_block > current_block {
            let num_of_seconds: u32 =
                ((p.poll_start_block - current_block) * 6u32.into()).unique_saturated_into();
            p.poll_start_timestamp = current_timestamp + num_of_seconds.into();
        } else {
            let num_of_seconds: u32 =
                ((current_block - p.poll_start_block) * 6u32.into()).unique_saturated_into();
            p.poll_start_timestamp = current_timestamp - num_of_seconds.into();
        }

        if p.poll_end_block > current_block {
            let num_of_seconds: u32 =
                ((p.poll_end_block - current_block) * 6u32.into()).unique_saturated_into();
            p.poll_end_timestamp = current_timestamp + num_of_seconds.into();
        } else {
            let num_of_seconds: u32 =
                ((current_block - p.poll_end_block) * 6u32.into()).unique_saturated_into();
            p.poll_end_timestamp = current_timestamp - num_of_seconds.into();
        }

        Some(p)
    });

    debug::info!(
        target: "runtime",
        "PollInfo migrated to new version with timestamp fields"
    );

    // The exact weight of the StorageMap::translate_values() is unknown
    // Since runtime upgrade is executed regardless the weight we can use approximate value
    weight = weight.saturating_add(T::DbWeight::get().writes(1000));

    weight
}
