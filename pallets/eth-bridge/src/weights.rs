use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::{Pays, Weight};

pub struct WeightInfo<T>(PhantomData<T>);

impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
    fn register_bridge() -> Weight {
        Default::default()
    }
    fn add_asset() -> Weight {
        Default::default()
    }
    fn add_sidechain_token() -> Weight {
        Default::default()
    }
    fn transfer_to_sidechain() -> Weight {
        (157_969_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(14 as Weight))
            .saturating_add(T::DbWeight::get().writes(7 as Weight))
    }
    fn request_from_sidechain() -> Weight {
        (53_020_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(6 as Weight))
            .saturating_add(T::DbWeight::get().writes(5 as Weight))
    }
    fn add_peer() -> Weight {
        Default::default()
    }
    fn remove_peer() -> Weight {
        Default::default()
    }
    fn force_add_peer() -> Weight {
        Default::default()
    }
    fn prepare_for_migration() -> Weight {
        Default::default()
    }
    fn migrate() -> Weight {
        Default::default()
    }
    fn register_incoming_request() -> (Weight, Pays) {
        (
            (75_024_000 as Weight)
                .saturating_add(T::DbWeight::get().reads(6 as Weight))
                .saturating_add(T::DbWeight::get().writes(7 as Weight)),
            Pays::No,
        )
    }
    fn finalize_incoming_request() -> (Weight, Pays) {
        (
            (125_610_000 as Weight)
                .saturating_add(T::DbWeight::get().reads(9 as Weight))
                .saturating_add(T::DbWeight::get().writes(4 as Weight)),
            Pays::No,
        )
    }
    fn approve_request() -> (Weight, Pays) {
        (
            (288_688_000 as Weight)
                .saturating_add(T::DbWeight::get().reads(8 as Weight))
                .saturating_add(T::DbWeight::get().writes(1 as Weight)),
            Pays::No,
        )
    }
    fn approve_request_finalize() -> (Weight, Pays) {
        (
            (357_267_000 as Weight)
                .saturating_add(T::DbWeight::get().reads(13 as Weight))
                .saturating_add(T::DbWeight::get().writes(4 as Weight)),
            Pays::No,
        )
    }
    fn abort_request() -> (Weight, Pays) {
        (
            (88_293_000 as Weight)
                .saturating_add(T::DbWeight::get().reads(8 as Weight))
                .saturating_add(T::DbWeight::get().writes(3 as Weight)),
            Pays::No,
        )
    }
}

impl crate::WeightInfo for () {
    fn register_bridge() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn add_asset() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn add_sidechain_token() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn transfer_to_sidechain() -> Weight {
        10 * EXTRINSIC_FIXED_WEIGHT
    }
    fn request_from_sidechain() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn add_peer() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn remove_peer() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn force_add_peer() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn prepare_for_migration() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn migrate() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn register_incoming_request() -> (Weight, Pays) {
        (EXTRINSIC_FIXED_WEIGHT, Pays::No)
    }
    fn finalize_incoming_request() -> (Weight, Pays) {
        (EXTRINSIC_FIXED_WEIGHT, Pays::No)
    }
    fn approve_request() -> (Weight, Pays) {
        (EXTRINSIC_FIXED_WEIGHT, Pays::No)
    }
    fn approve_request_finalize() -> (Weight, Pays) {
        (EXTRINSIC_FIXED_WEIGHT, Pays::No)
    }
    fn abort_request() -> (Weight, Pays) {
        (EXTRINSIC_FIXED_WEIGHT, Pays::No)
    }
}
