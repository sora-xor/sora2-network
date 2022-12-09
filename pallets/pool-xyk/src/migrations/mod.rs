use frame_support::traits::GetStorageVersion;
use frame_support::weights::Weight;

use crate::{Config, Pallet};

pub mod v1_1;
pub mod v1_2;
pub mod v2;

pub fn migrate<T: Config>() -> Weight {
    let version = Pallet::<T>::on_chain_storage_version();
    if version < 2 {
        v2::migrate::<T>()
    } else {
        Weight::zero()
    }
}
