//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0-rc5

use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use common::weights::PresetWeightInfo;
use frame_support::weights::constants::RocksDbWeight as DbWeight;
use frame_support::weights::Weight;

// impl crate::WeightInfo for () {
//     fn swap_pair() -> Weight {
//         (550_000_000 as Weight)
//             .saturating_add(DbWeight::get().reads(15 as Weight))
//             .saturating_add(DbWeight::get().writes(5 as Weight))
//     }
//     fn deposit_liquidity() -> Weight {
//         (500_000_000 as Weight)
//             .saturating_add(DbWeight::get().reads(18 as Weight))
//             .saturating_add(DbWeight::get().writes(6 as Weight))
//     }
//     fn withdraw_liquidity() -> Weight {
//         (500_000_000 as Weight)
//             .saturating_add(DbWeight::get().reads(17 as Weight))
//             .saturating_add(DbWeight::get().writes(6 as Weight))
//     }
//     fn initialize_pool() -> Weight {
//         (200_000_000 as Weight)
//             .saturating_add(DbWeight::get().reads(14 as Weight))
//             .saturating_add(DbWeight::get().writes(9 as Weight))
//     }
// }

impl crate::WeightInfo for () {
    fn swap_pair() -> Weight {
        (5_763_813_000 as Weight)
            .saturating_add(DbWeight::get().reads(15 as Weight))
            .saturating_add(DbWeight::get().writes(6 as Weight))
    }
    fn deposit_liquidity() -> Weight {
        (6_074_732_000 as Weight)
            .saturating_add(DbWeight::get().reads(19 as Weight))
            .saturating_add(DbWeight::get().writes(8 as Weight))
    }
    fn withdraw_liquidity() -> Weight {
        (4_389_551_000 as Weight)
            .saturating_add(DbWeight::get().reads(15 as Weight))
            .saturating_add(DbWeight::get().writes(4 as Weight))
    }
    fn initialize_pool() -> Weight {
        (4_087_898_000 as Weight)
            .saturating_add(DbWeight::get().reads(15 as Weight))
            .saturating_add(DbWeight::get().writes(15 as Weight))
    }
}

impl<T> crate::WeightInfo for PresetWeightInfo<T> {
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
