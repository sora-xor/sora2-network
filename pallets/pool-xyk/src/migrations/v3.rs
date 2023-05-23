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
use common::{Balance, LiquiditySourceType, ToFeeAccount, TradingPair};
use frame_support::pallet_prelude::{Get, StorageVersion};
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::{log::info, weights::Weight};
use sp_std::prelude::Vec;

use crate::{PoolProviders, Properties, Reserves, TotalIssuances, WeightInfo};
use ceres_liquidity_locker::LockerData;
use demeter_farming_platform::UserInfos;
use trading_pair::EnabledSources;

pub struct XYKPoolUpgrade<T, L>(core::marker::PhantomData<(T, L)>);

impl<T, L> XYKPoolUpgrade<T, L>
where
    T: crate::Config,
{
    /// Unlocks and withdraws a liquidity portion provided by `user_account`
    /// Returns resulting weight and error flag, indicating that there was some problem
    /// with liquidity withdrawal for the provided user account
    fn pull_out_user_from_pool(
        user_account: T::AccountId,
        base_asset: T::AssetId,
        target_asset: T::AssetId,
        pool_account: &T::AccountId,
        dex_id: T::DEXId,
        lp_tokens: u128,
    ) -> (Weight, bool) {
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

        if let Err(_) = Pallet::<T>::withdraw_liquidity_unchecked(
            user_account.clone(),
            dex_id,
            base_asset,
            target_asset,
            lp_tokens,
            1,
            1,
        ) {
            info!("Error encountered during liquidity withdrawal for account {:?} in pool {:?}, skipping", user_account, pool_account);
            (
                T::DbWeight::get()
                    .reads_writes(2, 2)
                    .saturating_add(<T as Config>::WeightInfo::withdraw_liquidity()),
                false,
            )
        } else {
            (
                T::DbWeight::get()
                    .reads_writes(3, 3)
                    .saturating_add(<T as Config>::WeightInfo::withdraw_liquidity()),
                true,
            )
        }
    }

    /// Removes corresponding entries from Properties, Reserves and TotalIssuances
    /// Also deregisters pool account and fee account from technical pallet
    fn remove_pool(
        dex_id: T::DEXId,
        base_asset: T::AssetId,
        target_asset: T::AssetId,
        pool_account: T::AccountId,
    ) -> Weight {
        let (_, tech_acc_id) =
            Pallet::<T>::tech_account_from_dex_and_asset_pair(dex_id, base_asset, target_asset)
                .unwrap();

        let pair = TradingPair::<T::AssetId> {
            base_asset_id: base_asset.clone(),
            target_asset_id: target_asset.clone(),
        };

        EnabledSources::<T>::mutate(&dex_id, &pair, |opt_set| {
            opt_set
                .as_mut()
                .unwrap()
                .remove(&LiquiditySourceType::XYKPool)
        });
        Properties::<T>::remove(base_asset, target_asset);

        let fee_acc_id = tech_acc_id.to_fee_account().unwrap();

        technical::Pallet::<T>::deregister_tech_account_id(tech_acc_id).unwrap();
        technical::Pallet::<T>::deregister_tech_account_id(fee_acc_id).unwrap();

        Reserves::<T>::remove(&base_asset, &target_asset);

        TotalIssuances::<T>::remove(&pool_account);

        return T::DbWeight::get().reads_writes(4, 8);
    }
}

/// Migration which removes invalid pools from `XYKPool` and their corresponding dependencies.
impl<T, L> OnRuntimeUpgrade for XYKPoolUpgrade<T, L>
where
    T: crate::Config,
    L: Get<Vec<(T::AssetId, T::AssetId, T::DEXId)>>,
{
    fn on_runtime_upgrade() -> Weight {
        if StorageVersion::get::<Pallet<T>>() >= StorageVersion::new(3) {
            info!("Migration to version 3 has already been applied");
            return Weight::zero();
        }

        info!("Migrating PoolXYK to v3");

        let swap_pairs_to_be_deleted: Vec<(T::AssetId, T::AssetId, T::DEXId)> = L::get();

        let resulting_weight = swap_pairs_to_be_deleted.into_iter().fold(
            Weight::zero(),
            |weight_acc, (base_asset, target_asset, dex_id)| {
                let pool_account =
                    if let Some(pool_property) = Properties::<T>::get(&base_asset, &target_asset) {
                        pool_property.0
                    } else {
                        info!(
                        "Pool with base asset {:?} and target asset {:?} is not present, skipping",
                        base_asset, target_asset
                    );
                        return weight_acc.saturating_add(T::DbWeight::get().reads(1));
                    };

                info!("Pool with assets {:?} and {:?} reserves before liquidity withdrawal: {:?}", base_asset, target_asset, Reserves::<T>::get(&base_asset, &target_asset));

                // `Self::pull_out_user_from_pool` triggers `withdraw_liquidity_unchecked` which modifies `PoolProviders`
                // StorageDoubleMap, so we collect the users first to safely call `withdraw_liquidity_unchecked`
                let liquidity_holders: Vec<(T::AccountId, Balance)> = PoolProviders::<T>::iter_prefix(&pool_account)
                    .collect();

                // For each liquidity holder we remove locks in ceres and demeter platforms and withdraw the corresponding amount of liquidity
                // If there some error is encountered during withdrawal, we still process other liquidity holders
                // but keeping the pool after this
                let (liquidity_withdrawal_weight, is_liquidity_withdrawal_ok): (Weight, bool) = liquidity_holders.into_iter()
                    .fold((Weight::zero(), true),
                        |(weight_acc, is_liquidity_withdrawal_ok_acc), (user_account, lp_tokens)| {
                            let (weight, is_liquidity_withdrawal_ok) = Self::pull_out_user_from_pool(
                                user_account,
                                base_asset,
                                target_asset,
                                &pool_account,
                                dex_id,
                                lp_tokens,
                            );
                            (
                                weight_acc.saturating_add(weight),
                                is_liquidity_withdrawal_ok_acc && is_liquidity_withdrawal_ok
                            )
                        },
                    );

                info!("Pool with assets {:?} and {:?} reserves after liquidity withdrawal: {:?}", base_asset, target_asset, Reserves::<T>::get(&base_asset, &target_asset));

                if !is_liquidity_withdrawal_ok {
                    info!("Error encountered during liquidity withdrawal, the pool could not be deleted");
                    return weight_acc
                        .saturating_add(liquidity_withdrawal_weight)
                }

                let pool_removal_weight = Self::remove_pool(dex_id, base_asset, target_asset, pool_account);

                weight_acc
                    .saturating_add(liquidity_withdrawal_weight)
                    .saturating_add(pool_removal_weight)
                    .saturating_add(T::DbWeight::get().reads(1))
            },
        );

        StorageVersion::new(3).put::<Pallet<T>>();
        resulting_weight.saturating_add(T::DbWeight::get().reads_writes(1, 1))
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
