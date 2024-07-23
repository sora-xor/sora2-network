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
use common::{
    balance, AssetIdOf, AssetInfoProvider, AssetManager, Balance, DEXInfo, DexIdOf,
    DexInfoProvider, TradingPair, TradingPairSourceManager, XOR,
};
use frame_support::dispatch::RawOrigin;
use sp_arithmetic::traits::CheckedMul;
use sp_runtime::DispatchError;
use sp_std::fmt::Debug;
use sp_std::vec::Vec;

#[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
pub struct AssetPairInput<DEXId, AssetId> {
    pub dex_id: DEXId,
    pub asset_a: AssetId,
    pub asset_b: AssetId,
    /// Price of `asset_a` in terms of `asset_b` (how much `asset_b` is needed to buy 1 `asset_a`)
    pub price: Balance,
    /// Custom amount of `asset_a` reserves. If not defined, the default value is used. `asset_b` reserves are calculated with `asset_a` reserves and `price`.
    pub maybe_asset_a_reserves: Option<Balance>,
}

impl<DEXId, AssetId> AssetPairInput<DEXId, AssetId> {
    // `price` - Price of `asset_a` in terms of `asset_b` (how much `asset_b` is needed to buy 1 `asset_a`)
    pub fn new(
        dex_id: DEXId,
        asset_a: AssetId,
        asset_b: AssetId,
        price: Balance,
        maybe_asset_a_reserves: Option<Balance>,
    ) -> Self {
        Self {
            dex_id,
            asset_a,
            asset_b,
            price,
            maybe_asset_a_reserves,
        }
    }
}

/// `None` if neither of the assets is base
fn trading_pair_from_asset_ids<T: Config>(
    dex_info: DEXInfo<AssetIdOf<T>>,
    asset_a: AssetIdOf<T>,
    asset_b: AssetIdOf<T>,
) -> Option<TradingPair<AssetIdOf<T>>> {
    if asset_a == dex_info.base_asset_id {
        Some(TradingPair {
            base_asset_id: asset_a,
            target_asset_id: asset_b,
        })
    } else if asset_b == dex_info.base_asset_id {
        Some(TradingPair {
            base_asset_id: asset_b,
            target_asset_id: asset_a,
        })
    } else {
        None
    }
}

/// Initialize xyk liquidity source for multiple asset pairs at once.
///
/// ## Return
///
/// Due to limited precision of fixed-point numbers, the requested price might not be precisely
/// obtainable. Therefore, actual resulting price is returned.
///
/// Note: with current implementation the prices should always be equal
pub fn initialize<T: Config + pool_xyk::Config>(
    caller: T::AccountId,
    pairs: Vec<AssetPairInput<DexIdOf<T>, AssetIdOf<T>>>,
) -> Result<Vec<AssetPairInput<DexIdOf<T>, AssetIdOf<T>>>, DispatchError> {
    let mut actual_prices = pairs.clone();
    for (
        AssetPairInput {
            dex_id,
            asset_a,
            asset_b,
            price: expected_price,
            maybe_asset_a_reserves,
        },
        AssetPairInput {
            price: actual_price,
            ..
        },
    ) in pairs.into_iter().zip(actual_prices.iter_mut())
    {
        if <T as Config>::AssetInfoProvider::is_non_divisible(&asset_a)
            || <T as Config>::AssetInfoProvider::is_non_divisible(&asset_b)
        {
            return Err(Error::<T>::AssetsMustBeDivisible.into());
        }

        let dex_info = <T as Config>::DexInfoProvider::get_dex_info(&dex_id)?;
        let trading_pair = trading_pair_from_asset_ids::<T>(dex_info, asset_a, asset_b)
            .ok_or(pool_xyk::Error::<T>::BaseAssetIsNotMatchedWithAnyAssetArguments)?;

        if !<T as Config>::TradingPairSourceManager::is_trading_pair_enabled(
            &dex_id,
            &trading_pair.base_asset_id,
            &trading_pair.target_asset_id,
        )? {
            <T as Config>::TradingPairSourceManager::register_pair(
                dex_id,
                trading_pair.base_asset_id,
                trading_pair.target_asset_id,
            )?
        }

        pool_xyk::Pallet::<T>::initialize_pool(
            RawOrigin::Signed(caller.clone()).into(),
            dex_id,
            asset_a,
            asset_b,
        )
        .map_err(|e| e.error)?;

        let value_a: BalanceUnit = if let Some(asset_a_reserves) = maybe_asset_a_reserves {
            asset_a_reserves.into()
        } else {
            // Some magic numbers taken from existing init code
            // https://github.com/soramitsu/sora2-api-tests/blob/f590995abbd3b191a57b988ba3c10607a89d6f89/tests/testAccount/mintTokensForPairs.test.ts#L136
            if asset_a == XOR.into() {
                balance!(1000000).into()
            } else {
                balance!(10000).into()
            }
        };

        let price = BalanceUnit::divisible(expected_price);
        let value_b = value_a
            .checked_mul(&price)
            .ok_or(Error::<T>::ArithmeticError)?;

        T::AssetManager::mint_unchecked(&asset_a, &caller, *value_a.balance())?;
        T::AssetManager::mint_unchecked(&asset_b, &caller, *value_b.balance())?;

        *actual_price = *(value_b / value_a).balance();
        pool_xyk::Pallet::<T>::deposit_liquidity(
            RawOrigin::Signed(caller.clone()).into(),
            dex_id,
            asset_a,
            asset_b,
            *value_a.balance(),
            *value_b.balance(),
            // no need for range when the pool is empty
            *value_a.balance(),
            *value_b.balance(),
        )
        .map_err(|e| e.error)?;
    }
    Ok(actual_prices)
}
