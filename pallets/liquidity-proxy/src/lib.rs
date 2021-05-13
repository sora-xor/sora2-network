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

#[macro_use]
extern crate alloc;

use codec::{Decode, Encode};
use core::convert::{TryFrom, TryInto};

use common::prelude::fixnum::ops::{Bounded, CheckedMul, CheckedSub, One, Zero as _};
use common::prelude::{Balance, FixedWrapper, SwapAmount, SwapOutcome, SwapVariant};
use common::{
    fixed, fixed_wrapper, linspace, FilterMode, Fixed, FixedInner, GetMarketInfo, GetPoolReserves,
    IntervalEndpoints, LiquidityRegistry, LiquiditySource, LiquiditySourceFilter,
    LiquiditySourceId, LiquiditySourceType, RewardReason, TradingPair, VestedRewardsTrait,
};
use frame_support::traits::Get;
use frame_support::weights::Weight;
use frame_support::{ensure, fail, RuntimeDebug};
use frame_system::ensure_signed;
use sp_runtime::traits::{UniqueSaturatedFrom, Zero};
use sp_runtime::DispatchError;
use sp_std::prelude::*;

type LiquiditySourceIdOf<T> = LiquiditySourceId<<T as common::Config>::DEXId, LiquiditySourceType>;

type Rewards<AssetId> = Vec<(Balance, AssetId, RewardReason)>;

pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod algo;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"liquidity-proxy";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";

pub enum ExchangePath<T: Config> {
    Direct {
        from_asset_id: T::AssetId,
        to_asset_id: T::AssetId,
    },
    Twofold {
        from_asset_id: T::AssetId,
        intermediate_asset_id: T::AssetId,
        to_asset_id: T::AssetId,
    },
}

impl<T: Config> ExchangePath<T> {
    pub fn as_vec(self) -> Vec<(T::AssetId, T::AssetId)> {
        match self {
            ExchangePath::Direct {
                from_asset_id,
                to_asset_id,
            } => [(from_asset_id, to_asset_id)].into(),
            ExchangePath::Twofold {
                from_asset_id,
                intermediate_asset_id,
                to_asset_id,
            } => [
                (from_asset_id, intermediate_asset_id),
                (intermediate_asset_id, to_asset_id),
            ]
            .into(),
        }
    }
}

/// Output of the aggregated LiquidityProxy::quote() price.
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AggregatedSwapOutcome<LiquiditySourceType, AmountType> {
    /// A distribution of shares each liquidity sources gets to swap in the entire trade
    pub distribution: Vec<(LiquiditySourceType, Fixed)>,
    /// The best possible output/input amount for a given trade and a set of liquidity sources
    pub amount: AmountType,
    /// Total fee amount, nominated in XOR
    pub fee: AmountType,
}

impl<LiquiditySourceIdType, AmountType> AggregatedSwapOutcome<LiquiditySourceIdType, AmountType> {
    pub fn new(
        distribution: Vec<(LiquiditySourceIdType, Fixed)>,
        amount: AmountType,
        fee: AmountType,
    ) -> Self {
        Self {
            distribution,
            amount,
            fee,
        }
    }
}

/// Indicates that particular object can be used to perform exchanges with aggregation capability.
pub trait LiquidityProxyTrait<DEXId: PartialEq + Copy, AccountId, AssetId> {
    /// Get spot price of tokens based on desired amount, None returned if liquidity source
    /// does not have available exchange methods for indicated path.
    fn quote(
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError>;

    /// Perform exchange based on desired amount.
    fn exchange(
        sender: &AccountId,
        receiver: &AccountId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError>;
}

impl<DEXId: PartialEq + Copy, AccountId, AssetId> LiquidityProxyTrait<DEXId, AccountId, AssetId>
    for ()
{
    fn quote(
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _amount: SwapAmount<Balance>,
        _filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        unimplemented!()
    }

    fn exchange(
        _sender: &AccountId,
        _receiver: &AccountId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _amount: SwapAmount<Balance>,
        _filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        unimplemented!()
    }
}

pub trait WeightInfo {
    fn swap(amount: SwapVariant) -> Weight;
}

impl<T: Config> Pallet<T> {
    /// Temporary workaround to prevent tbc oracle exploit with xyk-only filter.
    pub fn is_forbidden_filter(
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        selected_source_types: &Vec<LiquiditySourceType>,
        filter_mode: &FilterMode,
    ) -> bool {
        let tbc_reserve_assets = T::PrimaryMarket::enabled_collaterals();
        // check if user has selected only xyk either explicitly or by excluding other types
        let is_xyk_only = selected_source_types.contains(&LiquiditySourceType::XYKPool)
            && !selected_source_types
                .contains(&LiquiditySourceType::MulticollateralBondingCurvePool)
            && filter_mode == &FilterMode::AllowSelected
            || selected_source_types
                .contains(&LiquiditySourceType::MulticollateralBondingCurvePool)
                && !selected_source_types.contains(&LiquiditySourceType::XYKPool)
                && filter_mode == &FilterMode::ForbidSelected;
        // check if either of tbc reserve assets is present
        let reserve_asset_present = tbc_reserve_assets.contains(input_asset_id)
            || tbc_reserve_assets.contains(output_asset_id);

        is_xyk_only && reserve_asset_present
    }

    /// Sample a single liquidity source with a range of swap amounts to get respective prices for the exchange.
    fn sample_liquidity_source(
        liquidity_source_id: &LiquiditySourceIdOf<T>,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Fixed>,
        num_samples: usize,
    ) -> Vec<SwapOutcome<Fixed>> {
        match amount {
            SwapAmount::WithDesiredInput {
                desired_amount_in: amount,
                min_amount_out: min_out,
            } => linspace(Fixed::ZERO, amount, num_samples, IntervalEndpoints::Right)
                .into_iter()
                .zip(
                    linspace(Fixed::ZERO, min_out, num_samples, IntervalEndpoints::Right)
                        .into_iter(),
                )
                .map(|(x, y)| {
                    let amount = match (x.into_bits().try_into(), y.into_bits().try_into()) {
                        (Ok(x), Ok(y)) => {
                            let v = T::LiquidityRegistry::quote(
                                liquidity_source_id,
                                input_asset_id,
                                output_asset_id,
                                SwapAmount::with_desired_input(x, y),
                            )
                            .and_then(|o| {
                                o.try_into()
                                    .map_err(|_| Error::<T>::CalculationError.into())
                            });
                            v
                        }
                        _ => Err(Error::<T>::CalculationError.into()),
                    };
                    amount.unwrap_or_else(|_| SwapOutcome::new(Fixed::ZERO, Fixed::ZERO))
                })
                .collect::<Vec<_>>(),
            SwapAmount::WithDesiredOutput {
                desired_amount_out: amount,
                max_amount_in: max_in,
            } => linspace(Fixed::ZERO, amount, num_samples, IntervalEndpoints::Right)
                .into_iter()
                .zip(
                    linspace(Fixed::ZERO, max_in, num_samples, IntervalEndpoints::Right)
                        .into_iter(),
                )
                .map(|(x, y)| {
                    let amount = match (x.into_bits().try_into(), y.into_bits().try_into()) {
                        (Ok(x), Ok(y)) => {
                            let v = T::LiquidityRegistry::quote(
                                liquidity_source_id,
                                input_asset_id,
                                output_asset_id,
                                SwapAmount::with_desired_output(x, y),
                            )
                            .and_then(|o| {
                                o.try_into()
                                    .map_err(|_| Error::<T>::CalculationError.into())
                            });
                            v
                        }
                        _ => Err(Error::<T>::CalculationError.into()),
                    };
                    amount.unwrap_or_else(|_| SwapOutcome::new(Fixed::MAX, Fixed::ZERO))
                })
                .collect::<Vec<_>>(),
        }
    }

    /// Applies trivial routing (via Base Asset), resulting in a poly-swap which may contain several individual swaps.
    /// Those individual swaps are subject to liquidity aggregation algorithm.
    ///
    /// This a wrapper for `exchange_single`.
    pub fn exchange(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        ensure!(
            input_asset_id != output_asset_id,
            Error::<T>::UnavailableExchangePath
        );
        common::with_transaction(|| {
            match Self::construct_trivial_path(*input_asset_id, *output_asset_id) {
                ExchangePath::Direct {
                    from_asset_id,
                    to_asset_id,
                } => {
                    let outcome = Self::exchange_single(
                        sender,
                        receiver,
                        &from_asset_id,
                        &to_asset_id,
                        amount,
                        filter,
                    )?;
                    let xor_volume =
                        Self::get_xor_amount(from_asset_id, to_asset_id, amount, outcome.clone());
                    T::VestedRewardsAggregator::update_market_maker_records(
                        &sender, xor_volume, 1,
                    )?;
                    Ok(outcome)
                }
                ExchangePath::Twofold {
                    from_asset_id,
                    intermediate_asset_id,
                    to_asset_id,
                } => match amount {
                    SwapAmount::WithDesiredInput {
                        desired_amount_in,
                        min_amount_out,
                    } => {
                        let transit_account = T::GetTechnicalAccountId::get();
                        let first_swap = Self::exchange_single(
                            sender,
                            &transit_account,
                            &from_asset_id,
                            &intermediate_asset_id,
                            SwapAmount::with_desired_input(desired_amount_in, Balance::zero()),
                            filter.clone(),
                        )?;
                        let second_swap = Self::exchange_single(
                            &transit_account,
                            receiver,
                            &intermediate_asset_id,
                            &to_asset_id,
                            SwapAmount::with_desired_input(first_swap.amount, Balance::zero()),
                            filter,
                        )?;
                        ensure!(
                            second_swap.amount >= min_amount_out,
                            Error::<T>::SlippageNotTolerated
                        );
                        T::VestedRewardsAggregator::update_market_maker_records(
                            &sender,
                            first_swap.amount,
                            2,
                        )?;
                        let cumulative_fee = first_swap
                            .fee
                            .checked_add(second_swap.fee)
                            .ok_or(Error::<T>::CalculationError)?;
                        Ok(SwapOutcome::new(second_swap.amount, cumulative_fee))
                    }
                    SwapAmount::WithDesiredOutput {
                        desired_amount_out,
                        max_amount_in,
                    } => {
                        let (second_quote, _) = Self::quote_single(
                            &intermediate_asset_id,
                            &to_asset_id,
                            SwapAmount::with_desired_output(desired_amount_out, Balance::MAX),
                            filter.clone(),
                            true,
                        )?;
                        let (first_quote, _) = Self::quote_single(
                            &from_asset_id,
                            &intermediate_asset_id,
                            SwapAmount::with_desired_output(second_quote.amount, Balance::MAX),
                            filter.clone(),
                            true,
                        )?;
                        ensure!(
                            first_quote.amount <= max_amount_in,
                            Error::<T>::SlippageNotTolerated
                        );
                        let transit_account = T::GetTechnicalAccountId::get();
                        let first_swap = Self::exchange_single(
                            sender,
                            &transit_account,
                            &from_asset_id,
                            &intermediate_asset_id,
                            SwapAmount::with_desired_input(first_quote.amount, Balance::zero()),
                            filter.clone(),
                        )?;
                        let second_swap = Self::exchange_single(
                            &transit_account,
                            receiver,
                            &intermediate_asset_id,
                            &to_asset_id,
                            SwapAmount::with_desired_input(first_swap.amount, Balance::zero()),
                            filter,
                        )?;
                        T::VestedRewardsAggregator::update_market_maker_records(
                            &sender,
                            first_swap.amount,
                            2,
                        )?;
                        let cumulative_fee = first_swap
                            .fee
                            .checked_add(second_swap.fee)
                            .ok_or(Error::<T>::CalculationError)?;
                        Ok(SwapOutcome::new(first_quote.amount, cumulative_fee))
                    }
                },
            }
        })
    }

    /// Performs a swap given a number of liquidity sources and a distribuition of the swap amount across the sources.
    fn exchange_single(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        common::with_transaction(|| {
            let fx_amount: SwapAmount<Fixed> = amount
                .try_into()
                .map_err(|_| Error::CalculationError::<T>)?;
            let res = Self::quote_single(input_asset_id, output_asset_id, amount, filter, true)?
                .0
                .distribution
                .into_iter()
                .filter(|(_src, share)| *share > Fixed::ZERO)
                .map(|(src, share)| {
                    let filter = fx_amount * share;
                    let filter = filter
                        .try_into()
                        .map_err(|_| Error::CalculationError::<T>)?;
                    T::LiquidityRegistry::exchange(
                        sender,
                        receiver,
                        &src,
                        input_asset_id,
                        output_asset_id,
                        filter,
                    )
                })
                .collect::<Result<Vec<SwapOutcome<Balance>>, DispatchError>>()?;

            let (amount, fee): (FixedWrapper, FixedWrapper) = res.into_iter().fold(
                (fixed_wrapper!(0), fixed_wrapper!(0)),
                |(amount_acc, fee_acc), x| {
                    (
                        amount_acc + FixedWrapper::from(x.amount),
                        fee_acc + FixedWrapper::from(x.fee),
                    )
                },
            );
            let amount = amount
                .try_into_balance()
                .map_err(|_| Error::CalculationError::<T>)?;
            let fee = fee
                .try_into_balance()
                .map_err(|_| Error::CalculationError::<T>)?;

            Ok(SwapOutcome::new(amount, fee))
        })
    }

    /// Applies trivial routing (via Base Asset), resulting in a poly-swap which may contain several individual swaps.
    /// Those individual swaps are subject to liquidity aggregation algorithm.
    ///
    /// This a wrapper for `quote_single`.
    pub fn quote(
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
        skip_info: bool,
    ) -> Result<(SwapOutcome<Balance>, Rewards<T::AssetId>), DispatchError> {
        ensure!(
            input_asset_id != output_asset_id,
            Error::<T>::UnavailableExchangePath
        );
        match Self::construct_trivial_path(*input_asset_id, *output_asset_id) {
            ExchangePath::Direct {
                from_asset_id,
                to_asset_id,
            } => Self::quote_single(&from_asset_id, &to_asset_id, amount, filter, skip_info)
                .map(|(aso, rewards)| (SwapOutcome::new(aso.amount, aso.fee).into(), rewards)),
            ExchangePath::Twofold {
                from_asset_id,
                intermediate_asset_id,
                to_asset_id,
            } => match amount {
                SwapAmount::WithDesiredInput {
                    desired_amount_in, ..
                } => {
                    let (first_quote, rewards_a) = Self::quote_single(
                        &from_asset_id,
                        &intermediate_asset_id,
                        SwapAmount::with_desired_input(desired_amount_in, 0),
                        filter.clone(),
                        skip_info,
                    )?;
                    let (second_quote, mut rewards_b) = Self::quote_single(
                        &intermediate_asset_id,
                        &to_asset_id,
                        SwapAmount::with_desired_input(first_quote.amount, 0),
                        filter,
                        skip_info,
                    )?;
                    let cumulative_fee = first_quote
                        .fee
                        .checked_add(second_quote.fee)
                        .ok_or(Error::<T>::CalculationError)?;
                    let mut rewards = rewards_a;
                    rewards.append(&mut rewards_b);
                    Ok((
                        SwapOutcome::new(second_quote.amount, cumulative_fee),
                        rewards,
                    ))
                }
                SwapAmount::WithDesiredOutput {
                    desired_amount_out, ..
                } => {
                    let (second_quote, mut rewards_b) = Self::quote_single(
                        &intermediate_asset_id,
                        &to_asset_id,
                        SwapAmount::with_desired_output(desired_amount_out, Balance::MAX),
                        filter.clone(),
                        skip_info,
                    )?;
                    let (first_quote, rewards_a) = Self::quote_single(
                        &from_asset_id,
                        &intermediate_asset_id,
                        SwapAmount::with_desired_output(second_quote.amount, Balance::MAX),
                        filter,
                        skip_info,
                    )?;
                    let cumulative_fee = first_quote
                        .fee
                        .checked_add(second_quote.fee)
                        .ok_or(Error::<T>::CalculationError)?;
                    let mut rewards = rewards_a;
                    rewards.append(&mut rewards_b);
                    Ok((
                        SwapOutcome::new(first_quote.amount, cumulative_fee),
                        rewards,
                    ))
                }
            },
        }
    }

    /// Computes the optimal distribution across available liquidity sources to exectute the requested trade
    /// given the input and output assets, the trade amount and a liquidity sources filter.
    ///
    /// - `input_asset_id` - ID of the asset to sell,
    /// - `output_asset_id` - ID of the asset to buy,
    /// - `amount` - the amount with "direction" (sell or buy) together with the maximum price impact (slippage),
    /// - `filter` - a filter composed of a list of liquidity sources IDs to accept or ban for this trade.
    /// - `skip_info` - flag that indicates that additional info should not be shown, that is needed when actual exchange is performed.
    ///
    fn quote_single(
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
        skip_info: bool,
    ) -> Result<
        (
            AggregatedSwapOutcome<LiquiditySourceIdOf<T>, Balance>,
            Rewards<T::AssetId>,
        ),
        DispatchError,
    > {
        let sources =
            T::LiquidityRegistry::list_liquidity_sources(input_asset_id, output_asset_id, filter)?;

        ensure!(!sources.is_empty(), Error::<T>::UnavailableExchangePath);

        // Check if we have exactly one source => no split required
        if sources.len() == 1 {
            let src = sources.first().unwrap();
            let outcome =
                T::LiquidityRegistry::quote(src, input_asset_id, output_asset_id, amount.clone())?;
            let rewards = if skip_info {
                Vec::new()
            } else {
                let (input_amount, output_amount) =
                    Self::sort_amount_outcome(amount, outcome.clone());
                T::LiquidityRegistry::check_rewards(
                    src,
                    input_asset_id,
                    output_asset_id,
                    input_amount,
                    output_amount,
                )
                .unwrap_or(Vec::new())
            };
            return Ok((
                AggregatedSwapOutcome::new(
                    vec![(src.clone(), fixed!(1.0))],
                    outcome.amount,
                    outcome.fee,
                ),
                rewards,
            ));
        }

        // Check if we have exactly two sources: the primary market and the secondary market
        // Do the "smart" swap split (with fallback)
        if sources.len() == 2 {
            let mut primary_market: Option<LiquiditySourceIdOf<T>> = None;
            let mut secondary_market: Option<LiquiditySourceIdOf<T>> = None;

            for src in &sources {
                if src.liquidity_source_index
                    == LiquiditySourceType::MulticollateralBondingCurvePool
                {
                    primary_market = Some(src.clone());
                } else {
                    secondary_market = Some(src.clone());
                }
            }
            if let (Some(mcbc), Some(xyk)) = (primary_market, secondary_market) {
                let outcome = Self::smart_split_with_fallback(
                    &mcbc,
                    &xyk,
                    input_asset_id,
                    output_asset_id,
                    amount.clone(),
                    skip_info,
                )?;

                return Ok(outcome);
            }
        }

        // Otherwise, fall back to the general source-agnostic procedure based on sampling
        Self::generic_split(sources, input_asset_id, output_asset_id, amount, skip_info)
    }

    pub fn construct_trivial_path(
        input_asset_id: T::AssetId,
        output_asset_id: T::AssetId,
    ) -> ExchangePath<T> {
        let base_asset_id = T::GetBaseAssetId::get();
        if input_asset_id == base_asset_id || output_asset_id == base_asset_id {
            ExchangePath::Direct {
                from_asset_id: input_asset_id,
                to_asset_id: output_asset_id,
            }
        } else {
            ExchangePath::Twofold {
                from_asset_id: input_asset_id,
                intermediate_asset_id: base_asset_id,
                to_asset_id: output_asset_id,
            }
        }
    }

    /// Check if given two arbitrary tokens can be used to perform an exchange via any available sources.
    pub fn is_path_available(
        dex_id: T::DEXId,
        input_asset_id: T::AssetId,
        output_asset_id: T::AssetId,
    ) -> Result<bool, DispatchError> {
        let path = Self::construct_trivial_path(input_asset_id, output_asset_id);
        let path_exists = match path {
            ExchangePath::Direct {
                from_asset_id,
                to_asset_id,
            } => {
                let pair = Self::weak_sort_pair(from_asset_id, to_asset_id);
                !trading_pair::Pallet::<T>::list_enabled_sources_for_trading_pair(
                    &dex_id,
                    &pair.base_asset_id,
                    &pair.target_asset_id,
                )?
                .is_empty()
            }
            ExchangePath::Twofold {
                from_asset_id,
                intermediate_asset_id,
                to_asset_id,
            } => {
                !trading_pair::Pallet::<T>::list_enabled_sources_for_trading_pair(
                    &dex_id,
                    &intermediate_asset_id,
                    &from_asset_id,
                )?
                .is_empty()
                    && !trading_pair::Pallet::<T>::list_enabled_sources_for_trading_pair(
                        &dex_id,
                        &intermediate_asset_id,
                        &to_asset_id,
                    )?
                    .is_empty()
            }
        };
        Ok(path_exists)
    }

    /// Given two arbitrary tokens return all sources that can be used in exchange if path exists.
    pub fn list_enabled_sources_for_path(
        dex_id: T::DEXId,
        input_asset_id: T::AssetId,
        output_asset_id: T::AssetId,
    ) -> Result<Vec<LiquiditySourceType>, DispatchError> {
        let path = Self::construct_trivial_path(input_asset_id, output_asset_id);
        match path {
            ExchangePath::Direct {
                from_asset_id,
                to_asset_id,
            } => {
                let pair = Self::weak_sort_pair(from_asset_id, to_asset_id);
                let sources = trading_pair::Pallet::<T>::list_enabled_sources_for_trading_pair(
                    &dex_id,
                    &pair.base_asset_id,
                    &pair.target_asset_id,
                )?;
                if sources.is_empty() {
                    fail!(Error::<T>::UnavailableExchangePath);
                } else {
                    Ok(sources.into_iter().collect())
                }
            }
            ExchangePath::Twofold {
                from_asset_id,
                intermediate_asset_id,
                to_asset_id,
            } => {
                let first_swap = trading_pair::Pallet::<T>::list_enabled_sources_for_trading_pair(
                    &dex_id,
                    &intermediate_asset_id,
                    &from_asset_id,
                )?;
                let second_swap = trading_pair::Pallet::<T>::list_enabled_sources_for_trading_pair(
                    &dex_id,
                    &intermediate_asset_id,
                    &to_asset_id,
                )?;
                if !first_swap.is_empty() && !second_swap.is_empty() {
                    Ok(first_swap.union(&second_swap).cloned().collect())
                } else {
                    fail!(Error::<T>::UnavailableExchangePath);
                }
            }
        }
    }

    pub fn list_enabled_sources_for_path_with_xyk_forbidden(
        dex_id: T::DEXId,
        input_asset_id: T::AssetId,
        output_asset_id: T::AssetId,
    ) -> Result<Vec<LiquiditySourceType>, DispatchError> {
        let tbc_reserve_assets = T::PrimaryMarket::enabled_collaterals();
        let mut initial_result =
            Self::list_enabled_sources_for_path(dex_id, input_asset_id, output_asset_id)?;
        if tbc_reserve_assets.contains(&input_asset_id)
            || tbc_reserve_assets.contains(&output_asset_id)
        {
            initial_result.retain(|&lst| lst != LiquiditySourceType::XYKPool);
        }
        Ok(initial_result)
    }

    // Not full sort, just ensure that if there is XOR then it's first.
    fn weak_sort_pair(
        input_asset_id: T::AssetId,
        output_asset_id: T::AssetId,
    ) -> TradingPair<T::AssetId> {
        if input_asset_id == T::GetBaseAssetId::get() {
            TradingPair {
                base_asset_id: input_asset_id,
                target_asset_id: output_asset_id,
            }
        } else {
            TradingPair {
                base_asset_id: output_asset_id,
                target_asset_id: input_asset_id,
            }
        }
    }

    /// For direct path (when input token or output token are xor), extract xor portions of exchange result.
    fn get_xor_amount(
        input_asset_id: T::AssetId,
        _output_asset_id: T::AssetId,
        amount: SwapAmount<Balance>,
        outcome: SwapOutcome<Balance>,
    ) -> Balance {
        match amount {
            SwapAmount::WithDesiredInput {
                desired_amount_in, ..
            } => {
                if &input_asset_id == &T::GetBaseAssetId::get() {
                    desired_amount_in
                } else {
                    outcome.amount
                }
            }
            SwapAmount::WithDesiredOutput {
                desired_amount_out, ..
            } => {
                if &input_asset_id == &T::GetBaseAssetId::get() {
                    outcome.amount
                } else {
                    desired_amount_out
                }
            }
        }
    }

    // Position desired amount with outcome such that input and output values are aligned.
    fn sort_amount_outcome(
        amount: SwapAmount<Balance>,
        outcome: SwapOutcome<Balance>,
    ) -> (Balance, Balance) {
        match amount {
            SwapAmount::WithDesiredInput { .. } => (amount.amount(), outcome.amount),
            SwapAmount::WithDesiredOutput { .. } => (outcome.amount, amount.amount()),
        }
    }

    /// Implements the "smart" split algorithm.
    ///
    /// - `primary_source_id` - ID of the primary market liquidity source,
    /// - `secondary_source_id` - ID of the secondary market liquidity source,
    /// - `input_asset_id` - ID of the asset to sell,
    /// - `output_asset_id` - ID of the asset to buy,
    /// - `amount` - the amount with "direction" (sell or buy) together with the maximum price impact (slippage).
    /// - `skip_info` - flag that indicates that additional info should not be shown, that is needed when actual exchange is performed.
    ///
    fn smart_split_with_fallback(
        primary_source_id: &LiquiditySourceIdOf<T>,
        secondary_source_id: &LiquiditySourceIdOf<T>,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        skip_info: bool,
    ) -> Result<
        (
            AggregatedSwapOutcome<LiquiditySourceIdOf<T>, Balance>,
            Rewards<T::AssetId>,
        ),
        DispatchError,
    > {
        // The "smart" split algo is based on the following reasoning.
        // First, we try to calculate spot price of the `input_asset_id` in both
        // primary and secondary market. If the price in the secondary market is
        // better than that in primary market, we allocate as much of the `amount` to
        // be swapped in secondary market as we can until the prices level up.
        // The rest will be swapped in the primary market as it is (in most situations)
        // more attractive for the caller.
        //
        // In case the default partitioning between sources returns an error, it can
        // only be due to the MCBC pool not being available or initialized.
        // In this case the primary market weight is zeroed out and the entire `amount`
        // is sent to the secondary market (regardless whether the latter has enough
        // reserves to actually execute such swap).
        //
        // In case the "smart" procedure has returned some weights (a, b), such that
        // a > 0, b > 0, a + b == 1.0, and neither of the arms fails due to insufficient
        // reserves, we must still account for the fact that the algorithm tends to overweigh
        // the MCBC share which can lead to substantially non-optimal results
        // (especially when selling XOR to the MCBC).
        // To limit the impact of this imbalance we want to always compare the result of
        // the "smart" split with the purely secondary market one.
        // Comparing the result with the purely MCBC swap doesn't make sense in this case
        // because the "smart" swap is always at least as good as the 100% MCBC one.

        let base_asset = &T::GetBaseAssetId::get();

        ensure!(
            input_asset_id == base_asset || output_asset_id == base_asset,
            Error::<T>::UnavailableExchangePath
        );
        let other_asset = if base_asset == input_asset_id {
            output_asset_id
        } else {
            input_asset_id
        };

        let (reserves_base, reserves_other) = T::SecondaryMarket::reserves(base_asset, other_asset);

        let default_mcbc_weight = if output_asset_id == base_asset {
            // XOR is being bought
            Self::decide_mcbc_share_buying_base_asset(
                base_asset,
                other_asset,
                amount.clone(),
                (reserves_base, reserves_other),
            )
            .unwrap_or(
                // Error can only be due to MCBC pool, hence zeroing it out
                Fixed::ZERO,
            )
        } else {
            // XOR is being sold
            Self::decide_mcbc_share_selling_base_asset(
                base_asset,
                other_asset,
                amount.clone(),
                (reserves_base, reserves_other),
            )
            .unwrap_or(Fixed::ZERO)
        };

        let (is_better, extremum): (fn(a: Balance, b: Balance) -> bool, Balance) = match amount {
            SwapAmount::WithDesiredInput { .. } => (|a, b| a > b, Balance::zero()),
            _ => (|a, b| a < b, Balance::MAX),
        };

        let mut best: Balance = extremum;
        let mut total_fee: Balance = 0;
        let mut rewards = Vec::new();
        let mut distr: Vec<(LiquiditySourceIdOf<T>, Fixed)> = Vec::new();
        let mut maybe_error: Option<DispatchError> = None;

        if default_mcbc_weight > Fixed::ZERO {
            // Attempting to quote according to the default sources weights
            let amount_prim = if default_mcbc_weight < Fixed::ONE {
                <SwapAmount<Balance>>::unique_saturated_from(
                    <SwapAmount<Fixed>>::unique_saturated_from(amount) * default_mcbc_weight,
                )
            } else {
                amount.clone()
            };
            let intermediary_result = T::LiquidityRegistry::quote(
                primary_source_id,
                input_asset_id,
                output_asset_id,
                amount_prim.clone(),
            )
            .and_then(|outcome_prim| {
                if default_mcbc_weight < Fixed::ONE {
                    // TODO: implement Saturating trait for SwapAmount
                    let limit = match amount_prim {
                        SwapAmount::WithDesiredInput {
                            min_amount_out: l, ..
                        } => l,
                        SwapAmount::WithDesiredOutput {
                            max_amount_in: l, ..
                        } => l,
                    };
                    let amount_sec = match amount {
                        SwapAmount::WithDesiredInput {
                            desired_amount_in,
                            min_amount_out,
                        } => SwapAmount::with_desired_input(
                            desired_amount_in.saturating_sub(amount_prim.amount()),
                            min_amount_out.saturating_sub(limit),
                        ),
                        SwapAmount::WithDesiredOutput {
                            desired_amount_out,
                            max_amount_in,
                        } => SwapAmount::with_desired_output(
                            desired_amount_out.saturating_sub(amount_prim.amount()),
                            max_amount_in.saturating_sub(limit),
                        ),
                    };
                    T::LiquidityRegistry::quote(
                        secondary_source_id,
                        input_asset_id,
                        output_asset_id,
                        amount_sec.clone(),
                    )
                    .and_then(|outcome_sec| {
                        if !skip_info {
                            for info in vec![
                                (primary_source_id, amount_prim, outcome_prim.clone()),
                                (secondary_source_id, amount_sec, outcome_sec.clone()),
                            ] {
                                let (input_amount, output_amount) =
                                    Self::sort_amount_outcome(info.1, info.2);
                                rewards.append(
                                    &mut T::LiquidityRegistry::check_rewards(
                                        info.0,
                                        input_asset_id,
                                        output_asset_id,
                                        input_amount,
                                        output_amount,
                                    )
                                    .unwrap_or(Vec::new()),
                                );
                            }
                        };
                        best = outcome_prim.amount + outcome_sec.amount;
                        total_fee = outcome_prim.fee + outcome_sec.fee;
                        distr = vec![
                            (primary_source_id.clone(), default_mcbc_weight),
                            (
                                secondary_source_id.clone(),
                                Fixed::ONE.saturating_sub(default_mcbc_weight),
                            ),
                        ];
                        Ok(())
                    })
                } else {
                    best = outcome_prim.amount;
                    total_fee = outcome_prim.fee;
                    distr = vec![
                        (primary_source_id.clone(), Fixed::ONE),
                        (secondary_source_id.clone(), Fixed::ZERO),
                    ];
                    Ok(())
                }
            });
            if let Err(e) = intermediary_result {
                maybe_error = Some(e);
            }
        }

        // Regardless whether we have got any result so far, we still must do
        // calculations for the secondary market alone
        let xyk_result = T::LiquidityRegistry::quote(
            secondary_source_id,
            input_asset_id,
            output_asset_id,
            amount.clone(),
        )
        .and_then(|outcome| {
            if is_better(outcome.amount, best) {
                best = outcome.amount;
                total_fee = outcome.fee;
                distr = vec![
                    (primary_source_id.clone(), Fixed::ZERO),
                    (secondary_source_id.clone(), Fixed::ONE),
                ];
                if !skip_info {
                    let (input_amount, output_amount) =
                        Self::sort_amount_outcome(amount, outcome.clone());
                    rewards = T::LiquidityRegistry::check_rewards(
                        secondary_source_id,
                        input_asset_id,
                        output_asset_id,
                        input_amount,
                        output_amount,
                    )
                    .unwrap_or(Vec::new());
                };
            };
            Ok(())
        });

        // Check if we have got a result at either of the steps
        if let Err(err) = xyk_result {
            // If both attempts to get the price failed, return the first error
            if let Some(e) = maybe_error {
                // Quote at the first step was attempted and failed
                return Err(e);
            }
            if best == extremum {
                // The quote at first step was never attempted, returning the current error
                return Err(err);
            }
        }

        Ok((AggregatedSwapOutcome::new(distr, best, total_fee), rewards))
    }

    /// Implements a generic source-agnostic split algorithm to partition a trade between
    /// an arbitrary number of liquidity sources of arbitrary types.
    ///
    /// - `sources` - a vector of liquidity sources IDs,
    /// - `input_asset_id` - ID of the asset to sell,
    /// - `output_asset_id` - ID of the asset to buy,
    /// - `amount` - the amount with "direction" (sell or buy) together with the maximum price impact (slippage).
    /// - `skip_info` - flag that indicates that additional info should not be shown, that is needed when actual exchange is performed.
    ///
    fn generic_split(
        sources: Vec<LiquiditySourceIdOf<T>>,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        skip_info: bool,
    ) -> Result<
        (
            AggregatedSwapOutcome<LiquiditySourceIdOf<T>, Balance>,
            Rewards<T::AssetId>,
        ),
        DispatchError,
    > {
        let amount = <SwapAmount<Fixed>>::unique_saturated_from(amount);
        let num_samples = T::GetNumSamples::get();
        let (sample_data, sample_fees): (Vec<Vec<Fixed>>, Vec<Vec<Fixed>>) = sources
            .iter()
            .map(|src| {
                Self::sample_liquidity_source(
                    src,
                    input_asset_id,
                    output_asset_id,
                    amount,
                    num_samples,
                )
            })
            .map(|row| row.iter().map(|x| (x.amount, x.fee)).unzip())
            .unzip();

        let (distr, best) = match amount {
            SwapAmount::WithDesiredInput { .. } => {
                algo::find_distribution(sample_data.clone(), false)
            }
            _ => algo::find_distribution(sample_data.clone(), true),
        };

        ensure!(
            best > Fixed::ZERO && best < Fixed::MAX,
            Error::<T>::AggregationError
        );

        let num_samples =
            FixedInner::try_from(num_samples).map_err(|_| Error::CalculationError::<T>)?;
        let total_fee: FixedWrapper = (0..distr.len()).fold(fixed!(0), |acc, i| {
            let idx = match distr[i].cmul(num_samples) {
                Err(_) => return acc,
                Ok(index) => index.rounding_to_i64(),
            };
            acc + *sample_fees[i]
                .get((idx - 1) as usize)
                .unwrap_or(&Fixed::ZERO)
        });
        let total_fee = total_fee.get().map_err(|_| Error::CalculationError::<T>)?;

        let mut rewards = Vec::new();
        if !skip_info {
            for i in 0..distr.len() {
                let idx = match distr[i].cmul(num_samples) {
                    Err(_) => continue,
                    Ok(index) => index.rounding_to_i64(),
                };
                let amount_a = match sample_data[i]
                    .get((idx - 1) as usize)
                    .unwrap_or(&Fixed::ZERO)
                    .into_bits()
                    .try_into()
                {
                    Err(_) => continue,
                    Ok(amt) => amt,
                };
                let amount_b = match (distr[i] * amount).amount().into_bits().try_into() {
                    Err(_) => continue,
                    Ok(amt) => amt,
                };
                let (input_amount, output_amount) = match amount {
                    SwapAmount::WithDesiredInput { .. } => (amount_b, amount_a),
                    SwapAmount::WithDesiredOutput { .. } => (amount_a, amount_b),
                };
                let source = match sources.get(i) {
                    None => continue,
                    Some(source) => source,
                };
                let mut current_rewards = T::LiquidityRegistry::check_rewards(
                    source,
                    input_asset_id,
                    output_asset_id,
                    input_amount,
                    output_amount,
                )
                .unwrap_or(Vec::new());
                rewards.append(&mut current_rewards);
            }
        }

        Ok((
            AggregatedSwapOutcome::new(
                sources
                    .into_iter()
                    .zip(distr.into_iter())
                    .collect::<Vec<_>>(),
                best.into_bits()
                    .try_into()
                    .map_err(|_| Error::CalculationError::<T>)?,
                total_fee
                    .into_bits()
                    .try_into()
                    .map_err(|_| Error::CalculationError::<T>)?,
            ),
            rewards,
        ))
    }

    /// Determines the share of a swap that should be exchanged in the primary market
    /// (i.e. the multi-collateral bonding curve pool) based on the current reserves of
    /// the base asset and the collateral asset in the secondary market (e.g. an XYK pool)
    /// provided the base asset is being bought.
    ///
    /// - `base_asset_id` - ID of the base asset,
    /// - `collateral_asset_id` - ID of the collateral asset,
    /// - `amount` - the swap amount with "direction" (fixed input vs fixed output),
    /// - `secondary_market_reserves` - a pair (base_reserve, collateral_reserve) in the secondary market
    ///
    fn decide_mcbc_share_buying_base_asset(
        base_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        secondary_market_reserves: (Balance, Balance),
    ) -> Result<Fixed, DispatchError> {
        let (reserves_base, reserves_other) = secondary_market_reserves;
        let x: FixedWrapper = reserves_base.into();
        let y: FixedWrapper = reserves_other.into();
        let sqrt_k: FixedWrapper = x.multiply_and_sqrt(&y);
        let secondary_price: FixedWrapper = if x > fixed_wrapper!(0) {
            y.clone() / x.clone()
        } else {
            Fixed::MAX.into()
        };

        let primary_buy_price: FixedWrapper =
            T::PrimaryMarket::buy_price(base_asset_id, collateral_asset_id)
                .map_err(|_| Error::<T>::CalculationError)?
                .into();
        let sqrt_buy_price = primary_buy_price.clone().sqrt_accurate();

        match amount {
            SwapAmount::WithDesiredInput {
                desired_amount_in, ..
            } => {
                let mut fraction_sec: Fixed = fixed!(0);
                let wrapped_amount: FixedWrapper = desired_amount_in.into();
                if secondary_price < primary_buy_price {
                    let delta_y = sqrt_k * sqrt_buy_price - y; // always > 0
                    fraction_sec = (delta_y / wrapped_amount.clone())
                        .get()
                        .unwrap_or(fixed!(0));
                    if fraction_sec > fixed!(1) {
                        fraction_sec = fixed!(1);
                    }
                    if fraction_sec < fixed!(0) {
                        fraction_sec = fixed!(0);
                    }
                }
                let fraction_prim: Fixed = (Fixed::ONE - fraction_sec.into()).get().unwrap();

                Ok(fraction_prim)
            }
            SwapAmount::WithDesiredOutput {
                desired_amount_out, ..
            } => {
                let mut fraction_sec: Fixed = fixed!(0);
                let wrapped_amount: FixedWrapper = desired_amount_out.into();
                if secondary_price < primary_buy_price {
                    let delta_x = x - sqrt_k / sqrt_buy_price; // always > 0
                    fraction_sec = (delta_x / wrapped_amount.clone())
                        .get()
                        .unwrap_or(fixed!(0));
                    if fraction_sec > fixed!(1) {
                        fraction_sec = fixed!(1);
                    }
                    if fraction_sec < fixed!(0) {
                        fraction_sec = fixed!(0);
                    }
                }
                let fraction_prim: Fixed = (Fixed::ONE - fraction_sec.into()).get().unwrap();

                Ok(fraction_prim)
            }
        }
    }

    /// Determines the share of a swap that should be exchanged in the primary market
    /// (i.e. the multi-collateral bonding curve pool) based on the current reserves of
    /// the base asset and the collateral asset in the secondary market (e.g. an XYK pool)
    /// provided the base asset is being sold.
    ///
    /// - `base_asset_id` - ID of the base asset,
    /// - `collateral_asset_id` - ID of the collateral asset,
    /// - `amount` - the swap amount with "direction" (fixed input vs fixed output),
    /// - `secondary_market_reserves` - a pair (base_reserve, collateral_reserve) in the secondary market
    ///
    fn decide_mcbc_share_selling_base_asset(
        base_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        secondary_market_reserves: (Balance, Balance),
    ) -> Result<Fixed, DispatchError> {
        let (reserves_base, reserves_other) = secondary_market_reserves;
        let x: FixedWrapper = reserves_base.into();
        let y: FixedWrapper = reserves_other.into();
        let sqrt_k: FixedWrapper = x.multiply_and_sqrt(&y);
        let secondary_price: FixedWrapper = if x > fixed_wrapper!(0) {
            y.clone() / x.clone()
        } else {
            Fixed::ZERO.into()
        };

        let primary_sell_price: FixedWrapper =
            T::PrimaryMarket::sell_price(base_asset_id, collateral_asset_id)
                .map_err(|_| Error::<T>::CalculationError)?
                .into();
        let sqrt_sell_price = primary_sell_price.clone().sqrt_accurate();

        match amount {
            SwapAmount::WithDesiredInput {
                desired_amount_in, ..
            } => {
                let mut fraction_sec: Fixed = fixed!(0);
                let wrapped_amount: FixedWrapper = desired_amount_in.into();
                if secondary_price > primary_sell_price {
                    let delta_x = sqrt_k / sqrt_sell_price - x; // always > 0
                    fraction_sec = (delta_x / wrapped_amount.clone())
                        .get()
                        .unwrap_or(fixed!(0));
                    if fraction_sec > fixed!(1) {
                        fraction_sec = fixed!(1);
                    }
                    if fraction_sec < fixed!(0) {
                        fraction_sec = fixed!(0);
                    }
                }
                let fraction_prim: Fixed = (Fixed::ONE - fraction_sec.into()).get().unwrap();

                Ok(fraction_prim)
            }
            SwapAmount::WithDesiredOutput {
                desired_amount_out, ..
            } => {
                let mut fraction_sec: Fixed = fixed!(0);
                let wrapped_amount: FixedWrapper = desired_amount_out.into();
                if secondary_price > primary_sell_price {
                    let delta_y = y - sqrt_k * sqrt_sell_price; // always > 0
                    fraction_sec = (delta_y / wrapped_amount.clone())
                        .get()
                        .unwrap_or(fixed!(0));
                    if fraction_sec > fixed!(1) {
                        fraction_sec = fixed!(1);
                    }
                    if fraction_sec < fixed!(0) {
                        fraction_sec = fixed!(0);
                    }
                }
                let fraction_prim: Fixed = (Fixed::ONE - fraction_sec.into()).get().unwrap();

                Ok(fraction_prim)
            }
        }
    }
}

impl<T: Config> LiquidityProxyTrait<T::DEXId, T::AccountId, T::AssetId> for Pallet<T> {
    /// Applies trivial routing (via Base Asset), resulting in a poly-swap which may contain several individual swaps.
    /// Those individual swaps are subject to liquidity aggregation algorithm.
    ///
    /// This a wrapper for `quote_single`.
    fn quote(
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        ensure!(
            input_asset_id != output_asset_id,
            Error::<T>::UnavailableExchangePath
        );
        match Self::construct_trivial_path(*input_asset_id, *output_asset_id) {
            ExchangePath::Direct {
                from_asset_id,
                to_asset_id,
            } => Self::quote_single(&from_asset_id, &to_asset_id, amount, filter, true)
                .map(|(aso, _)| SwapOutcome::new(aso.amount, aso.fee).into()),
            ExchangePath::Twofold {
                from_asset_id,
                intermediate_asset_id,
                to_asset_id,
            } => match amount {
                SwapAmount::WithDesiredInput {
                    desired_amount_in, ..
                } => {
                    let (first_quote, _) = Self::quote_single(
                        &from_asset_id,
                        &intermediate_asset_id,
                        SwapAmount::with_desired_input(desired_amount_in, Balance::zero()),
                        filter.clone(),
                        true,
                    )?;
                    let (second_quote, _) = Self::quote_single(
                        &intermediate_asset_id,
                        &to_asset_id,
                        SwapAmount::with_desired_input(first_quote.amount, Balance::zero()),
                        filter,
                        true,
                    )?;
                    let cumulative_fee = first_quote.fee + second_quote.fee;
                    Ok(SwapOutcome::new(second_quote.amount, cumulative_fee))
                }
                SwapAmount::WithDesiredOutput {
                    desired_amount_out, ..
                } => {
                    let (second_quote, _) = Self::quote_single(
                        &intermediate_asset_id,
                        &to_asset_id,
                        SwapAmount::with_desired_output(desired_amount_out, Balance::MAX),
                        filter.clone(),
                        true,
                    )?;
                    let (first_quote, _) = Self::quote_single(
                        &from_asset_id,
                        &intermediate_asset_id,
                        SwapAmount::with_desired_output(second_quote.amount, Balance::MAX),
                        filter,
                        true,
                    )?;
                    let cumulative_fee = first_quote.fee + second_quote.fee;
                    Ok(SwapOutcome::new(first_quote.amount, cumulative_fee))
                }
            },
        }
    }

    /// Applies trivial routing (via Base Asset), resulting in a poly-swap which may contain several individual swaps.
    /// Those individual swaps are subject to liquidity aggregation algorithm.
    ///
    /// This a wrapper for `exchange_single`.
    fn exchange(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        ensure!(
            input_asset_id != output_asset_id,
            Error::<T>::UnavailableExchangePath
        );
        common::with_transaction(|| {
            match Self::construct_trivial_path(*input_asset_id, *output_asset_id) {
                ExchangePath::Direct {
                    from_asset_id,
                    to_asset_id,
                } => {
                    let outcome = Self::exchange_single(
                        sender,
                        receiver,
                        &from_asset_id,
                        &to_asset_id,
                        amount.clone(),
                        filter,
                    )?;
                    let xor_volume =
                        Self::get_xor_amount(from_asset_id, to_asset_id, amount, outcome.clone());
                    T::VestedRewardsAggregator::update_market_maker_records(
                        &sender, xor_volume, 1,
                    )?;
                    Ok(outcome)
                }
                ExchangePath::Twofold {
                    from_asset_id,
                    intermediate_asset_id,
                    to_asset_id,
                } => match amount {
                    SwapAmount::WithDesiredInput {
                        desired_amount_in,
                        min_amount_out,
                    } => {
                        let transit_account = T::GetTechnicalAccountId::get();
                        let first_swap = Self::exchange_single(
                            sender,
                            &transit_account,
                            &from_asset_id,
                            &intermediate_asset_id,
                            SwapAmount::with_desired_input(desired_amount_in, Balance::zero()),
                            filter.clone(),
                        )?;
                        let second_swap = Self::exchange_single(
                            &transit_account,
                            receiver,
                            &intermediate_asset_id,
                            &to_asset_id,
                            SwapAmount::with_desired_input(first_swap.amount, Balance::zero()),
                            filter,
                        )?;
                        ensure!(
                            second_swap.amount >= min_amount_out,
                            Error::<T>::SlippageNotTolerated
                        );
                        T::VestedRewardsAggregator::update_market_maker_records(
                            &sender,
                            first_swap.amount,
                            2,
                        )?;
                        let cumulative_fee = first_swap.fee + second_swap.fee;
                        Ok(SwapOutcome::new(second_swap.amount, cumulative_fee))
                    }
                    SwapAmount::WithDesiredOutput {
                        desired_amount_out,
                        max_amount_in,
                    } => {
                        let (second_quote, _) = Self::quote_single(
                            &intermediate_asset_id,
                            &to_asset_id,
                            SwapAmount::with_desired_output(desired_amount_out, Balance::MAX),
                            filter.clone(),
                            true,
                        )?;
                        let (first_quote, _) = Self::quote_single(
                            &from_asset_id,
                            &intermediate_asset_id,
                            SwapAmount::with_desired_output(second_quote.amount, Balance::MAX),
                            filter.clone(),
                            true,
                        )?;
                        ensure!(
                            first_quote.amount <= max_amount_in,
                            Error::<T>::SlippageNotTolerated
                        );
                        let transit_account = T::GetTechnicalAccountId::get();
                        let first_swap = Self::exchange_single(
                            sender,
                            &transit_account,
                            &from_asset_id,
                            &intermediate_asset_id,
                            SwapAmount::with_desired_input(first_quote.amount, Balance::zero()),
                            filter.clone(),
                        )?;
                        let second_swap = Self::exchange_single(
                            &transit_account,
                            receiver,
                            &intermediate_asset_id,
                            &to_asset_id,
                            SwapAmount::with_desired_input(first_swap.amount, Balance::zero()),
                            filter,
                        )?;
                        T::VestedRewardsAggregator::update_market_maker_records(
                            &sender,
                            first_swap.amount,
                            2,
                        )?;
                        let cumulative_fee = first_swap.fee + second_swap.fee;
                        Ok(SwapOutcome::new(first_quote.amount, cumulative_fee))
                    }
                },
            }
        })
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use assets::AssetIdOf;
    use common::{AccountIdOf, DexIdOf};
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + common::Config + assets::Config + trading_pair::Config
    {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type LiquidityRegistry: LiquidityRegistry<
            Self::DEXId,
            Self::AccountId,
            Self::AssetId,
            LiquiditySourceType,
            Balance,
            DispatchError,
        >;
        type GetNumSamples: Get<usize>;
        type GetTechnicalAccountId: Get<Self::AccountId>;
        type PrimaryMarket: GetMarketInfo<Self::AssetId>;
        type SecondaryMarket: GetPoolReserves<Self::AssetId>;
        type VestedRewardsAggregator: VestedRewardsTrait<Self::AccountId>;
        /// Weight information for the extrinsics in this Pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Perform swap of tokens (input/output defined via SwapAmount direction).
        ///
        /// - `origin`: the account on whose behalf the transaction is being executed,
        /// - `dex_id`: DEX ID for which liquidity sources aggregation is being done,
        /// - `input_asset_id`: ID of the asset being sold,
        /// - `output_asset_id`: ID of the asset being bought,
        /// - `swap_amount`: the exact amount to be sold (either in input_asset_id or output_asset_id units with corresponding slippage tolerance absolute bound),
        /// - `selected_source_types`: list of selected LiquiditySource types, selection effect is determined by filter_mode,
        /// - `filter_mode`: indicate either to allow or forbid selected types only, or disable filtering.
        #[pallet::weight(<T as Config>::WeightInfo::swap((*swap_amount).into()))]
        pub fn swap(
            origin: OriginFor<T>,
            dex_id: T::DEXId,
            input_asset_id: T::AssetId,
            output_asset_id: T::AssetId,
            swap_amount: SwapAmount<Balance>,
            selected_source_types: Vec<LiquiditySourceType>,
            filter_mode: FilterMode,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            if Self::is_forbidden_filter(
                &input_asset_id,
                &output_asset_id,
                &selected_source_types,
                &filter_mode,
            ) {
                fail!(Error::<T>::ForbiddenFilter);
            }

            let outcome = Self::exchange(
                &who,
                &who,
                &input_asset_id,
                &output_asset_id,
                swap_amount,
                LiquiditySourceFilter::with_mode(dex_id, filter_mode, selected_source_types),
            )?;

            let (input_amount, output_amount, fee_amount) = match swap_amount {
                SwapAmount::WithDesiredInput {
                    desired_amount_in, ..
                } => (desired_amount_in, outcome.amount, outcome.fee),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out, ..
                } => (outcome.amount, desired_amount_out, outcome.fee),
            };
            Self::deposit_event(Event::<T>::Exchange(
                who,
                dex_id,
                input_asset_id,
                output_asset_id,
                input_amount,
                output_amount,
                fee_amount,
            ));

            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId", AssetIdOf<T> = "AssetId", DexIdOf<T> = "DEXId")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Exchange of tokens has been performed
        /// [Caller Account, DEX Id, Input Asset Id, Output Asset Id, Input Amount, Output Amount, Fee Amount]
        Exchange(
            AccountIdOf<T>,
            DexIdOf<T>,
            AssetIdOf<T>,
            AssetIdOf<T>,
            Balance,
            Balance,
            Balance,
        ),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// No route exists in a given DEX for given parameters to carry out the swap
        UnavailableExchangePath,
        /// Max fee exceeded
        MaxFeeExceeded,
        /// Fee value outside of the basis points range [0..10000]
        InvalidFeeValue,
        /// None of the sources has enough reserves to execute a trade
        InsufficientLiquidity,
        /// Path exists but it's not possible to perform exchange with currently available liquidity on pools.
        AggregationError,
        /// Specified parameters lead to arithmetic error
        CalculationError,
        /// Slippage either exceeds minimum tolerated output or maximum tolerated input.
        SlippageNotTolerated,
        /// Selected filtering request is not allowed.
        ForbiddenFilter,
    }
}
