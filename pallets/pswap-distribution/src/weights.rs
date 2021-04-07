use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use sp_std::marker::PhantomData;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
    fn claim_incentive() -> Weight {
        (114_700_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(10 as Weight))
            .saturating_add(T::DbWeight::get().writes(6 as Weight))
    }
    fn on_initialize(is_distributing: bool) -> Weight {
        if is_distributing {
            (848_112_300_000 as Weight)
                .saturating_add(T::DbWeight::get().reads(10057 as Weight))
                .saturating_add(T::DbWeight::get().writes(1021 as Weight))
        } else {
            (118_300_000 as Weight).saturating_add(T::DbWeight::get().reads(10 as Weight))
        }
    }
}

impl crate::WeightInfo for () {
    fn claim_incentive() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn on_initialize(_is_distributing: bool) -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
