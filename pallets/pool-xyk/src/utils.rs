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

use common::prelude::FixedWrapper;
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::ensure;
use frame_support::weights::Weight;
use sp_core::Get;

use crate::aliases::{TechAccountIdOf, TechAssetIdOf};
use crate::bounds::*;
use crate::{Config, Error, Pallet, PoolProviders, TotalIssuances};
use common::prelude::{Balance, SwapAmount};
use common::{
    AccountIdOf, AssetIdOf, DexInfoProvider, ToFeeAccount, ToXykTechUnitFromDEXAndTradingPair,
    TradingPair,
};

pub struct AdditionalSwapParams<AssetId> {
    pub is_fee_from_destination: bool,
    pub is_chameleon_pool: bool,
    pub base_chameleon_asset: Option<AssetId>,
}

impl<T: Config> Pallet<T> {
    pub fn get_additional_swap_params(
        base_asset_id: &AssetIdOf<T>,
        asset_a: &AssetIdOf<T>,
        asset_b: &AssetIdOf<T>,
    ) -> Result<AdditionalSwapParams<AssetIdOf<T>>, DispatchError> {
        let (tpair, base_chameleon_asset, is_chameleon_pool) =
            Self::get_pair_info(base_asset_id, asset_a, asset_b)?;
        let is_fee_from_destination = if &tpair.target_asset_id == asset_a {
            true
        } else if &tpair.target_asset_id == asset_b {
            false
        } else {
            return Err(Error::<T>::UnsupportedQuotePath.into());
        };
        Ok(AdditionalSwapParams {
            is_chameleon_pool,
            is_fee_from_destination,
            base_chameleon_asset,
        })
    }

    pub fn decide_is_fee_from_destination(
        base_asset_id: &AssetIdOf<T>,
        asset_a: &AssetIdOf<T>,
        asset_b: &AssetIdOf<T>,
    ) -> Result<bool, DispatchError> {
        let tpair = Self::get_trading_pair(base_asset_id, asset_a, asset_b)?;
        if &tpair.target_asset_id == asset_a {
            Ok(true)
        } else if &tpair.target_asset_id == asset_b {
            Ok(false)
        } else {
            Err(Error::<T>::UnsupportedQuotePath.into())
        }
    }

    pub fn get_fee_account(
        tech_acc: &TechAccountIdOf<T>,
    ) -> Result<TechAccountIdOf<T>, DispatchError> {
        let fee_acc = tech_acc
            .to_fee_account()
            .ok_or(Error::<T>::UnableToDeriveFeeAccount)?;
        Ok(fee_acc)
    }

    pub fn is_fee_account_valid_for(
        _asset_id: AssetIdOf<T>,
        tech_acc: &TechAccountIdOf<T>,
        fee_acc: &TechAccountIdOf<T>,
    ) -> DispatchResult {
        let recommended = Self::get_fee_account(tech_acc)?;
        if fee_acc != &recommended {
            Err(Error::<T>::FeeAccountIsInvalid)?;
        }
        Ok(())
    }

    pub fn is_pool_account_valid_for(
        _asset_id: AssetIdOf<T>,
        tech_acc: &TechAccountIdOf<T>,
    ) -> DispatchResult {
        technical::Pallet::<T>::ensure_tech_account_registered(tech_acc)?;
        // TODO: Maybe checking that asset and dex is exist, it is not really needed if
        // registration of technical account is a guarantee that pair and dex exist.
        Ok(())
    }

    pub fn tech_account_from_dex_and_asset_pair(
        dex_id: T::DEXId,
        asset_a: AssetIdOf<T>,
        asset_b: AssetIdOf<T>,
    ) -> Result<(common::TradingPair<TechAssetIdOf<T>>, TechAccountIdOf<T>), DispatchError> {
        let dexinfo = T::DexInfoProvider::get_dex_info(&dex_id)?;
        let base_asset_id = dexinfo.base_asset_id;
        ensure!(asset_a != asset_b, Error::<T>::AssetsMustNotBeSame);
        let tpair = Self::get_trading_pair(&base_asset_id, &asset_a, &asset_b)?;
        let tpair: common::TradingPair<TechAssetIdOf<T>> = tpair.map(|a| a.into());
        Ok((
            tpair,
            TechAccountIdOf::<T>::to_xyk_tech_unit_from_dex_and_trading_pair(dex_id, tpair),
        ))
    }

    pub fn ensure_trading_pair_is_not_restricted(
        tpair: &common::TradingPair<AssetIdOf<T>>,
    ) -> Result<(), DispatchError> {
        if <T::GetTradingPairRestrictedFlag as orml_traits::GetByKey<_, _>>::get(tpair) {
            Err(Error::<T>::TargetAssetIsRestricted.into())
        } else {
            Ok(())
        }
    }

    pub fn get_bounds_from_swap_amount(
        swap_amount: SwapAmount<Balance>,
    ) -> Result<(Bounds<Balance>, Bounds<Balance>), DispatchError> {
        match swap_amount {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => Ok((
                Bounds::Desired(desired_amount_in),
                Bounds::Min(min_amount_out),
            )),
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => Ok((
                Bounds::Max(max_amount_in),
                Bounds::Desired(desired_amount_out),
            )),
        }
    }

    pub fn burn(
        pool_account: &AccountIdOf<T>,
        user_account: &AccountIdOf<T>,
        pool_tokens: Balance,
    ) -> Result<(), DispatchError> {
        let result: Result<_, Error<T>> =
            PoolProviders::<T>::mutate_exists(pool_account, user_account, |balance| {
                let old_balance = balance.ok_or(Error::<T>::AccountBalanceIsInvalid)?;
                let new_balance = old_balance
                    .checked_sub(pool_tokens)
                    .ok_or(Error::<T>::AccountBalanceIsInvalid)?;
                *balance = (new_balance != 0).then(|| new_balance);
                if balance.is_none() {
                    // does not return anything, so we don't need to handle errors
                    frame_system::Pallet::<T>::dec_consumers(user_account)
                }
                Ok(())
            });
        result?;
        let result: Result<_, Error<T>> = TotalIssuances::<T>::mutate(pool_account, |issuance| {
            let old_issuance = issuance.ok_or(Error::<T>::PoolIsInvalid)?;
            let new_issuance = old_issuance
                .checked_sub(pool_tokens)
                .ok_or(Error::<T>::PoolIsInvalid)?;
            *issuance = Some(new_issuance);
            Ok(())
        });
        result?;
        Ok(())
    }

    pub fn mint(
        pool_account: &AccountIdOf<T>,
        user_account: &AccountIdOf<T>,
        pool_tokens: Balance,
    ) -> Result<(), DispatchError> {
        let result: Result<_, Error<T>> =
            PoolProviders::<T>::mutate(pool_account, user_account, |balance| {
                if balance.is_none() {
                    frame_system::Pallet::<T>::inc_consumers(user_account)
                        .map_err(|_| Error::<T>::IncRefError)?;
                }
                *balance = Some(balance.unwrap_or(0) + pool_tokens);
                Ok(())
            });
        result?;
        let result: Result<_, Error<T>> = TotalIssuances::<T>::mutate(&pool_account, |issuance| {
            let new_issuance = issuance
                .unwrap_or(0)
                .checked_add(pool_tokens)
                .ok_or(Error::<T>::PoolTokenSupplyOverflow)?;
            *issuance = Some(new_issuance);
            Ok(())
        });
        result?;
        Ok(())
    }

    // Returns trading pair, chameleon base asset id (if exists) and chameleon pool flag
    pub fn get_pair_info(
        base_asset_id: &AssetIdOf<T>,
        asset_a: &AssetIdOf<T>,
        asset_b: &AssetIdOf<T>,
    ) -> Result<(TradingPair<AssetIdOf<T>>, Option<AssetIdOf<T>>, bool), DispatchError> {
        ensure!(asset_a != asset_b, Error::<T>::AssetsMustNotBeSame);
        let chameleon_pools =
            <T::GetChameleonPools as orml_traits::GetByKey<_, _>>::get(base_asset_id);
        let ta = if base_asset_id == asset_a {
            asset_b
        } else if base_asset_id == asset_b {
            asset_a
        } else if let Some((base_chameleon_asset_id, targets)) = &chameleon_pools {
            if base_chameleon_asset_id == asset_a {
                ensure!(
                    targets.contains(asset_b),
                    Error::<T>::RestrictedChameleonPool
                );
                asset_b
            } else if base_chameleon_asset_id == asset_b {
                ensure!(
                    targets.contains(asset_a),
                    Error::<T>::RestrictedChameleonPool
                );
                asset_a
            } else {
                Err(Error::<T>::BaseAssetIsNotMatchedWithAnyAssetArguments)?
            }
        } else {
            Err(Error::<T>::BaseAssetIsNotMatchedWithAnyAssetArguments)?
        };
        let (base_chameleon_asset_id, is_allowed_target) = chameleon_pools
            .map(|(asset_id, targets)| (asset_id, targets.contains(ta)))
            .unzip();
        let tpair = common::TradingPair::<AssetIdOf<T>> {
            base_asset_id: *base_asset_id,
            target_asset_id: *ta,
        };
        Ok((
            tpair,
            base_chameleon_asset_id,
            is_allowed_target.unwrap_or(false),
        ))
    }

    /// Get trading pair from assets
    pub fn get_trading_pair(
        base_asset_id: &AssetIdOf<T>,
        asset_a: &AssetIdOf<T>,
        asset_b: &AssetIdOf<T>,
    ) -> Result<TradingPair<AssetIdOf<T>>, DispatchError> {
        let (tpair, _, _) = Self::get_pair_info(base_asset_id, asset_a, asset_b)?;
        Ok(tpair)
    }

    pub fn adjust_liquidity_in_pool(
        dex_id: T::DEXId,
        base_asset_id: &AssetIdOf<T>,
        target_asset_id: &AssetIdOf<T>,
        weight: &mut Weight,
    ) -> DispatchResult {
        // get dex info + 3 * get balance + get current issuance
        *weight = weight.saturating_add(T::DbWeight::get().reads(5));

        let (_, pool_acc) =
            Self::tech_account_from_dex_and_asset_pair(dex_id, *base_asset_id, *target_asset_id)?;
        let pool_acc = technical::Pallet::<T>::tech_account_id_to_account_id(&pool_acc)?;

        let (b_in_pool, t_in_pool, _max_output_available) =
            Self::get_actual_reserves(&pool_acc, base_asset_id, base_asset_id, target_asset_id)?;
        let fxw_real_issuance =
            to_fixed_wrapper!(b_in_pool).multiply_and_sqrt(&to_fixed_wrapper!(t_in_pool));
        let current_issuance =
            TotalIssuances::<T>::get(&pool_acc).ok_or(Error::<T>::PoolIsEmpty)?;
        let fxw_current_issuance = to_fixed_wrapper!(current_issuance);
        let fxw_issuance_ratio = fxw_current_issuance / fxw_real_issuance;
        // We handle only case when current issuance is larger than real issuance
        if fxw_issuance_ratio < to_fixed_wrapper!(T::GetMaxIssuanceRatio::get()) {
            return Ok(());
        }
        let mut new_issuance: Balance = 0;
        let mut providers = 0;
        for (account, share) in PoolProviders::<T>::iter_prefix(&pool_acc) {
            providers += 1;
            *weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
            let fxw_real_share = to_fixed_wrapper!(share) / fxw_issuance_ratio.clone();
            let real_share = to_balance!(fxw_real_share);
            // Safe, because we don't remove either add keys
            PoolProviders::<T>::insert(&pool_acc, &account, real_share);
            // To ensure that issuance = sum of shares
            new_issuance = new_issuance
                .checked_add(real_share)
                // Should not happen, because real issuance always less than current issuance
                .ok_or(Error::<T>::PoolIsInvalid)?;
        }
        *weight = weight.saturating_add(T::DbWeight::get().writes(1));
        TotalIssuances::<T>::insert(&pool_acc, new_issuance);
        frame_support::log::debug!(
            "Pool adjusted {} for {} providers: issuance {} -> {}",
            pool_acc,
            providers,
            current_issuance,
            new_issuance
        );
        Self::deposit_event(crate::Event::<T>::PoolAdjusted {
            pool: pool_acc,
            old_issuance: current_issuance,
            new_issuance,
            providers,
        });
        Ok(())
    }

    pub fn fix_pool_parameters(
        dex_id: T::DEXId,
        pool_account: &AccountIdOf<T>,
        asset_a: &AssetIdOf<T>,
        asset_b: &AssetIdOf<T>,
    ) -> DispatchResult {
        let (reserve_a, reserve_b, _) =
            Self::get_actual_reserves(pool_account, asset_a, asset_a, asset_b)?;
        Self::update_reserves(asset_a, asset_a, asset_b, (&reserve_a, &reserve_b));
        frame_support::log::debug!(
            "Updated reserves for {:?}({}) => {:?}({})",
            asset_a,
            reserve_a,
            asset_b,
            reserve_b
        );
        Self::adjust_liquidity_in_pool(dex_id, asset_a, asset_b, &mut Default::default())?;
        Ok(())
    }
}
