//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0-rc5

use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use common::weights::PresetWeightInfo;
use frame_support::weights::constants::RocksDbWeight as DbWeight;
use frame_support::weights::Weight;

use common::prelude::SwapVariant;

impl crate::WeightInfo for () {
    fn transfer() -> Weight {
        (1_868_364_000 as Weight)
            .saturating_add(DbWeight::get().reads(9 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
}

impl crate::WeightInfo for PresetWeightInfo {
    fn transfer() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
