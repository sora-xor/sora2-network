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
    PollData::<T>::translate_values::<(u32, T::BlockNumber, T::BlockNumber), _>(
        |(number_of_options, poll_start_block, poll_end_block)| {
            let poll_start_timestamp: T::Moment;
            let poll_end_timestamp: T::Moment;

            if poll_start_block > current_block {
                let num_of_seconds: u32 =
                    ((poll_start_block - current_block) * 6u32.into()).unique_saturated_into();
                poll_start_timestamp = current_timestamp + num_of_seconds.into();
            } else {
                let num_of_seconds: u32 =
                    ((current_block - poll_start_block) * 6u32.into()).unique_saturated_into();
                poll_start_timestamp = current_timestamp - num_of_seconds.into();
            }

            if poll_end_block > current_block {
                let num_of_seconds: u32 =
                    ((poll_end_block - current_block) * 6u32.into()).unique_saturated_into();
                poll_end_timestamp = current_timestamp + num_of_seconds.into();
            } else {
                let num_of_seconds: u32 =
                    ((current_block - poll_end_block) * 6u32.into()).unique_saturated_into();
                poll_end_timestamp = current_timestamp - num_of_seconds.into();
            }

            weight += 1;
            Some(PollInfo {
                number_of_options,
                poll_start_timestamp,
                poll_end_timestamp,
            })
        },
    );

    debug::info!(
        target: "runtime",
        "PollInfo migrated to new version with timestamp fields"
    );

    T::DbWeight::get().reads_writes(weight, weight)
}
