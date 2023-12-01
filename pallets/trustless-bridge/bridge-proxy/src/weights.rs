use frame_support::weights::Weight;

pub trait WeightInfo {
    fn burn() -> Weight;
    fn add_limited_asset() -> Weight;
    fn remove_limited_asset() -> Weight;
    fn update_transfer_limit() -> Weight;
}

impl WeightInfo for () {
    fn burn() -> Weight {
        Default::default()
    }

    fn add_limited_asset() -> Weight {
        Default::default()
    }

    fn remove_limited_asset() -> Weight {
        Default::default()
    }

    fn update_transfer_limit() -> Weight {
        Default::default()
    }
}
