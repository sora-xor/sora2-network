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

use crate::pallet::{Config, Pallet};
use common::{AssetIdOf, Balance, EnabledSourcesManager, ToFeeAccount};
use frame_support::pallet_prelude::Weight;
use frame_support::pallet_prelude::{Get, StorageVersion};
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::weights::WeightMeter;
use log::{error, info};
use sp_runtime::DispatchResult;
use sp_std::prelude::Vec;

use crate::{PoolProviders, Properties, Reserves, TotalIssuances, WeightInfo};
use ceres_liquidity_locker::LockerData;
use demeter_farming_platform::UserInfos;

pub struct XYKPoolUpgrade<T, L>(core::marker::PhantomData<(T, L)>);

impl<T, L> XYKPoolUpgrade<T, L>
where
    T: crate::Config,
    L: Get<Vec<(AssetIdOf<T>, AssetIdOf<T>, T::DEXId)>>,
{
    /// Unlocks and withdraws a liquidity portion provided by `user_account`
    /// Returns resulting weight and error flag, indicating that there was some problem
    /// with liquidity withdrawal for the provided user account
    fn pull_out_user_from_pool(
        weight_meter: &mut WeightMeter,
        user_account: T::AccountId,
        base_asset: AssetIdOf<T>,
        target_asset: AssetIdOf<T>,
        dex_id: T::DEXId,
        lp_tokens: u128,
    ) -> DispatchResult {
        weight_meter.check_accrue(
            T::DbWeight::get()
                .reads_writes(2, 2)
                .saturating_add(<T as Config>::WeightInfo::withdraw_liquidity()),
        );

        UserInfos::<T>::mutate(&user_account, |user_infos| {
            for user_info in user_infos.iter_mut() {
                if user_info.is_farm == true
                    && user_info.base_asset == base_asset
                    && user_info.pool_asset == target_asset
                {
                    user_info.pooled_tokens = 0;
                }
            }
        });

        LockerData::<T>::mutate_exists(&user_account, |user_lock_infos| {
            if let Some(ref mut infos_vec) = *user_lock_infos {
                infos_vec.retain(|user_info| {
                    user_info.asset_a != base_asset && user_info.asset_b != target_asset
                });
                if infos_vec.is_empty() {
                    *user_lock_infos = None;
                }
            }
        });

        Pallet::<T>::withdraw_liquidity_unchecked(
            user_account.clone(),
            dex_id,
            base_asset,
            target_asset,
            lp_tokens,
            1,
            1,
        )?;

        Ok(())
    }

    /// Removes corresponding entries from Properties, Reserves and TotalIssuances
    /// Also deregisters pool account and fee account from technical pallet
    fn remove_pool(
        weight_meter: &mut WeightMeter,
        dex_id: T::DEXId,
        base_asset: AssetIdOf<T>,
        target_asset: AssetIdOf<T>,
        pool_account: T::AccountId,
    ) -> DispatchResult {
        weight_meter.check_accrue(T::DbWeight::get().reads_writes(4, 8));

        let (_, tech_acc_id) =
            Pallet::<T>::tech_account_from_dex_and_asset_pair(dex_id, base_asset, target_asset)?;

        T::EnabledSourcesManager::mutate_remove(
            &dex_id,
            &base_asset.clone(),
            &target_asset.clone(),
        );
        Properties::<T>::remove(base_asset, target_asset);

        let fee_acc_id = tech_acc_id
            .to_fee_account()
            .ok_or(crate::Error::<T>::FeeAccountIsInvalid)?;

        technical::Pallet::<T>::deregister_tech_account_id(tech_acc_id)?;
        technical::Pallet::<T>::deregister_tech_account_id(fee_acc_id)?;

        Reserves::<T>::remove(&base_asset, &target_asset);

        TotalIssuances::<T>::remove(&pool_account);

        Ok(())
    }

    pub fn migrate(weight_meter: &mut WeightMeter) -> DispatchResult {
        weight_meter.check_accrue(T::DbWeight::get().reads(1));
        if StorageVersion::get::<Pallet<T>>() >= StorageVersion::new(3) {
            info!("Migration to version 3 has already been applied");
            return Ok(());
        }

        info!("Migrating PoolXYK to v3");

        let swap_pairs_to_be_deleted: Vec<(AssetIdOf<T>, AssetIdOf<T>, T::DEXId)> = L::get();

        for (base_asset, target_asset, dex_id) in swap_pairs_to_be_deleted {
            weight_meter.check_accrue(T::DbWeight::get().reads(1));

            let pool_account =
                if let Some(pool_property) = Properties::<T>::get(&base_asset, &target_asset) {
                    pool_property.0
                } else {
                    info!(
                        "Pool with base asset {:?} and target asset {:?} is not present, skipping",
                        base_asset, target_asset
                    );
                    continue;
                };

            info!(
                "Pool with assets {:?} and {:?} reserves before liquidity withdrawal: {:?}",
                base_asset,
                target_asset,
                Reserves::<T>::get(&base_asset, &target_asset)
            );

            // `Self::pull_out_user_from_pool` triggers `withdraw_liquidity_unchecked` which modifies `PoolProviders`
            // StorageDoubleMap, so we collect the users first to safely call `withdraw_liquidity_unchecked`
            let liquidity_holders: Vec<(T::AccountId, Balance)> =
                PoolProviders::<T>::iter_prefix(&pool_account)
                    .inspect(|_| {
                        weight_meter.check_accrue(T::DbWeight::get().reads(1));
                    })
                    .collect();

            // For each liquidity holder we remove locks in ceres and demeter platforms and withdraw the corresponding amount of liquidity
            // If there some error is encountered during withdrawal, we still process other liquidity holders
            // but keeping the pool after this
            for (user_account, lp_tokens) in liquidity_holders {
                Self::pull_out_user_from_pool(
                    weight_meter,
                    user_account,
                    base_asset,
                    target_asset,
                    dex_id,
                    lp_tokens,
                )?;
            }

            info!(
                "Pool with assets {:?} and {:?} reserves after liquidity withdrawal: {:?}",
                base_asset,
                target_asset,
                Reserves::<T>::get(&base_asset, &target_asset)
            );

            Self::remove_pool(weight_meter, dex_id, base_asset, target_asset, pool_account)?;
        }

        weight_meter.check_accrue(T::DbWeight::get().writes(1));
        StorageVersion::new(3).put::<Pallet<T>>();
        Ok(())
    }
}

/// Migration which removes invalid pools from `XYKPool` and their corresponding dependencies.
impl<T, L> OnRuntimeUpgrade for XYKPoolUpgrade<T, L>
where
    T: crate::Config,
    L: Get<Vec<(AssetIdOf<T>, AssetIdOf<T>, T::DEXId)>>,
{
    fn on_runtime_upgrade() -> Weight {
        // new() returns max limit
        let mut weight_meter = WeightMeter::new();

        if let Err(err) =
            frame_support::storage::with_storage_layer(|| Self::migrate(&mut weight_meter))
        {
            error!("Failed to migrate PoolXYK to v3: {:?}, rollback", err);
        } else {
            info!("Successfully migrated PoolXYK to v3");
        };
        weight_meter.consumed()
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
        frame_support::ensure!(
            StorageVersion::get::<Pallet<T>>() == StorageVersion::new(2),
            "must upgrade linearly"
        );
        Ok(Vec::new())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
        frame_support::ensure!(
            StorageVersion::get::<Pallet<T>>() == StorageVersion::new(3),
            "should be upgraded to version 3"
        );
        Ok(())
    }
}
