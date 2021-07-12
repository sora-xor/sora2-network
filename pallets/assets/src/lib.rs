// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

//! # Assets Pallet
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

pub mod weights;

mod benchmarking;
mod migration;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use codec::{Decode, Encode};
use common::prelude::Balance;
use common::{hash, Amount, AssetName, AssetSymbol, BalancePrecision, DEFAULT_BALANCE_PRECISION};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::sp_runtime::traits::{MaybeSerializeDeserialize, Member};
use frame_support::traits::Get;
use frame_support::weights::Weight;
use frame_support::{ensure, Parameter};
use frame_system::ensure_signed;
use permissions::{Scope, BURN, MINT};
use sp_core::hash::H512;
use sp_core::H256;
use sp_runtime::traits::Zero;
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
    fn set_non_mintable() -> Weight;
}

pub type AssetIdOf<T> = <T as Config>::AssetId;
pub type Permissions<T> = permissions::Pallet<T>;

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
type CurrencyIdOf<T> =
    <<T as Config>::Currency as MultiCurrency<<T as frame_system::Config>::AccountId>>::CurrencyId;

const ASSET_SYMBOL_MAX_LENGTH: usize = 7;
const ASSET_NAME_MAX_LENGTH: usize = 33;
const MAX_ALLOWED_PRECISION: u8 = 18;

#[derive(Clone, Copy, Eq, PartialEq, Encode, Decode)]
pub enum AssetRecordArg<T: Config> {
    GenericI32(i32),
    GenericU64(u64),
    GenericU128(u128),
    GenericU8x32([u8; 32]),
    GenericH256(H256),
    GenericH512(H512),
    LeafAssetId(AssetIdOf<T>),
    AssetRecordAssetId(AssetIdOf<T>),
    Extra(T::ExtraAssetRecordArg),
}

#[derive(Clone, Copy, Eq, PartialEq, Encode, Decode)]
pub enum AssetRecord<T: Config> {
    Arity0,
    Arity1(AssetRecordArg<T>),
    Arity2(AssetRecordArg<T>, AssetRecordArg<T>),
    Arity3(AssetRecordArg<T>, AssetRecordArg<T>, AssetRecordArg<T>),
    Arity4(
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
    ),
    Arity5(
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
    ),
    Arity6(
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
    ),
    Arity7(
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
    ),
    Arity8(
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
    ),
    Arity9(
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
        AssetRecordArg<T>,
    ),
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + permissions::Config + tokens::Config + common::Config
    {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type ExtraAccountId: Clone
            + Copy
            + Encode
            + Decode
            + Eq
            + PartialEq
            + From<Self::AccountId>
            + Into<Self::AccountId>;
        type ExtraAssetRecordArg: Clone
            + Copy
            + Encode
            + Decode
            + Eq
            + PartialEq
            + From<common::AssetIdExtraAssetRecordArg<Self::DEXId, Self::LstId, Self::ExtraAccountId>>
            + Into<common::AssetIdExtraAssetRecordArg<Self::DEXId, Self::LstId, Self::ExtraAccountId>>;

        /// DEX assets (currency) identifier.
        type AssetId: Parameter
            + Member
            + Copy
            + MaybeSerializeDeserialize
            + Ord
            + Default
            + Into<CurrencyIdOf<Self>>
            + From<common::AssetId32<common::PredefinedAssetId>>
            + From<H256>
            + Into<H256>
            + Into<<Self as tokens::Config>::CurrencyId>;

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

        /// Account dedicated for PSWAP to be distributed among team in future.
        type GetTeamReservesAccountId: Get<Self::AccountId>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_runtime_upgrade() -> Weight {
            migration::migrate::<T>()
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Performs an asset registration.
        ///
        /// Registers new `AssetId` for the given `origin`.
        /// AssetSymbol should represent string with only uppercase latin chars with max length of 7.
        /// AssetName should represent string with only uppercase or lowercase latin chars or numbers or spaces, with max length of 33.
        #[pallet::weight(<T as Config>::WeightInfo::register())]
        pub fn register(
            origin: OriginFor<T>,
            symbol: AssetSymbol,
            name: AssetName,
            initial_supply: Balance,
            is_mintable: bool,
        ) -> DispatchResultWithPostInfo {
            let author = ensure_signed(origin)?;
            let _asset_id = Self::register_from(
                &author,
                symbol,
                name,
                DEFAULT_BALANCE_PRECISION,
                initial_supply,
                is_mintable,
            )?;
            Ok(().into())
        }

        /// Performs a checked Asset transfer.
        ///
        /// - `origin`: caller Account, from which Asset amount is withdrawn,
        /// - `asset_id`: Id of transferred Asset,
        /// - `to`: Id of Account, to which Asset amount is deposited,
        /// - `amount`: transferred Asset amount.
        #[pallet::weight(<T as Config>::WeightInfo::transfer())]
        pub fn transfer(
            origin: OriginFor<T>,
            asset_id: T::AssetId,
            to: T::AccountId,
            amount: Balance,
        ) -> DispatchResultWithPostInfo {
            let from = ensure_signed(origin.clone())?;
            Self::transfer_from(&asset_id, &from, &to, amount)?;
            Self::deposit_event(Event::Transfer(from, to, asset_id, amount));
            Ok(().into())
        }

        /// Performs a checked Asset mint, can only be done
        /// by corresponding asset owner account.
        ///
        /// - `origin`: caller Account, which issues Asset minting,
        /// - `asset_id`: Id of minted Asset,
        /// - `to`: Id of Account, to which Asset amount is minted,
        /// - `amount`: minted Asset amount.
        #[pallet::weight(<T as Config>::WeightInfo::mint())]
        pub fn mint(
            origin: OriginFor<T>,
            asset_id: T::AssetId,
            to: T::AccountId,
            amount: Balance,
        ) -> DispatchResultWithPostInfo {
            let issuer = ensure_signed(origin.clone())?;
            Self::mint_to(&asset_id, &issuer, &to, amount)?;
            Self::deposit_event(Event::Mint(issuer, to, asset_id.clone(), amount));
            Ok(().into())
        }

        /// Performs a checked Asset burn, can only be done
        /// by corresponding asset owner with own account.
        ///
        /// - `origin`: caller Account, from which Asset amount is burned,
        /// - `asset_id`: Id of burned Asset,
        /// - `amount`: burned Asset amount.
        #[pallet::weight(<T as Config>::WeightInfo::burn())]
        pub fn burn(
            origin: OriginFor<T>,
            asset_id: T::AssetId,
            amount: Balance,
        ) -> DispatchResultWithPostInfo {
            let issuer = ensure_signed(origin.clone())?;
            Self::burn_from(&asset_id, &issuer, &issuer, amount)?;
            Self::deposit_event(Event::Burn(issuer, asset_id.clone(), amount));
            Ok(().into())
        }

        /// Set given asset to be non-mintable, i.e. it can no longer be minted, only burned.
        /// Operation can not be undone.
        ///
        /// - `origin`: caller Account, should correspond to Asset owner
        /// - `asset_id`: Id of burned Asset,
        #[pallet::weight(<T as Config>::WeightInfo::set_non_mintable())]
        pub fn set_non_mintable(
            origin: OriginFor<T>,
            asset_id: T::AssetId,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin.clone())?;
            Self::set_non_mintable_from(&asset_id, &who)?;
            Self::deposit_event(Event::AssetSetNonMintable(asset_id.clone()));
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId", AssetIdOf<T> = "AssetId")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// New asset has been registered. [Asset Id, Asset Owner Account]
        AssetRegistered(AssetIdOf<T>, AccountIdOf<T>),
        /// Asset amount has been transfered. [From Account, To Account, Tranferred Asset Id, Amount Transferred]
        Transfer(AccountIdOf<T>, AccountIdOf<T>, AssetIdOf<T>, Balance),
        /// Asset amount has been minted. [Issuer Account, Target Account, Minted Asset Id, Amount Minted]
        Mint(AccountIdOf<T>, AccountIdOf<T>, AssetIdOf<T>, Balance),
        /// Asset amount has been burned. [Issuer Account, Burned Asset Id, Amount Burned]
        Burn(AccountIdOf<T>, AssetIdOf<T>, Balance),
        /// Asset is set as non-mintable. [Target Asset Id]
        AssetSetNonMintable(AssetIdOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// An asset with a given ID already exists.
        AssetIdAlreadyExists,
        /// An asset with a given ID not exists.
        AssetIdNotExists,
        /// A number is out of range of the balance type.
        InsufficientBalance,
        /// Symbol is not valid. It must contain only uppercase latin characters or numbers, length <= 7.
        InvalidAssetSymbol,
        /// Name is not valid. It must contain only uppercase or lowercase latin characters or numbers or spaces, length <= 33.
        InvalidAssetName,
        /// Precision value is not valid, it should represent a number of decimal places for number, max is 30.
        InvalidPrecision,
        /// Minting for particular asset id is disabled.
        AssetSupplyIsNotMintable,
        /// Caller does not own requested asset.
        InvalidAssetOwner,
        /// Increment account reference error.
        IncRefError,
    }

    /// Asset Id -> Owner Account Id
    #[pallet::storage]
    #[pallet::getter(fn asset_owner)]
    pub(super) type AssetOwners<T: Config> =
        StorageMap<_, Twox64Concat, T::AssetId, T::AccountId, OptionQuery>;

    /// Asset Id -> (Symbol, Precision, Is Mintable)
    #[pallet::storage]
    #[pallet::getter(fn asset_infos)]
    pub type AssetInfos<T: Config> = StorageMap<
        _,
        Twox64Concat,
        T::AssetId,
        (AssetSymbol, AssetName, BalancePrecision, bool),
        ValueQuery,
    >;

    /// Asset Id -> AssetRecord<T>
    #[pallet::storage]
    #[pallet::getter(fn tuple_from_asset_id)]
    pub type AssetRecordAssetId<T: Config> =
        StorageMap<_, Twox64Concat, T::AssetId, AssetRecord<T>>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub endowed_assets: Vec<(
            T::AssetId,
            T::AccountId,
            AssetSymbol,
            AssetName,
            BalancePrecision,
            Balance,
            bool,
        )>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                endowed_assets: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            self.endowed_assets.iter().cloned().for_each(
                |(asset_id, account_id, symbol, name, precision, initial_supply, is_mintable)| {
                    Pallet::<T>::register_asset_id(
                        account_id,
                        asset_id,
                        symbol,
                        name,
                        precision,
                        initial_supply,
                        is_mintable,
                    )
                    .expect("Failed to register asset.");
                },
            )
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Generates an `AssetId` for the given `AssetRecord<T>`, and insert record to storage map.
    pub fn register_asset_id_from_tuple(tuple: &AssetRecord<T>) -> T::AssetId {
        let mut keccak = Keccak::v256();
        keccak.update(b"From AssetRecord");
        keccak.update(&tuple.encode());
        let mut output = [0u8; 32];
        keccak.finalize(&mut output);
        // More safe to escape.
        output[0] = 0u8;
        let asset_id = T::AssetId::from(H256(output));
        AssetRecordAssetId::<T>::insert(asset_id, tuple);
        asset_id
    }

    /// Generates an `AssetId` from an `Encode` value.
    pub fn gen_asset_id_from_any(value: &impl Encode) -> T::AssetId {
        let mut keccak = Keccak::v256();
        keccak.update(b"Sora Asset Id Any");
        keccak.update(&value.encode());
        let mut output = [0u8; 32];
        keccak.finalize(&mut output);
        // More safe to escape.
        output[0] = 0u8;
        T::AssetId::from(H256(output))
    }

    /// Generates an `AssetId` for the given `AccountId`.
    pub fn gen_asset_id(account_id: &T::AccountId) -> T::AssetId {
        let mut keccak = Keccak::v256();
        keccak.update(b"Sora Asset Id");
        keccak.update(&account_id.encode());
        keccak.update(&frame_system::Pallet::<T>::account_nonce(&account_id).encode());
        let mut output = [0u8; 32];
        keccak.finalize(&mut output);
        // More safe to escape.
        output[0] = 0u8;
        T::AssetId::from(H256(output))
    }

    /// Register the given `AssetId`.
    pub fn register_asset_id(
        account_id: T::AccountId,
        asset_id: T::AssetId,
        symbol: AssetSymbol,
        name: AssetName,
        precision: BalancePrecision,
        initial_supply: Balance,
        is_mintable: bool,
    ) -> DispatchResult {
        ensure!(
            precision <= MAX_ALLOWED_PRECISION,
            Error::<T>::InvalidPrecision
        );
        ensure!(
            crate::is_symbol_valid(&symbol),
            Error::<T>::InvalidAssetSymbol
        );
        ensure!(crate::is_name_valid(&name), Error::<T>::InvalidAssetName);
        ensure!(
            !Self::asset_exists(&asset_id),
            Error::<T>::AssetIdAlreadyExists
        );
        frame_system::Pallet::<T>::inc_consumers(&account_id)
            .map_err(|_| Error::<T>::IncRefError)?;
        AssetOwners::<T>::insert(asset_id, account_id.clone());
        AssetInfos::<T>::insert(asset_id, (symbol, name, precision, is_mintable));
        let scope = Scope::Limited(hash(&asset_id));
        let permission_ids = [MINT, BURN];
        for permission_id in &permission_ids {
            Permissions::<T>::assign_permission(
                account_id.clone(),
                &account_id,
                *permission_id,
                scope,
            )?;
        }
        if !initial_supply.is_zero() {
            T::Currency::deposit(asset_id.clone(), &account_id, initial_supply)?;
        }
        frame_system::Pallet::<T>::inc_account_nonce(&account_id);
        Self::deposit_event(Event::AssetRegistered(asset_id, account_id));
        Ok(())
    }

    /// Generates new `AssetId` and registers it from the `account_id`.
    pub fn register_from(
        account_id: &T::AccountId,
        symbol: AssetSymbol,
        name: AssetName,
        precision: BalancePrecision,
        initial_supply: Balance,
        is_mintable: bool,
    ) -> Result<T::AssetId, DispatchError> {
        common::with_transaction(|| {
            let asset_id = Self::gen_asset_id(account_id);
            Self::register_asset_id(
                account_id.clone(),
                asset_id,
                symbol,
                name,
                precision,
                initial_supply,
                is_mintable,
            )?;
            Ok(asset_id)
        })
    }

    #[inline]
    pub fn asset_exists(asset_id: &T::AssetId) -> bool {
        AssetOwners::<T>::contains_key(asset_id)
    }

    #[inline]
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
        .or_else(|_| {
            Permissions::<T>::check_permission_with_scope(
                issuer.clone(),
                permission_id,
                &Scope::Unlimited,
            )
        })?;
        Ok(())
    }

    pub fn total_issuance(asset_id: &T::AssetId) -> Result<Balance, DispatchError> {
        let r = T::Currency::total_issuance(asset_id.clone());
        if r == Default::default() {
            Self::ensure_asset_exists(asset_id)?;
        }
        Ok(r)
    }

    pub fn total_balance(
        asset_id: &T::AssetId,
        who: &T::AccountId,
    ) -> Result<Balance, DispatchError> {
        let r = T::Currency::total_balance(asset_id.clone(), who);
        if r == Default::default() {
            Self::ensure_asset_exists(asset_id)?;
        }
        Ok(r)
    }

    pub fn free_balance(
        asset_id: &T::AssetId,
        who: &T::AccountId,
    ) -> Result<Balance, DispatchError> {
        let r = T::Currency::free_balance(asset_id.clone(), who);
        if r == Default::default() {
            Self::ensure_asset_exists(asset_id)?;
        }
        Ok(r)
    }

    pub fn ensure_can_withdraw(
        asset_id: &T::AssetId,
        who: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult {
        let r = T::Currency::ensure_can_withdraw(asset_id.clone(), who, amount);
        if r.is_err() {
            Self::ensure_asset_exists(asset_id)?;
        }
        r
    }

    pub fn transfer_from(
        asset_id: &T::AssetId,
        from: &T::AccountId,
        to: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult {
        let r = T::Currency::transfer(asset_id.clone(), from, to, amount);
        if r.is_err() {
            Self::ensure_asset_exists(asset_id)?;
        }
        r
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
        let (_, _, _, is_mintable) = AssetInfos::<T>::get(asset_id);
        ensure!(is_mintable, Error::<T>::AssetSupplyIsNotMintable);
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
        // Holder can burn its funds.
        if issuer != to {
            Self::check_permission_maybe_with_parameters(issuer, BURN, asset_id)?;
        }
        T::Currency::withdraw(*asset_id, to, amount)
    }

    pub fn update_balance(
        asset_id: &T::AssetId,
        who: &T::AccountId,
        by_amount: Amount,
    ) -> DispatchResult {
        Self::check_permission_maybe_with_parameters(who, MINT, asset_id)?;
        Self::check_permission_maybe_with_parameters(who, BURN, asset_id)?;
        if by_amount.is_positive() {
            let (_, _, _, is_mintable) = AssetInfos::<T>::get(asset_id);
            ensure!(is_mintable, Error::<T>::AssetSupplyIsNotMintable);
        }
        T::Currency::update_balance(asset_id.clone(), who, by_amount)
    }

    pub fn can_reserve(asset_id: T::AssetId, who: &T::AccountId, amount: Balance) -> bool {
        T::Currency::can_reserve(asset_id, who, amount)
    }

    pub fn reserve(
        asset_id: T::AssetId,
        who: &T::AccountId,
        amount: Balance,
    ) -> Result<(), DispatchError> {
        let r = T::Currency::reserve(asset_id, who, amount);
        if r.is_err() {
            Self::ensure_asset_exists(&asset_id)?;
        }
        r
    }

    pub fn unreserve(
        asset_id: T::AssetId,
        who: &T::AccountId,
        amount: Balance,
    ) -> Result<Balance, DispatchError> {
        let amount = T::Currency::unreserve(asset_id, who, amount);
        if amount != Default::default() {
            Self::ensure_asset_exists(&asset_id)?;
        }
        Ok(amount)
    }

    pub fn set_non_mintable_from(asset_id: &T::AssetId, who: &T::AccountId) -> DispatchResult {
        ensure!(
            Self::is_asset_owner(asset_id, who),
            Error::<T>::InvalidAssetOwner
        );
        AssetInfos::<T>::mutate(asset_id, |(_, _, _, ref mut is_mintable)| {
            ensure!(*is_mintable, Error::<T>::AssetSupplyIsNotMintable);
            *is_mintable = false;
            Ok(())
        })
    }

    pub fn list_registered_asset_ids() -> Vec<T::AssetId> {
        AssetInfos::<T>::iter().map(|(key, _)| key).collect()
    }

    pub fn list_registered_asset_infos(
    ) -> Vec<(T::AssetId, AssetSymbol, AssetName, BalancePrecision, bool)> {
        AssetInfos::<T>::iter()
            .map(|(key, (symbol, name, precision, is_mintable))| {
                (key, symbol, name, precision, is_mintable)
            })
            .collect()
    }

    pub fn get_asset_info(
        asset_id: &T::AssetId,
    ) -> (AssetSymbol, AssetName, BalancePrecision, bool) {
        AssetInfos::<T>::get(asset_id)
    }
}

/// According to UTF-8 encoding, graphemes that start with byte 0b0XXXXXXX belong
/// to ASCII range and are of single byte, therefore passing check in range 'A' to 'Z'
/// and '0' to '9' guarantees that all graphemes are of length 1, therefore length check is valid.
pub fn is_symbol_valid(symbol: &AssetSymbol) -> bool {
    symbol.0.len() <= ASSET_SYMBOL_MAX_LENGTH
        && symbol
            .0
            .iter()
            .all(|byte| (b'A'..=b'Z').contains(&byte) || (b'0'..=b'9').contains(&byte))
}

/// According to UTF-8 encoding, graphemes that start with byte 0b0XXXXXXX belong
/// to ASCII range and are of single byte, therefore passing check in range 'A' to 'z'
/// guarantees that all graphemes are of length 1, therefore length check is valid.
pub fn is_name_valid(name: &AssetName) -> bool {
    name.0.len() <= ASSET_NAME_MAX_LENGTH
        && name.0.iter().all(|byte| {
            (b'A'..=b'Z').contains(&byte)
                || (b'a'..=b'z').contains(&byte)
                || (b'0'..=b'9').contains(&byte)
                || byte == &b' '
        })
}
