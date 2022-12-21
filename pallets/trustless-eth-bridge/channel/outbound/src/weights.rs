use frame_support::weights::Weight;
pub trait WeightInfo {
    fn on_initialize(num_messages: u32, avg_payload_bytes: u32) -> Weight;
    fn on_initialize_non_interval() -> Weight;
    fn on_initialize_no_messages() -> Weight;
    fn register_channel() -> Weight;
    fn set_fee() -> Weight;
}

impl WeightInfo for () {
    fn on_initialize(_: u32, _: u32) -> Weight {
        Weight::zero()
    }
    fn on_initialize_non_interval() -> Weight {
        Weight::zero()
    }
    fn on_initialize_no_messages() -> Weight {
        Weight::zero()
    }
    fn set_fee() -> Weight {
        Weight::zero()
    }
    fn register_channel() -> Weight {
        Weight::zero()
    }
}
