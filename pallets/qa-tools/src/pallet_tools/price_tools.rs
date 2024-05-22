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

use crate::{Config, Error};
use codec::{Decode, Encode};
use common::prelude::BalanceUnit;
use common::{balance, AssetIdOf, Balance, PriceToolsProvider, PriceVariant, XOR};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::ensure;
use sp_arithmetic::traits::{CheckedDiv, One};

/// Directly set buy & sell XOR prices of `asset_id` (prices of XOR in terms of `asset_id`);
/// verifying the values beforehand.
pub fn set_xor_prices<T: Config>(
    asset_id: &AssetIdOf<T>,
    xor_prices: AssetPrices,
) -> DispatchResult {
    ensure!(
        xor_prices.buy >= xor_prices.sell,
        Error::<T>::BuyLessThanSell
    );
    set_price_unchecked::<T>(asset_id, xor_prices.buy, PriceVariant::Buy)?;
    set_price_unchecked::<T>(asset_id, xor_prices.sell, PriceVariant::Sell)?;
    Ok(())
}

/// Set XOR prices of `asset_id` in `price_tools` given prices of `asset_id` in terms of
/// `reference_asset_id`.
///
/// Returns actual achieved prices of `asset_id` in terms of `reference_asset_id`.
pub fn setup_reference_prices<T: Config>(
    asset_id: &AssetIdOf<T>,
    reference_asset_id: &AssetIdOf<T>,
    ref_prices: AssetPrices,
) -> Result<AssetPrices, DispatchError> {
    let xor_prices = calculate_xor_prices::<T>(
        asset_id,
        reference_asset_id,
        ref_prices.buy,
        ref_prices.sell,
    )?;
    let actual_prices = relative_prices::<T>(&xor_prices)?;
    let CalculatedXorPrices {
        asset_a: collateral_xor_prices,
        asset_b: _,
    } = xor_prices;
    set_xor_prices::<T>(asset_id, collateral_xor_prices)?;
    Ok(actual_prices)
}

/// Set price for the asset in `price_tools` ignoring internal limits on change of the price.
/// Note that the values are not checked for sanity; it's possible to set values that result in
/// crossed market.
pub(crate) fn set_price_unchecked<T: Config>(
    asset_id: &AssetIdOf<T>,
    price: Balance,
    variant: PriceVariant,
) -> DispatchResult {
    let _ = price_tools::Pallet::<T>::register_asset(asset_id);

    // feed failures in order to ignore the limits
    for _ in 0..price_tools::AVG_BLOCK_SPAN {
        price_tools::Pallet::<T>::incoming_spot_price_failure(asset_id, variant);
    }

    for _ in 0..price_tools::AVG_BLOCK_SPAN + 1 {
        price_tools::Pallet::<T>::incoming_spot_price(asset_id, price, variant)?;
    }
    Ok(())
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
pub struct AssetPrices {
    pub buy: Balance,
    pub sell: Balance,
}

/// Amount of the asset per 1 XOR. The same format as used in price tools.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CalculatedXorPrices {
    /// Amount of asset A per XOR
    pub asset_a: AssetPrices,
    /// Amount of asset B per XOR
    pub asset_b: AssetPrices,
}

/// calculates prices of A in terms of B (B per A) given XOR prices of both
pub fn relative_prices<T: Config>(
    xor_prices: &CalculatedXorPrices,
) -> Result<AssetPrices, DispatchError> {
    // formulae from `price_tools::get_average_price`
    let quote_a_buy = BalanceUnit::one()
        .checked_div(&BalanceUnit::divisible(xor_prices.asset_a.sell))
        .ok_or(Error::<T>::ArithmeticError)?;
    let quote_b_buy = BalanceUnit::divisible(xor_prices.asset_b.buy);
    let quote_a_sell = BalanceUnit::one()
        .checked_div(&BalanceUnit::divisible(xor_prices.asset_a.buy))
        .ok_or(Error::<T>::ArithmeticError)?;
    let quote_b_sell = BalanceUnit::divisible(xor_prices.asset_b.sell);

    Ok(AssetPrices {
        buy: *(quote_a_buy * quote_b_buy).balance(),
        sell: *(quote_a_sell * quote_b_sell).balance(),
    })
}
/// Calculate prices of XOR in the assets A and B given the expected relative price A in terms of B.
/// The resulting prices can be directly used for [`set_xor_prices`]/`price_tools::incoming_spot_price`,
/// as they require prices of XOR in terms of an asset.
///
/// Note that if both A and B != XOR, then B must already have some price in the `price_tools`.
pub fn calculate_xor_prices<T: Config>(
    asset_a: &AssetIdOf<T>,
    asset_b: &AssetIdOf<T>,
    b_per_a_buy: Balance,
    b_per_a_sell: Balance,
) -> Result<CalculatedXorPrices, DispatchError> {
    match (asset_a, asset_b) {
        (xor, _b) if xor == &XOR.into() => {
            Ok(CalculatedXorPrices {
                // xor
                asset_a: AssetPrices {
                    buy: balance!(1),
                    sell: balance!(1),
                },
                // price of xor in b
                asset_b: AssetPrices {
                    buy: b_per_a_buy,
                    sell: b_per_a_sell,
                },
            })
        }
        (_a, xor) if xor == &XOR.into() => {
            // Variant is inverted, just like in `price_tools`
            let a_per_xor_sell = *BalanceUnit::one()
                .checked_div(&BalanceUnit::divisible(b_per_a_buy))
                .ok_or::<Error<T>>(Error::<T>::ArithmeticError)?
                .balance();
            let a_per_xor_buy = *BalanceUnit::one()
                .checked_div(&BalanceUnit::divisible(b_per_a_sell))
                .ok_or::<Error<T>>(Error::<T>::ArithmeticError)?
                .balance();
            Ok(CalculatedXorPrices {
                // price of xor in a
                asset_a: AssetPrices {
                    buy: a_per_xor_buy,
                    sell: a_per_xor_sell,
                },
                // xor
                asset_b: AssetPrices {
                    buy: balance!(1),
                    sell: balance!(1),
                },
            })
        }
        (_a, _b) => {
            // To obtain XOR prices, these formulae should be followed:
            //
            // Buy:
            // (A -buy-> B) = (A -buy-> XOR) * (XOR -buy-> B) =
            // = (1 / (XOR -sell-> A)) * (XOR -buy-> B)
            //
            // Sell:
            // (A -sell-> B) = (A -sell-> XOR) * (XOR -sell-> B) =
            // = (1 / (XOR -buy-> A)) * (XOR -sell-> B)

            // in the code notation "A -sell-> B" is represented by `a_sell_b`
            // because it's easier to comprehend the formulae with this instead of `b_per_a_sell`

            // Get known values from the formula:
            let a_buy_b = BalanceUnit::divisible(b_per_a_buy);
            let xor_buy_b = price_tools::Pallet::<T>::get_average_price(
                &XOR.into(),
                asset_b,
                PriceVariant::Buy,
            )
            .map_err(|_| Error::<T>::ReferenceAssetPriceNotFound)?;
            let xor_buy_b = BalanceUnit::divisible(xor_buy_b);
            let a_sell_b = BalanceUnit::divisible(b_per_a_sell);
            let xor_sell_b = price_tools::Pallet::<T>::get_average_price(
                &XOR.into(),
                asset_b,
                PriceVariant::Sell,
            )
            .map_err(|_| Error::<T>::ReferenceAssetPriceNotFound)?;
            let xor_sell_b = BalanceUnit::divisible(xor_sell_b);

            // Buy:
            // (A -buy-> B) = (XOR -buy-> B) / (XOR -sell-> A)
            //
            // known:
            // (A -buy-> B), (XOR -buy-> B)
            //
            // solving for unknown:
            // (XOR -sell-> A) = (XOR -buy-> B) / (A -buy-> B)
            let xor_sell_a = xor_buy_b / a_buy_b;

            // Sell:
            // (A -sell-> B) = (XOR -sell-> B) / (XOR -buy-> A)
            //
            // known:
            // (A -sell-> B), (XOR -sell-> B)
            //
            // solving for unknown:
            // (XOR -buy-> A) = (XOR -sell-> B) / (A -sell-> B)
            let xor_buy_a = xor_sell_b / a_sell_b;
            Ok(CalculatedXorPrices {
                // xor
                asset_a: AssetPrices {
                    buy: *xor_buy_a.balance(),
                    sell: *xor_sell_a.balance(),
                },
                asset_b: AssetPrices {
                    buy: *xor_buy_b.balance(),
                    sell: *xor_sell_b.balance(),
                },
            })
        }
    }
}
