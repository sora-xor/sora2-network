use frame_support::weights::Weight;

pub trait WeightInfo {
    fn submit() -> Weight;
    fn register_channel() -> Weight;
}

impl WeightInfo for () {
    fn submit() -> Weight {
        0
    }
    fn register_channel() -> Weight {
        0
    }
}
