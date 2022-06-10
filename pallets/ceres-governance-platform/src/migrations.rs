use crate::{Config, PollData, PollInfo, Timestamp, Weight};
use frame_support::debug;
use frame_support::traits::Get;
use sp_runtime::traits::UniqueSaturatedInto;

pub fn migrate<T: Config>() -> Weight {
    debug::RuntimeLogger::init();
    migrate_poll_data::<T>()
}

pub fn migrate_poll_data<T: Config>() -> Weight {
    let mut weight: u64 = 0;

    let current_timestamp = Timestamp::<T>::get();
    let current_block = frame_system::Pallet::<T>::block_number();
    PollData::<T>::translate_values::<PollInfo<T::BlockNumber, T::Moment>, _>(|mut p| {
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
        weight += 1;
        Some(p)
    });

    debug::info!(
        target: "runtime",
        "PollInfo migrated to new version with timestamp fields"
    );

    T::DbWeight::get().reads_writes(weight, weight)
}
