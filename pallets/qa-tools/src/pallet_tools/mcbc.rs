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
use assets::AssetIdOf;
use codec::{Decode, Encode};
use common::prelude::FixedWrapper;
use common::{AssetInfoProvider, Balance, PriceVariant, TBCD};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::ensure;
use frame_support::traits::Get;
use pallet_tools::price_tools::{AssetPrices, CalculatedXorPrices};

/// Input for initializing collateral assets except TBCD.
#[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
pub struct OtherCollateralInput<AssetId> {
    /// Collateral asset id
    pub asset: AssetId,
    /// Price of collateral in terms of reference asset. Linearly affects the exchange amounts.
    /// (if collateral costs 10x more sell output should be 10x smaller)
    pub ref_prices: Option<AssetPrices>,
    /// Desired amount of collateral asset in the MCBC reserve account. Affects actual sell
    /// price according to formulae.
    pub reserves: Option<Balance>,
}

/// Input for initializing TBCD collateral.
#[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
pub struct TbcdCollateralInput {
    /// Price of collateral in terms of reference asset. Linearly affects the exchange amounts.
    /// (if collateral costs 10x more sell output should be 10x smaller)
    pub ref_prices: Option<AssetPrices>,
    /// Desired amount of collateral asset in the MCBC reserve account. Affects actual sell
    /// price according to formulae.
    pub reserves: Option<Balance>,
    pub xor_ref_prices: AssetPrices,
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
pub struct BaseSupply<AccountId> {
    pub base_supply_collector: AccountId,
    pub new_base_supply: Balance,
}

fn set_reference_prices<T: Config>(
    asset_id: AssetIdOf<T>,
    reference_asset_id: AssetIdOf<T>,
    ref_prices: AssetPrices,
) -> AssetPrices {
    let xor_prices = pallet_tools::price_tools::calculate_xor_prices::<T>(
        &asset_id,
        &reference_asset_id,
        ref_prices.buy,
        ref_prices.sell,
    )?;
    let actual_prices = pallet_tools::price_tools::relative_prices::<T>(&xor_prices)?;
    let CalculatedXorPrices {
        asset_a: collateral_xor_prices,
        asset_b: _,
    } = xor_prices;

    ensure!(
        collateral_xor_prices.buy >= collateral_xor_prices.sell,
        Error::<T>::BuyLessThanSell
    );
    pallet_tools::price_tools::set_price::<T>(
        &asset_id,
        collateral_xor_prices.buy,
        PriceVariant::Buy,
    )?;
    pallet_tools::price_tools::set_price::<T>(
        &asset_id,
        collateral_xor_prices.sell,
        PriceVariant::Sell,
    )?;
    actual_prices
}

/// Initialize collateral-specific variables in MCBC pricing. Reserves affect the actual sell
/// price, whereas the reference prices (seems like linearly) scale the output.
///
/// ## Return
/// Due to limited precision of fixed-point numbers, the requested reference prices might not
/// be precisely obtainable. Therefore, actual price of collaterals are returned.
pub fn initialize_single_collateral<T: Config>(
    input: OtherCollateralInput<T::AssetId>,
) -> Result<Option<AssetPrices>, DispatchError> {
    let reference_asset = multicollateral_bonding_curve_pool::ReferenceAssetId::<T>::get();

    // todo: register TP if not exist
    // TradingPair::register(
    //     RuntimeOrigin::signed(alice()),
    //     DEXId::Polkaswap.into(),
    //     XOR,
    //     VAL,
    // )
    // .expect("Failed to register trading pair.");
    // TradingPair::register(
    //     RuntimeOrigin::signed(alice()),
    //     DEXId::Polkaswap.into(),
    //     XOR,
    //     XSTUSD,
    // )
    // .expect("Failed to register trading pair.");

    // todo: initialize pool if not already
    // MBCPool::initialize_pool_unchecked(VAL, false).expect("Failed to initialize pool.");

    let actual_ref_prices = input
        .ref_prices
        .map(|p| set_reference_prices(input.asset, reference_asset, p));
    // initialize reserves

    // todo: register account if not present???
    // let bonding_curve_tech_account_id = TechAccountId::Pure(
    //     DEXId::Polkaswap,
    //     TechPurpose::Identifier(b"bonding_curve_tech_account_id".to_vec()),
    // );
    // Technical::register_tech_account_id(bonding_curve_tech_account_id.clone())?;
    // MBCPool::set_reserves_account_id(bonding_curve_tech_account_id.clone())?;

    // todo: use traits where possible (not only here, in whole pallet)

    // let pool_reference_amount = reserve_amount_expected * ratio;
    // let pool_reference_amount = pool_reference_amount
    //     .try_into_balance()
    //     .map_err(|_| Error::<T>::ArithmeticError)?;
    // let pool_val_amount = <T as Config>::LiquidityProxy::quote(
    //     DEXId::Polkaswap.into(),
    //     &reference_asset,
    //     &input.asset,
    //     QuoteAmount::with_desired_input(pool_reference_amount),
    //     LiquiditySourceFilter::empty(DEXId::Polkaswap.into()),
    //     true,
    // )?;

    // let reserves_account =
    //     multicollateral_bonding_curve_pool::Pallet::<T>::reserves_account_id();
    // technical::Pallet::<T>::mint(&input.asset, &reserves_account, pool_val_amount.amount)?;

    Ok(actual_ref_prices)
}

/// Initialize TBCD collateral asset - a special case in MCBC pallet.
/// In addition, it sets up XOR reference price, since it also affects the results.
///
/// For other parameters see [`initialize_single_collateral`].
///
/// ## Return
/// See [`initialize_single_collateral`].
pub fn initialize_tbcd_collateral<T: Config>(
    input: TbcdCollateralInput,
) -> Result<Option<AssetPrices>, DispatchError> {
    // handle xor ref price
    // input.xor_ref_prices

    initialize_single_collateral::<T>(OtherCollateralInput {
        asset: TBCD.into(),
        ref_prices: input.ref_prices,
        reserves: input.reserves,
    })
}

/// Initialize supply of base asset. It is the main variable in the bonding curve pricing formulae.
///
/// For TBCD use [`initialize_tbcd_collateral`]
pub fn initialize_base_supply<T: Config>(input: BaseSupply<T::AccountId>) -> DispatchResult {
    let base_asset_id = &T::GetBaseAssetId::get();
    let current_base_supply: FixedWrapper =
        assets::Pallet::<T>::total_issuance(base_asset_id)?.into();
    let supply_delta = input.new_base_supply - current_base_supply;
    let supply_delta = supply_delta
        .get()
        .map_err(|_| Error::<T>::ArithmeticError)?
        .into_bits();

    // realistically the error should never be triggered
    let owner =
        assets::Pallet::<T>::asset_owner(&base_asset_id).ok_or(Error::<T>::UnknownMCBCAsset)?;
    if supply_delta > 0 {
        let mint_amount = supply_delta
            .try_into()
            .map_err(|_| Error::<T>::ArithmeticError)?;
        assets::Pallet::<T>::mint_to(
            base_asset_id,
            &owner,
            &input.base_supply_collector,
            mint_amount,
        )?;
    } else if supply_delta < 0 {
        let burn_amount = supply_delta
            .abs()
            .try_into()
            .map_err(|_| Error::<T>::ArithmeticError)?;
        assets::Pallet::<T>::burn_from(
            base_asset_id,
            &owner,
            &input.base_supply_collector,
            burn_amount,
        )?;
    }
    Ok(())
}
