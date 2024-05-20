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
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[allow(unused_imports)]
#[macro_use]
extern crate alloc;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod migration;

use codec::{Decode, Encode};
use common::prelude::{
    AssetIdOf, Balance, Fixed, FixedWrapper, LiquiditySourceType, PriceToolsProvider, QuoteAmount,
    TradingPairSourceManager,
};
use common::{
    balance, fixed_const, fixed_wrapper, DEXId, LiquidityProxyTrait, LiquiditySourceFilter,
    OnPoolReservesChanged, PriceVariant, XOR,
};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::weights::Weight;
use frame_support::{ensure, fail};
use sp_std::collections::vec_deque::VecDeque;
use sp_std::convert::TryInto;

pub use pallet::*;

/// Count of blocks to participate in avg value calculation.
pub const AVG_BLOCK_SPAN: u32 = 30;

/// Max percentage difference for average value between blocks when price goes down for buy price.
const MAX_BUY_BLOCK_DEC_AVG_DIFFERENCE: Fixed = fixed_const!(0.00002); // 0.002%
/// Max percentage difference for average value between blocks when price goes up for buy price.
const MAX_BUY_BLOCK_INC_AVG_DIFFERENCE: Fixed = fixed_const!(0.00197); // 0.197%

/// Max percentage difference for average value between blocks when price goes down for sell price.
const MAX_SELL_BLOCK_DEC_AVG_DIFFERENCE: Fixed = fixed_const!(0.00197); // 0.197%
/// Max percentage difference for average value between blocks when price goes up for sell price.
const MAX_SELL_BLOCK_INC_AVG_DIFFERENCE: Fixed = fixed_const!(0.00002); // 0.002%

pub use weights::WeightInfo;

#[derive(Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, Debug, scale_info::TypeInfo)]
pub struct PriceInfo {
    price_failures: u32,
    spot_prices: VecDeque<Balance>,
    average_price: Balance,
    needs_update: bool,
    last_spot_price: Balance,
}

impl Default for PriceInfo {
    fn default() -> Self {
        Self {
            price_failures: 0,
            spot_prices: Default::default(),
            average_price: Default::default(),
            needs_update: true,
            last_spot_price: Default::default(),
        }
    }
}

#[derive(
    Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, Debug, scale_info::TypeInfo, Default,
)]
pub struct AggregatedPriceInfo {
    buy: PriceInfo,
    sell: PriceInfo,
}

impl AggregatedPriceInfo {
    pub fn price_mut_of(&mut self, price_variant: PriceVariant) -> &mut PriceInfo {
        match price_variant {
            PriceVariant::Buy => &mut self.buy,
            PriceVariant::Sell => &mut self.sell,
        }
    }

    pub fn price_of(self, price_variant: PriceVariant) -> PriceInfo {
        match price_variant {
            PriceVariant::Buy => self.buy,
            PriceVariant::Sell => self.sell,
        }
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::LiquidityProxyTrait;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + common::Config + technical::Config + pool_xyk::Config
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type LiquidityProxy: LiquidityProxyTrait<Self::DEXId, Self::AccountId, AssetIdOf<Self>>;
        type TradingPairSourceManager: TradingPairSourceManager<Self::DEXId, AssetIdOf<Self>>;
        type WeightInfo: WeightInfo;
    }

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_block_num: T::BlockNumber) -> Weight {
            let (n_b, m_b) = Pallet::<T>::average_prices_calculation_routine(PriceVariant::Buy);
            let (n_s, m_s) = Pallet::<T>::average_prices_calculation_routine(PriceVariant::Sell);
            <T as Config>::WeightInfo::on_initialize(n_b + n_s, m_b + m_s)
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // no extrinsics
    }

    #[pallet::event]
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
        /// Spot price for asset has not changed but info for last spot price is unavailable.
        CantDuplicateLastPrice,
    }

    #[pallet::storage]
    #[pallet::getter(fn price_infos)]
    pub type PriceInfos<T: Config> = StorageMap<_, Identity, AssetIdOf<T>, AggregatedPriceInfo>;
}

impl<T: Config> Pallet<T> {
    /// Query averaged price from past data for supported paths, i.e. paths with enabled targets or XOR.
    pub fn get_average_price(
        input_asset: &AssetIdOf<T>,
        output_asset: &AssetIdOf<T>,
        price_variant: PriceVariant,
    ) -> Result<Balance, DispatchError> {
        if input_asset == output_asset {
            return Ok(balance!(1));
        }
        match (input_asset, output_asset) {
            (xor, output) if xor == &XOR.into() => {
                Self::get_asset_average_price(output, price_variant)
            }
            (input, xor) if xor == &XOR.into() => {
                // Buy price should always be greater or equal to sell price, so we need to invert price_variant here
                Self::get_asset_average_price(input, price_variant.switched()).and_then(
                    |average_price| {
                        (fixed_wrapper!(1) / average_price)
                            .try_into_balance()
                            .map_err(|_| Error::<T>::FailedToQuoteAveragePrice.into())
                    },
                )
            }
            (input, output) => {
                let quote_a =
                    FixedWrapper::from(Self::get_average_price(input, &XOR.into(), price_variant)?);
                let quote_b = FixedWrapper::from(Self::get_average_price(
                    &XOR.into(),
                    output,
                    price_variant,
                )?);
                (quote_a * quote_b)
                    .try_into_balance()
                    .map_err(|_| Error::<T>::FailedToQuoteAveragePrice.into())
            }
        }
    }

    fn get_asset_average_price(
        asset_id: &AssetIdOf<T>,
        price_variant: PriceVariant,
    ) -> Result<Balance, DispatchError> {
        let avg_count: usize = AVG_BLOCK_SPAN
            .try_into()
            .map_err(|_| Error::<T>::FailedToQuoteAveragePrice)?;

        PriceInfos::<T>::get(asset_id)
            .map(|aggregated_price_info| aggregated_price_info.price_of(price_variant))
            .map_or_else(
                || Err(Error::<T>::UnsupportedQuotePath.into()),
                |price_info| {
                    ensure!(
                        price_info.spot_prices.len() == avg_count,
                        Error::<T>::InsufficientSpotPriceData
                    );
                    Ok(price_info.average_price)
                },
            )
    }

    /// Add new price to queue and recalculate average.
    pub fn incoming_spot_price(
        asset_id: &AssetIdOf<T>,
        price: Balance,
        price_variant: PriceVariant,
    ) -> DispatchResult {
        // reset failure streak for spot prices if needed
        if PriceInfos::<T>::get(asset_id).is_some() {
            let avg_count: usize = AVG_BLOCK_SPAN
                .try_into()
                .map_err(|_| Error::<T>::UpdateAverageWithSpotPriceFailed)?;
            PriceInfos::<T>::mutate(asset_id, |opt| {
                let val = opt.as_mut().unwrap().price_mut_of(price_variant);
                // reset failure streak
                val.price_failures = 0;
                val.needs_update = false;
                // spot price history is consistent, normal behavior
                if val.spot_prices.len() == avg_count {
                    let old_value = val.spot_prices.pop_front().unwrap();

                    let mut new_avg = Self::replace_in_average(
                        val.average_price,
                        old_value,
                        price,
                        AVG_BLOCK_SPAN,
                    )?;
                    new_avg =
                        Self::adjust_to_difference(val.average_price, new_avg, price_variant)?;
                    let adjusted_incoming_price = Self::adjusted_spot_price(
                        val.average_price,
                        new_avg,
                        old_value,
                        AVG_BLOCK_SPAN,
                    )?;
                    val.spot_prices.push_back(adjusted_incoming_price);
                    val.average_price = new_avg;
                // spot price history has been recovered/initiated, create initial average value
                } else if val.spot_prices.len() == avg_count - 1 {
                    val.spot_prices.push_back(price);
                    let sum = val
                        .spot_prices
                        .iter()
                        .fold(FixedWrapper::from(0), |a, b| a + *b);
                    let avg = (sum / balance!(val.spot_prices.len()))
                        .try_into_balance()
                        .map_err(|_| Error::<T>::UpdateAverageWithSpotPriceFailed)?;
                    val.average_price = avg;
                } else {
                    val.spot_prices.push_back(price);
                }
                val.last_spot_price = price;

                Ok(())
            })
        } else {
            fail!(Error::<T>::UnsupportedQuotePath);
        }
    }

    /// Register spot price quote failure, continuous failure has to block average price quotation.
    pub fn incoming_spot_price_failure(asset_id: &AssetIdOf<T>, price_variant: PriceVariant) {
        PriceInfos::<T>::mutate(asset_id, |opt| {
            if let Some(agg_price_info) = opt.as_mut() {
                let val = agg_price_info.price_mut_of(price_variant);
                if val.price_failures < AVG_BLOCK_SPAN {
                    val.price_failures += 1;
                    if val.price_failures == AVG_BLOCK_SPAN {
                        val.spot_prices.clear();
                    }
                }
            }
        })
    }

    /// Bound `new_avg` value by percentage difference with respect to `old_avg` value. Result will be capped
    /// by `MAX_BLOCK_AVG_DIFFERENCE` either in positive or negative difference.
    pub fn adjust_to_difference(
        old_avg: Balance,
        new_avg: Balance,
        price_variant: PriceVariant,
    ) -> Result<Balance, DispatchError> {
        let mut adjusted_avg = FixedWrapper::from(new_avg);
        let old_avg = FixedWrapper::from(old_avg);
        let diff: Fixed = ((adjusted_avg.clone() - old_avg.clone()) / old_avg.clone())
            .get()
            .map_err(|_| Error::<T>::UpdateAverageWithSpotPriceFailed)?;
        let (max_inc, max_dec) = match price_variant {
            PriceVariant::Buy => (
                MAX_BUY_BLOCK_INC_AVG_DIFFERENCE,
                MAX_BUY_BLOCK_DEC_AVG_DIFFERENCE,
            ),
            PriceVariant::Sell => (
                MAX_SELL_BLOCK_INC_AVG_DIFFERENCE,
                MAX_SELL_BLOCK_DEC_AVG_DIFFERENCE,
            ),
        };

        if diff > max_inc {
            adjusted_avg = old_avg * (fixed_wrapper!(1) + max_inc);
        } else if diff < max_dec.cneg().unwrap() {
            adjusted_avg = old_avg * (fixed_wrapper!(1) - max_dec);
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
    pub fn spot_price(asset_id: &AssetIdOf<T>) -> Result<Balance, DispatchError> {
        T::LiquidityProxy::quote(
            DEXId::Polkaswap.into(),
            &XOR.into(),
            &asset_id,
            QuoteAmount::with_desired_input(balance!(1)),
            Self::secondary_market_filter(),
            false,
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

    /// Calculate fitting incoming spot price to satisfy given average price change.
    fn adjusted_spot_price(
        old_average: Balance,
        new_average: Balance,
        old_value: Balance,
        count: u32,
    ) -> Result<Balance, DispatchError> {
        let old_average = FixedWrapper::from(old_average);
        let new_average = FixedWrapper::from(new_average);
        let old_value = FixedWrapper::from(old_value);
        let count: FixedWrapper = balance!(count).into();
        let adjusted_new_value = new_average * count.clone() + old_value - old_average * count;
        Ok(adjusted_new_value
            .try_into_balance()
            .map_err(|_| Error::<T>::AveragePriceCalculationFailed)?)
    }

    /// Returns (number of active pairs, number of pairs with needed update)
    pub fn average_prices_calculation_routine(price_variant: PriceVariant) -> (u32, u32) {
        let mut count_active = 0;
        let mut count_updated = 0;
        let price_infos_iter = PriceInfos::<T>::iter()
            .map(|(a, mut agg_price_info)| (a, agg_price_info.price_mut_of(price_variant).clone()));
        for (asset_id, price_info) in price_infos_iter {
            let price = if price_info.needs_update {
                count_updated += 1;
                Self::spot_price(&asset_id)
            } else {
                // if price hasn't changed duplicate latest known to update average
                Ok(price_info.last_spot_price)
            };
            if let Ok(val) = price {
                let _ = Self::incoming_spot_price(&asset_id, val, price_variant);
            } else {
                Self::incoming_spot_price_failure(&asset_id, price_variant);
            }
            count_active += 1;
        }
        (count_active, count_updated)
    }
}

impl<T: Config> PriceToolsProvider<AssetIdOf<T>> for Pallet<T> {
    fn get_average_price(
        input_asset_id: &AssetIdOf<T>,
        output_asset_id: &AssetIdOf<T>,
        price_variant: PriceVariant,
    ) -> Result<Balance, DispatchError> {
        Pallet::<T>::get_average_price(input_asset_id, output_asset_id, price_variant)
    }

    fn register_asset(asset_id: &AssetIdOf<T>) -> DispatchResult {
        if PriceInfos::<T>::get(asset_id).is_none() {
            PriceInfos::<T>::insert(asset_id.clone(), AggregatedPriceInfo::default());
            Ok(())
        } else {
            fail!(Error::<T>::AssetAlreadyRegistered);
        }
    }
}

impl<T: Config> OnPoolReservesChanged<AssetIdOf<T>> for Pallet<T> {
    fn reserves_changed(target_asset_id: &AssetIdOf<T>) {
        if let Some(agg_price_info) = PriceInfos::<T>::get(target_asset_id) {
            if !agg_price_info.buy.needs_update || !agg_price_info.sell.needs_update {
                PriceInfos::<T>::mutate(target_asset_id, |opt| {
                    let agg_price_info = opt.as_mut().unwrap();
                    agg_price_info.buy.needs_update = true;
                    agg_price_info.sell.needs_update = true;
                })
            }
        }
    }
}
