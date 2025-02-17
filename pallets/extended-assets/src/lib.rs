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

#[cfg(any(test, feature = "test", feature = "runtime-benchmarks"))]
pub mod test_utils;

pub mod weights;

use codec::{Decode, Encode, MaxEncodedLen};
use common::{
    permissions::{PermissionId, BURN, MINT, TRANSFER},
    AssetIdOf, AssetInfoProvider, AssetManager, AssetName, AssetRegulator, AssetSymbol, AssetType,
    BalancePrecision, ContentSource, Description, ExtendedAssetsManager, IsValid,
};
use frame_support::ensure;
use frame_support::sp_runtime::DispatchError;
use frame_support::BoundedBTreeSet;
use sp_core::Get;
use weights::WeightInfo;

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
type Technical<T> = technical::Pallet<T>;
type Timestamp<T> = pallet_timestamp::Pallet<T>;
pub use pallet::*;

#[derive(Clone, Eq, Encode, Decode, scale_info::TypeInfo, PartialEq, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxRegulatedAssetsPerSBT))]
pub struct SoulboundTokenMetadata<Moment, AssetId, MaxRegulatedAssetsPerSBT: Get<u32>> {
    /// External link of issued place
    pub external_url: Option<ContentSource>,
    /// Issuance Timestamp
    pub issued_at: Moment,
    /// List of regulated assets permissioned by this token
    pub regulated_assets: BoundedBTreeSet<AssetId, MaxRegulatedAssetsPerSBT>,
}

#[frame_support::pallet]
pub mod pallet {

    use super::*;
    use common::{Balance, DEFAULT_BALANCE_PRECISION};
    use frame_support::pallet_prelude::{OptionQuery, ValueQuery, *};
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;
    use sp_core::TryCollect;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + common::Config + technical::Config + pallet_timestamp::Config
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

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

        /// Max number of regulated assets per one Soulbound Token
        #[pallet::constant]
        type MaxRegulatedAssetsPerSBT: Get<u32>;

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
        /// Registers a new regulated asset, representing that the asset will only operate between KYC-verified wallets.
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `symbol`: AssetSymbol should represent string with only uppercase latin chars with max length of 7.
        /// - `name`: AssetName should represent string with only uppercase or lowercase latin chars or numbers or spaces, with max length of 33.
        /// - `initial_supply`: Balance type representing the total amount of the asset to be issued initially.
        /// - `is_indivisible`: A boolean flag indicating whether the asset can be divided into smaller units or not.
        /// - `opt_content_src`: An optional parameter of type `ContentSource`, which can include a URI or a reference to a content source that provides more details about the asset.
        /// - `opt_desc`: An optional parameter of type `Description`, which is a string providing a short description or commentary about the asset.
        ///
        /// ## Events
        ///
        /// Emits `RegulatedAssetRegistered` event when the asset is successfully registered.
        ///
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::register_regulated_asset())]
        pub fn register_regulated_asset(
            origin: OriginFor<T>,
            symbol: AssetSymbol,
            name: AssetName,
            initial_supply: Balance,
            is_mintable: bool,
            is_indivisible: bool,
            opt_content_src: Option<ContentSource>,
            opt_desc: Option<Description>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let precision = if is_indivisible {
                0
            } else {
                DEFAULT_BALANCE_PRECISION
            };

            let asset_id = T::AssetManager::register_from(
                &who,
                symbol,
                name,
                precision,
                initial_supply,
                is_mintable,
                AssetType::Regulated,
                opt_content_src,
                opt_desc,
            )?;

            Self::deposit_event(Event::RegulatedAssetRegistered { asset_id });

            Ok(().into())
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
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::issue_sbt())]
        pub fn issue_sbt(
            origin: OriginFor<T>,
            symbol: AssetSymbol,
            name: AssetName,
            description: Option<Description>,
            image: Option<ContentSource>,
            external_url: Option<ContentSource>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let now_timestamp = Timestamp::<T>::now();

            if let Some(ext_url) = &external_url {
                ensure!(ext_url.is_valid(), Error::<T>::InvalidExternalUrl);
            }

            let sbt_asset_id = T::AssetManager::register_from(
                &who,
                symbol,
                name.clone(),
                0,
                1,
                true,
                AssetType::Soulbound,
                image.clone(),
                description.clone(),
            )?;

            Self::set_metadata(&sbt_asset_id, external_url.clone(), now_timestamp);

            Self::deposit_event(Event::SoulboundTokenIssued {
                asset_id: sbt_asset_id,
                owner: who,
                image,
                external_url,
                issued_at: now_timestamp,
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
            sbt_asset_id: AssetIdOf<T>,
            new_expires_at: Option<T::Moment>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Ensure the asset exists and is an SBT
            Self::soulbound_asset(sbt_asset_id).ok_or(Error::<T>::SBTNotFound)?;

            // Ensure the caller is the owner of the SBT
            ensure!(
                <T as Config>::AssetInfoProvider::is_asset_owner(&sbt_asset_id, &who),
                Error::<T>::NotSBTOwner
            );

            let old_expires_at = Self::sbt_asset_expiration(&account_id, sbt_asset_id);
            if old_expires_at == new_expires_at {
                return Ok(());
            }

            // Update the expiration date
            SBTExpiration::<T>::set(account_id, sbt_asset_id, new_expires_at);

            Self::deposit_event(Event::SBTExpirationUpdated {
                sbt_asset_id,
                old_expires_at,
                new_expires_at,
            });

            Ok(())
        }

        /// Binds a regulated asset to a Soulbound Token (SBT).
        ///
        /// This function binds a regulated asset to a specified SBT, ensuring the asset and
        /// the SBT meet the required criteria.
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `sbt_asset_id`: The ID of the SBT to bind the regulated asset to.
        /// - `regulated_asset_id`: The ID of the regulated asset to bind.
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::bind_regulated_asset_to_sbt())]
        pub fn bind_regulated_asset_to_sbt(
            origin: OriginFor<T>,
            sbt_asset_id: AssetIdOf<T>,
            regulated_asset_id: AssetIdOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Ensure the caller is the owner of the SBT
            ensure!(
                <T as Config>::AssetInfoProvider::is_asset_owner(&sbt_asset_id, &who),
                Error::<T>::NotSBTOwner
            );

            ensure!(
                <T as Config>::AssetInfoProvider::is_asset_owner(&regulated_asset_id, &who),
                Error::<T>::RegulatedAssetNoOwnedBySBTIssuer
            );

            Self::bind_regulated_asset_to_sbt_asset(&sbt_asset_id, &regulated_asset_id)?;

            Self::deposit_event(Event::RegulatedAssetBoundToSBT {
                regulated_asset_id,
                sbt_asset_id,
            });

            Ok(())
        }

        /// Marks an asset as regulated, representing that the asset will only operate between KYC-verified wallets.
        ///
        /// ## Parameters
        ///
        /// - `origin`: The origin of the transaction.
        /// - `asset_id`: The identifier of the asset.
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::regulate_asset())]
        pub fn regulate_asset(origin: OriginFor<T>, asset_id: AssetIdOf<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            <T as Config>::AssetInfoProvider::ensure_asset_exists(&asset_id)?;
            ensure!(
                <T as Config>::AssetInfoProvider::is_asset_owner(&asset_id, &who),
                <Error<T>>::OnlyAssetOwnerCanRegulate
            );
            ensure!(
                !Self::is_asset_regulated(&asset_id),
                <Error<T>>::AssetAlreadyRegulated
            );
            ensure!(
                Self::soulbound_asset(asset_id).is_none(),
                <Error<T>>::NotAllowedToRegulateSoulboundAsset
            );

            T::AssetManager::update_asset_type(&asset_id, &AssetType::Regulated)?;
            Self::deposit_event(Event::AssetRegulated { asset_id });
            Ok(())
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Emits When a new regulated asset is registered
        RegulatedAssetRegistered { asset_id: AssetIdOf<T> },
        /// Emits When an SBT is issued
        SoulboundTokenIssued {
            asset_id: AssetIdOf<T>,
            owner: AccountIdOf<T>,
            image: Option<ContentSource>,
            external_url: Option<ContentSource>,
            issued_at: T::Moment,
        },
        /// Emits When the expiration date of an SBT is updated
        SBTExpirationUpdated {
            sbt_asset_id: AssetIdOf<T>,
            old_expires_at: Option<T::Moment>,
            new_expires_at: Option<T::Moment>,
        },
        /// When a regulated asset is successfully bound to an SBT
        RegulatedAssetBoundToSBT {
            regulated_asset_id: AssetIdOf<T>,
            sbt_asset_id: AssetIdOf<T>,
        },
        /// Emits When an asset is regulated
        AssetRegulated { asset_id: AssetIdOf<T> },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// SBT is not operationable by any asset operation
        SoulboundAssetNotOperationable,
        /// SBT is not transferable
        SoulboundAssetNotTransferable,
        /// All involved users of a regulated asset operation should hold valid SBT
        AllInvolvedUsersShouldHoldValidSBT,
        /// All Allowed assets must be owned by SBT issuer
        RegulatedAssetNoOwnedBySBTIssuer,
        /// All Allowed assets must be regulated
        AssetNotRegulated,
        /// SBT not found
        SBTNotFound,
        /// Caller is not the owner of the SBT
        NotSBTOwner,
        /// Not allowed to regulate SBT
        NotAllowedToRegulateSoulboundAsset,
        /// Invalid External URL
        InvalidExternalUrl,
        /// Regulated Assets per SBT exceeded
        RegulatedAssetsPerSBTExceeded,
        /// Only asset owner can regulate
        OnlyAssetOwnerCanRegulate,
        /// Asset is already regulated
        AssetAlreadyRegulated,
    }

    /// Mapping from SBT (asset_id) to its metadata
    #[pallet::storage]
    #[pallet::getter(fn soulbound_asset)]
    pub type SoulboundAsset<T: Config> = StorageMap<
        _,
        Identity,
        AssetIdOf<T>,
        SoulboundTokenMetadata<T::Moment, AssetIdOf<T>, T::MaxRegulatedAssetsPerSBT>,
        OptionQuery,
    >;

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

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub assets_metadata: Vec<(AssetIdOf<T>, Option<ContentSource>, AssetIdOf<T>)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                assets_metadata: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            self.assets_metadata.iter().cloned().for_each(
                |(sbt_asset_id, external_url, regulated_asset)| {
                    let metadata = SoulboundTokenMetadata {
                        external_url,
                        issued_at: Timestamp::<T>::now(),
                        regulated_assets: vec![regulated_asset]
                            .into_iter()
                            .try_collect()
                            .expect("Failed to bind regulated assets with soulbound asset."),
                    };

                    <SoulboundAsset<T>>::insert(sbt_asset_id, metadata);
                    <RegulatedAssetToSoulboundAsset<T>>::set(regulated_asset, sbt_asset_id);
                },
            );
        }
    }
}

impl<T: Config> Pallet<T> {
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

    pub fn is_asset_regulated(asset_id: &AssetIdOf<T>) -> bool {
        let asset_type = <T as Config>::AssetInfoProvider::get_asset_type(asset_id);
        asset_type == AssetType::Regulated
    }

    pub fn set_metadata(
        sbt_asset_id: &AssetIdOf<T>,
        external_url: Option<ContentSource>,
        issued_at: T::Moment,
    ) {
        let metadata = SoulboundTokenMetadata {
            external_url,
            issued_at,
            regulated_assets: Default::default(),
        };

        <SoulboundAsset<T>>::insert(sbt_asset_id, metadata);
    }

    pub fn bind_regulated_asset_to_sbt_asset(
        sbt_asset_id: &AssetIdOf<T>,
        regulated_asset_id: &AssetIdOf<T>,
    ) -> Result<(), DispatchError> {
        // Ensure the asset exists and is an SBT
        Self::soulbound_asset(sbt_asset_id).ok_or(Error::<T>::SBTNotFound)?;

        ensure!(
            Self::is_asset_regulated(regulated_asset_id),
            Error::<T>::AssetNotRegulated
        );

        // In case the regulated asset is already bound to another SBT, we need to unbind it
        // by removing it from the previous SBT's regulated assets list.
        if <RegulatedAssetToSoulboundAsset<T>>::contains_key(regulated_asset_id) {
            let previous_sbt = Self::regulated_asset_to_sbt(regulated_asset_id);
            <SoulboundAsset<T>>::mutate(previous_sbt, |metadata| {
                if let Some(metadata) = metadata {
                    metadata.regulated_assets.remove(regulated_asset_id);
                }
            });
        }

        // Bind the regulated asset to the SBT and update the SBT's metadata
        <RegulatedAssetToSoulboundAsset<T>>::set(regulated_asset_id, *sbt_asset_id);
        <SoulboundAsset<T>>::try_mutate(sbt_asset_id, |metadata| -> Result<(), DispatchError> {
            if let Some(metadata) = metadata {
                metadata
                    .regulated_assets
                    .try_insert(*regulated_asset_id)
                    .map_err(|_| Error::<T>::RegulatedAssetsPerSBTExceeded)?;
            }
            Ok(())
        })?;

        Ok(())
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
                if *permission_id == TRANSFER {
                    return Err(Error::<T>::SoulboundAssetNotTransferable.into());
                }
                return Ok(());
            } else if *permission_id == MINT || *permission_id == BURN {
                // These persmissions are checked in `permissions` pallet
                return Ok(());
            } else {
                return Err(Error::<T>::SoulboundAssetNotOperationable.into());
            }
        }

        // If asset is not regulated, then no need to check permissions
        if !Self::is_asset_regulated(asset_id) {
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

impl<T: Config> ExtendedAssetsManager<AssetIdOf<T>, T::Moment, ContentSource> for Pallet<T> {
    fn set_metadata(
        sbt_asset_id: &AssetIdOf<T>,
        external_url: Option<ContentSource>,
        issued_at: T::Moment,
    ) {
        Self::set_metadata(sbt_asset_id, external_url, issued_at);
    }

    fn bind_regulated_asset_to_sbt_asset(
        sbt_asset_id: &AssetIdOf<T>,
        regulated_asset_id: &AssetIdOf<T>,
    ) -> Result<(), DispatchError> {
        Self::bind_regulated_asset_to_sbt_asset(sbt_asset_id, regulated_asset_id)
    }
}
