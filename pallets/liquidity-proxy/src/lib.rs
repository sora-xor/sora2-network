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

use common::prelude::fixnum::ops::{Bounded, Zero as _};
use common::prelude::{Balance, FixedWrapper, QuoteAmount, SwapAmount, SwapOutcome, SwapVariant};
use common::{
    balance, fixed_wrapper, FilterMode, Fixed, GetMarketInfo, GetPoolReserves, LiquidityRegistry,
    LiquiditySource, LiquiditySourceFilter, LiquiditySourceId, LiquiditySourceType, RewardReason,
    TradingPair, VestedRewardsPallet, XSTUSD,
};
use frame_support::traits::Get;
use frame_support::weights::Weight;
use frame_support::{ensure, fail, RuntimeDebug};
use frame_system::ensure_signed;
use sp_runtime::traits::{CheckedSub, Zero};
use sp_runtime::DispatchError;
use sp_std::prelude::*;

pub type LiquiditySourceIdOf<T> =
    LiquiditySourceId<<T as common::Config>::DEXId, LiquiditySourceType>;

type Rewards<AssetId> = Vec<(Balance, AssetId, RewardReason)>;

pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

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
    /// A distribution of amounts each liquidity sources gets to swap in the entire trade
    pub distribution: Vec<(LiquiditySourceType, QuoteAmount<AmountType>)>,
    /// The best possible output/input amount for a given trade and a set of liquidity sources
    pub amount: AmountType,
    /// Total fee amount, nominated in XOR
    pub fee: AmountType,
}

impl<LiquiditySourceIdType, AmountType> AggregatedSwapOutcome<LiquiditySourceIdType, AmountType> {
    pub fn new(
        distribution: Vec<(LiquiditySourceIdType, QuoteAmount<AmountType>)>,
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
pub trait LiquidityProxyTrait<DEXId: PartialEq + Copy, AccountId, AssetId, LiquiditySourceIdOf> {
    /// Get spot price of tokens based on desired amount, None returned if liquidity source
    /// does not have available exchange methods for indicated path.
    fn quote(
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Balance>,
        filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
        deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance>, DispatchError>;

    /// Perform exchange based on desired amount.
    fn exchange(
        sender: &AccountId,
        receiver: &AccountId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<(SwapOutcome<Balance>, Vec<LiquiditySourceIdOf>), DispatchError>;
}

impl<DEXId: PartialEq + Copy, AccountId, AssetId, LiquiditySourceIdOf>
    LiquidityProxyTrait<DEXId, AccountId, AssetId, LiquiditySourceIdOf> for ()
{
    fn quote(
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _amount: QuoteAmount<Balance>,
        _filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
        _deduce_fee: bool,
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
    ) -> Result<(SwapOutcome<Balance>, Vec<LiquiditySourceIdOf>), DispatchError> {
        unimplemented!()
    }
}

pub trait WeightInfo {
    fn swap(variant: SwapVariant) -> Weight;
}

impl<T: Config> Pallet<T> {
    /// Temporary workaround to prevent tbc oracle exploit with xyk-only filter.
    pub fn is_forbidden_filter(
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        selected_source_types: &Vec<LiquiditySourceType>,
        filter_mode: &FilterMode,
    ) -> bool {
        let tbc_reserve_assets = T::PrimaryMarketTBC::enabled_target_assets();
        // check if user has selected only xyk either explicitly or by excluding other types
        // FIXME: such detection approach is unreliable, come up with better way
        let is_xyk_only = selected_source_types.contains(&LiquiditySourceType::XYKPool)
            && !selected_source_types
                .contains(&LiquiditySourceType::MulticollateralBondingCurvePool)
            && !selected_source_types.contains(&LiquiditySourceType::XSTPool)
            && filter_mode == &FilterMode::AllowSelected
            || selected_source_types
                .contains(&LiquiditySourceType::MulticollateralBondingCurvePool)
                && selected_source_types.contains(&LiquiditySourceType::XSTPool)
                && !selected_source_types.contains(&LiquiditySourceType::XYKPool)
                && filter_mode == &FilterMode::ForbidSelected;
        // check if either of tbc reserve assets is present
        let reserve_asset_present = tbc_reserve_assets.contains(input_asset_id)
            || tbc_reserve_assets.contains(output_asset_id);

        is_xyk_only && reserve_asset_present
    }

    fn get_liqidity_sources_for_event(
        dex_id: T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        selected_source_types: Vec<LiquiditySourceType>,
        filter_mode: FilterMode,
    ) -> Result<Vec<LiquiditySourceIdOf<T>>, DispatchError> {
        let filter = LiquiditySourceFilter::with_mode(dex_id, filter_mode, selected_source_types);
        let sources =
            T::LiquidityRegistry::list_liquidity_sources(input_asset_id, output_asset_id, filter)?;

        match sources.len() {
            1 | 2 => Ok(sources),
            _ => fail!(Error::<T>::UnavailableExchangePath),
        }
    }

    pub fn inner_swap(
        sender: T::AccountId,
        receiver: T::AccountId,
        dex_id: T::DEXId,
        input_asset_id: T::AssetId,
        output_asset_id: T::AssetId,
        swap_amount: SwapAmount<Balance>,
        selected_source_types: Vec<LiquiditySourceType>,
        filter_mode: FilterMode,
    ) -> Result<(), DispatchError> {
        if Self::is_forbidden_filter(
            &input_asset_id,
            &output_asset_id,
            &selected_source_types,
            &filter_mode,
        ) {
            fail!(Error::<T>::ForbiddenFilter);
        }

        let sources_for_event = Self::get_liqidity_sources_for_event(
            dex_id,
            &input_asset_id,
            &output_asset_id,
            selected_source_types.clone(),
            filter_mode.clone(),
        )?;

        let (outcome, sources) = Self::inner_exchange(
            &sender,
            &receiver,
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
            sender,
            dex_id,
            input_asset_id,
            output_asset_id,
            input_amount,
            output_amount,
            fee_amount,
            sources_for_event,
        ));

        Ok(().into())
    }

    /// Applies trivial routing (via Base Asset), resulting in a poly-swap which may contain several individual swaps.
    /// Those individual swaps are subject to liquidity aggregation algorithm.
    ///
    /// This a wrapper for `exchange_single`.
    pub fn inner_exchange(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<(SwapOutcome<Balance>, Vec<LiquiditySourceIdOf<T>>), DispatchError> {
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
                    let (outcome, sources) = Self::exchange_single(
                        sender,
                        receiver,
                        &from_asset_id,
                        &to_asset_id,
                        amount,
                        filter,
                    )?;
                    let xor_volume =
                        Self::get_xor_amount(from_asset_id, to_asset_id, amount, outcome.clone());
                    T::VestedRewardsPallet::update_market_maker_records(
                        &sender,
                        xor_volume,
                        1,
                        &from_asset_id,
                        &to_asset_id,
                        None,
                    )?;
                    Ok((outcome, sources))
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
                        let (first_swap, mut first_sources) = Self::exchange_single(
                            sender,
                            &transit_account,
                            &from_asset_id,
                            &intermediate_asset_id,
                            SwapAmount::with_desired_input(desired_amount_in, Balance::zero()),
                            filter.clone(),
                        )?;
                        let (second_swap, second_sources) = Self::exchange_single(
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
                        T::VestedRewardsPallet::update_market_maker_records(
                            &sender,
                            first_swap.amount,
                            2,
                            &from_asset_id,
                            &to_asset_id,
                            Some(&intermediate_asset_id),
                        )?;
                        let cumulative_fee = first_swap
                            .fee
                            .checked_add(second_swap.fee)
                            .ok_or(Error::<T>::CalculationError)?;
                        first_sources.extend(second_sources);
                        Ok((
                            SwapOutcome::new(second_swap.amount, cumulative_fee),
                            first_sources,
                        ))
                    }
                    SwapAmount::WithDesiredOutput {
                        desired_amount_out,
                        max_amount_in,
                    } => {
                        let (second_quote, _, _) = Self::quote_single(
                            &intermediate_asset_id,
                            &to_asset_id,
                            QuoteAmount::with_desired_output(desired_amount_out),
                            filter.clone(),
                            true,
                            true,
                        )?;
                        let (first_quote, _, _) = Self::quote_single(
                            &from_asset_id,
                            &intermediate_asset_id,
                            QuoteAmount::with_desired_output(second_quote.amount),
                            filter.clone(),
                            true,
                            true,
                        )?;
                        ensure!(
                            first_quote.amount <= max_amount_in,
                            Error::<T>::SlippageNotTolerated
                        );
                        let transit_account = T::GetTechnicalAccountId::get();
                        let (first_swap, mut first_sources) = Self::exchange_single(
                            sender,
                            &transit_account,
                            &from_asset_id,
                            &intermediate_asset_id,
                            SwapAmount::with_desired_input(first_quote.amount, Balance::zero()),
                            filter.clone(),
                        )?;
                        let (second_swap, second_sources) = Self::exchange_single(
                            &transit_account,
                            receiver,
                            &intermediate_asset_id,
                            &to_asset_id,
                            SwapAmount::with_desired_input(first_swap.amount, Balance::zero()),
                            filter,
                        )?;
                        T::VestedRewardsPallet::update_market_maker_records(
                            &sender,
                            first_swap.amount,
                            2,
                            &from_asset_id,
                            &to_asset_id,
                            Some(&intermediate_asset_id),
                        )?;
                        let cumulative_fee = first_swap
                            .fee
                            .checked_add(second_swap.fee)
                            .ok_or(Error::<T>::CalculationError)?;
                        first_sources.extend(second_sources);
                        Ok((
                            SwapOutcome::new(first_quote.amount, cumulative_fee),
                            first_sources,
                        ))
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
    ) -> Result<(SwapOutcome<Balance>, Vec<LiquiditySourceIdOf<T>>), DispatchError> {
        common::with_transaction(|| {
            let quote_tuple = Self::quote_single(
                input_asset_id,
                output_asset_id,
                amount.into(),
                filter,
                true,
                true,
            )?;
            let sources = quote_tuple.2;

            let res = quote_tuple
                .0
                .distribution
                .into_iter()
                .filter(|(_src, part_amount)| part_amount.amount() > balance!(0))
                .map(|(src, part_amount)| {
                    let part_amount = part_amount.amount();
                    let part_limit = (FixedWrapper::from(part_amount) / amount.amount()
                        * amount.limit())
                    .try_into_balance()
                    .map_err(|_| Error::CalculationError::<T>)?;
                    T::LiquidityRegistry::exchange(
                        sender,
                        receiver,
                        &src,
                        input_asset_id,
                        output_asset_id,
                        amount.copy_direction(part_amount, part_limit),
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

            Ok((SwapOutcome::new(amount, fee), sources))
        })
    }

    /// Applies trivial routing (via Base Asset), resulting in a poly-swap which may contain several individual swaps.
    /// Those individual swaps are subject to liquidity aggregation algorithm.
    ///
    /// This a wrapper for `quote_single`.
    pub fn inner_quote(
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
        skip_info: bool,
        deduce_fee: bool,
    ) -> Result<
        (
            SwapOutcome<Balance>,
            Rewards<T::AssetId>,
            Option<Balance>,
            Vec<LiquiditySourceIdOf<T>>,
        ),
        DispatchError,
    > {
        ensure!(
            input_asset_id != output_asset_id,
            Error::<T>::UnavailableExchangePath
        );
        match Self::construct_trivial_path(*input_asset_id, *output_asset_id) {
            ExchangePath::Direct {
                from_asset_id,
                to_asset_id,
            } => {
                let (aso, rewards, liquidity_sources) = Self::quote_single(
                    &from_asset_id,
                    &to_asset_id,
                    amount,
                    filter,
                    skip_info,
                    deduce_fee,
                )?;
                let quote_without_impact = if skip_info {
                    None
                } else {
                    Some(Self::calculate_amount_without_impact(
                        input_asset_id,
                        output_asset_id,
                        &aso.distribution,
                        deduce_fee,
                    )?)
                };
                Ok((
                    SwapOutcome::new(aso.amount, aso.fee).into(),
                    rewards,
                    quote_without_impact,
                    liquidity_sources,
                ))
            }
            ExchangePath::Twofold {
                from_asset_id,
                intermediate_asset_id,
                to_asset_id,
            } => match amount {
                QuoteAmount::WithDesiredInput { desired_amount_in } => {
                    let (first_quote, rewards_a, mut first_liquidity_sources) = Self::quote_single(
                        &from_asset_id,
                        &intermediate_asset_id,
                        QuoteAmount::with_desired_input(desired_amount_in),
                        filter.clone(),
                        skip_info,
                        deduce_fee,
                    )?;
                    let (second_quote, mut rewards_b, second_liquidity_sources) =
                        Self::quote_single(
                            &intermediate_asset_id,
                            &to_asset_id,
                            QuoteAmount::with_desired_input(first_quote.amount),
                            filter,
                            skip_info,
                            deduce_fee,
                        )?;
                    let quote_without_impact = if skip_info {
                        None
                    } else {
                        let first_quote_without_impact = Self::calculate_amount_without_impact(
                            &from_asset_id,
                            &intermediate_asset_id,
                            &first_quote.distribution,
                            deduce_fee,
                        )?;
                        let ratio_to_actual = FixedWrapper::from(first_quote_without_impact)
                            / FixedWrapper::from(first_quote.amount);
                        let distribution: Result<Vec<_>, _> = second_quote
                            .distribution
                            .iter()
                            .cloned()
                            .map(|(ls, am)| {
                                let am_adjusted = (FixedWrapper::from(am.amount())
                                    * ratio_to_actual.clone())
                                .try_into_balance();
                                if am_adjusted.is_ok() {
                                    Ok((ls, am.copy_direction(am_adjusted.unwrap())))
                                } else {
                                    Err(Error::<T>::FailedToCalculatePriceWithoutImpact)
                                }
                            })
                            .collect();
                        let second_quote_without_impact = Self::calculate_amount_without_impact(
                            &intermediate_asset_id,
                            &to_asset_id,
                            &distribution?,
                            deduce_fee,
                        )?;
                        Some(second_quote_without_impact)
                    };
                    let cumulative_fee = first_quote
                        .fee
                        .checked_add(second_quote.fee)
                        .ok_or(Error::<T>::CalculationError)?;
                    let mut rewards = rewards_a;
                    rewards.append(&mut rewards_b);
                    first_liquidity_sources.extend(second_liquidity_sources);
                    Ok((
                        SwapOutcome::new(second_quote.amount, cumulative_fee),
                        rewards,
                        quote_without_impact,
                        first_liquidity_sources,
                    ))
                }
                QuoteAmount::WithDesiredOutput { desired_amount_out } => {
                    let (second_quote, mut rewards_b, mut second_liquidity_sources) =
                        Self::quote_single(
                            &intermediate_asset_id,
                            &to_asset_id,
                            QuoteAmount::with_desired_output(desired_amount_out),
                            filter.clone(),
                            skip_info,
                            deduce_fee,
                        )?;
                    let (first_quote, rewards_a, first_liquidity_sources) = Self::quote_single(
                        &from_asset_id,
                        &intermediate_asset_id,
                        QuoteAmount::with_desired_output(second_quote.amount),
                        filter,
                        skip_info,
                        deduce_fee,
                    )?;
                    let quote_without_impact = if skip_info {
                        None
                    } else {
                        let second_quote_without_impact = Self::calculate_amount_without_impact(
                            &intermediate_asset_id,
                            &to_asset_id,
                            &second_quote.distribution,
                            deduce_fee,
                        )?;
                        let ratio_to_actual = FixedWrapper::from(second_quote_without_impact)
                            / FixedWrapper::from(second_quote.amount);
                        let distribution: Result<Vec<_>, _> = first_quote
                            .distribution
                            .iter()
                            .cloned()
                            .map(|(ls, am)| {
                                let am_adjusted = (FixedWrapper::from(am.amount())
                                    * ratio_to_actual.clone())
                                .try_into_balance();
                                if am_adjusted.is_ok() {
                                    Ok((ls, am.copy_direction(am_adjusted.unwrap())))
                                } else {
                                    Err(Error::<T>::FailedToCalculatePriceWithoutImpact)
                                }
                            })
                            .collect();
                        let first_quote_without_impact = Self::calculate_amount_without_impact(
                            &from_asset_id,
                            &intermediate_asset_id,
                            &distribution?,
                            deduce_fee,
                        )?;
                        Some(first_quote_without_impact)
                    };
                    let cumulative_fee = first_quote
                        .fee
                        .checked_add(second_quote.fee)
                        .ok_or(Error::<T>::CalculationError)?;
                    let mut rewards = rewards_a;
                    rewards.append(&mut rewards_b);
                    second_liquidity_sources.extend(first_liquidity_sources);
                    Ok((
                        SwapOutcome::new(first_quote.amount, cumulative_fee),
                        rewards,
                        quote_without_impact,
                        second_liquidity_sources,
                    ))
                }
            },
        }
    }

    /// Computes the optimal distribution across available liquidity sources to execute the requested trade
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
        amount: QuoteAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
        skip_info: bool,
        deduce_fee: bool,
    ) -> Result<
        (
            AggregatedSwapOutcome<LiquiditySourceIdOf<T>, Balance>,
            Rewards<T::AssetId>,
            Vec<LiquiditySourceIdOf<T>>,
        ),
        DispatchError,
    > {
        let sources =
            T::LiquidityRegistry::list_liquidity_sources(input_asset_id, output_asset_id, filter)?;

        ensure!(!sources.is_empty(), Error::<T>::UnavailableExchangePath);

        // Check if we have exactly one source => no split required
        if sources.len() == 1 {
            let src = sources.first().unwrap();
            let outcome = T::LiquidityRegistry::quote(
                src,
                input_asset_id,
                output_asset_id,
                amount.into(),
                deduce_fee,
            )?;
            let rewards = if skip_info {
                Vec::new()
            } else {
                let (input_amount, output_amount) = amount.place_input_and_output(outcome.clone());
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
                    vec![(src.clone(), amount)],
                    outcome.amount,
                    outcome.fee,
                ),
                rewards,
                sources,
            ));
        }

        // Check if we have exactly two sources: the primary market and the secondary market
        // Do the "smart" swap split (with fallback)
        // NOTE: we assume here that XST tokens are not added to TBC reserves. If they are in the future, this
        // logic should be redone!
        if sources.len() == 2 {
            let mut primary_market: Option<LiquiditySourceIdOf<T>> = None;
            let mut secondary_market: Option<LiquiditySourceIdOf<T>> = None;

            for src in &sources {
                if src.liquidity_source_index
                    == LiquiditySourceType::MulticollateralBondingCurvePool
                    || src.liquidity_source_index == LiquiditySourceType::XSTPool
                {
                    primary_market = Some(src.clone());
                } else if src.liquidity_source_index == LiquiditySourceType::XYKPool
                    || src.liquidity_source_index == LiquiditySourceType::MockPool
                {
                    secondary_market = Some(src.clone());
                }
            }
            if let (Some(primary_mkt), Some(xyk)) = (primary_market, secondary_market) {
                let outcome = Self::smart_split(
                    &primary_mkt,
                    &xyk,
                    input_asset_id,
                    output_asset_id,
                    amount.clone(),
                    skip_info,
                    deduce_fee,
                )?;

                return Ok((outcome.0, outcome.1, sources));
            }
        }

        fail!(Error::<T>::UnavailableExchangePath);
    }

    fn calculate_amount_without_impact(
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        distribution: &Vec<(
            LiquiditySourceId<T::DEXId, LiquiditySourceType>,
            QuoteAmount<Balance>,
        )>,
        deduce_fee: bool,
    ) -> Result<Balance, DispatchError> {
        let mut outcome_without_impact: Balance = 0;
        for (src, part_amount) in distribution
            .iter()
            .filter(|(_src, part_amount)| part_amount.amount() > balance!(0))
        {
            let part_outcome = T::LiquidityRegistry::quote_without_impact(
                src,
                input_asset_id,
                output_asset_id,
                part_amount.clone(),
                deduce_fee,
            )?;
            outcome_without_impact = outcome_without_impact
                .checked_add(part_outcome.amount)
                .ok_or(Error::<T>::FailedToCalculatePriceWithoutImpact)?;
        }
        Ok(outcome_without_impact)
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

    /// Given two arbitrary tokens return sources that can be used to cover full path. If all sources can cover only part of path,
    /// but overall path is possible - list will be empty.
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
                ensure!(!sources.is_empty(), Error::<T>::UnavailableExchangePath);
                Ok(sources.into_iter().collect())
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
                ensure!(
                    !first_swap.is_empty() && !second_swap.is_empty(),
                    Error::<T>::UnavailableExchangePath
                );
                Ok(first_swap.intersection(&second_swap).cloned().collect())
            }
        }
    }

    pub fn list_enabled_sources_for_path_with_xyk_forbidden(
        dex_id: T::DEXId,
        input_asset_id: T::AssetId,
        output_asset_id: T::AssetId,
    ) -> Result<Vec<LiquiditySourceType>, DispatchError> {
        let tbc_reserve_assets = T::PrimaryMarketTBC::enabled_target_assets();
        let mut initial_result =
            Self::list_enabled_sources_for_path(dex_id, input_asset_id, output_asset_id)?;
        if tbc_reserve_assets.contains(&input_asset_id)
            || tbc_reserve_assets.contains(&output_asset_id)
        {
            initial_result.retain(|&lst| lst != LiquiditySourceType::XYKPool);
        }
        Ok(initial_result)
    }

    // Not full sort, just ensure that if there is base asset then it's sorted, otherwise order is unchanged.
    fn weak_sort_pair(asset_a: T::AssetId, asset_b: T::AssetId) -> TradingPair<T::AssetId> {
        if asset_b == T::GetBaseAssetId::get() {
            TradingPair {
                base_asset_id: asset_b,
                target_asset_id: asset_a,
            }
        } else {
            TradingPair {
                base_asset_id: asset_a,
                target_asset_id: asset_b,
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

    /// Implements the "smart" split algorithm.
    ///
    /// - `primary_source_id` - ID of the primary market liquidity source,
    /// - `secondary_source_id` - ID of the secondary market liquidity source,
    /// - `input_asset_id` - ID of the asset to sell,
    /// - `output_asset_id` - ID of the asset to buy,
    /// - `amount` - the amount with "direction" (sell or buy) together with the maximum price impact (slippage).
    /// - `skip_info` - flag that indicates that additional info should not be shown, that is needed when actual exchange is performed.
    ///
    fn smart_split(
        primary_source_id: &LiquiditySourceIdOf<T>,
        secondary_source_id: &LiquiditySourceIdOf<T>,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        skip_info: bool,
        deduce_fee: bool,
    ) -> Result<
        (
            AggregatedSwapOutcome<LiquiditySourceIdOf<T>, Balance>,
            Rewards<T::AssetId>,
        ),
        DispatchError,
    > {
        // The "smart" split algo is based on the following reasoning.
        // First, we try to calculate the spot price of the `input_asset_id` in both
        // the primary and secondary markets. If the price in the secondary market is
        // better than that in the primary market, we allocate as much of the `amount` to
        // be swapped in the secondary market as we can until the prices level up.
        // The rest will be swapped in the primary market.
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

        let amount_primary = if output_asset_id == base_asset {
            // XOR is being bought
            Self::decide_primary_market_amount_buying_base_asset(
                base_asset,
                other_asset,
                amount.clone(),
                (reserves_base, reserves_other),
            )
            .unwrap_or(
                // Error can only be due to MCBC or XST pool, hence zeroing it out
                amount.copy_direction(balance!(0)),
            )
        } else {
            // XOR is being sold
            Self::decide_primary_market_amount_selling_base_asset(
                base_asset,
                other_asset,
                amount.clone(),
                (reserves_base, reserves_other),
            )
            .unwrap_or(amount.copy_direction(balance!(0)))
        };

        let (is_better, extremum): (fn(a: Balance, b: Balance) -> bool, Balance) = match amount {
            QuoteAmount::WithDesiredInput { .. } => (|a, b| a > b, Balance::zero()),
            _ => (|a, b| a < b, Balance::MAX),
        };

        let mut best: Balance = extremum;
        let mut total_fee: Balance = 0;
        let mut rewards = Vec::new();
        let mut distr = Vec::new();
        let mut maybe_error: Option<DispatchError> = None;

        if amount_primary.amount() > Balance::zero() {
            // Attempting to quote according to the default sources weights
            let intermediary_result = T::LiquidityRegistry::quote(
                primary_source_id,
                input_asset_id,
                output_asset_id,
                amount_primary.clone(),
                deduce_fee,
            )
            .and_then(|outcome_primary| {
                if amount_primary.amount() < amount.amount() {
                    let amount_secondary = amount
                        .checked_sub(&amount_primary)
                        .ok_or(Error::<T>::CalculationError)?;
                    T::LiquidityRegistry::quote(
                        secondary_source_id,
                        input_asset_id,
                        output_asset_id,
                        amount_secondary.clone(),
                        deduce_fee,
                    )
                    .and_then(|outcome_secondary| {
                        if !skip_info {
                            for info in vec![
                                (primary_source_id, amount_primary, outcome_primary.clone()),
                                (
                                    secondary_source_id,
                                    amount_secondary,
                                    outcome_secondary.clone(),
                                ),
                            ] {
                                let (input_amount, output_amount) =
                                    info.1.place_input_and_output(info.2);
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
                        best = outcome_primary.amount + outcome_secondary.amount;
                        total_fee = outcome_primary.fee + outcome_secondary.fee;
                        distr = vec![
                            (primary_source_id.clone(), amount_primary),
                            (secondary_source_id.clone(), amount_secondary),
                        ];
                        Ok(())
                    })
                } else {
                    best = outcome_primary.amount;
                    total_fee = outcome_primary.fee;
                    distr = vec![(primary_source_id.clone(), amount_primary)];
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
            deduce_fee,
        )
        .and_then(|outcome| {
            if is_better(outcome.amount, best) {
                best = outcome.amount;
                total_fee = outcome.fee;
                distr = vec![(secondary_source_id.clone(), amount.clone())];
                if !skip_info {
                    let (input_amount, output_amount) =
                        amount.place_input_and_output(outcome.clone());
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

    /// Determines the share of a swap that should be exchanged in the primary market
    /// (i.e., the multi-collateral bonding curve pool) based on the current reserves of
    /// the base asset and the collateral asset in the secondary market (e.g., an XYK pool)
    /// provided the base asset is being bought.
    ///
    /// - `base_asset_id` - ID of the base asset,
    /// - `collateral_asset_id` - ID of the collateral asset,
    /// - `amount` - the swap amount with "direction" (fixed input vs fixed output),
    /// - `secondary_market_reserves` - a pair (base_reserve, collateral_reserve) in the secondary market
    ///
    fn decide_primary_market_amount_buying_base_asset(
        base_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        secondary_market_reserves: (Balance, Balance),
    ) -> Result<QuoteAmount<Balance>, DispatchError> {
        let (reserves_base, reserves_other) = secondary_market_reserves;
        let x: FixedWrapper = reserves_base.into();
        let y: FixedWrapper = reserves_other.into();
        let k: FixedWrapper = x.clone() * y.clone();
        let secondary_price: FixedWrapper = if x > fixed_wrapper!(0) {
            y.clone() / x.clone()
        } else {
            Fixed::MAX.into()
        };

        macro_rules! match_buy_price {
            ($source_type:ident) => {
                T::$source_type::buy_price(base_asset_id, collateral_asset_id)
                    .map_err(|_| Error::<T>::CalculationError)?
                    .into()
            };
        }
        let primary_buy_price: FixedWrapper = if collateral_asset_id == &XSTUSD.into() {
            match_buy_price!(PrimaryMarketXST)
        } else {
            match_buy_price!(PrimaryMarketTBC)
        };

        match amount {
            QuoteAmount::WithDesiredInput { desired_amount_in } => {
                let wrapped_amount: FixedWrapper = desired_amount_in.into();
                // checking that secondary price is better than primary initially
                let amount_primary = if secondary_price < primary_buy_price {
                    // find intercept between secondary and primary market curves:
                    // 1) (x - x1) * (y + y1) = k // xyk equation
                    // 2) (y + y1) / (x - x1) = p // desired price `p` equation
                    // composing 1 and 2: (y + y1) * (y + y1) = k * p
                    // √(k * p) - y = y1
                    // √(k) * √(p) - y = y1 // to prevent overflow
                    // where
                    // * x is base reserve, x1 is base amount, y is target reserve, y1 is target amount
                    // * p is desired price i.e. target/base
                    let k_sqrt = k.sqrt_accurate();
                    let primary_buy_price_sqrt = primary_buy_price.sqrt_accurate();
                    let amount_secondary = k_sqrt * primary_buy_price_sqrt - y; // always > 0
                    if amount_secondary >= wrapped_amount {
                        balance!(0)
                    } else if amount_secondary <= fixed_wrapper!(0) {
                        desired_amount_in
                    } else {
                        (wrapped_amount - amount_secondary)
                            .try_into_balance()
                            .unwrap()
                    }
                } else {
                    desired_amount_in
                };
                Ok(QuoteAmount::with_desired_input(amount_primary))
            }
            QuoteAmount::WithDesiredOutput { desired_amount_out } => {
                let wrapped_amount: FixedWrapper = desired_amount_out.into();
                // checking that secondary price is better than primary initially
                let amount_primary = if secondary_price < primary_buy_price {
                    // find intercept between secondary and primary market curves:
                    // 1) (x - x1) * (y + y1) = k // xyk equation
                    // 2) (y + y1) / (x - x1) = p // desired price `p` equation
                    // composing 1 and 2: (x - x1) * (x - x1) * p = k
                    // x - √(k / p) = x1
                    // where
                    // * x is base reserve, x1 is base amount, y is target reserve, y1 is target amount
                    // * p is desired price i.e. target/base
                    let amount_secondary = x - (k / primary_buy_price).sqrt_accurate(); // always > 0
                    if amount_secondary >= wrapped_amount {
                        balance!(0)
                    } else if amount_secondary <= fixed_wrapper!(0) {
                        desired_amount_out
                    } else {
                        (wrapped_amount - amount_secondary)
                            .try_into_balance()
                            .unwrap()
                    }
                } else {
                    desired_amount_out
                };
                Ok(QuoteAmount::with_desired_output(amount_primary))
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
    fn decide_primary_market_amount_selling_base_asset(
        base_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        secondary_market_reserves: (Balance, Balance),
    ) -> Result<QuoteAmount<Balance>, DispatchError> {
        let (reserves_base, reserves_other) = secondary_market_reserves;
        let x: FixedWrapper = reserves_base.into();
        let y: FixedWrapper = reserves_other.into();
        let k: FixedWrapper = x.clone() * y.clone();
        let secondary_price: FixedWrapper = if x > fixed_wrapper!(0) {
            y.clone() / x.clone()
        } else {
            Fixed::ZERO.into()
        };

        macro_rules! match_sell_price {
            ($source_type:ident) => {
                T::$source_type::sell_price(base_asset_id, collateral_asset_id)
                    .map_err(|_| Error::<T>::CalculationError)?
                    .into()
            };
        }
        let primary_sell_price: FixedWrapper = if collateral_asset_id == &XSTUSD.into() {
            match_sell_price!(PrimaryMarketXST)
        } else {
            match_sell_price!(PrimaryMarketTBC)
        };

        match amount {
            QuoteAmount::WithDesiredInput { desired_amount_in } => {
                let wrapped_amount: FixedWrapper = desired_amount_in.into();
                // checking that secondary price is better than primary initially
                let amount_primary = if secondary_price > primary_sell_price {
                    // find intercept between secondary and primary market curves:
                    // 1) (x + x1) * (y - y1) = k // xyk equation
                    // 2) (y - y1) / (x + x1) = p // desired price `p` equation
                    // composing 1 and 2: (x + x1) * (x + x1) * p = k
                    // √(k / p) - x = x1
                    // where
                    // * x is base reserve, x1 is base amount, y is target reserve, y1 is target amount
                    // * p is desired price i.e. target/base
                    let amount_secondary = (k / primary_sell_price).sqrt_accurate() - x; // always > 0
                    if amount_secondary >= wrapped_amount {
                        balance!(0)
                    } else if amount_secondary <= fixed_wrapper!(0) {
                        desired_amount_in
                    } else {
                        (wrapped_amount - amount_secondary)
                            .try_into_balance()
                            .unwrap()
                    }
                } else {
                    desired_amount_in
                };
                Ok(QuoteAmount::with_desired_input(amount_primary))
            }
            QuoteAmount::WithDesiredOutput { desired_amount_out } => {
                let wrapped_amount: FixedWrapper = desired_amount_out.into();
                // checking that secondary price is better than primary initially
                let amount_primary = if secondary_price > primary_sell_price {
                    // find intercept between secondary and primary market curves:
                    // 1) (x + x1) * (y - y1) = k // xyk equation
                    // 2) (y - y1) / (x + x1) = p // desired price `p` equation
                    // composing 1 and 2: (y - y1) * (y - y1) = k * p
                    // y - √(k * p) = y1
                    // where
                    // * x is base reserve, x1 is base amount, y is target reserve, y1 is target amount
                    // * p is desired price i.e. target/base
                    let amount_secondary = y - (k * primary_sell_price).sqrt_accurate();
                    if amount_secondary >= wrapped_amount {
                        balance!(0)
                    } else if amount_secondary <= fixed_wrapper!(0) {
                        desired_amount_out
                    } else {
                        (wrapped_amount - amount_secondary)
                            .try_into_balance()
                            .unwrap()
                    }
                } else {
                    desired_amount_out
                };
                Ok(QuoteAmount::with_desired_output(amount_primary))
            }
        }
    }
}

impl<T: Config> LiquidityProxyTrait<T::DEXId, T::AccountId, T::AssetId, LiquiditySourceIdOf<T>>
    for Pallet<T>
{
    /// Applies trivial routing (via Base Asset), resulting in a poly-swap which may contain several individual swaps.
    /// Those individual swaps are subject to liquidity aggregation algorithm.
    ///
    /// This is a wrapper for `quote_single`.
    fn quote(
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
        deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        Pallet::<T>::inner_quote(
            input_asset_id,
            output_asset_id,
            amount,
            filter,
            true,
            deduce_fee,
        )
        .map(|(outcome, _rewards, _amount_without_impact, _)| outcome)
    }

    /// Applies trivial routing (via Base Asset), resulting in a poly-swap which may contain several individual swaps.
    /// Those individual swaps are subject to liquidity aggregation algorithm.
    ///
    /// This is a wrapper for `exchange_single`.
    fn exchange(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<(SwapOutcome<Balance>, Vec<LiquiditySourceIdOf<T>>), DispatchError> {
        Pallet::<T>::inner_exchange(
            sender,
            receiver,
            input_asset_id,
            output_asset_id,
            amount,
            filter,
        )
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
        type PrimaryMarketTBC: GetMarketInfo<Self::AssetId>;
        type PrimaryMarketXST: GetMarketInfo<Self::AssetId>;
        type SecondaryMarket: GetPoolReserves<Self::AssetId>;
        type VestedRewardsPallet: VestedRewardsPallet<Self::AccountId, Self::AssetId>;
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
            Self::inner_swap(
                who.clone(),
                who,
                dex_id,
                input_asset_id,
                output_asset_id,
                swap_amount,
                selected_source_types,
                filter_mode,
            )?;
            Ok(().into())
        }

        /// Perform swap of tokens (input/output defined via SwapAmount direction).
        ///
        /// - `origin`: the account on whose behalf the transaction is being executed,
        /// - `receiver`: the account that receives the output,
        /// - `dex_id`: DEX ID for which liquidity sources aggregation is being done,
        /// - `input_asset_id`: ID of the asset being sold,
        /// - `output_asset_id`: ID of the asset being bought,
        /// - `swap_amount`: the exact amount to be sold (either in input_asset_id or output_asset_id units with corresponding slippage tolerance absolute bound),
        /// - `selected_source_types`: list of selected LiquiditySource types, selection effect is determined by filter_mode,
        /// - `filter_mode`: indicate either to allow or forbid selected types only, or disable filtering.
        #[pallet::weight(<T as Config>::WeightInfo::swap((*swap_amount).into()))]
        pub fn swap_transfer(
            origin: OriginFor<T>,
            receiver: T::AccountId,
            dex_id: T::DEXId,
            input_asset_id: T::AssetId,
            output_asset_id: T::AssetId,
            swap_amount: SwapAmount<Balance>,
            selected_source_types: Vec<LiquiditySourceType>,
            filter_mode: FilterMode,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::inner_swap(
                who,
                receiver,
                dex_id,
                input_asset_id,
                output_asset_id,
                swap_amount,
                selected_source_types,
                filter_mode,
            )?;
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
            Vec<LiquiditySourceIdOf<T>>,
            // LiquiditySourcesList<T>,
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
        /// Failure while calculating price ignoring non-linearity of liquidity source.
        FailedToCalculatePriceWithoutImpact,
    }
}
