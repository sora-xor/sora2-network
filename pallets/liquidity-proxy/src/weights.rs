//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0-rc5

use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use common::weights::PresetWeightInfo;
use frame_support::weights::constants::RocksDbWeight as DbWeight;
use frame_support::weights::Weight;

use common::prelude::SwapVariant;

impl crate::WeightInfo for () {
    fn swap(amount: SwapVariant) -> Weight {
        match amount {
            SwapVariant::WithDesiredInput => (10_700_000_000 as Weight)
                .saturating_add(DbWeight::get().reads(19 as Weight))
                .saturating_add(DbWeight::get().writes(5 as Weight)),
            _ => (19_400_000_000 as Weight)
                .saturating_add(DbWeight::get().reads(19 as Weight))
                .saturating_add(DbWeight::get().writes(5 as Weight)),
        }
    }
}

impl crate::WeightInfo for PresetWeightInfo {
    fn swap(_amount: SwapVariant) -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
