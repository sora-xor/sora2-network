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

//! Regulated Assets pallet provides an ability to configure an access to regulated assets.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod weights;

use codec::{Decode, Encode};
use common::{
    permissions::{PermissionId, ISSUE_SBT, TRANSFER},
    AssetIdOf, AssetInfoProvider, AssetManager, AssetName, AssetRegulator, AssetSymbol,
    BalancePrecision, ContentSource, Description,
};
use frame_support::sp_runtime::DispatchError;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::vec::Vec;
use weights::WeightInfo;

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
type Permissions<T> = permissions::Pallet<T>;
type Technical<T> = technical::Pallet<T>;

pub use pallet::*;

#[derive(Debug, Encode, Decode, scale_info::TypeInfo, Clone, PartialEq)]
pub struct SoulboundTokenMetadata<AssetId> {
    name: AssetName,
    description: Option<Description>,
    allowed_assets: Vec<AssetId>,
}

#[frame_support::pallet]
pub mod pallet {

    use super::*;
    use common::{Balance, DEFAULT_BALANCE_PRECISION};
    use frame_support::pallet_prelude::{OptionQuery, ValueQuery, *};
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + common::Config + permissions::Config + technical::Config
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type WeightInfo: WeightInfo;

        type AssetInfoProvider: AssetInfoProvider<
            AssetIdOf<Self>,
            AccountIdOf<Self>,
            AssetSymbol,
            AssetName,
            BalancePrecision,
            ContentSource,
            Description,
        >;
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
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::regulate_asset())]
        pub fn regulate_asset(origin: OriginFor<T>, asset_id: AssetIdOf<T>) -> DispatchResult {
            // validate
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

            // act
            <RegulatedAsset<T>>::set(asset_id, true);
            Self::deposit_event(Event::AssetRegulated { asset_id });

            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::issue_sbt())]
        pub fn issue_sbt(
            origin: OriginFor<T>,
            symbol: AssetSymbol,
            name: AssetName,
            initial_supply: Balance,
            allowed_assets: Vec<AssetIdOf<T>>,
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
                initial_supply,
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

            for allowed_asset in allowed_assets {
                <SBTsByAsset<T>>::mutate(allowed_asset, |sbts| {
                    sbts.insert(sbt_asset_id);
                });
            }

            Self::deposit_event(Event::SoulboundTokenIssued {
                asset_id: sbt_asset_id,
                owner: who,
                metadata,
            });

            Ok(())
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        AssetRegulated {
            asset_id: AssetIdOf<T>,
        },
        SoulboundTokenIssued {
            asset_id: AssetIdOf<T>,
            owner: AccountIdOf<T>,
            metadata: SoulboundTokenMetadata<AssetIdOf<T>>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        SoulboundAssetNotOperationable,
        SoulboundAssetNotTransferable,
        OnlyAssetOwnerCanRegulate,
        AssetAlreadyRegulated,
        AllInvolvedUsersShouldHoldSBT,
    }

    #[pallet::type_value]
    pub fn DefaultRegulatedAsset<T: Config>() -> bool {
        false
    }

    #[pallet::storage]
    #[pallet::getter(fn regulated_asset)]
    pub type RegulatedAsset<T: Config> =
        StorageMap<_, Identity, AssetIdOf<T>, bool, ValueQuery, DefaultRegulatedAsset<T>>;

    #[pallet::storage]
    #[pallet::getter(fn soulbound_asset)]
    pub type SoulboundAsset<T: Config> =
        StorageMap<_, Identity, AssetIdOf<T>, SoulboundTokenMetadata<AssetIdOf<T>>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn sbts_by_asset)]
    pub type SBTsByAsset<T: Config> =
        StorageMap<_, Identity, AssetIdOf<T>, BTreeSet<AssetIdOf<T>>, ValueQuery>;
}

impl<T: Config> AssetRegulator<AccountIdOf<T>, AssetIdOf<T>> for Pallet<T> {
    fn check_permission(
        issuer: &AccountIdOf<T>,
        affected_account: &AccountIdOf<T>,
        asset_id: &AssetIdOf<T>,
        permission_id: &PermissionId,
    ) -> Result<(), DispatchError> {
        if let Some(_metadata) = Self::soulbound_asset(asset_id) {
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
        if let Ok(_) = Technical::<T>::lookup_tech_account_id(&issuer) {
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

    fn assign_permission(
        _owner: &AccountIdOf<T>,
        _asset_id: &AssetIdOf<T>,
        _permission_id: &PermissionId,
    ) -> Result<(), DispatchError> {
        Ok(())
    }
}
