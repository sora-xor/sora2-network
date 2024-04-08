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

use crate::Config;
use crate::{pallet_tools, Error};
use codec::{Decode, Encode};
use common::prelude::FixedWrapper;
use common::{AssetIdOf, AssetInfoProvider, Balance, DEXId, TradingPairSourceManager, TBCD};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::ensure;
use frame_support::traits::Get;
use pallet_tools::price_tools::AssetPrices;

/// Parameters relevant for TBCD and other collaterals
#[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
pub struct CollateralCommonParameters {
    /// Price of collateral in terms of reference asset. Linearly affects the exchange amounts.
    /// (if collateral costs 10x more sell output should be 10x smaller)
    pub ref_prices: Option<AssetPrices>,
    /// Desired amount of collateral asset in the MCBC reserve account. Affects actual sell
    /// price according to formulae.
    pub reserves: Option<Balance>,
}

/// Input for initializing collateral assets except TBCD.
#[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
pub struct OtherCollateralInput<AssetId> {
    /// Collateral asset id
    pub asset: AssetId,
    pub parameters: CollateralCommonParameters,
}

/// Input for initializing TBCD collateral.
#[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
pub struct TbcdCollateralInput {
    pub parameters: CollateralCommonParameters,
    /// Price of XOR in terms of reference asset.
    /// For TBCD, the buy function is `(XOR price in reference asset) + 1`
    pub ref_xor_prices: Option<AssetPrices>,
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
pub struct BaseSupply<AccountId> {
    /// Target account for mint/burn for achieving `target` supply
    pub asset_collector: AccountId,
    /// Target total supply of base asset
    pub target: Balance,
}

fn set_reserves<T: Config>(asset: &AssetIdOf<T>, target_reserves: Balance) -> DispatchResult {
    let reserves_tech_account_id =
        multicollateral_bonding_curve_pool::Pallet::<T>::reserves_account_id();
    let reserves_account_id =
        technical::Pallet::<T>::tech_account_id_to_account_id(&reserves_tech_account_id)?;
    let current_reserves_amount: FixedWrapper =
        assets::Pallet::<T>::free_balance(asset, &reserves_account_id)?.into();
    let reserves_delta = target_reserves - current_reserves_amount;
    let reserves_delta = reserves_delta
        .get()
        .map_err(|_| Error::<T>::ArithmeticError)?
        .into_bits();
    pallet_tools::assets::change_balance_by::<T>(&reserves_account_id, asset, reserves_delta)
        .map_err(|e| match e {
            // realistically the error should never be triggered
            pallet_tools::assets::Error::UnknownAsset => Error::<T>::UnknownMcbcAsset.into(),
            pallet_tools::assets::Error::Other(e) => e,
        })?;
    Ok(())
}

fn initialize_single_collateral_unchecked<T: Config>(
    input: OtherCollateralInput<T::AssetId>,
) -> Result<Option<AssetPrices>, DispatchError> {
    let base_asset = T::GetBaseAssetId::get();
    let reference_asset = multicollateral_bonding_curve_pool::ReferenceAssetId::<T>::get();

    if !<T as Config>::TradingPairSourceManager::is_trading_pair_enabled(
        &DEXId::Polkaswap.into(),
        &base_asset,
        &input.asset,
    )? {
        <T as Config>::TradingPairSourceManager::register_pair(
            DEXId::Polkaswap.into(),
            base_asset,
            input.asset,
        )?;
    }

    if !multicollateral_bonding_curve_pool::EnabledTargets::<T>::get().contains(&input.asset) {
        multicollateral_bonding_curve_pool::Pallet::<T>::initialize_pool_unchecked(
            input.asset,
            false,
        )
        .expect("Failed to initialize pool");
    }

    let actual_ref_prices = if let Some(p) = input.parameters.ref_prices {
        Some(pallet_tools::price_tools::setup_reference_prices::<T>(
            &input.asset,
            &reference_asset,
            p,
        )?)
    } else {
        None
    };

    if let Some(target_reserves) = input.parameters.reserves {
        set_reserves::<T>(&input.asset, target_reserves)?;
    }

    Ok(actual_ref_prices)
}

/// Initialize collateral-specific variables in MCBC pricing. Reserves affect the actual sell
/// price, whereas the reference prices (seems like linearly) scale the output.
///
/// Note that TBCD must be initialized via [`initialize_tbcd_collateral`]
///
/// ## Return
/// Due to limited precision of fixed-point numbers, the requested reference prices might not
/// be precisely obtainable. Therefore, actual price of collaterals are returned.
pub fn initialize_single_collateral<T: Config>(
    input: OtherCollateralInput<T::AssetId>,
) -> Result<Option<AssetPrices>, DispatchError> {
    ensure!(
        input.asset != TBCD.into(),
        Error::<T>::IncorrectCollateralAsset
    );
    initialize_single_collateral_unchecked::<T>(input)
}

/// Initialize TBCD collateral asset - a special case in MCBC pallet.
/// In addition, it sets up XOR reference price, since it also affects the results.
///
/// For other parameters see [`initialize_single_collateral`].
///
/// ## Usage note
/// TBCD should be initialized before other collaterals.
///
/// An added parameter for TBCD is `ref_xor_prices` (price tools price of reference asset in XOR).
/// Values calculated for initialization of other collaterals are calculated based on this value.
/// Thus, updating the reference asset price actually affects `quote`/`exchange` of other
/// collaterals, and the price should be set before other initializations.
///
/// The extrinsic call does this in correct order, so this nuance has to be noted only when using
/// the inner functions directly.
///
/// Example of such behaviour can be found in test
/// [`ref_xor_price_update_changes_quote`](crate::tests::mcbc::ref_xor_price_update_changes_quote)
///
/// ## Return
/// See [`initialize_single_collateral`].
pub fn initialize_tbcd_collateral<T: Config>(
    input: TbcdCollateralInput,
) -> Result<Option<AssetPrices>, DispatchError> {
    if let Some(ref_xor_prices) = input.ref_xor_prices {
        let reference_asset = multicollateral_bonding_curve_pool::ReferenceAssetId::<T>::get();
        pallet_tools::price_tools::set_xor_prices::<T>(&reference_asset, ref_xor_prices)?;
    }

    initialize_single_collateral_unchecked::<T>(OtherCollateralInput {
        asset: TBCD.into(),
        parameters: input.parameters,
    })
}

/// Initialize supply of base asset. It is the main variable in the bonding curve pricing formulae.
///
/// For TBCD use [`initialize_tbcd_collateral`]
pub fn initialize_base_supply<T: Config>(input: BaseSupply<T::AccountId>) -> DispatchResult {
    let base_asset_id = &T::GetBaseAssetId::get();
    let current_base_supply: FixedWrapper =
        assets::Pallet::<T>::total_issuance(base_asset_id)?.into();
    let supply_delta = input.target - current_base_supply;
    let supply_delta = supply_delta
        .get()
        .map_err(|_| Error::<T>::ArithmeticError)?
        .into_bits();

    pallet_tools::assets::change_balance_by::<T>(
        &input.asset_collector,
        base_asset_id,
        supply_delta,
    )
    .map_err(|e| match e {
        // realistically the error should never be triggered
        pallet_tools::assets::Error::UnknownAsset => Error::<T>::UnknownMcbcAsset.into(),
        pallet_tools::assets::Error::Other(e) => e,
    })
}
