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

#![cfg_attr(not(feature = "std"), no_std)]

pub mod weights;

mod benchmarking;

#[allow(unused_imports)]
#[macro_use]
extern crate alloc;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use common::prelude::{
    Balance, Fixed, FixedWrapper, LiquiditySourceType, PriceToolsPallet, QuoteAmount,
};
use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use common::{
    balance, fixed_const, fixed_wrapper, DEXId, LiquiditySourceFilter, DAI, ETH, PSWAP, VAL, XOR,
};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::ensure;
use frame_support::weights::Weight;
use liquidity_proxy::LiquidityProxyTrait;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::collections::vec_deque::VecDeque;
use sp_std::convert::TryInto;

pub use pallet::*;

/// Count of blocks to participate in avg value calculation.
pub const AVG_BLOCK_SPAN: u32 = 30;
/// Max percentage difference for average value between blocks.
const MAX_BLOCK_AVG_DIFFERENCE: Fixed = fixed_const!(0.005); // 0.5%

pub trait WeightInfo {
    fn on_initialize(elems: u32) -> Weight;
}

impl crate::WeightInfo for () {
    fn on_initialize(_elems: u32) -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use liquidity_proxy::LiquidityProxyTrait;

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + assets::Config
        + common::Config
        + technical::Config
        + pool_xyk::Config
        + trading_pair::Config
    {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type LiquidityProxy: LiquidityProxyTrait<Self::DEXId, Self::AccountId, Self::AssetId>;
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_block_num: T::BlockNumber) -> Weight {
            let elems = Module::<T>::average_prices_calculation_routine();
            <T as Config>::WeightInfo::on_initialize(elems)
        }

        fn on_runtime_upgrade() -> Weight {
            match Pallet::<T>::storage_version() {
                // if pallet didn't exist, i.e. added with runtime upgrade, then initial tbc assets should be created
                None => {
                    EnabledTargets::<T>::mutate(|set| {
                        *set = [VAL.into(), PSWAP.into(), DAI.into(), ETH.into()]
                            .iter()
                            .cloned()
                            .collect()
                    });
                }
                _ => (),
            };
            T::DbWeight::get().writes(1)
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // no extrinsics
    }

    #[pallet::event]
    #[pallet::metadata()]
    // #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        // no events
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Failed to calculate new average price.
        AveragePriceCalculationFailed,
        /// Failed to add new spot price to average.
        UpdateAverageWithSpotPriceFailed,
        /// Either spot price records has been reset or not initialized yet. Wait till spot price
        /// quote is recovered and span is recalculated.
        InsufficientSpotPriceData,
        /// Requested quote path is not supported.
        UnsupportedQuotePath,
        /// Failed to perform quote to get average price.
        FailedToQuoteAveragePrice,
        /// AssetId has been already registered.
        AssetAlreadyRegistered,
    }

    /// For pair XOR-AssetB, stores prices of XOR in terms of AssetB.
    #[pallet::storage]
    #[pallet::getter(fn spot_prices)]
    pub type SpotPrices<T: Config> =
        StorageMap<_, Identity, T::AssetId, VecDeque<Balance>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn spot_price_failures)]
    pub type SpotPriceFailures<T: Config> = StorageMap<_, Identity, T::AssetId, u32, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn average_price)]
    pub type AveragePrice<T: Config> = StorageMap<_, Identity, T::AssetId, Balance, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn enabled_assets)]
    pub type EnabledTargets<T: Config> = StorageValue<_, BTreeSet<T::AssetId>, ValueQuery>;
}

impl<T: Config> Pallet<T> {
    /// Query averaged price from past data for supported paths, i.e. paths with enabled targets or XOR.
    pub fn get_average_price(
        input_asset: &T::AssetId,
        output_asset: &T::AssetId,
    ) -> Result<Balance, DispatchError> {
        let enabled_targets = EnabledTargets::<T>::get();
        let avg_count: usize = AVG_BLOCK_SPAN
            .try_into()
            .map_err(|_| Error::<T>::FailedToQuoteAveragePrice)?;
        if input_asset == &XOR.into() {
            ensure!(
                enabled_targets.contains(output_asset),
                Error::<T>::UnsupportedQuotePath
            );
            ensure!(
                SpotPrices::<T>::get(output_asset).len() == avg_count,
                Error::<T>::InsufficientSpotPriceData
            );
            Ok(AveragePrice::<T>::get(output_asset))
        } else if output_asset == &XOR.into() {
            ensure!(
                enabled_targets.contains(input_asset),
                Error::<T>::UnsupportedQuotePath
            );
            ensure!(
                SpotPrices::<T>::get(input_asset).len() == avg_count,
                Error::<T>::InsufficientSpotPriceData
            );
            Ok((fixed_wrapper!(1) / AveragePrice::<T>::get(input_asset))
                .try_into_balance()
                .map_err(|_| Error::<T>::FailedToQuoteAveragePrice)?)
        } else {
            let quote_a = FixedWrapper::from(Self::get_average_price(input_asset, &XOR.into())?);
            let quote_b = FixedWrapper::from(Self::get_average_price(&XOR.into(), output_asset)?);
            (quote_a * quote_b)
                .try_into_balance()
                .map_err(|_| Error::<T>::FailedToQuoteAveragePrice.into())
        }
    }

    /// Add new price to queue and recalculate average.
    pub fn incoming_spot_price(asset_id: &T::AssetId, price: Balance) -> DispatchResult {
        // reset failure streak for spot prices if needed
        SpotPriceFailures::<T>::mutate(asset_id, |val| {
            if *val > 0 {
                *val = 0
            }
        });
        SpotPrices::<T>::mutate(asset_id, |vec| {
            let avg_count: usize = AVG_BLOCK_SPAN
                .try_into()
                .map_err(|_| Error::<T>::UpdateAverageWithSpotPriceFailed)?;
            // spot price history is consistent, normal behavior
            if vec.len() == avg_count {
                let old_value = vec.pop_front().unwrap();
                vec.push_back(price);
                let curr_avg = AveragePrice::<T>::get(asset_id);
                let mut new_avg =
                    Self::replace_in_average(curr_avg, old_value, price, AVG_BLOCK_SPAN)?;
                new_avg = Self::adjust_to_difference(curr_avg, new_avg)?;
                AveragePrice::<T>::insert(asset_id, new_avg);
            // spot price history has been recovered/initiated, create initial average value
            } else if vec.len() == avg_count - 1 {
                vec.push_back(price);
                let sum = vec.iter().fold(FixedWrapper::from(0), |a, b| a + *b);
                let avg = (sum / balance!(vec.len()))
                    .try_into_balance()
                    .map_err(|_| Error::<T>::UpdateAverageWithSpotPriceFailed)?;
                AveragePrice::<T>::insert(asset_id, avg);
            } else {
                vec.push_back(price);
            }

            Ok(())
        })
    }

    /// Register spot price quote failure, continuous failure has to block average price quotation.
    pub fn incoming_spot_price_failure(asset_id: &T::AssetId) {
        SpotPriceFailures::<T>::mutate(asset_id, |val| {
            if *val < AVG_BLOCK_SPAN {
                *val += 1;
                if *val == AVG_BLOCK_SPAN {
                    SpotPrices::<T>::mutate(asset_id, |vec| vec.clear())
                }
            }
        });
    }

    /// Bound `new_avg` value by percentage difference with respect to `old_avg` value. Result will be capped
    /// by `MAX_BLOCK_AVG_DIFFERENCE` either in positive or nagative difference.
    pub fn adjust_to_difference(
        old_avg: Balance,
        new_avg: Balance,
    ) -> Result<Balance, DispatchError> {
        let mut adjusted_avg = FixedWrapper::from(new_avg);
        let old_avg = FixedWrapper::from(old_avg);
        let diff: Fixed = ((adjusted_avg.clone() - old_avg.clone()) / old_avg.clone())
            .get()
            .map_err(|_| Error::<T>::UpdateAverageWithSpotPriceFailed)?;

        if diff > MAX_BLOCK_AVG_DIFFERENCE {
            adjusted_avg = old_avg * (fixed_wrapper!(1) + MAX_BLOCK_AVG_DIFFERENCE);
        } else if diff < MAX_BLOCK_AVG_DIFFERENCE.cneg().unwrap() {
            adjusted_avg = old_avg * (fixed_wrapper!(1) - MAX_BLOCK_AVG_DIFFERENCE);
        }
        let adjusted_avg = adjusted_avg
            .try_into_balance()
            .map_err(|_| Error::<T>::UpdateAverageWithSpotPriceFailed)?;
        Ok(adjusted_avg)
    }

    fn secondary_market_filter() -> LiquiditySourceFilter<T::DEXId, LiquiditySourceType> {
        LiquiditySourceFilter::with_allowed(
            DEXId::Polkaswap.into(),
            [LiquiditySourceType::XYKPool].into(),
        )
    }

    /// Get current spot price for
    pub fn spot_price(asset_id: &T::AssetId) -> Result<Balance, DispatchError> {
        <T as pallet::Config>::LiquidityProxy::quote(
            &XOR.into(),
            &asset_id,
            QuoteAmount::with_desired_input(balance!(1)),
            Self::secondary_market_filter(),
        )
        .map(|so| so.amount)
    }

    fn replace_in_average(
        average: Balance,
        old_value: Balance,
        new_value: Balance,
        count: u32,
    ) -> Result<Balance, DispatchError> {
        let average = FixedWrapper::from(average);
        let new_value = FixedWrapper::from(new_value);
        let old_value = FixedWrapper::from(old_value);
        let count: FixedWrapper = balance!(count).into();
        let new_avg: FixedWrapper = (count.clone() * average - old_value + new_value) / count;
        Ok(new_avg
            .try_into_balance()
            .map_err(|_| Error::<T>::AveragePriceCalculationFailed)?)
    }

    /// Returns number of pairs recalculated.
    pub fn average_prices_calculation_routine() -> u32 {
        let mut count = 0;
        for asset_id in EnabledTargets::<T>::get().iter() {
            let price = Self::spot_price(asset_id);
            if let Ok(val) = price {
                let _ = Self::incoming_spot_price(asset_id, val);
            } else {
                Self::incoming_spot_price_failure(asset_id);
            }
            count += 1;
        }
        count
    }
}

impl<T: Config> PriceToolsPallet<T::AssetId> for Module<T> {
    fn get_average_price(
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
    ) -> Result<Balance, DispatchError> {
        Module::<T>::get_average_price(input_asset_id, output_asset_id)
    }

    fn register_asset(asset_id: &T::AssetId) -> DispatchResult {
        EnabledTargets::<T>::mutate(|set| {
            if set.contains(asset_id) {
                Err(Error::<T>::AssetAlreadyRegistered.into())
            } else {
                set.insert(asset_id.clone());
                Ok(())
            }
        })
    }
}
