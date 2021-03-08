//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0-rc5

use common::weights::{constants::EXTRINSIC_FIXED_WEIGHT, PresetWeightInfo};
use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

impl crate::WeightInfo for () {
    fn swap_pair() -> Weight {
        (550_000_000 as Weight)
            .saturating_add(DbWeight::get().reads(15 as Weight))
            .saturating_add(DbWeight::get().writes(5 as Weight))
    }
    fn deposit_liquidity() -> Weight {
        (500_000_000 as Weight)
            .saturating_add(DbWeight::get().reads(18 as Weight))
            .saturating_add(DbWeight::get().writes(6 as Weight))
    }
    fn withdraw_liquidity() -> Weight {
        (500_000_000 as Weight)
            .saturating_add(DbWeight::get().reads(17 as Weight))
            .saturating_add(DbWeight::get().writes(6 as Weight))
    }
    fn initialize_pool() -> Weight {
        (200_000_000 as Weight)
            .saturating_add(DbWeight::get().reads(14 as Weight))
            .saturating_add(DbWeight::get().writes(9 as Weight))
    }
}

impl crate::WeightInfo for PresetWeightInfo {
    fn swap_pair() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn deposit_liquidity() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn withdraw_liquidity() -> Weight {
        10 * EXTRINSIC_FIXED_WEIGHT
    }
    fn initialize_pool() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
