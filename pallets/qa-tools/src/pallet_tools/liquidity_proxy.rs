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
use common::prelude::{BalanceUnit, FixedWrapper};

pub struct XYKPair<T: Config> {
    pub dex_id: T::DEXId,
    pub asset_a: T::AssetId,
    pub asset_b: T::AssetId,
    /// Price of `asset_a` in terms of `asset_b` (how much `asset_b` is needed to buy 1 `asset_a`)
    pub price: BalanceUnit,
    pub slippage_tolerance: FixedWrapper,
}

impl<T: Config> XYKPair<T> {
    // `price` - Price of `asset_a` in terms of `asset_b` (how much `asset_b` is needed to buy 1
    // `asset_a`)
    pub fn new(
        dex_id: T::DEXId,
        asset_a: T::AssetId,
        asset_b: T::AssetId,
        price: BalanceUnit,
        slippage_tolerance: FixedWrapper,
    ) -> Self {
        Self {
            dex_id,
            asset_a,
            asset_b,
            price,
            slippage_tolerance,
        }
    }
}

pub mod source_initializers {
    use crate::pallet_tools::liquidity_proxy::XYKPair;
    use crate::{Config, Error};
    use common::prelude::{BalanceUnit, FixedWrapper};
    use common::{balance, AssetInfoProvider, XOR};
    use frame_support::dispatch::{DispatchResult, RawOrigin};
    use frame_support::ensure;
    use frame_system::pallet_prelude::BlockNumberFor;
    use order_book::{MomentOf, OrderBookId};
    use sp_runtime::traits::CheckedMul;
    use sp_std::vec::Vec;
    use std::ops::{Mul, Sub};

    pub fn xyk<T: Config + pool_xyk::Config>(
        caller: T::AccountId,
        pairs: Vec<XYKPair<T>>,
    ) -> DispatchResult {
        for XYKPair {
            dex_id,
            asset_a,
            asset_b,
            price,
            slippage_tolerance,
        } in pairs
        {
            if <T as Config>::AssetInfoProvider::is_non_divisible(&asset_a)
                || <T as Config>::AssetInfoProvider::is_non_divisible(&asset_b)
            {
                return Err(Error::<T>::AssetsMustBeDivisible.into());
            }

            // todo: enable trading pair

            pool_xyk::Pallet::<T>::initialize_pool(
                RawOrigin::Signed(caller.clone()).into(),
                dex_id,
                asset_a,
                asset_b,
            )
            .map_err(|e| e.error)?;

            fn subtract_slippage(
                value: BalanceUnit,
                slippage_tolerance: FixedWrapper,
            ) -> BalanceUnit {
                let slippage = BalanceUnit::divisible(slippage_tolerance.try_into_balance()?);
                let slippage = if !value.is_divisible() {
                    slippage.into_divisible()?
                } else {
                    slippage
                };
                value.sub(value.mul(slippage))
            }

            let value_a: BalanceUnit = if asset_a == XOR.into() {
                balance!(1000000).into()
            } else {
                balance!(10000).into()
            };
            let value_b = value_a
                .checked_mul(&price)
                .ok_or(Error::<T>::ArithmeticError)?;
            let value_a_min = subtract_slippage(value_a, slippage_tolerance.clone());
            let value_b_min = subtract_slippage(value_b, slippage_tolerance);
            pool_xyk::Pallet::<T>::deposit_liquidity(
                RawOrigin::Signed(caller.clone()).into(),
                dex_id,
                asset_a,
                asset_b,
                *value_a.balance(),
                *value_b.balance(),
                *value_a_min.balance(),
                *value_b_min.balance(),
            )
            .map_err(|e| e.error)?;
        }
        Ok(())
    }

    /// Create multiple order books with default parameters if do not exist and
    /// fill them according to given parameters.
    ///
    /// Balance for placing the orders is minted automatically, trading pairs are
    /// created if needed.
    ///
    /// Parameters:
    /// - `bids_owner`: Creator of the buy orders placed on the order books,
    /// - `asks_owner`: Creator of the sell orders placed on the order books,
    /// - `settings`: Parameters for creation of the order book and placing the orders in each order book.
    pub fn order_book<T: Config>(
        bids_owner: T::AccountId,
        asks_owner: T::AccountId,
        settings: Vec<(
            OrderBookId<T::AssetId, T::DEXId>,
            settings::OrderBookAttributes,
            settings::OrderBookFill<MomentOf<T>, BlockNumberFor<T>>,
        )>,
    ) -> DispatchResult {
        let creation_settings: Vec<_> = settings
            .iter()
            .map(|(id, attributes, _)| (*id, *attributes))
            .collect();
        for (order_book_id, _) in creation_settings.iter() {
            ensure!(
                !order_book::OrderBooks::<T>::contains_key(order_book_id),
                crate::Error::<T>::OrderBookAlreadyExists
            );
        }
        crate::pallet_tools::order_book::create_multiple_empty_unchecked::<T>(creation_settings)?;

        let orders_settings: Vec<_> = settings
            .into_iter()
            .map(|(id, _, fill_settings)| (id, fill_settings))
            .collect();
        crate::pallet_tools::order_book::fill_multiple_empty_unchecked::<T>(
            bids_owner,
            asks_owner,
            orders_settings,
        )?;
        Ok(())
    }
}

pub mod source_filling {
    use crate::{settings, Config};
    use frame_support::dispatch::DispatchResult;
    use frame_support::ensure;
    use frame_system::pallet_prelude::BlockNumberFor;
    use order_book::{MomentOf, OrderBookId};
    use sp_std::vec::Vec;

    /// Fill the order books according to given parameters.
    ///
    /// Balance for placing the orders is minted automatically.
    ///
    /// Parameters:
    /// - `bids_owner`: Creator of the buy orders placed on the order books,
    /// - `asks_owner`: Creator of the sell orders placed on the order books,
    /// - `settings`: Parameters for placing the orders in each order book.
    pub fn order_book<T: Config>(
        bids_owner: T::AccountId,
        asks_owner: T::AccountId,
        settings: Vec<(
            OrderBookId<T::AssetId, T::DEXId>,
            settings::OrderBookFill<MomentOf<T>, BlockNumberFor<T>>,
        )>,
    ) -> DispatchResult {
        for (order_book_id, _) in settings.iter() {
            ensure!(
                order_book::OrderBooks::<T>::contains_key(order_book_id),
                crate::Error::<T>::CannotFillUnknownOrderBook
            );
        }
        crate::pallet_tools::order_book::fill_multiple_empty_unchecked::<T>(
            bids_owner, asks_owner, settings,
        )?;
        Ok(())
    }
}
