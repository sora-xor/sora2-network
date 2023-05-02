use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use frame_support::dispatch::Weight;

pub trait WeightInfo {
    fn burn() -> Weight;
    fn mint() -> Weight;
    fn register_network() -> Weight;
    fn register_network_with_existing_asset() -> Weight;
}

impl WeightInfo for () {
    fn burn() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn mint() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn register_network() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn register_network_with_existing_asset() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
