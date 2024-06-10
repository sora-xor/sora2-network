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

//! The Regulated Assets pallet allows for the configuration and management of access to regulated assets.
//! It provides functionalities to issue Soulbound Tokens (SBTs) and regulate assets, ensuring only
//! authorized users can operate with these assets.
//! The pallet checks permissions based on asset ownership and SBT holdings, preventing unauthorized operations and transfers.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod weights;

use codec::{Decode, Encode, MaxEncodedLen};
use common::{
    permissions::{PermissionId, ISSUE_SBT, TRANSFER},
    AssetIdOf, AssetInfoProvider, AssetManager, AssetName, AssetRegulator, AssetSymbol,
    BalancePrecision, ContentSource, Description,
};
use frame_support::sp_runtime::DispatchError;
use frame_support::{BoundedBTreeSet, BoundedVec};
use sp_core::Get;
use sp_std::vec::Vec;
use weights::WeightInfo;

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
type Permissions<T> = permissions::Pallet<T>;
type Technical<T> = technical::Pallet<T>;

pub use pallet::*;

#[derive(Clone, Eq, Encode, Decode, scale_info::TypeInfo, PartialEq, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxAllowedTokensPerSBT))]
pub struct SoulboundTokenMetadata<AssetId, MaxAllowedTokensPerSBT: Get<u32>> {
    name: AssetName,
    description: Option<Description>,
    allowed_assets: BoundedVec<AssetId, MaxAllowedTokensPerSBT>,
}

#[frame_support::pallet]
pub mod pallet {

    use super::*;
    use common::DEFAULT_BALANCE_PRECISION;
    use frame_support::pallet_prelude::{OptionQuery, ValueQuery, *};
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + common::Config + permissions::Config + technical::Config
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Max number of allowed tokens per one Soulbound Token
        #[pallet::constant]
        type MaxAllowedTokensPerSBT: Get<u32>;

        /// Max number of SBTs per one Soulbound Token
        #[pallet::constant]
        type MaxSBTsPerAsset: Get<u32>;

        /// To retrieve asset info
        type AssetInfoProvider: AssetInfoProvider<
            AssetIdOf<Self>,
            AccountIdOf<Self>,
            AssetSymbol,
            AssetName,
            BalancePrecision,
            ContentSource,
            Description,
        >;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;
    }

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Marks an asset as regulated, representing that the asset will only operate between KYC-verified wallets.
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `asset_id`: The identifier of the asset.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::regulate_asset())]
        pub fn regulate_asset(origin: OriginFor<T>, asset_id: AssetIdOf<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            <T as Config>::AssetInfoProvider::ensure_asset_exists(&asset_id)?;
            ensure!(
                <T as Config>::AssetInfoProvider::is_asset_owner(&asset_id, &who),
                <Error<T>>::OnlyAssetOwnerCanRegulate
            );
            ensure!(
                !Self::regulated_asset(asset_id),
                <Error<T>>::AssetAlreadyRegulated
            );

            <RegulatedAsset<T>>::set(asset_id, true);
            Self::deposit_event(Event::AssetRegulated { asset_id });

            Ok(())
        }

        /// Issues a new Soulbound Token (SBT).
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `symbol`: The symbol of the SBT which should represent string with only uppercase latin chars with max length of 7.
        /// - `name`: The name of the SBT should represent string with only uppercase or lowercase latin chars or numbers or spaces, with max length of 33.
        /// - `allowed_assets`: TThe list of assets allowed to be operated with by holding the SBT.
        /// - `description`: The description of the SBT. (Optional)
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::issue_sbt())]
        pub fn issue_sbt(
            origin: OriginFor<T>,
            symbol: AssetSymbol,
            name: AssetName,
            allowed_assets: BoundedVec<AssetIdOf<T>, T::MaxAllowedTokensPerSBT>,
            description: Option<Description>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Check permission `who` can issue SBT
            Permissions::<T>::check_permission(who.clone(), ISSUE_SBT)?;

            let sbt_asset_id = T::AssetManager::register_from(
                &who,
                symbol,
                name.clone(),
                DEFAULT_BALANCE_PRECISION,
                0,
                true,
                None,
                description.clone(),
            )?;
            let metadata = SoulboundTokenMetadata {
                name,
                description,
                allowed_assets: allowed_assets.clone(),
            };
            <SoulboundAsset<T>>::insert(sbt_asset_id, &metadata);

            for allowed_asset in allowed_assets.clone().into_iter() {
                <SBTsByAsset<T>>::mutate(allowed_asset, |sbts| {
                    sbts.try_insert(sbt_asset_id).ok();
                });
            }

            Self::deposit_event(Event::SoulboundTokenIssued {
                asset_id: sbt_asset_id,
                owner: who,
                allowed_assets: allowed_assets.clone().into(),
            });

            Ok(())
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Emits When an asset is regulated
        AssetRegulated { asset_id: AssetIdOf<T> },
        /// Emits When an SBT is issued
        SoulboundTokenIssued {
            asset_id: AssetIdOf<T>,
            owner: AccountIdOf<T>,
            allowed_assets: Vec<AssetIdOf<T>>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// SBT is not operationable by any asset operation
        SoulboundAssetNotOperationable,
        /// SBT is not transferable
        SoulboundAssetNotTransferable,
        /// Only asset owner can regulate
        OnlyAssetOwnerCanRegulate,
        /// Asset is already regulated
        AssetAlreadyRegulated,
        /// All involved users of a regulated asset operation should hold SBT
        AllInvolvedUsersShouldHoldSBT,
    }

    #[pallet::type_value]
    pub fn DefaultRegulatedAsset<T: Config>() -> bool {
        false
    }

    /// Mapping from asset id to whether it is regulated or not
    #[pallet::storage]
    #[pallet::getter(fn regulated_asset)]
    pub type RegulatedAsset<T: Config> =
        StorageMap<_, Identity, AssetIdOf<T>, bool, ValueQuery, DefaultRegulatedAsset<T>>;

    /// Mapping from SBT (asset_id) to its metadata
    #[pallet::storage]
    #[pallet::getter(fn soulbound_asset)]
    pub type SoulboundAsset<T: Config> = StorageMap<
        _,
        Identity,
        AssetIdOf<T>,
        SoulboundTokenMetadata<AssetIdOf<T>, T::MaxAllowedTokensPerSBT>,
        OptionQuery,
    >;

    /// Mapping from `asset_id` to its SBTs which grant permission to transfer, mint, and burn the `asset_id`
    #[pallet::storage]
    #[pallet::getter(fn sbts_by_asset)]
    pub type SBTsByAsset<T: Config> = StorageMap<
        _,
        Identity,
        AssetIdOf<T>,
        BoundedBTreeSet<AssetIdOf<T>, T::MaxSBTsPerAsset>,
        ValueQuery,
    >;
}

impl<T: Config> AssetRegulator<AccountIdOf<T>, AssetIdOf<T>> for Pallet<T> {
    fn assign_permission(
        _owner: &AccountIdOf<T>,
        _asset_id: &AssetIdOf<T>,
        _permission_id: &PermissionId,
    ) -> Result<(), DispatchError> {
        Ok(())
    }

    fn check_permission(
        issuer: &AccountIdOf<T>,
        affected_account: &AccountIdOf<T>,
        asset_id: &AssetIdOf<T>,
        permission_id: &PermissionId,
    ) -> Result<(), DispatchError> {
        if Self::soulbound_asset(asset_id).is_some() {
            // Check if the issuer is the asset owner
            let is_asset_owner = <T as Config>::AssetInfoProvider::is_asset_owner(asset_id, issuer);

            if is_asset_owner {
                // Asset owner of the SBT can do all asset operations except transfer
                if permission_id == &TRANSFER {
                    return Err(Error::<T>::SoulboundAssetNotTransferable.into());
                }
                return Ok(());
            } else {
                return Err(Error::<T>::SoulboundAssetNotOperationable.into());
            }
        }

        // If asset is not regulated, then no need to check permissions
        if !Self::regulated_asset(asset_id) {
            return Ok(());
        }

        // If the account is a technical account, then it can do all operations
        if Technical::<T>::lookup_tech_account_id(issuer).is_ok() {
            return Ok(());
        }

        let sbts = Self::sbts_by_asset(asset_id);

        let issuer_has_sbt = sbts.iter().any(|sbt| {
            <T as Config>::AssetInfoProvider::total_balance(sbt, issuer)
                .map_or(false, |balance| balance > 0)
        });

        let affected_account_has_sbt = sbts.iter().any(|sbt| {
            <T as Config>::AssetInfoProvider::total_balance(sbt, affected_account)
                .map_or(false, |balance| balance > 0)
        });

        if !issuer_has_sbt || !affected_account_has_sbt {
            return Err(Error::<T>::AllInvolvedUsersShouldHoldSBT.into());
        }

        Ok(())
    }
}
