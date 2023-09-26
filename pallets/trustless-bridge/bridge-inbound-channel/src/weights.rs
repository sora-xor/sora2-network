use frame_support::weights::Weight;

pub trait WeightInfo {
    fn submit() -> Weight;
    fn batch_dispatched() -> Weight;
    fn set_reward_fraction() -> Weight;
    fn register_channel() -> Weight;
}

impl WeightInfo for () {
    fn submit() -> Weight {
        Weight::zero()
    }
    fn batch_dispatched() -> Weight {
        Weight::zero()
    }
    fn set_reward_fraction() -> Weight {
        Weight::zero()
    }
    fn register_channel() -> Weight {
        Weight::zero()
    }
}
