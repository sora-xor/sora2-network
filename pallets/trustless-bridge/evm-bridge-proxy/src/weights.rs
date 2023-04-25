use frame_support::weights::Weight;

pub trait WeightInfo {
    fn burn() -> Weight;
}

impl WeightInfo for () {
    fn burn() -> Weight {
        Default::default()
    }
}
