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

#![cfg_attr(not(feature = "std"), no_std)]

pub mod weights;

mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use codec::{Decode, Encode};
use common::prelude::{Balance, SwapAmount};
use common::{
    hash, Amount, AssetIdOf, AssetInfoProvider, AssetName, AssetSymbol, BalancePrecision,
    ContentSource, Description, IsValid, LiquidityProxyTrait, LiquiditySourceFilter,
    DEFAULT_BALANCE_PRECISION,
};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::ensure;
use frame_support::traits::Get;
use frame_system::ensure_signed;
use permissions::{Scope, BURN, MINT};
use sp_core::hash::H512;
use sp_core::H256;
use sp_runtime::traits::Zero;
use sp_std::vec::Vec;
use tiny_keccak::{Hasher, Keccak};
use traits::{MultiCurrency, MultiCurrencyExtended, MultiReservableCurrency};
pub use weights::WeightInfo;

pub type Permissions<T> = permissions::Pallet<T>;

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

const MAX_ALLOWED_PRECISION: u8 = 18;

#[derive(Clone, Copy, Eq, PartialEq, Encode, Decode, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
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

#[derive(Clone, Copy, Eq, PartialEq, Encode, Decode, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
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

pub trait GetTotalBalance<T: Config> {
    fn total_balance(asset_id: &T::AssetId, who: &T::AccountId) -> Result<Balance, DispatchError>;
}

impl<T: Config> GetTotalBalance<T> for () {
    fn total_balance(
        _asset_id: &T::AssetId,
        _who: &T::AccountId,
    ) -> Result<Balance, DispatchError> {
        Ok(0)
    }
}

pub use pallet::*;

#[frame_support::pallet]
#[allow(clippy::too_many_arguments)]
pub mod pallet {
    use super::*;
    use common::{AmountOf, ContentSource, CurrencyIdOf, Description};
    use frame_support::pallet_prelude::*;
    use frame_system::{ensure_root, pallet_prelude::*};

    #[pallet::config]
    pub trait Config: frame_system::Config + permissions::Config + common::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type ExtraAccountId: Clone
            + Copy
            + Encode
            + Decode
            + scale_info::TypeInfo
            + Eq
            + PartialEq
            + From<Self::AccountId>
            + Into<Self::AccountId>;
        type ExtraAssetRecordArg: Clone
            + Copy
            + Encode
            + Decode
            + scale_info::TypeInfo
            + Eq
            + PartialEq
            + From<common::AssetIdExtraAssetRecordArg<Self::DEXId, Self::LstId, Self::ExtraAccountId>>
            + Into<common::AssetIdExtraAssetRecordArg<Self::DEXId, Self::LstId, Self::ExtraAccountId>>;

        /// The base asset as the core asset in all trading pairs
        type GetBaseAssetId: Get<Self::AssetId>;

        /// Assets that will be buy-backed and burned for every [`GetBuyBackPercentage`] of [`GetBuyBackSupplyAssets`] mints
        type GetBuyBackAssetId: Get<Self::AssetId>;

        /// Assets, [`GetBuyBackPercentage`] of minted amount of which will be used to buy-back and burn [`GetBuyBackAssetId`]
        type GetBuyBackSupplyAssets: Get<Vec<Self::AssetId>>;

        /// The percentage of minted [`GetBuyBackSupplyAssets`] that will be used to buy-back and burn [`GetBuyBackAssetId`]
        type GetBuyBackPercentage: Get<u8>;

        /// Account which will be used to buy-back and burn [`GetBuyBackAssetId`]
        type GetBuyBackAccountId: Get<Self::AccountId>;

        /// DEX id to buy-back and burn [`GetBuyBackAssetId`] through [`BuyBackLiquidityProxy`]
        type GetBuyBackDexId: Get<Self::DEXId>;

        /// Liquidity proxy to perform [`GetBuyBackAssetId`] buy-back and burn
        type BuyBackLiquidityProxy: LiquidityProxyTrait<Self::DEXId, Self::AccountId, Self::AssetId>;

        /// Get the balance from other components
        type GetTotalBalance: GetTotalBalance<Self>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Performs an asset registration.
        ///
        /// Registers new `AssetId` for the given `origin`.
        /// AssetSymbol should represent string with only uppercase latin chars with max length of 7.
        /// AssetName should represent string with only uppercase or lowercase latin chars or numbers or spaces, with max length of 33.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::register())]
        pub fn register(
            origin: OriginFor<T>,
            symbol: AssetSymbol,
            name: AssetName,
            initial_supply: Balance,
            is_mintable: bool,
            is_indivisible: bool,
            opt_content_src: Option<ContentSource>,
            opt_desc: Option<Description>,
        ) -> DispatchResultWithPostInfo {
            let author = ensure_signed(origin)?;
            let precision = if is_indivisible {
                0
            } else {
                DEFAULT_BALANCE_PRECISION
            };

            Self::register_from(
                &author,
                symbol,
                name,
                precision,
                initial_supply,
                is_mintable,
                opt_content_src,
                opt_desc,
            )?;

            Ok(().into())
        }

        /// Performs a checked Asset transfer.
        ///
        /// - `origin`: caller Account, from which Asset amount is withdrawn,
        /// - `asset_id`: Id of transferred Asset,
        /// - `to`: Id of Account, to which Asset amount is deposited,
        /// - `amount`: transferred Asset amount.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::transfer())]
        pub fn transfer(
            origin: OriginFor<T>,
            asset_id: T::AssetId,
            to: T::AccountId,
            amount: Balance,
        ) -> DispatchResultWithPostInfo {
            let from = ensure_signed(origin.clone())?;
            Self::transfer_from(&asset_id, &from, &to, amount)?;
            Ok(().into())
        }

        /// Performs a checked Asset mint, can only be done
        /// by corresponding asset owner account.
        ///
        /// - `origin`: caller Account, which issues Asset minting,
        /// - `asset_id`: Id of minted Asset,
        /// - `to`: Id of Account, to which Asset amount is minted,
        /// - `amount`: minted Asset amount.
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::mint())]
        pub fn mint(
            origin: OriginFor<T>,
            asset_id: T::AssetId,
            to: T::AccountId,
            amount: Balance,
        ) -> DispatchResultWithPostInfo {
            let issuer = ensure_signed(origin.clone())?;

            Self::mint_to(&asset_id, &issuer, &to, amount)?;
            Self::deposit_event(Event::Mint(issuer, to, asset_id, amount));
            Ok(().into())
        }

        /// Performs an unchecked Asset mint, can only be done
        /// by root account.
        ///
        /// Should be used as extrinsic call only.
        /// `Currencies::updated_balance()` should be deprecated. Using `force_mint` allows us to
        /// perform extra actions for minting, such as buy-back, extra-minting and etc.
        ///
        /// - `origin`: caller Account, which issues Asset minting,
        /// - `asset_id`: Id of minted Asset,
        /// - `to`: Id of Account, to which Asset amount is minted,
        /// - `amount`: minted Asset amount.
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::force_mint())]
        pub fn force_mint(
            origin: OriginFor<T>,
            asset_id: T::AssetId,
            to: T::AccountId,
            amount: Balance,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin.clone())?;

            let amount_to_distribute = {
                if T::GetBuyBackSupplyAssets::get().contains(&asset_id) {
                    let amount_to_buy_back = amount
                        .checked_mul(T::GetBuyBackPercentage::get() as Balance)
                        .ok_or(Error::<T>::Overflow)?
                        .checked_div(100)
                        .expect("Non-zero division should never fail");
                    Self::buy_back_and_burn(&asset_id, amount_to_buy_back)?;
                    Result::<_, DispatchError>::Ok(amount - amount_to_buy_back)
                } else {
                    Ok(amount)
                }
            }?;

            Self::mint_unchecked(&asset_id, &to, amount_to_distribute)?;
            Ok(().into())
        }

        /// Performs a checked Asset burn, can only be done
        /// by corresponding asset owner with own account.
        ///
        /// - `origin`: caller Account, from which Asset amount is burned,
        /// - `asset_id`: Id of burned Asset,
        /// - `amount`: burned Asset amount.
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::burn())]
        pub fn burn(
            origin: OriginFor<T>,
            asset_id: T::AssetId,
            amount: Balance,
        ) -> DispatchResultWithPostInfo {
            let issuer = ensure_signed(origin.clone())?;
            Self::burn_from(&asset_id, &issuer, &issuer, amount)?;
            Self::deposit_event(Event::Burn(issuer, asset_id, amount));
            Ok(().into())
        }

        /// Add or remove abs(`by_amount`) from the balance of `who` under
        /// `currency_id`. If positive `by_amount`, do add, else do remove.
        ///
        /// Basically a wrapper of `MultiCurrencyExtended::update_balance`
        /// for testing purposes.
        ///
        /// TODO: move into tests extrinsic collection pallet
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::update_balance())]
        pub fn update_balance(
            origin: OriginFor<T>,
            who: T::AccountId,
            currency_id: CurrencyIdOf<T>,
            amount: AmountOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            T::Currency::update_balance(currency_id, &who, amount)
        }

        /// Set given asset to be non-mintable, i.e. it can no longer be minted, only burned.
        /// Operation can not be undone.
        ///
        /// - `origin`: caller Account, should correspond to Asset owner
        /// - `asset_id`: Id of burned Asset,
        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::set_non_mintable())]
        pub fn set_non_mintable(
            origin: OriginFor<T>,
            asset_id: T::AssetId,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin.clone())?;
            Self::set_non_mintable_from(&asset_id, &who)?;
            Self::deposit_event(Event::AssetSetNonMintable(asset_id));
            Ok(().into())
        }

        /// Change information about asset. Can only be done by root
        ///
        /// - `origin`: caller Account, should be root
        /// - `asset_id`: Id of asset to change,
        /// - `new_symbol`: New asset symbol. If None asset symbol will not change
        /// - `new_name`: New asset name. If None asset name will not change
        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::update_info())]
        pub fn update_info(
            origin: OriginFor<T>,
            asset_id: T::AssetId,
            new_symbol: Option<AssetSymbol>,
            new_name: Option<AssetName>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            Self::ensure_asset_exists(&asset_id)?;
            AssetInfos::<T>::mutate(asset_id, |(ref mut symbol, ref mut name, ..)| {
                if let Some(new_name) = new_name.clone() {
                    ensure!(new_name.is_valid(), Error::<T>::InvalidAssetName);
                    *name = new_name;
                }
                if let Some(new_symbol) = new_symbol.clone() {
                    ensure!(new_symbol.is_valid(), Error::<T>::InvalidAssetSymbol);
                    *symbol = new_symbol;
                }
                DispatchResult::Ok(())
            })?;
            Self::deposit_event(Event::<T>::AssetUpdated(asset_id, new_symbol, new_name));
            Ok(().into())
        }
    }

    #[pallet::event]
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
        /// Asset info has been updated
        AssetUpdated(AssetIdOf<T>, Option<AssetSymbol>, Option<AssetName>),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// An asset with a given ID already exists.
        AssetIdAlreadyExists,
        /// An asset with a given ID not exists.
        AssetIdNotExists,
        /// A number is out of range of the balance type.
        InsufficientBalance,
        /// Symbol is not valid. It must contain only uppercase latin characters or numbers, length is from 1 to 7.
        InvalidAssetSymbol,
        /// Name is not valid. It must contain only uppercase or lowercase latin characters or numbers or spaces, length is from 1 to 33.
        InvalidAssetName,
        /// Precision value is not valid, it should represent a number of decimal places for number, max is 30.
        InvalidPrecision,
        /// Minting for particular asset id is disabled.
        AssetSupplyIsNotMintable,
        /// Caller does not own requested asset.
        InvalidAssetOwner,
        /// Increment account reference error.
        IncRefError,
        /// Content source is not valid. It must be ascii only and `common::ASSET_CONTENT_SOURCE_MAX_LENGTH` characters long at max.
        InvalidContentSource,
        /// Description is not valid. It must be `common::ASSET_DESCRIPTION_MAX_LENGTH` characters long at max.
        InvalidDescription,
        /// The asset is not mintable and its initial balance is 0.
        DeadAsset,
        /// Computation overflow.
        Overflow,
    }

    /// Asset Id -> Owner Account Id
    #[pallet::storage]
    #[pallet::getter(fn asset_owner)]
    pub type AssetOwners<T: Config> =
        StorageMap<_, Twox64Concat, T::AssetId, T::AccountId, OptionQuery>;

    /// Asset Id -> (Symbol, Name, Precision, Is Mintable, Content Source, Description)
    #[pallet::storage]
    #[pallet::getter(fn asset_infos)]
    pub type AssetInfos<T: Config> = StorageMap<
        _,
        Twox64Concat,
        T::AssetId,
        (
            AssetSymbol,
            AssetName,
            BalancePrecision,
            bool,
            Option<ContentSource>,
            Option<Description>,
        ),
        ValueQuery,
    >;

    /// Asset Id -> AssetRecord<T>
    #[pallet::storage]
    #[pallet::getter(fn tuple_from_asset_id)]
    pub type AssetRecordAssetId<T: Config> =
        StorageMap<_, Twox64Concat, T::AssetId, AssetRecord<T>>;

    #[allow(clippy::type_complexity)]
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
            Option<ContentSource>,
            Option<Description>,
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
                |(
                    asset_id,
                    account_id,
                    symbol,
                    name,
                    precision,
                    initial_supply,
                    is_mintable,
                    content_source,
                    description,
                )| {
                    Pallet::<T>::register_asset_id(
                        account_id,
                        asset_id,
                        symbol,
                        name,
                        precision,
                        initial_supply,
                        is_mintable,
                        content_source,
                        description,
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
        keccak.update(&frame_system::Pallet::<T>::account_nonce(account_id).encode());
        let mut output = [0u8; 32];
        keccak.finalize(&mut output);
        // More safe to escape.
        output[0] = 0u8;
        T::AssetId::from(H256(output))
    }

    /// Register the given `AssetId`.
    #[allow(clippy::too_many_arguments)]
    pub fn register_asset_id(
        account_id: T::AccountId,
        asset_id: T::AssetId,
        symbol: AssetSymbol,
        name: AssetName,
        precision: BalancePrecision,
        initial_supply: Balance,
        is_mintable: bool,
        opt_content_src: Option<ContentSource>,
        opt_desc: Option<Description>,
    ) -> DispatchResult {
        ensure!(
            precision <= MAX_ALLOWED_PRECISION,
            Error::<T>::InvalidPrecision
        );
        ensure!(symbol.is_valid(), Error::<T>::InvalidAssetSymbol);
        ensure!(name.is_valid(), Error::<T>::InvalidAssetName);
        ensure!(initial_supply > 0 || is_mintable, Error::<T>::DeadAsset);
        ensure!(
            !Self::asset_exists(&asset_id),
            Error::<T>::AssetIdAlreadyExists
        );
        if let Some(content_src) = &opt_content_src {
            ensure!(content_src.is_valid(), Error::<T>::InvalidContentSource)
        }
        if let Some(desc) = &opt_desc {
            ensure!(desc.is_valid(), Error::<T>::InvalidDescription)
        }

        // Storage
        frame_system::Pallet::<T>::inc_consumers(&account_id)
            .map_err(|_| Error::<T>::IncRefError)?;
        AssetOwners::<T>::insert(asset_id, account_id.clone());
        AssetInfos::<T>::insert(
            asset_id,
            (
                symbol,
                name,
                precision,
                is_mintable,
                opt_content_src,
                opt_desc,
            ),
        );

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
            T::Currency::deposit(asset_id, &account_id, initial_supply)?;
        }

        frame_system::Pallet::<T>::inc_account_nonce(&account_id);
        Self::deposit_event(Event::AssetRegistered(asset_id, account_id));
        Ok(())
    }

    /// Generates new `AssetId` and registers it from the `account_id`.
    #[allow(clippy::too_many_arguments)]
    pub fn register_from(
        account_id: &T::AccountId,
        symbol: AssetSymbol,
        name: AssetName,
        precision: BalancePrecision,
        initial_supply: Balance,
        is_mintable: bool,
        opt_content_src: Option<ContentSource>,
        opt_desc: Option<Description>,
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
                opt_content_src,
                opt_desc,
            )?;
            Ok(asset_id)
        })
    }

    #[inline]
    pub fn ensure_asset_is_mintable(asset_id: &T::AssetId) -> DispatchResult {
        let (_, _, _, is_mintable, ..) = AssetInfos::<T>::get(asset_id);
        ensure!(is_mintable, Error::<T>::AssetSupplyIsNotMintable);
        Ok(())
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

    pub fn transfer_from(
        asset_id: &T::AssetId,
        from: &T::AccountId,
        to: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult {
        let r = T::Currency::transfer(*asset_id, from, to, amount);
        if r.is_err() {
            Self::ensure_asset_exists(asset_id)?;
        }
        Self::deposit_event(Event::Transfer(from.clone(), to.clone(), *asset_id, amount));
        r
    }

    pub fn mint_to(
        asset_id: &T::AssetId,
        issuer: &T::AccountId,
        to: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult {
        // No need to check if asset exist.
        // `ensure_asset_is_mintable` will get Default::default() aka `is_mintable == false` and return an error.
        Self::ensure_asset_is_mintable(asset_id)?;
        Self::check_permission_maybe_with_parameters(issuer, MINT, asset_id)?;

        Self::mint_unchecked(asset_id, to, amount)
    }

    pub fn mint_unchecked(
        asset_id: &T::AssetId,
        to: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult {
        T::Currency::deposit(*asset_id, to, amount)
    }

    pub fn burn_from(
        asset_id: &T::AssetId,
        issuer: &T::AccountId,
        from: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult {
        // Holder can burn its funds.
        if issuer != from {
            Self::check_permission_maybe_with_parameters(issuer, BURN, asset_id)?;
        }

        Self::burn_unchecked(asset_id, from, amount)
    }

    fn burn_unchecked(
        asset_id: &T::AssetId,
        from: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult {
        let r = T::Currency::withdraw(*asset_id, from, amount);
        if r.is_err() {
            Self::ensure_asset_exists(asset_id)?;
        }
        r
    }

    fn buy_back_and_burn(asset_id: &T::AssetId, amount: Balance) -> DispatchResult {
        let dex_id = T::GetBuyBackDexId::get();
        let technical_account = T::GetBuyBackAccountId::get();
        let buy_back_asset_id = T::GetBuyBackAssetId::get();

        Self::mint_unchecked(asset_id, &technical_account, amount)?;
        let outcome = T::BuyBackLiquidityProxy::exchange(
            dex_id,
            &technical_account,
            &technical_account,
            asset_id,
            &buy_back_asset_id,
            SwapAmount::with_desired_input(amount, Balance::zero()),
            LiquiditySourceFilter::empty(dex_id),
        )?;
        Self::burn_from(
            &buy_back_asset_id,
            &technical_account,
            &technical_account,
            outcome.amount,
        )
    }

    pub fn update_own_balance(
        asset_id: &T::AssetId,
        who: &T::AccountId,
        by_amount: Amount,
    ) -> DispatchResult {
        Self::check_permission_maybe_with_parameters(who, MINT, asset_id)?;
        Self::check_permission_maybe_with_parameters(who, BURN, asset_id)?;
        if by_amount.is_positive() {
            Self::ensure_asset_is_mintable(asset_id)?;
        }
        T::Currency::update_balance(*asset_id, who, by_amount)
    }

    pub fn can_reserve(asset_id: T::AssetId, who: &T::AccountId, amount: Balance) -> bool {
        T::Currency::can_reserve(asset_id, who, amount)
    }

    pub fn reserve(asset_id: &T::AssetId, who: &T::AccountId, amount: Balance) -> DispatchResult {
        let r = T::Currency::reserve(*asset_id, who, amount);
        if r.is_err() {
            Self::ensure_asset_exists(asset_id)?;
        }
        r
    }

    pub fn unreserve(
        asset_id: &T::AssetId,
        who: &T::AccountId,
        amount: Balance,
    ) -> Result<Balance, DispatchError> {
        let amount = T::Currency::unreserve(*asset_id, who, amount);
        if amount != Default::default() {
            Self::ensure_asset_exists(asset_id)?;
        }
        Ok(amount)
    }

    pub fn set_non_mintable_from(asset_id: &T::AssetId, who: &T::AccountId) -> DispatchResult {
        ensure!(
            Self::is_asset_owner(asset_id, who),
            Error::<T>::InvalidAssetOwner
        );
        AssetInfos::<T>::mutate(asset_id, |(_, _, _, ref mut is_mintable, ..)| {
            ensure!(*is_mintable, Error::<T>::AssetSupplyIsNotMintable);
            *is_mintable = false;
            Ok(())
        })
    }

    pub fn list_registered_asset_ids() -> Vec<T::AssetId> {
        AssetInfos::<T>::iter().map(|(key, _)| key).collect()
    }

    #[allow(clippy::type_complexity)]
    pub fn list_registered_asset_infos() -> Vec<(
        T::AssetId,
        AssetSymbol,
        AssetName,
        BalancePrecision,
        bool,
        Option<ContentSource>,
        Option<Description>,
    )> {
        AssetInfos::<T>::iter()
            .map(
                |(key, (symbol, name, precision, is_mintable, content_source, description))| {
                    (
                        key,
                        symbol,
                        name,
                        precision,
                        is_mintable,
                        content_source,
                        description,
                    )
                },
            )
            .collect()
    }
}

impl<T: Config>
    AssetInfoProvider<
        T::AssetId,
        T::AccountId,
        AssetSymbol,
        AssetName,
        BalancePrecision,
        ContentSource,
        Description,
    > for Pallet<T>
{
    #[inline]
    fn asset_exists(asset_id: &T::AssetId) -> bool {
        AssetOwners::<T>::contains_key(asset_id)
    }

    #[inline]
    fn ensure_asset_exists(asset_id: &T::AssetId) -> DispatchResult {
        if !Self::asset_exists(asset_id) {
            return Err(Error::<T>::AssetIdNotExists.into());
        }
        Ok(())
    }

    #[inline]
    fn is_asset_owner(asset_id: &T::AssetId, account_id: &T::AccountId) -> bool {
        Self::asset_owner(asset_id)
            .map(|x| &x == account_id)
            .unwrap_or(false)
    }

    fn get_asset_info(
        asset_id: &T::AssetId,
    ) -> (
        AssetSymbol,
        AssetName,
        BalancePrecision,
        bool,
        Option<ContentSource>,
        Option<Description>,
    ) {
        let (symbol, name, precision, is_mintable, content_source, description) =
            AssetInfos::<T>::get(asset_id);
        (
            symbol,
            name,
            precision,
            is_mintable,
            content_source,
            description,
        )
    }

    fn is_non_divisible(asset_id: &T::AssetId) -> bool {
        AssetInfos::<T>::get(asset_id).2 == 0
    }

    fn get_asset_content_src(asset_id: &T::AssetId) -> Option<ContentSource> {
        AssetInfos::<T>::get(asset_id).4
    }

    fn get_asset_description(asset_id: &T::AssetId) -> Option<Description> {
        AssetInfos::<T>::get(asset_id).5
    }

    fn total_issuance(asset_id: &T::AssetId) -> Result<Balance, DispatchError> {
        let r = T::Currency::total_issuance(*asset_id);
        if r == Default::default() {
            Self::ensure_asset_exists(asset_id)?;
        }
        Ok(r)
    }

    fn total_balance(asset_id: &T::AssetId, who: &T::AccountId) -> Result<Balance, DispatchError> {
        let r = T::Currency::total_balance(*asset_id, who);
        if r == Default::default() {
            Self::ensure_asset_exists(asset_id)?;
        }
        Ok(r + T::GetTotalBalance::total_balance(asset_id, who)?)
    }

    fn free_balance(asset_id: &T::AssetId, who: &T::AccountId) -> Result<Balance, DispatchError> {
        let r = T::Currency::free_balance(*asset_id, who);
        if r == Default::default() {
            Self::ensure_asset_exists(asset_id)?;
        }
        Ok(r)
    }

    fn ensure_can_withdraw(
        asset_id: &T::AssetId,
        who: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult {
        let r = T::Currency::ensure_can_withdraw(*asset_id, who, amount);
        if r.is_err() {
            Self::ensure_asset_exists(asset_id)?;
        }
        r
    }
}
