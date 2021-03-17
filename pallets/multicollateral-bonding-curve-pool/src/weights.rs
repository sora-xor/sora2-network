//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0-rc5

use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use common::weights::PresetWeightInfo;
use frame_support::weights::Weight;

impl crate::WeightInfo for () {
    fn initialize_pool() -> Weight {
        Default::default()
    }
    fn set_reference_asset() -> Weight {
        Default::default()
    }
    fn set_optional_reward_multiplier() -> Weight {
        Default::default()
    }
    fn claim_incentives() -> Weight {
        Default::default()
    }
}

impl crate::WeightInfo for PresetWeightInfo {
    fn initialize_pool() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn set_reference_asset() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn set_optional_reward_multiplier() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn claim_incentives() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
