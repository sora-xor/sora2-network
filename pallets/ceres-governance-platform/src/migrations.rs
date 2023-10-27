/*
use crate::{Config, PollData, PollInfo, Timestamp};
use common::convert_block_number_to_timestamp;
use frame_support::dispatch::Weight;
use frame_support::log;
use frame_support::traits::Get;

pub fn migrate<T: Config>() -> Weight {
    sp_runtime::runtime_logger::RuntimeLogger::init();
    migrate_poll_data::<T>()
}

pub fn migrate_poll_data<T: Config>() -> Weight {
    let mut weight: u64 = 0;

    let current_timestamp = Timestamp::<T>::get();
    let current_block = frame_system::Pallet::<T>::block_number();
    PollData::<T>::translate_values::<(u32, T::BlockNumber, T::BlockNumber), _>(
        |(number_of_options, poll_start_block, poll_end_block)| {
            let poll_start_timestamp = convert_block_number_to_timestamp::<T>(
                poll_start_block,
                current_block,
                current_timestamp,
            );

            let poll_end_timestamp = convert_block_number_to_timestamp::<T>(
                poll_end_block,
                current_block,
                current_timestamp,
            );

            weight += 1;
            Some(PollInfo {
                number_of_options,
                poll_start_timestamp,
                poll_end_timestamp,
            })
        },
    );

    log::info!(
        target: "runtime",
        "PollInfo migrated to new version with timestamp fields"
    );

    T::DbWeight::get().reads_writes(weight, weight)
}
*/
