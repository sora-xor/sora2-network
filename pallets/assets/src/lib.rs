//! # Assets Module
//!
//! ## Overview
//!
//! The assets module serves as an extension of `currencies` pallet.
//! It allows to explicitly register new assets and store their owners' account IDs.
//! This allows to restrict some actions on assets for non-owners.
//!
//! ### Dispatchable Functions
//!
//! - `register` - registers new asset by a given ID.

#![cfg_attr(not(feature = "std"), no_std)]

#[allow(unused_imports)]
#[macro_use]
extern crate alloc;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use common::{hash, prelude::Balance, Amount};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::sp_runtime::traits::{MaybeSerializeDeserialize, Member};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, dispatch, ensure, traits::Get, Parameter,
};
use frame_system::{ensure_signed, RawOrigin};
use permissions::{Scope, BURN, MINT, SLASH, TRANSFER};
use sp_core::H512;
use traits::{
    MultiCurrency, MultiCurrencyExtended, MultiLockableCurrency, MultiReservableCurrency,
};

pub type AssetIdOf<T> = <T as Trait>::AssetId;
pub type Permissions<T> = permissions::Module<T>;

type CurrencyIdOf<T> =
    <<T as Trait>::Currency as MultiCurrency<<T as frame_system::Trait>::AccountId>>::CurrencyId;

pub trait Trait: frame_system::Trait + permissions::Trait + tokens::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;

    /// DEX assets (currency) identifier.
    type AssetId: Parameter
        + Member
        + Copy
        + MaybeSerializeDeserialize
        + Ord
        + Default
        + Into<CurrencyIdOf<Self>>
        + From<common::AssetId>;

    /// The base asset as the core asset in all trading pairs
    type GetBaseAssetId: Get<Self::AssetId>;

    /// Currency to transfer, reserve/unreserve, lock/unlock assets
    type Currency: MultiLockableCurrency<
            Self::AccountId,
            Moment = Self::BlockNumber,
            CurrencyId = Self::AssetId,
            Balance = Balance,
        > + MultiReservableCurrency<Self::AccountId, CurrencyId = Self::AssetId, Balance = Balance>
        + MultiCurrencyExtended<Self::AccountId, Amount = Amount>;
}

decl_storage! {
    trait Store for Module<T: Trait> as AssetsModule {
        AssetOwners get(fn asset_owners): map hasher(twox_64_concat) T::AssetId => T::AccountId;
    }
    add_extra_genesis {
        config(endowed_assets): Vec<(T::AssetId, T::AccountId)>;

        build(|config: &GenesisConfig<T>| {
            config.endowed_assets.iter().for_each(|(asset_id, account_id)| {
                Module::<T>::register(RawOrigin::Signed(account_id.clone()).into(), asset_id.clone())
                    .expect("Failed to register asset.");
            })
        })
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
        AssetId = <T as Trait>::AssetId,
    {
        AssetRegistered(AssetId, AccountId),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// An asset with a given ID already exists.
        AssetIdAlreadyExists,
        /// An asset with a given ID not exists.
        AssetIdNotExists,
        /// Permissions error
        Permissions,
        /// A number is out of range of the balance type.
        InsufficientBalance,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        /// Performs an asset registration.
        ///
        /// Basically, this function checks the if given `asset_id` has an owner
        /// and if not, inserts it.
        #[weight = 10_000 + T::DbWeight::get().writes(1)]
        pub fn register(origin, asset_id: T::AssetId) -> dispatch::DispatchResult {
            let author = ensure_signed(origin.clone())?;
            ensure!(Self::asset_owner(&asset_id).is_none(), Error::<T>::AssetIdAlreadyExists);
            AssetOwners::<T>::insert(asset_id.clone(), author.clone());
            let scope = Scope::Limited(hash(&asset_id));
            let permission_ids = [TRANSFER, MINT, BURN, SLASH];
            for permission_id in &permission_ids {
                if Permissions::<T>::assign_permission(author.clone(), &author, *permission_id, scope).is_err() {
                    return Err(Error::<T>::Permissions.into());
                }
            }
            Self::deposit_event(RawEvent::AssetRegistered(asset_id, author));
            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    pub fn asset_owner(asset_id: &T::AssetId) -> Option<T::AccountId> {
        let account_id = Self::asset_owners(&asset_id);
        if account_id == T::AccountId::default() {
            None
        } else {
            Some(account_id)
        }
    }

    #[inline]
    pub fn asset_exists(asset_id: &T::AssetId) -> bool {
        Self::asset_owner(asset_id).is_some()
    }

    pub fn ensure_asset_exists(asset_id: &T::AssetId) -> DispatchResult {
        if !Self::asset_exists(asset_id) {
            return Err(Error::<T>::AssetIdNotExists.into());
        }
        Ok(())
    }

    #[inline]
    pub fn is_asset_owner(asset_id: &T::AssetId, account_id: &T::AccountId) -> bool {
        Self::asset_owner(asset_id)
            .map(|x| &x == account_id)
            .unwrap_or(false)
    }

    fn check_permission_maybe_with_parameters(
        issuer: &T::AccountId,
        permission_id: u32,
        asset_id: &T::AssetId,
    ) -> Result<(), DispatchError> {
        Permissions::<T>::check_permission_with_scope(
            issuer.clone(),
            permission_id,
            &Scope::Limited(hash(asset_id)),
        )
        .or(Permissions::<T>::check_permission_with_scope(
            issuer.clone(),
            permission_id,
            &Scope::Limited(H512::zero()),
        ))?;
        Ok(())
    }

    pub fn total_issuance(asset_id: &T::AssetId) -> Result<Balance, DispatchError> {
        Self::ensure_asset_exists(asset_id)?;
        Ok(T::Currency::total_issuance(asset_id.clone()))
    }

    pub fn total_balance(
        asset_id: &T::AssetId,
        who: &T::AccountId,
    ) -> Result<Balance, DispatchError> {
        Self::ensure_asset_exists(asset_id)?;
        Ok(T::Currency::total_balance(asset_id.clone(), who))
    }

    pub fn free_balance(
        asset_id: &T::AssetId,
        who: &T::AccountId,
    ) -> Result<Balance, DispatchError> {
        Self::ensure_asset_exists(asset_id)?;
        Ok(T::Currency::free_balance(asset_id.clone(), who))
    }

    pub fn ensure_can_withdraw(
        asset_id: &T::AssetId,
        who: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult {
        Self::ensure_asset_exists(asset_id)?;
        T::Currency::ensure_can_withdraw(asset_id.clone(), who, amount)
    }

    pub fn transfer(
        asset_id: &T::AssetId,
        from: &T::AccountId,
        to: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult {
        Self::ensure_asset_exists(asset_id)?;
        Self::check_permission_maybe_with_parameters(from, TRANSFER, asset_id)?;
        T::Currency::transfer(asset_id.clone(), from, to, amount)
    }

    pub fn force_transfer(
        asset_id: &T::AssetId,
        from: &T::AccountId,
        to: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult {
        T::Currency::transfer(asset_id.clone(), from, to, amount)
    }

    pub fn mint(
        asset_id: &T::AssetId,
        issuer: &T::AccountId,
        to: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult {
        Self::ensure_asset_exists(asset_id)?;
        Self::check_permission_maybe_with_parameters(issuer, MINT, asset_id)?;
        T::Currency::deposit(asset_id.clone(), to, amount)
    }

    pub fn burn(
        asset_id: &T::AssetId,
        issuer: &T::AccountId,
        to: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult {
        Self::ensure_asset_exists(asset_id)?;
        Self::check_permission_maybe_with_parameters(issuer, BURN, asset_id)?;
        T::Currency::withdraw(asset_id.clone(), to, amount)
    }

    pub fn can_slash(
        asset_id: &T::AssetId,
        who: &T::AccountId,
        amount: Balance,
    ) -> Result<bool, DispatchError> {
        Self::ensure_asset_exists(asset_id)?;
        Self::check_permission_maybe_with_parameters(who, SLASH, asset_id)?;
        Ok(T::Currency::can_slash(asset_id.clone(), who, amount))
    }

    pub fn slash(
        asset_id: &T::AssetId,
        who: &T::AccountId,
        amount: Balance,
    ) -> Result<Balance, DispatchError> {
        Self::ensure_asset_exists(asset_id)?;
        Self::check_permission_maybe_with_parameters(who, SLASH, asset_id)?;
        Ok(T::Currency::slash(asset_id.clone(), who, amount))
    }

    pub fn update_balance(
        asset_id: &T::AssetId,
        who: &T::AccountId,
        by_amount: Amount,
    ) -> DispatchResult {
        Self::check_permission_maybe_with_parameters(who, MINT, asset_id)?;
        Self::check_permission_maybe_with_parameters(who, BURN, asset_id)?;
        T::Currency::update_balance(asset_id.clone(), who, by_amount)
    }
}
