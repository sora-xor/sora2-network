//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0-rc5

use common::weights::{constants::EXTRINSIC_FIXED_WEIGHT, PresetWeightInfo};
use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

impl crate::WeightInfo for () {
    fn register() -> Weight {
        (220_000_000 as Weight)
            .saturating_add(DbWeight::get().reads(10 as Weight))
            .saturating_add(DbWeight::get().writes(7 as Weight))
    }
    fn transfer() -> Weight {
        (130_000_000 as Weight).saturating_add(DbWeight::get().reads(4 as Weight))
    }
    fn mint() -> Weight {
        (180_000_000 as Weight)
            .saturating_add(DbWeight::get().reads(6 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
    fn burn() -> Weight {
        (200_000_000 as Weight)
            .saturating_add(DbWeight::get().reads(6 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
    fn set_non_mintable() -> Weight {
        Default::default()
    }
}

impl crate::WeightInfo for PresetWeightInfo {
    fn register() -> Weight {
        10 * EXTRINSIC_FIXED_WEIGHT
    }
    fn transfer() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn mint() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn burn() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn set_non_mintable() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
