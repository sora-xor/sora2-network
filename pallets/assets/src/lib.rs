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

use common::hash;
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::sp_runtime::traits::{MaybeSerializeDeserialize, Member};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, dispatch, ensure, traits::Get, Parameter,
};
use frame_system::ensure_signed;
use permissions::{BURN, MINT, SLASH, TRANSFER};
use traits::MultiCurrency;
use traits::MultiCurrencyExtended;

pub type AssetIdOf<T> = <T as Trait>::AssetId;

type CurrencyIdOf<T> = <<T as currencies::Trait>::MultiCurrency as MultiCurrency<
    <T as frame_system::Trait>::AccountId,
>>::CurrencyId;

type BalanceOf<T> = <<T as currencies::Trait>::MultiCurrency as MultiCurrency<
    <T as frame_system::Trait>::AccountId,
>>::Balance;

type AmountOf<T> = <<T as currencies::Trait>::MultiCurrency as MultiCurrencyExtended<
    <T as frame_system::Trait>::AccountId,
>>::Amount;

pub trait Trait: frame_system::Trait + currencies::Trait + permissions::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    /// DEX assets (currency) identifier.
    type AssetId: Parameter
        + Member
        + Copy
        + MaybeSerializeDeserialize
        + Ord
        + Default
        + Into<CurrencyIdOf<Self>>;
    /// The base asset as the core asset in all trading pairs
    type GetBaseAssetId: Get<Self::AssetId>;
}

decl_storage! {
    trait Store for Module<T: Trait> as AssetsModule {
        AssetOwners get(fn asset_owners): map hasher(twox_64_concat) T::AssetId => T::AccountId;
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
            let permission = permissions::Permission::<T>::with_parameters(
                author.clone(),
                hash(&asset_id),
            );
            permissions::Module::<T>::create_permission(
                author.clone(),
                author.clone(),
                TRANSFER,
                permission.clone(),
            )?;
            permissions::Module::<T>::create_permission(
                author.clone(),
                author.clone(),
                MINT,
                permission.clone(),
            )?;
            permissions::Module::<T>::create_permission(
                author.clone(),
                author.clone(),
                BURN,
                permission.clone(),
            )?;
            permissions::Module::<T>::create_permission(
                author.clone(),
                author.clone(),
                SLASH,
                permission.clone(),
            )?;
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

    pub fn ensure_asset_exists(asset_id: &T::AssetId) -> Result<(), Error<T>> {
        if !Self::asset_exists(asset_id) {
            return Err(Error::<T>::AssetIdNotExists);
        }
        Ok(())
    }

    #[inline]
    pub fn is_asset_owner(asset_id: &T::AssetId, account_id: &T::AccountId) -> bool {
        Self::asset_owner(asset_id)
            .map(|x| &x == account_id)
            .unwrap_or(false)
    }

    pub fn total_issuance(asset_id: &T::AssetId) -> Result<BalanceOf<T>, Error<T>> {
        Self::ensure_asset_exists(asset_id)?;
        let currency_id: CurrencyIdOf<T> = (*asset_id).into();
        Ok(currencies::Module::<T>::total_issuance(currency_id))
    }

    pub fn total_balance(
        asset_id: &T::AssetId,
        who: &T::AccountId,
    ) -> Result<BalanceOf<T>, Error<T>> {
        Self::ensure_asset_exists(asset_id)?;
        Ok(currencies::Module::<T>::total_balance(
            (*asset_id).into(),
            who,
        ))
    }

    pub fn free_balance(
        asset_id: &T::AssetId,
        who: &T::AccountId,
    ) -> Result<BalanceOf<T>, Error<T>> {
        Self::ensure_asset_exists(asset_id)?;
        Ok(currencies::Module::<T>::free_balance(
            (*asset_id).into(),
            who,
        ))
    }

    pub fn ensure_can_withdraw(
        asset_id: &T::AssetId,
        who: &T::AccountId,
        amount: BalanceOf<T>,
    ) -> DispatchResult {
        Self::ensure_asset_exists(asset_id)?;
        currencies::Module::<T>::ensure_can_withdraw((*asset_id).into(), who, amount)
    }

    pub fn transfer(
        asset_id: &T::AssetId,
        from: &T::AccountId,
        to: &T::AccountId,
        amount: BalanceOf<T>,
    ) -> DispatchResult {
        Self::ensure_asset_exists(asset_id)?;
        permissions::Module::<T>::check_permission_with_parameters(
            from.clone(),
            TRANSFER,
            hash(asset_id),
        )?;
        <currencies::Module<T> as MultiCurrency<_>>::transfer((*asset_id).into(), from, to, amount)
    }

    pub fn mint(asset_id: &T::AssetId, who: &T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
        permissions::Module::<T>::check_permission_with_parameters(
            who.clone(),
            MINT,
            hash(asset_id),
        )?;
        currencies::Module::<T>::deposit((*asset_id).into(), who, amount)
    }

    pub fn burn(asset_id: &T::AssetId, who: &T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
        permissions::Module::<T>::check_permission_with_parameters(
            who.clone(),
            BURN,
            hash(asset_id),
        )?;
        currencies::Module::<T>::withdraw((*asset_id).into(), who, amount)
    }

    pub fn can_slash(
        asset_id: &T::AssetId,
        who: &T::AccountId,
        amount: BalanceOf<T>,
    ) -> Result<bool, Error<T>> {
        Self::ensure_asset_exists(asset_id)?;
        Ok(currencies::Module::<T>::can_slash(
            (*asset_id).into(),
            who,
            amount,
        ))
    }

    pub fn slash(
        asset_id: &T::AssetId,
        who: &T::AccountId,
        amount: BalanceOf<T>,
    ) -> Result<BalanceOf<T>, DispatchError> {
        Self::ensure_asset_exists(asset_id)?;
        permissions::Module::<T>::check_permission_with_parameters(
            who.clone(),
            SLASH,
            hash(asset_id),
        )?;
        Ok(currencies::Module::<T>::slash(
            (*asset_id).into(),
            who,
            amount,
        ))
    }

    pub fn update_balance(
        asset_id: &T::AssetId,
        who: &T::AccountId,
        by_amount: AmountOf<T>,
    ) -> DispatchResult {
        permissions::Module::<T>::check_permission_with_parameters(
            who.clone(),
            MINT,
            hash(asset_id),
        )?;
        permissions::Module::<T>::check_permission_with_parameters(
            who.clone(),
            BURN,
            hash(asset_id),
        )?;
        <currencies::Module<T> as MultiCurrencyExtended<_>>::update_balance(
            (*asset_id).into(),
            who,
            by_amount,
        )
    }
}
