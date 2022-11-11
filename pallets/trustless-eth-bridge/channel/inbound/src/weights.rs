use frame_support::weights::Weight;

pub trait WeightInfo {
    fn submit() -> Weight;
    fn message_dispatched() -> Weight;
    fn set_reward_fraction() -> Weight;
    fn register_channel() -> Weight;
}

impl WeightInfo for () {
    fn submit() -> Weight {
        0
    }
    fn message_dispatched() -> Weight {
        0
    }
    fn set_reward_fraction() -> Weight {
        0
    }
    fn register_channel() -> Weight {
        0
    }
}
