#![cfg_attr(not(feature = "std"), no_std)]

use common::prelude::*;
pub use domain::*;
use frame_support::{
    codec::{Decode, Encode},
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::DispatchResult,
    RuntimeDebug,
};
use frame_system::ensure_signed;
use pallet_timestamp as timestamp;
use sp_core::hash::H512;

mod domain;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub type FarmName = H512;

pub trait Trait:
    frame_system::Trait
    + timestamp::Trait
    + permissions::Trait
    + technical::Trait
    + sp_std::fmt::Debug
    + sp_std::default::Default
{
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
}

decl_storage! {
    trait Store for Module<T: Trait> as FarmsStoreModule
    {
        /// FarmName --> Farm
        pub Farms: map hasher(blake2_128_concat) FarmName => Option<Farm<T>>;
        /// FarmName, FarmerId --> Farm
        pub Farmers: double_map hasher(blake2_128_concat) FarmName, hasher(blake2_128_concat) T::AccountId => Option<Farmer<T>>;
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
    {
        /// Farm was created [farm owner].
        FarmCreated(AccountId),
        /// Farmer was created [farmer].
        FarmerCreated(AccountId),
        /// Farmer claimed incentive [farmer].
        IncentiveClaimed(AccountId),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        FarmNotFound,
        TechAccountIsMissing,
        FarmAlreadyClosed,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin
    {
        type Error = Error<T>;
        fn deposit_event() = default;

        /// Create `Farm` dispatchable.
        /// Generates `Farm` identifier and creates `TechAccount` to collect incentives.
        #[weight = 10_000]
        pub fn create(origin, name: FarmName, parameters: Parameters<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            permissions::Module::<T>::check_permission(who.clone(), permissions::CREATE_FARM)?;
            let technical_account_id = T::TechAccountId::from_generic_pair(who.encode(), name.encode());
            technical::Module::<T>::register_tech_account_id(technical_account_id.clone())?;
            technical::Module::<T>::transfer_in(&parameters.incentive.asset_id, &who, &technical_account_id, parameters.incentive.amount)?;
            let technical_account_id = technical::Module::<T>::tech_account_id_to_account_id(&technical_account_id)?;
            Farms::<T>::insert(name, Farm::new(who.clone(), technical_account_id, parameters));
            Self::deposit_event(RawEvent::FarmCreated(who));
            Ok(())
        }

        /// Invest tokens to `Farm`.
        /// Creates new `Farmer` and includes they into incentives distribution.
        #[weight = 5_000]
        pub fn invest(origin, farm_name: FarmName, amount: Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            permissions::Module::<T>::check_permission(who.clone(), permissions::INVEST_TO_FARM)?;
            let farm = Farms::<T>::get(&farm_name).ok_or(Error::<T>::FarmNotFound)?;
            if !farm.is_open(<timestamp::Module<T>>::get()) {
                return Err(Error::<T>::FarmAlreadyClosed)?;
            }
            let technical_account_id = T::TechAccountId::from_generic_pair(who.encode(), farm_name.encode());
            technical::Module::<T>::register_tech_account_id(technical_account_id.clone())?;
            technical::Module::<T>::transfer_in(
                &farm.parameters.incentive.asset_id,
                &who, &technical_account_id, amount
            )?;
            let farmer = Farmer::new(technical::Module::<T>::tech_account_id_to_account_id(&technical_account_id)?, amount);
            Farmers::<T>::insert(farm_name, who.clone(), farmer);
            //TODO: grant permission
            Self::deposit_event(RawEvent::FarmerCreated(who));
            Ok(())
        }

        /// Claim incentitives from `Farm`.
        /// Calculates incentives and transfer them into Farmer's Account.
        #[weight = 15_000]
        pub fn claim(origin, farm_name: FarmName, amount: Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            //TODO: check_permission_with_parameters
            permissions::Module::<T>::check_permission(who.clone(), permissions::CLAIM_FROM_FARM)?;
            //TODO: check_lock_periods
            let technical_account_id = T::TechAccountId::from_generic_pair(who.encode(), farm_name.encode());
            technical::Module::<T>::ensure_tech_account_registered(&technical_account_id)?;
            technical::Module::<T>::transfer_out(
                &Farms::<T>::get(&farm_name).ok_or(Error::<T>::FarmNotFound)?.parameters.incentive.asset_id,
                &technical_account_id, &who, amount
            )?;
            Self::deposit_event(RawEvent::IncentiveClaimed(who));
            Ok(())
        }
    }
}
