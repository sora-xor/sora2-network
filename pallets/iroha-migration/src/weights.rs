use frame_support::weights::Weight;

use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use common::weights::PresetWeightInfo;

use crate::WeightInfo;

impl WeightInfo for () {
    fn migrate() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}

impl WeightInfo for PresetWeightInfo {
    fn migrate() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
