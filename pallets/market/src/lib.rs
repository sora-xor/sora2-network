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

#![cfg_attr(not(feature = "std"), no_std)]

use common::AssetInfoProvider;
use common::AssetManager;
use common::AssetRegulator;
use common::Balance;
use common::{AccountIdOf, AssetIdOf};
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use sp_runtime::traits::Zero;
use sp_runtime::Saturating;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::prelude::*;
pub use weights::WeightInfo;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod benchmarking;

pub mod weights;

pub use pallet::*;

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, Copy, PartialEq, Eq)]
pub enum Extension<BlockNumber, AssetId> {
    MaxAmount(Balance),
    RequiredProduct(AssetId, Balance),
    DisallowedProduct(AssetId),
    Expirable(BlockNumber),
}

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq, Eq)]
pub struct Product<BlockNumber, AssetId> {
    price_asset: AssetId,
    price: Balance,
    extensions: Vec<Extension<BlockNumber, AssetId>>,
}

#[frame_support::pallet]
pub mod pallet {

    use super::*;

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + common::Config {
        /// Event type of this pallet.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type AssetInfoProvider: common::AssetInfoProvider<
            AssetIdOf<Self>,
            AccountIdOf<Self>,
            common::AssetSymbol,
            common::AssetName,
            common::BalancePrecision,
            common::ContentSource,
            common::Description,
        >;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn product)]
    pub type Products<T: Config> = StorageMap<
        _,
        Identity,
        AssetIdOf<T>,
        Product<BlockNumberFor<T>, AssetIdOf<T>>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn expiration)]
    pub type Expirations<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        BlockNumberFor<T>,
        Identity,
        (T::AccountId, AssetIdOf<T>),
        Balance,
        ValueQuery,
    >;

    #[pallet::hooks]
    impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
        fn on_initialize(now: T::BlockNumber) -> Weight {
            let mut processed_expirations = 0;
            for ((account_id, product_id), value_to_burn) in Expirations::<T>::drain_prefix(now) {
                processed_expirations += 1;
                if let Err(err) =
                    T::AssetManager::burn_unchecked(&product_id, &account_id, value_to_burn)
                {
                    frame_support::log::warn!(
                        "Unable to burn expired product, product_id: {:?}, account_id {:?}, amount: {}, err: {:?}",
                        product_id,
                        account_id,
                        value_to_burn,
                        err
                    );
                }
            }
            <T as Config>::WeightInfo::on_initialize(processed_expirations)
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ProductRegistered { asset_id: AssetIdOf<T> },
    }

    #[pallet::error]
    pub enum Error<T> {
        ArithmeticError,
        ProductNotFound,
        ProductAlreadyExist,
        OperationIsNotPermittedForProduct,
        MaxAmountExceeded,
        MissingRequiredProduct,
        HaveDisallowedProduct,
        ZeroMaxAmount,
        ZeroExpiration,
        MultipleExtensionsNotAllowed,
        AmbigiousProductRequirements,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::create_product())]
        pub fn create_product(
            origin: OriginFor<T>,
            name: common::AssetName,
            symbol: common::AssetSymbol,
            description: common::Description,
            content_source: common::ContentSource,
            product: Product<BlockNumberFor<T>, AssetIdOf<T>>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::create_new_product(who, name, symbol, description, content_source, product)?;
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::buy())]
        pub fn buy(
            origin: OriginFor<T>,
            product_id: AssetIdOf<T>,
            amount: Balance,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let product = Products::<T>::get(&product_id).ok_or(Error::<T>::ProductNotFound)?;
            let asset_owner = T::AssetInfoProvider::get_asset_owner(&product_id)?;
            let amount_to_pay = amount
                .checked_mul(product.price)
                .ok_or(Error::<T>::ArithmeticError)?;
            T::AssetManager::transfer_from(
                &product.price_asset,
                &who,
                &asset_owner,
                amount_to_pay,
            )?;
            for extension in product.extensions.iter() {
                Self::apply_extension(extension, &who, &product_id, amount)?;
            }
            T::AssetManager::mint_unchecked(&product_id, &who, amount)?;
            Ok(().into())
        }
    }
}

impl<T: Config> Pallet<T> {
    pub fn create_new_product(
        who: AccountIdOf<T>,
        name: common::AssetName,
        symbol: common::AssetSymbol,
        description: common::Description,
        content_source: common::ContentSource,
        product: Product<BlockNumberFor<T>, AssetIdOf<T>>,
    ) -> Result<AssetIdOf<T>, DispatchError> {
        Self::verify_extensions(&product.extensions)?;
        let asset_id = T::AssetManager::register_from(
            &who,
            symbol,
            name,
            0,
            0,
            true,
            Some(content_source),
            Some(description),
        )?;
        Products::<T>::insert(asset_id, product);
        Self::deposit_event(Event::<T>::ProductRegistered { asset_id });
        Ok(asset_id)
    }

    fn apply_extension(
        extension: &Extension<BlockNumberFor<T>, AssetIdOf<T>>,
        who: &T::AccountId,
        product_id: &AssetIdOf<T>,
        amount: Balance,
    ) -> DispatchResult {
        match extension {
            Extension::MaxAmount(max_amount) => {
                ensure!(
                    T::AssetInfoProvider::total_balance(product_id, who)?.saturating_add(amount)
                        <= *max_amount,
                    Error::<T>::MaxAmountExceeded
                );
            }
            Extension::RequiredProduct(product_id, min_amount) => {
                ensure!(
                    T::AssetInfoProvider::total_balance(product_id, who)? >= *min_amount,
                    Error::<T>::MissingRequiredProduct
                );
            }
            Extension::DisallowedProduct(product_id) => {
                ensure!(
                    T::AssetInfoProvider::total_balance(product_id, who)? == 0,
                    Error::<T>::HaveDisallowedProduct
                );
            }
            Extension::Expirable(expiration) => {
                let now = frame_system::Pallet::<T>::block_number();
                Expirations::<T>::mutate(
                    now.saturating_add(*expiration),
                    (who, product_id),
                    |amount_to_burn| {
                        *amount_to_burn = amount_to_burn.saturating_add(amount);
                    },
                )
            }
        }
        Ok(())
    }

    fn verify_extensions(
        extensions: &[Extension<BlockNumberFor<T>, AssetIdOf<T>>],
    ) -> DispatchResult {
        let mut has_max_amount = false;
        let mut has_expiration = false;
        let mut required_products = BTreeSet::new();
        let mut disallowed_products = BTreeSet::new();
        for extension in extensions {
            match extension {
                Extension::MaxAmount(max_amount) => {
                    ensure!(!has_max_amount, Error::<T>::MultipleExtensionsNotAllowed);
                    has_max_amount = true;
                    ensure!(!max_amount.is_zero(), Error::<T>::ZeroMaxAmount);
                }
                Extension::RequiredProduct(product_id, _min_amount) => {
                    ensure!(
                        !disallowed_products.contains(product_id),
                        Error::<T>::AmbigiousProductRequirements
                    );
                    ensure!(
                        required_products.insert(*product_id),
                        Error::<T>::AmbigiousProductRequirements
                    );
                    ensure!(
                        Products::<T>::contains_key(product_id),
                        Error::<T>::ProductNotFound
                    );
                }
                Extension::DisallowedProduct(product_id) => {
                    ensure!(
                        !required_products.contains(product_id),
                        Error::<T>::AmbigiousProductRequirements
                    );
                    ensure!(
                        disallowed_products.insert(*product_id),
                        Error::<T>::AmbigiousProductRequirements
                    );
                    ensure!(
                        Products::<T>::contains_key(product_id),
                        Error::<T>::ProductNotFound
                    );
                }
                Extension::Expirable(expiration) => {
                    ensure!(!has_expiration, Error::<T>::MultipleExtensionsNotAllowed);
                    has_expiration = true;
                    ensure!(!expiration.is_zero(), Error::<T>::ZeroExpiration);
                }
            }
        }
        Ok(())
    }
}

impl<T: Config> AssetRegulator<T::AccountId, AssetIdOf<T>> for Pallet<T> {
    fn assign_permission(
        _owner: &T::AccountId,
        _asset_id: &AssetIdOf<T>,
        _permission_id: &common::permissions::PermissionId,
    ) -> Result<(), DispatchError> {
        Ok(())
    }

    fn check_permission(
        _issuer: &T::AccountId,
        _affected_account: &T::AccountId,
        asset_id: &AssetIdOf<T>,
        permission_id: &common::permissions::PermissionId,
    ) -> Result<(), DispatchError> {
        if matches!(
            *permission_id,
            common::permissions::TRANSFER | common::permissions::BURN | common::permissions::MINT
        ) && Products::<T>::contains_key(asset_id)
        {
            frame_support::fail!(Error::<T>::OperationIsNotPermittedForProduct);
        }
        Ok(())
    }
}
