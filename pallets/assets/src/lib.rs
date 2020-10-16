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

// TODO: add info about weight

#![cfg_attr(not(feature = "std"), no_std)]

#[allow(unused_imports)]
#[macro_use]
extern crate alloc;

mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use codec::Encode;
use common::{hash, prelude::Balance, Amount, AssetSymbol, BalancePrecision};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::sp_runtime::traits::{MaybeSerializeDeserialize, Member};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get, weights::Weight,
    IterableStorageMap, Parameter,
};
use frame_system::ensure_signed;
use permissions::{Scope, BURN, MINT, SLASH, TRANSFER};
use sp_core::H256;
use sp_std::vec::Vec;
use tiny_keccak::{Hasher, Keccak};
use traits::{
    MultiCurrency, MultiCurrencyExtended, MultiLockableCurrency, MultiReservableCurrency,
};

pub trait WeightInfo {
    fn register() -> Weight;
    fn transfer() -> Weight;
    fn mint() -> Weight;
    fn burn() -> Weight;
}

pub type AssetIdOf<T> = <T as Trait>::AssetId;
pub type Permissions<T> = permissions::Module<T>;

type CurrencyIdOf<T> =
    <<T as Trait>::Currency as MultiCurrency<<T as frame_system::Trait>::AccountId>>::CurrencyId;

const ASSET_SYMBOL_MAX_LENGTH: usize = 7;

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
        + From<common::AssetId32<common::AssetId>>
        + From<H256>
        + Into<H256>;

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

    /// Weight information for extrinsics in this pallet.
    type WeightInfo: WeightInfo;
}

decl_storage! {
    trait Store for Module<T: Trait> as AssetsModule {
        AssetOwners get(fn asset_owners): map hasher(twox_64_concat) T::AssetId => T::AccountId;
        pub AssetInfos get(fn asset_infos): map hasher(twox_64_concat) T::AssetId => (AssetSymbol, BalancePrecision);
    }
    add_extra_genesis {
        config(endowed_assets): Vec<(T::AssetId, T::AccountId, AssetSymbol, BalancePrecision)>;

        build(|config: &GenesisConfig<T>| {
            config.endowed_assets.iter().for_each(|(asset_id, account_id, symbol, precision)| {
                Module::<T>::register_asset_id(account_id.clone(), *asset_id, symbol.clone(), precision.clone())
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
        /// New asset has been registered. [Asset Id, Asset Owner Account]
        AssetRegistered(AssetId, AccountId),
        /// Asset amount has been transfered. [From Account, To Account, Tranferred Asset Id, Amount Transferred]
        Transfer(AccountId, AccountId, AssetId, Balance),
        /// Asset amount has been minted. [Issuer Account, Target Account, Minted Asset Id, Amount Minted]
        Mint(AccountId, AccountId, AssetId, Balance),
        /// Asset amount has been burned. [Issuer Account, Burned Asset Id, Amount Burned]
        Burn(AccountId, AssetId, Balance),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// An asset with a given ID already exists.
        AssetIdAlreadyExists,
        /// An asset with a given ID not exists.
        AssetIdNotExists,
        /// A number is out of range of the balance type.
        InsufficientBalance,
        /// Symbol is not valid. It must contain only uppercase latin characters, length <= 5.
        InvalidAssetSymbol,
        /// Precision value is not valid, it should represent a number of decimal places for number, max is 30.
        InvalidPrecision,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        /// Performs an asset registration.
        ///
        /// Registers new `AssetId` for the given `origin`.
        /// AssetSymbol should represent string with only uppercase latin chars with max length of 5.
        #[weight = <T as Trait>::WeightInfo::register()]
        pub fn register(origin, symbol: AssetSymbol, precision: BalancePrecision) -> DispatchResult {
            let author = ensure_signed(origin)?;
            let _asset_id = Self::register_from(&author, symbol, precision)?;
            Ok(())
        }

        /// Performs a checked Asset transfer.
        ///
        /// - `origin`: caller Account, from which Asset amount is withdrawn,
        /// - `asset_id`: Id of transferred Asset,
        /// - `to`: Id of Account, to which Asset amount is deposited,
        /// - `amount`: transferred Asset amount.
        #[weight = <T as Trait>::WeightInfo::transfer()]
        pub fn transfer(
            origin,
            asset_id: T::AssetId,
            to: T::AccountId,
            amount: Balance
        ) -> DispatchResult {
            let from = ensure_signed(origin.clone())?;
            Self::transfer_from(&asset_id, &from, &to, amount)?;
            Self::deposit_event(RawEvent::Transfer(from, to, asset_id, amount));
            Ok(())
        }

        /// Performs a checked Asset mint, can only be done
        /// by corresponding asset owner account.
        ///
        /// - `origin`: caller Account, which issues Asset minting,
        /// - `asset_id`: Id of minted Asset,
        /// - `to`: Id of Account, to which Asset amount is minted,
        /// - `amount`: minted Asset amount.
        #[weight = <T as Trait>::WeightInfo::mint()]
        pub fn mint(
            origin,
            asset_id: T::AssetId,
            to: T::AccountId,
            amount: Balance,
        ) -> DispatchResult {
            let issuer = ensure_signed(origin.clone())?;
            Self::mint_to(&asset_id, &issuer, &to, amount)?;
            Self::deposit_event(RawEvent::Mint(issuer, to, asset_id.clone(), amount));
            Ok(())
        }

        /// Performs a checked Asset burn, can only be done
        /// by corresponding asset owner with own account.
        ///
        /// - `origin`: caller Account, from which Asset amount is burned,
        /// - `asset_id`: Id of burned Asset,
        /// - `amount`: burned Asset amount.
        #[weight = <T as Trait>::WeightInfo::burn()]
        pub fn burn(
            origin,
            asset_id: T::AssetId,
            amount: Balance,
        ) -> DispatchResult {
            let issuer = ensure_signed(origin.clone())?;
            Self::burn_from(&asset_id, &issuer, &issuer, amount)?;
            Self::deposit_event(RawEvent::Burn(issuer, asset_id.clone(), amount));
            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    /// Generates an `AssetId` for the given `AccountId`.
    pub fn gen_asset_id(account_id: &T::AccountId) -> T::AssetId {
        let mut keccak = Keccak::v256();
        keccak.update(b"Sora Asset Id");
        keccak.update(&account_id.encode());
        keccak.update(&frame_system::Module::<T>::account_nonce(&account_id).encode());
        let mut output = [0u8; 32];
        keccak.finalize(&mut output);
        T::AssetId::from(H256(output))
    }

    /// Register the given `AssetId`.
    pub fn register_asset_id(
        account_id: T::AccountId,
        asset_id: T::AssetId,
        symbol: AssetSymbol,
        precision: BalancePrecision,
    ) -> DispatchResult {
        ensure!(
            Self::asset_owner(&asset_id).is_none(),
            Error::<T>::AssetIdAlreadyExists
        );
        AssetOwners::<T>::insert(asset_id, account_id.clone());
        ensure!(
            Self::is_symbol_valid(&symbol),
            Error::<T>::InvalidAssetSymbol
        );
        AssetInfos::<T>::insert(asset_id, (symbol, precision));
        ensure!(precision <= 30u8, Error::<T>::InvalidPrecision);
        let scope = Scope::Limited(hash(&asset_id));
        let permission_ids = [TRANSFER, MINT, BURN, SLASH];
        for permission_id in &permission_ids {
            Permissions::<T>::assign_permission(
                account_id.clone(),
                &account_id,
                *permission_id,
                scope,
            )?;
        }
        frame_system::Module::<T>::inc_account_nonce(&account_id);
        Self::deposit_event(RawEvent::AssetRegistered(asset_id, account_id));
        Ok(())
    }

    /// Generates new `AssetId` and registers it from the `account_id`.
    pub fn register_from(
        account_id: &T::AccountId,
        symbol: AssetSymbol,
        precision: BalancePrecision,
    ) -> Result<T::AssetId, DispatchError> {
        let asset_id = Self::gen_asset_id(account_id);
        Self::register_asset_id(account_id.clone(), asset_id, symbol, precision)?;
        Ok(asset_id)
    }

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
            &Scope::Unlimited,
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

    pub fn transfer_from(
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

    pub fn mint_to(
        asset_id: &T::AssetId,
        issuer: &T::AccountId,
        to: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult {
        Self::ensure_asset_exists(asset_id)?;
        Self::check_permission_maybe_with_parameters(issuer, MINT, asset_id)?;
        T::Currency::deposit(asset_id.clone(), to, amount)
    }

    pub fn burn_from(
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

    pub fn reserve(
        asset_id: T::AssetId,
        who: &T::AccountId,
        amount: Balance,
    ) -> Result<(), DispatchError> {
        Self::ensure_asset_exists(&asset_id)?;
        T::Currency::reserve(asset_id, who, amount)
    }

    pub fn unreserve(
        asset_id: T::AssetId,
        who: &T::AccountId,
        amount: Balance,
    ) -> Result<(), DispatchError> {
        Self::ensure_asset_exists(&asset_id)?;
        let _ = T::Currency::unreserve(asset_id, who, amount);
        Ok(())
    }

    pub fn list_registered_asset_ids() -> Vec<T::AssetId> {
        AssetInfos::<T>::iter().map(|(key, _)| key).collect()
    }

    pub fn list_registered_asset_infos() -> Vec<(T::AssetId, AssetSymbol, BalancePrecision)> {
        AssetInfos::<T>::iter()
            .map(|(key, (symbol, precision))| (key, symbol, precision))
            .collect()
    }

    pub fn get_asset_info(asset_id: &T::AssetId) -> (AssetSymbol, BalancePrecision) {
        AssetInfos::<T>::get(asset_id)
    }

    /// According to UTF-8 encoding, graphemes that start with byte 0b0XXXXXXX belong
    /// to ASCII range and are of single byte, therefore passing check in range 'A' to 'Z'
    /// guarantees that all graphemes are of length 1, therefore length check is valid.
    fn is_symbol_valid(symbol: &AssetSymbol) -> bool {
        symbol.0.len() <= ASSET_SYMBOL_MAX_LENGTH
            && symbol.0.iter().all(|byte| (b'A'..=b'Z').contains(&byte))
    }
}
