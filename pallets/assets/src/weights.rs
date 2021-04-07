//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0-rc5

use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use common::weights::PresetWeightInfo;
use frame_support::weights::constants::RocksDbWeight as DbWeight;
use frame_support::weights::Weight;

// impl crate::WeightInfo for () {
//     fn register() -> Weight {
//         (220_000_000 as Weight)
//             .saturating_add(DbWeight::get().reads(10 as Weight))
//             .saturating_add(DbWeight::get().writes(7 as Weight))
//     }
//     fn transfer() -> Weight {
//         (130_000_000 as Weight).saturating_add(DbWeight::get().reads(4 as Weight))
//     }
//     fn mint() -> Weight {
//         (180_000_000 as Weight)
//             .saturating_add(DbWeight::get().reads(6 as Weight))
//             .saturating_add(DbWeight::get().writes(2 as Weight))
//     }
//     fn burn() -> Weight {
//         (200_000_000 as Weight)
//             .saturating_add(DbWeight::get().reads(6 as Weight))
//             .saturating_add(DbWeight::get().writes(2 as Weight))
//     }
//     fn set_non_mintable() -> Weight {
//         Default::default()
//     }
// }

impl crate::WeightInfo for () {
    fn register() -> Weight {
        (1_407_724_000 as Weight)
            .saturating_add(DbWeight::get().reads(9 as Weight))
            .saturating_add(DbWeight::get().writes(7 as Weight))
    }
    fn transfer() -> Weight {
        (704_917_000 as Weight).saturating_add(DbWeight::get().reads(4 as Weight))
    }
    fn mint() -> Weight {
        (971_546_000 as Weight)
            .saturating_add(DbWeight::get().reads(5 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn burn() -> Weight {
        (1_046_473_000 as Weight)
            .saturating_add(DbWeight::get().reads(4 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn set_non_mintable() -> Weight {
        (350_341_000 as Weight)
            .saturating_add(DbWeight::get().reads(2 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
}

impl<T> crate::WeightInfo for PresetWeightInfo<T> {
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
