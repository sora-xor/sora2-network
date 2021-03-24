use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use common::weights::PresetWeightInfo;
use frame_support::weights::Weight;

impl crate::WeightInfo for () {
    fn claim() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}

impl crate::WeightInfo for PresetWeightInfo {
    fn claim() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
