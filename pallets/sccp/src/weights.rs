use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::{constants::RocksDbWeight, Weight};

pub trait WeightInfo {
    fn burn() -> Weight;
    fn mint_from_proof() -> Weight;
    fn add_token_from_proof() -> Weight;
    fn pause_token_from_proof() -> Weight;
    fn resume_token_from_proof() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn burn() -> Weight {
        Weight::from_parts(120_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(3_u64))
    }

    fn mint_from_proof() -> Weight {
        Weight::from_parts(180_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(4_u64))
            .saturating_add(T::DbWeight::get().writes(2_u64))
    }

    fn add_token_from_proof() -> Weight {
        Weight::from_parts(100_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(6_u64))
            .saturating_add(T::DbWeight::get().writes(3_u64))
    }

    fn pause_token_from_proof() -> Weight {
        Weight::from_parts(70_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(2_u64))
    }

    fn resume_token_from_proof() -> Weight {
        Weight::from_parts(70_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(2_u64))
    }
}

impl WeightInfo for () {
    fn burn() -> Weight {
        Weight::from_parts(120_000_000, 0)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(3_u64))
    }

    fn mint_from_proof() -> Weight {
        Weight::from_parts(180_000_000, 0)
            .saturating_add(RocksDbWeight::get().reads(4_u64))
            .saturating_add(RocksDbWeight::get().writes(2_u64))
    }

    fn add_token_from_proof() -> Weight {
        Weight::from_parts(100_000_000, 0)
            .saturating_add(RocksDbWeight::get().reads(6_u64))
            .saturating_add(RocksDbWeight::get().writes(3_u64))
    }

    fn pause_token_from_proof() -> Weight {
        Weight::from_parts(70_000_000, 0)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(2_u64))
    }

    fn resume_token_from_proof() -> Weight {
        Weight::from_parts(70_000_000, 0)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(2_u64))
    }
}
