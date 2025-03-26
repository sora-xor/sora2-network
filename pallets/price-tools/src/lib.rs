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

pub mod migrations;

use core::marker::PhantomData;

use codec::{Decode, Encode};
use common::prelude::{
    AssetIdOf, Balance, Fixed, FixedWrapper, LiquiditySourceType, PriceToolsProvider, QuoteAmount,
    TradingPairSourceManager,
};
use common::{
    balance, fixed_const, fixed_wrapper, BalanceOf, DEXId, LiquidityProxyTrait,
    LiquiditySourceFilter, OnDenominate, OnPoolReservesChanged, PriceVariant, XOR,
};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::weights::Weight;
use frame_support::{ensure, fail};
use frame_support::{IterableStorageMap, StorageMap as StorageMapT};
use sp_std::collections::{btree_map::BTreeMap, vec_deque::VecDeque};
use sp_std::convert::TryInto;

pub use pallet::*;

/// Count of blocks to participate in avg value calculation.
pub const AVG_BLOCK_SPAN: usize = 30;

pub struct AdjustParameters {
    /// Max percentage difference for average value between blocks when price goes down for buy price.
    max_buy_dec: Fixed,
    /// Max percentage difference for average value between blocks when price goes up for buy price.
    max_buy_inc: Fixed,
    /// Max percentage difference for average value between blocks when price goes down for sell price.
    max_sell_dec: Fixed,
    /// Max percentage difference for average value between blocks when price goes up for sell price.
    max_sell_inc: Fixed,
}

pub const DEFAULT_PARAMETERS: AdjustParameters = AdjustParameters {
    max_buy_dec: fixed_const!(0.00002),  // 0.002%
    max_buy_inc: fixed_const!(0.00197),  // 0.197%
    max_sell_dec: fixed_const!(0.00197), // 0.197%
    max_sell_inc: fixed_const!(0.00002), // 0.002%
};

pub const FAST_PARAMETERS: AdjustParameters = AdjustParameters {
    max_buy_dec: fixed_const!(0.00197),  // 0.197%
    max_buy_inc: fixed_const!(0.00197),  // 0.197%
    max_sell_dec: fixed_const!(0.00197), // 0.197%
    max_sell_inc: fixed_const!(0.00197), // 0.197%
};

pub use weights::WeightInfo;

#[derive(Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, Debug, scale_info::TypeInfo)]
pub struct PriceInfo {
    price_failures: u32,
    spot_prices: VecDeque<Balance>,
    average_price: Balance,
    needs_update: bool,
    last_spot_price: Balance,
}

#[derive(Debug, Clone)]
pub enum PriceError {
    UpdateAverageWithSpotPriceFailed,
    AveragePriceCalculationFailed,
}

impl PriceInfo {
    /// Register spot price quote failure, continuous failure has to block average price quotation.
    pub fn incoming_spot_price_failure(&mut self) {
        if (self.price_failures as usize) < AVG_BLOCK_SPAN {
            self.price_failures += 1;
            if (self.price_failures as usize) == AVG_BLOCK_SPAN {
                self.spot_prices.clear();
            }
        }
    }

    /// Add new price to queue and recalculate average.
    pub fn incoming_spot_price(
        &mut self,
        price: Balance,
        price_variant: PriceVariant,
        adjust_params: &AdjustParameters,
    ) -> Result<(), PriceError> {
        // reset failure streak
        self.price_failures = 0;
        self.needs_update = false;
        // spot price history is consistent, normal behavior
        if self.spot_prices.len() == AVG_BLOCK_SPAN {
            let old_value = self.spot_prices.pop_front().expect("Checked above");

            let mut new_avg =
                Self::replace_in_average(self.average_price, old_value, price, AVG_BLOCK_SPAN)?;
            new_avg = Self::adjust_to_difference(
                self.average_price,
                new_avg,
                price_variant,
                adjust_params,
            )?;
            let adjusted_incoming_price =
                Self::adjusted_spot_price(self.average_price, new_avg, old_value, AVG_BLOCK_SPAN)?;
            self.spot_prices.push_back(adjusted_incoming_price);
            self.average_price = new_avg;
        // spot price history has been recovered/initiated, create initial average value
        } else if self.spot_prices.len() == AVG_BLOCK_SPAN - 1 {
            self.spot_prices.push_back(price);
            let sum = self
                .spot_prices
                .iter()
                .fold(FixedWrapper::from(0), |a, b| a + *b);
            let avg = (sum / balance!(self.spot_prices.len()))
                .try_into_balance()
                .map_err(|_| PriceError::UpdateAverageWithSpotPriceFailed)?;
            self.average_price = avg;
        } else {
            self.spot_prices.push_back(price);
        }
        self.last_spot_price = price;

        Ok(())
    }

    fn replace_in_average(
        average: Balance,
        old_value: Balance,
        new_value: Balance,
        count: usize,
    ) -> Result<Balance, PriceError> {
        let average = FixedWrapper::from(average);
        let new_value = FixedWrapper::from(new_value);
        let old_value = FixedWrapper::from(old_value);
        let count: FixedWrapper = balance!(count).into();
        let new_avg: FixedWrapper = (count.clone() * average - old_value + new_value) / count;
        Ok(new_avg
            .try_into_balance()
            .map_err(|_| PriceError::AveragePriceCalculationFailed)?)
    }

    /// Calculate fitting incoming spot price to satisfy given average price change.
    fn adjusted_spot_price(
        old_average: Balance,
        new_average: Balance,
        old_value: Balance,
        count: usize,
    ) -> Result<Balance, PriceError> {
        let old_average = FixedWrapper::from(old_average);
        let new_average = FixedWrapper::from(new_average);
        let old_value = FixedWrapper::from(old_value);
        let count: FixedWrapper = balance!(count).into();
        let adjusted_new_value = new_average * count.clone() + old_value - old_average * count;
        Ok(adjusted_new_value
            .try_into_balance()
            .map_err(|_| PriceError::AveragePriceCalculationFailed)?)
    }

    /// Bound `new_avg` value by percentage difference with respect to `old_avg` value. Result will be capped
    /// by `MAX_BLOCK_AVG_DIFFERENCE` either in positive or negative difference.
    pub fn adjust_to_difference(
        old_avg: Balance,
        new_avg: Balance,
        price_variant: PriceVariant,
        adjust_params: &AdjustParameters,
    ) -> Result<Balance, PriceError> {
        let mut adjusted_avg = FixedWrapper::from(new_avg);
        let old_avg = FixedWrapper::from(old_avg);
        let diff: Fixed = ((adjusted_avg.clone() - old_avg.clone()) / old_avg.clone())
            .get()
            .map_err(|_| PriceError::UpdateAverageWithSpotPriceFailed)?;
        let (max_inc, max_dec) = match price_variant {
            PriceVariant::Buy => (adjust_params.max_buy_inc, adjust_params.max_buy_dec),
            PriceVariant::Sell => (adjust_params.max_sell_inc, adjust_params.max_sell_dec),
        };

        if diff > max_inc {
            adjusted_avg = old_avg * (fixed_wrapper!(1) + max_inc);
        } else if diff < max_dec.cneg().unwrap() {
            adjusted_avg = old_avg * (fixed_wrapper!(1) - max_dec);
        }
        let adjusted_avg = adjusted_avg
            .try_into_balance()
            .map_err(|_| PriceError::UpdateAverageWithSpotPriceFailed)?;
        Ok(adjusted_avg)
    }
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
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(3);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_block_num: T::BlockNumber) -> Weight {
            let (n, m) = Pallet::<T>::average_prices_calculation_routine();
            <T as Config>::WeightInfo::on_initialize(n, m)
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

    #[pallet::storage]
    #[pallet::getter(fn fast_price_infos)]
    pub type FastPriceInfos<T: Config> = StorageMap<_, Identity, AssetIdOf<T>, AggregatedPriceInfo>;
}

impl<T: Config> Pallet<T> {
    /// Query averaged price from past data for supported paths, i.e. paths with enabled targets or XOR.
    fn get_average_price<S>(
        input_asset: &AssetIdOf<T>,
        output_asset: &AssetIdOf<T>,
        price_variant: PriceVariant,
    ) -> Result<Balance, DispatchError>
    where
        S: StorageMapT<AssetIdOf<T>, AggregatedPriceInfo, Query = Option<AggregatedPriceInfo>>,
    {
        if input_asset == output_asset {
            return Ok(balance!(1));
        }
        match (input_asset, output_asset) {
            (xor, output) if xor == &XOR.into() => {
                Self::get_asset_average_price::<S>(output, price_variant)
            }
            (input, xor) if xor == &XOR.into() => {
                // Buy price should always be greater or equal to sell price, so we need to invert price_variant here
                Self::get_asset_average_price::<S>(input, price_variant.switched()).and_then(
                    |average_price| {
                        (fixed_wrapper!(1) / average_price)
                            .try_into_balance()
                            .map_err(|_| Error::<T>::FailedToQuoteAveragePrice.into())
                    },
                )
            }
            (input, output) => {
                let quote_a = FixedWrapper::from(Self::get_average_price::<S>(
                    input,
                    &XOR.into(),
                    price_variant,
                )?);
                let quote_b = FixedWrapper::from(Self::get_average_price::<S>(
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

    fn get_asset_average_price<S>(
        asset_id: &AssetIdOf<T>,
        price_variant: PriceVariant,
    ) -> Result<Balance, DispatchError>
    where
        S: StorageMapT<AssetIdOf<T>, AggregatedPriceInfo, Query = Option<AggregatedPriceInfo>>,
    {
        S::get(asset_id)
            .map(|aggregated_price_info| aggregated_price_info.price_of(price_variant))
            .map_or_else(
                || Err(Error::<T>::UnsupportedQuotePath.into()),
                |price_info| {
                    ensure!(
                        price_info.spot_prices.len() == AVG_BLOCK_SPAN,
                        Error::<T>::InsufficientSpotPriceData
                    );
                    Ok(price_info.average_price)
                },
            )
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

    fn update_price<S>(
        price_cache: &mut BTreeMap<AssetIdOf<T>, Option<Balance>>,
        adjust_params: &AdjustParameters,
    ) -> (u32, u32)
    where
        S: IterableStorageMap<
            AssetIdOf<T>,
            AggregatedPriceInfo,
            Query = Option<AggregatedPriceInfo>,
        >,
    {
        let mut count_active = 0;
        let mut count_updated = 0;
        for asset_id in S::iter_keys() {
            S::mutate(asset_id, |opt_value| {
                let Some(value) = opt_value.as_mut() else {
                // Should not happen, because we get asset_id from iter_keys() call
                return;
            };
                for price_variant in [PriceVariant::Buy, PriceVariant::Sell] {
                    let price_info = value.price_mut_of(price_variant);
                    let price = if price_info.needs_update {
                        if let Some(price) = price_cache.get(&asset_id).cloned() {
                            price
                        } else {
                            count_updated += 1;
                            let price = Self::spot_price(&asset_id)
                                .map_err(|err| {
                                    frame_support::log::warn!(
                                        "Failed to get spot price for {asset_id:?}: {err:?}"
                                    );
                                    err
                                })
                                .ok();
                            price_cache.insert(asset_id, price);
                            price
                        }
                    } else {
                        // if price hasn't changed duplicate latest known to update average
                        Some(price_info.last_spot_price)
                    };
                    if let Some(val) = price {
                        if let Err(err) =
                            price_info.incoming_spot_price(val, price_variant, adjust_params)
                        {
                            frame_support::log::warn!("Failed to add spot price for {asset_id:?} with {price_variant:?} variant and {val} price: {err:?}");
                        }
                    } else {
                        price_info.incoming_spot_price_failure();
                    }
                    count_active += 1;
                }
            })
        }
        (count_active, count_updated)
    }

    /// Returns (number of active pairs, number of pairs with needed update)
    pub fn average_prices_calculation_routine() -> (u32, u32) {
        let mut price_cache = BTreeMap::new();
        let (count_active, count_updated) =
            Self::update_price::<PriceInfos<T>>(&mut price_cache, &DEFAULT_PARAMETERS);
        let (fast_count_active, fast_count_updated) =
            Self::update_price::<FastPriceInfos<T>>(&mut price_cache, &FAST_PARAMETERS);
        (
            count_active + fast_count_active,
            count_updated + fast_count_updated,
        )
    }

    pub fn register_asset_inner<S>(asset_id: &AssetIdOf<T>) -> DispatchResult
    where
        S: StorageMapT<AssetIdOf<T>, AggregatedPriceInfo, Query = Option<AggregatedPriceInfo>>,
    {
        S::try_mutate(asset_id, |opt_value| {
            if opt_value.is_none() {
                *opt_value = Some(Default::default());
                DispatchResult::Ok(())
            } else {
                fail!(Error::<T>::AssetAlreadyRegistered);
            }
        })?;
        Ok(())
    }

    pub fn on_reserves_changed<S>(asset_id: &AssetIdOf<T>)
    where
        S: StorageMapT<AssetIdOf<T>, AggregatedPriceInfo, Query = Option<AggregatedPriceInfo>>,
    {
        S::mutate(asset_id, |opt_val| {
            if let Some(agg_price_info) = opt_val {
                if !agg_price_info.buy.needs_update || !agg_price_info.sell.needs_update {
                    agg_price_info.buy.needs_update = true;
                    agg_price_info.sell.needs_update = true;
                }
            }
        })
    }
}

impl<T: Config> PriceToolsProvider<AssetIdOf<T>> for Pallet<T> {
    fn is_asset_registered(asset_id: &AssetIdOf<T>) -> bool {
        PriceInfos::<T>::get(asset_id).is_some()
    }

    fn get_average_price(
        input_asset_id: &AssetIdOf<T>,
        output_asset_id: &AssetIdOf<T>,
        price_variant: PriceVariant,
    ) -> Result<Balance, DispatchError> {
        Pallet::<T>::get_average_price::<PriceInfos<T>>(
            input_asset_id,
            output_asset_id,
            price_variant,
        )
    }

    fn register_asset(asset_id: &AssetIdOf<T>) -> DispatchResult {
        Pallet::<T>::register_asset_inner::<PriceInfos<T>>(asset_id)?;
        Pallet::<T>::register_asset_inner::<FastPriceInfos<T>>(asset_id)
    }
}

pub struct FastPriceTools<T>(PhantomData<T>);

impl<T: Config> PriceToolsProvider<AssetIdOf<T>> for FastPriceTools<T> {
    fn is_asset_registered(asset_id: &AssetIdOf<T>) -> bool {
        FastPriceInfos::<T>::get(asset_id).is_some()
    }

    fn get_average_price(
        input_asset_id: &AssetIdOf<T>,
        output_asset_id: &AssetIdOf<T>,
        price_variant: PriceVariant,
    ) -> Result<Balance, DispatchError> {
        Pallet::<T>::get_average_price::<FastPriceInfos<T>>(
            input_asset_id,
            output_asset_id,
            price_variant,
        )
    }

    fn register_asset(asset_id: &AssetIdOf<T>) -> DispatchResult {
        Pallet::<T>::register_asset_inner::<PriceInfos<T>>(asset_id)?;
        Pallet::<T>::register_asset_inner::<FastPriceInfos<T>>(asset_id)
    }
}

impl<T: Config> OnPoolReservesChanged<AssetIdOf<T>> for Pallet<T> {
    fn reserves_changed(target_asset_id: &AssetIdOf<T>) {
        Self::on_reserves_changed::<PriceInfos<T>>(target_asset_id);
        Self::on_reserves_changed::<FastPriceInfos<T>>(target_asset_id);
    }
}

pub struct DenominateXorAndTbcd<T: Config>(PhantomData<T>);
impl<T: Config> OnDenominate<BalanceOf<T>> for DenominateXorAndTbcd<T> {
    fn on_denominate(_factor: &BalanceOf<T>) -> DispatchResult {
        frame_support::log::info!("{}::on_denominate({})", module_path!(), _factor);
        Pallet::<T>::reserves_changed(&XOR.into());
        Pallet::<T>::reserves_changed(&common::TBCD.into());
        Ok(())
    }
}
