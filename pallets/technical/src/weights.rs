//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0-rc5

use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use common::weights::PresetWeightInfo;
use frame_support::weights::constants::RocksDbWeight as DbWeight;
use frame_support::weights::Weight;

impl crate::WeightInfo for () {
    fn create_swap() -> Weight {
        (100_000_000 as Weight)
            .saturating_add(DbWeight::get().reads(3 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
}

impl<T> crate::WeightInfo for PresetWeightInfo<T> {
    fn create_swap() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
