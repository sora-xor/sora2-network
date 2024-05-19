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

use frame_support::dispatch::DispatchError;
use frame_support::ensure;

use assets::AssetIdOf;
use common::prelude::{Balance, Fixed, FixedWrapper};
use common::{fixed_wrapper, AssetInfoProvider, TradingPair};

use crate::{to_balance, to_fixed_wrapper};

use crate::{Config, Error, Pallet};

impl<T: Config> Pallet<T> {
    // https://github.com/Uniswap/uniswap-v2-periphery/blob/dda62473e2da448bc9cb8f4514dadda4aeede5f4/contracts/libraries/UniswapV2Library.sol#L36
    // Original uniswap code.

    /// Given some amount of an asset and pair reserves, returns an equivalent amount of the other asset.
    pub fn calculate_quote(
        amount_a: &Balance,
        reserve_a: &Balance,
        reserve_b: &Balance,
    ) -> Result<Balance, DispatchError> {
        Ok(to_balance!(
            (to_fixed_wrapper!(amount_a) * to_fixed_wrapper!(reserve_b))
                / to_fixed_wrapper!(reserve_a)
        ))
    }

    // https://github.com/Uniswap/uniswap-v2-periphery/blob/dda62473e2da448bc9cb8f4514dadda4aeede5f4/contracts/UniswapV2Router02.sol#L48
    // Original uniswap code.

    /// Calculate optimal deposit using pool reserves and desired value.
    /// Pool reserves used to calculate it and quote, so important that information about pool
    /// reserves is used.
    /// Only one side is corrected, better is selected.
    pub fn calculate_optimal_deposit(
        _total_supply: Balance,
        reserve_a: Balance,
        reserve_b: Balance,
        amount_a_desired: Balance,
        amount_b_desired: Balance,
        amount_a_min: Balance,
        amount_b_min: Balance,
    ) -> Result<(Balance, Balance), DispatchError> {
        let opt_am_a_des = Pallet::<T>::calculate_quote(&amount_b_desired, &reserve_b, &reserve_a)?;
        let opt_am_b_des = Pallet::<T>::calculate_quote(&amount_a_desired, &reserve_a, &reserve_b)?;
        if opt_am_b_des <= amount_b_desired {
            ensure!(
                opt_am_b_des >= amount_b_min,
                Error::<T>::ImpossibleToDecideValidPairValuesFromRangeForThisPool
            );
            Ok((amount_a_desired, opt_am_b_des))
        } else {
            ensure!(
                opt_am_a_des >= amount_a_min && opt_am_a_des <= amount_a_desired,
                Error::<T>::ImpossibleToDecideValidPairValuesFromRangeForThisPool
            );
            Ok((opt_am_a_des, amount_b_desired))
        }
    }

    // https://github.com/Uniswap/uniswap-v2-core/blob/4dd59067c76dea4a0e8e4bfdda41877a6b16dedc/contracts/UniswapV2Pair.sol#L123
    // Original uniswap code.

    /// Additional function to calculate deposit liquidity, that using total_supply to calculate
    /// amount of pool tokens (liquidity markers).
    pub fn calc_deposit_liquidity_1(
        total_supply: Balance,
        reserve_a: Balance,
        reserve_b: Balance,
        amount_a_desired: Balance,
        amount_b_desired: Balance,
        amount_a_min: Balance,
        amount_b_min: Balance,
    ) -> Result<(Balance, Balance, Balance), DispatchError> {
        let (am_a_des, am_b_des) = Pallet::<T>::calculate_optimal_deposit(
            total_supply,
            reserve_a,
            reserve_b,
            amount_a_desired,
            amount_b_desired,
            amount_a_min,
            amount_b_min,
        )?;
        let lhs = to_balance!(
            to_fixed_wrapper!(am_a_des) * to_fixed_wrapper!(total_supply)
                / to_fixed_wrapper!(reserve_a)
        );
        let rhs = to_balance!(
            to_fixed_wrapper!(am_b_des) * to_fixed_wrapper!(total_supply)
                / to_fixed_wrapper!(reserve_b)
        );
        let min_value = lhs.min(rhs);
        Ok((am_a_des, am_b_des, min_value))
    }

    /// Calulate (y_output,fee) pair where fee can be fee_of_y1 or fee_of_x_in, and output is
    /// without fee.
    pub fn calc_output_for_exact_input(
        fee_fraction: Fixed,
        get_fee_from_destination: bool,
        x: &Balance,
        y: &Balance,
        x_in: &Balance,
        deduce_fee: bool,
    ) -> Result<(Balance, Balance), DispatchError> {
        let fxw_x = FixedWrapper::from(x.clone());
        let fxw_y = FixedWrapper::from(y.clone());
        let fxw_x_in = FixedWrapper::from(x_in.clone());
        if get_fee_from_destination {
            // output token is xor, user indicates desired input amount
            // y_1 = (x_in * y) / (x + x_in)
            // y_out = y_1 * (1 - fee)
            let nominator = fxw_x_in.clone() * fxw_y;
            let denominator = fxw_x + fxw_x_in;
            let y_out_with_fee = nominator / denominator;
            let y_out = if deduce_fee {
                y_out_with_fee.clone() * (fixed_wrapper!(1) - fee_fraction)
            } else {
                y_out_with_fee.clone()
            };
            Ok((
                to_balance!(y_out.clone()),
                to_balance!(y_out_with_fee - y_out),
            ))
        } else {
            // input token is xor, user indicates desired input amount
            // x_1 = x_in * (1 - fee)
            // y_out = (x_1 * y) / (x + x_1)
            let x_in_without_fee = if deduce_fee {
                fxw_x_in.clone() * (fixed_wrapper!(1) - fee_fraction)
            } else {
                fxw_x_in.clone()
            };
            let nominator = x_in_without_fee.clone() * fxw_y;
            let denominator = fxw_x + x_in_without_fee.clone();
            let y_out = nominator / denominator;
            Ok((to_balance!(y_out), to_balance!(fxw_x_in - x_in_without_fee)))
        }
    }

    /// Calculates (x_input,fee) pair where fee can be fee_of_y1 or fee_of_x_in, and input is
    /// without fee.
    pub fn calc_input_for_exact_output(
        fee_fraction: Fixed,
        get_fee_from_destination: bool,
        x: &Balance,
        y: &Balance,
        y_out: &Balance,
        deduce_fee: bool,
    ) -> Result<(Balance, Balance), DispatchError> {
        let fxw_x = FixedWrapper::from(x.clone());
        let fxw_y = FixedWrapper::from(y.clone());
        let fxw_y_out = FixedWrapper::from(y_out.clone());
        if get_fee_from_destination {
            // output token is xor, user indicates desired output amount:
            // y_1 = y_out / (1 - fee)
            // x_in = (x * y_1) / (y - y_1)
            let fxw_y_out = fxw_y_out.clone() + Fixed::from_bits(1); // by 1 correction to overestimate required input
            let y_out_with_fee = if deduce_fee {
                fxw_y_out.clone() / (fixed_wrapper!(1) - fee_fraction)
            } else {
                fxw_y_out.clone()
            };
            let nominator = fxw_x * y_out_with_fee.clone();
            let denominator = fxw_y - y_out_with_fee.clone();
            let x_in = nominator / denominator;
            Ok((to_balance!(x_in), to_balance!(y_out_with_fee - fxw_y_out)))
        } else {
            // input token is xor, user indicates desired output amount:
            // x_in * (1 - fee) = (x * y_out) / (y - y_out)
            let fxw_y_out = fxw_y_out.clone() + Fixed::from_bits(1); // by 1 correction to overestimate required input
            let nominator = fxw_x * fxw_y_out.clone();
            let denominator = fxw_y - fxw_y_out;
            let x_in_without_fee = nominator / denominator;
            let x_in = if deduce_fee {
                x_in_without_fee.clone() / (fixed_wrapper!(1) - fee_fraction)
            } else {
                x_in_without_fee.clone()
            };
            Ok((
                to_balance!(x_in.clone()),
                to_balance!(x_in - x_in_without_fee),
            ))
        }
    }

    pub fn get_base_asset_part_from_pool_account(
        pool_acc: &T::AccountId,
        trading_pair: &TradingPair<AssetIdOf<T>>,
        liq_amount: Balance,
    ) -> Result<Balance, DispatchError> {
        let b_in_pool = <T as Config>::AssetInfoProvider::free_balance(
            &trading_pair.base_asset_id.into(),
            pool_acc,
        )?;
        let t_in_pool = <T as Config>::AssetInfoProvider::free_balance(
            &trading_pair.target_asset_id.into(),
            pool_acc,
        )?;
        let fxw_liq_in_pool =
            to_fixed_wrapper!(b_in_pool).multiply_and_sqrt(&to_fixed_wrapper!(t_in_pool));
        let fxw_piece = fxw_liq_in_pool / to_fixed_wrapper!(liq_amount);
        let fxw_value = to_fixed_wrapper!(b_in_pool) / fxw_piece;
        let value = to_balance!(fxw_value);
        Ok(value)
    }
}
