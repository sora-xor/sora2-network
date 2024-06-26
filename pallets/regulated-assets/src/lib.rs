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
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod weights;

use codec::{Decode, Encode, MaxEncodedLen};
use common::{
    permissions::{PermissionId, TRANSFER},
    AssetIdOf, AssetInfoProvider, AssetManager, AssetName, AssetRegulator, AssetSymbol,
    BalancePrecision, ContentSource, Description, IsValid,
};
use frame_support::sp_runtime::DispatchError;
use frame_support::BoundedVec;
use sp_core::Get;
use sp_std::vec::Vec;
use weights::WeightInfo;

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
type Technical<T> = technical::Pallet<T>;
type Timestamp<T> = pallet_timestamp::Pallet<T>;
pub use pallet::*;

#[derive(Clone, Eq, Encode, Decode, scale_info::TypeInfo, PartialEq, MaxEncodedLen)]
pub struct SoulboundTokenMetadata<Moment> {
    /// External link of issued place
    external_url: Option<ContentSource>,
    /// Issuance Timestamp
    issued_at: Moment,
}

#[frame_support::pallet]
pub mod pallet {

    use super::*;
    use frame_support::pallet_prelude::{OptionQuery, ValueQuery, *};
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + common::Config + technical::Config + pallet_timestamp::Config
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Max number of allowed assets per one Soulbound Token
        #[pallet::constant]
        type MaxAllowedAssetsPerSBT: Get<u32>;

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
            ensure!(
                Self::soulbound_asset(asset_id).is_none(),
                <Error<T>>::NotAllowedToRegulateSoulboundAsset
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
        /// - `symbol`: The symbol of the SBT which should represent a string with only uppercase Latin characters with a maximum length of 7.
        /// - `name`: The name of the SBT which should represent a string with only uppercase or lowercase Latin characters, numbers, or spaces, with a maximum length of 33.
        /// - `description`: The description of the SBT. (Optional)
        /// - `image`: The URL or identifier for the image associated with the SBT. (Optional)
        /// - `external_url`: The URL pointing to an external resource related to the SBT. (Optional)
        /// - `allowed_assets`: The list of assets allowed to be operated with by holding the SBT.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::issue_sbt())]
        pub fn issue_sbt(
            origin: OriginFor<T>,
            symbol: AssetSymbol,
            name: AssetName,
            description: Option<Description>,
            image: Option<ContentSource>,
            external_url: Option<ContentSource>,
            allowed_assets: BoundedVec<AssetIdOf<T>, T::MaxAllowedAssetsPerSBT>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let now_timestamp = Timestamp::<T>::now();

            Self::check_allowed_assets_for_sbt_issuance(&allowed_assets, &who)?;

            if let Some(ext_url) = &external_url {
                ensure!(ext_url.is_valid(), Error::<T>::InvalidExternalUrl);
            }

            let sbt_asset_id = T::AssetManager::register_from(
                &who,
                symbol,
                name.clone(),
                0,
                0,
                true,
                image.clone(),
                description.clone(),
            )?;

            let metadata = SoulboundTokenMetadata {
                external_url: external_url.clone(),
                issued_at: now_timestamp,
            };

            <SoulboundAsset<T>>::insert(sbt_asset_id, &metadata);

            for allowed_asset in allowed_assets.clone().into_iter() {
                if <RegulatedAssetToSoulboundAsset<T>>::contains_key(allowed_asset) {
                    return Err(<Error<T>>::RegulatedAssetAlreadyMappedToSBT.into());
                }
                <RegulatedAssetToSoulboundAsset<T>>::set(allowed_asset, sbt_asset_id);
            }

            Self::deposit_event(Event::SoulboundTokenIssued {
                asset_id: sbt_asset_id,
                owner: who,
                image,
                external_url,
                issued_at: now_timestamp,
                allowed_assets: allowed_assets.clone().into(),
            });

            Ok(())
        }

        /// Sets the expiration date of a Soulbound Token (SBT) for the given account.
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `asset_id`: The ID of the SBT to update.
        /// - `account_id`: The ID of the account to set the expiration for.
        /// - `new_expires_at`: The new expiration timestamp for the SBT.
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::set_sbt_expiration())]
        pub fn set_sbt_expiration(
            origin: OriginFor<T>,
            account_id: T::AccountId,
            asset_id: AssetIdOf<T>,
            new_expires_at: Option<T::Moment>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Ensure the asset exists and is an SBT
            Self::soulbound_asset(asset_id).ok_or(Error::<T>::SBTNotFound)?;

            if let Some(new_expires_at) = new_expires_at {
                // Ensure the new expiration is in the future
                ensure!(
                    new_expires_at > Timestamp::<T>::now(),
                    Error::<T>::SBTExpirationDateCannotBeInThePast
                );
            }

            // Ensure the caller is the owner of the SBT
            ensure!(
                <T as Config>::AssetInfoProvider::is_asset_owner(&asset_id, &who),
                Error::<T>::NotSBTOwner
            );

            let old_expires_at = Self::sbt_asset_expiration(&account_id, asset_id);
            if old_expires_at == new_expires_at {
                return Ok(());
            }

            // Update the expiration date
            SBTExpiration::<T>::set(account_id, asset_id, new_expires_at);

            Self::deposit_event(Event::SBTExpirationUpdated {
                asset_id,
                old_expires_at,
                new_expires_at,
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
            image: Option<ContentSource>,
            external_url: Option<ContentSource>,
            issued_at: T::Moment,
            allowed_assets: Vec<AssetIdOf<T>>,
        },
        /// Emits When the expiration date of an SBT is updated
        SBTExpirationUpdated {
            asset_id: AssetIdOf<T>,
            old_expires_at: Option<T::Moment>,
            new_expires_at: Option<T::Moment>,
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
        /// All involved users of a regulated asset operation should hold valid SBT
        AllInvolvedUsersShouldHoldValidSBT,
        /// SBT Expiration Date cannot be in the past
        SBTExpirationDateCannotBeInThePast,
        /// All Allowed assets must be owned by SBT issuer
        AllowedAssetsMustBeOwnedBySBTIssuer,
        /// All Allowed assets must be regulated
        AllowedAssetsMustBeRegulated,
        /// SBT not found
        SBTNotFound,
        /// Caller is not the owner of the SBT
        NotSBTOwner,
        /// Not allowed to regulate SBT
        NotAllowedToRegulateSoulboundAsset,
        /// Asset is already mapped to SBT
        RegulatedAssetAlreadyMappedToSBT,
        /// Invalid External URL
        InvalidExternalUrl,
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
    pub type SoulboundAsset<T: Config> =
        StorageMap<_, Identity, AssetIdOf<T>, SoulboundTokenMetadata<T::Moment>, OptionQuery>;

    /// Mapping from Regulated asset id to SBT asset id
    #[pallet::storage]
    #[pallet::getter(fn regulated_asset_to_sbt)]
    pub type RegulatedAssetToSoulboundAsset<T: Config> =
        StorageMap<_, Identity, AssetIdOf<T>, AssetIdOf<T>, ValueQuery>;

    /// Mapping from SBT asset id to its expiration per account
    #[pallet::storage]
    #[pallet::getter(fn sbt_asset_expiration)]
    pub type SBTExpiration<T: Config> =
        StorageDoubleMap<_, Identity, T::AccountId, Identity, AssetIdOf<T>, T::Moment, OptionQuery>;
}

impl<T: Config> Pallet<T> {
    pub fn check_allowed_assets_for_sbt_issuance(
        allowed_assets: &BoundedVec<AssetIdOf<T>, T::MaxAllowedAssetsPerSBT>,
        sbt_issuer: &T::AccountId,
    ) -> Result<(), Error<T>> {
        for allowed_asset_id in allowed_assets.iter() {
            let is_asset_owner =
                <T as Config>::AssetInfoProvider::is_asset_owner(allowed_asset_id, sbt_issuer);

            if !is_asset_owner {
                return Err(Error::<T>::AllowedAssetsMustBeOwnedBySBTIssuer);
            }

            let is_asset_regulated = Self::regulated_asset(allowed_asset_id);
            if !is_asset_regulated {
                return Err(Error::<T>::AllowedAssetsMustBeRegulated);
            }
        }

        Ok(())
    }

    pub fn check_account_has_valid_sbt_for_regulated_asset(
        account_id: &T::AccountId,
        regulated_asset_id: &AssetIdOf<T>,
        now: &T::Moment,
    ) -> bool {
        if !<RegulatedAssetToSoulboundAsset<T>>::contains_key(regulated_asset_id) {
            return false;
        }

        let sbt_id = Self::regulated_asset_to_sbt(regulated_asset_id);
        let is_holding = <T as Config>::AssetInfoProvider::total_balance(&sbt_id, account_id)
            .map_or(false, |balance| balance > 0);

        let expires_at = Self::sbt_asset_expiration(account_id, sbt_id);
        let is_expired = expires_at.map_or(false, |expiration_date| expiration_date < *now);

        is_holding && !is_expired
    }
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

        let now_timestamp = Timestamp::<T>::now();

        let issuer_pass_check =
            Self::check_account_has_valid_sbt_for_regulated_asset(issuer, asset_id, &now_timestamp)
                || Technical::<T>::lookup_tech_account_id(issuer).is_ok();

        let affected_account_pass_check = if affected_account == issuer {
            issuer_pass_check
        } else {
            Self::check_account_has_valid_sbt_for_regulated_asset(
                affected_account,
                asset_id,
                &now_timestamp,
            ) || Technical::<T>::lookup_tech_account_id(affected_account).is_ok()
        };

        if !issuer_pass_check || !affected_account_pass_check {
            return Err(Error::<T>::AllInvolvedUsersShouldHoldValidSBT.into());
        }

        Ok(())
    }
}
