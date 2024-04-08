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

extern crate core;

pub mod liquidity_aggregator;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod new_tests;
#[cfg(test)]
mod test_utils;
#[cfg(test)]
mod tests;
pub mod weights;

use assets::AssetIdOf;
use assets::WeightInfo as _;
use codec::{Decode, Encode};
use common::prelude::{
    Balance, FixedWrapper, OutcomeFee, QuoteAmount, SwapAmount, SwapOutcome, SwapVariant,
};
use common::{
    balance, fixed_wrapper, AccountIdOf, AssetInfoProvider, BuyBackHandler, DEXInfo, DexIdOf,
    DexInfoProvider, FilterMode, Fixed, GetMarketInfo, GetPoolReserves, LiquidityProxyTrait,
    LiquidityRegistry, LiquiditySource, LiquiditySourceFilter, LiquiditySourceId,
    LiquiditySourceType, LockedLiquiditySourcesManager, RewardReason, TradingPair,
    TradingPairSourceManager, Vesting,
};
use core::marker::PhantomData;
use fallible_iterator::FallibleIterator as _;
use frame_support::dispatch::PostDispatchInfo;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use frame_support::{ensure, fail, RuntimeDebug};
use frame_system::ensure_signed;
use itertools::Itertools as _;
use liquidity_aggregator::AggregatedSwapOutcome;
pub use pallet::*;
use sp_runtime::traits::Zero;
use sp_runtime::DispatchError;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::prelude::*;
use sp_std::{cmp::Ord, cmp::Ordering, vec};
pub use weights::WeightInfo;

#[cfg(not(feature = "wip"))] // ALT
use {
    common::prelude::fixnum::ops::{Bounded, Zero as _},
    common::XSTUSD,
    sp_runtime::traits::CheckedSub,
};

#[cfg(feature = "wip")] // ALT
use liquidity_aggregator::LiquidityAggregator;

type LiquiditySourceIdOf<T> = LiquiditySourceId<<T as common::Config>::DEXId, LiquiditySourceType>;
type Rewards<AssetId> = Vec<(Balance, AssetId, RewardReason)>;

/// Exchange route as:
/// - from AssetId
/// - to AssetId
/// - swap amounts
/// Can be either WithDesiredInput or WithDesiredOutput.
type ExchangeRoute<AssetId> = Vec<(AssetId, AssetId, SwapAmount<Balance>)>;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"liquidity-proxy";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";

const REJECTION_WEIGHT: Weight = Weight::from_parts(u64::MAX, u64::MAX);

/// Possible exchange paths for two assets.
pub struct ExchangePath<T: Config>(Vec<T::AssetId>);

#[derive(Debug, Eq, PartialEq)]
enum AssetType {
    Base,
    SyntheticBase,
    Basic,
    Synthetic,
}

impl AssetType {
    fn determine<T: Config>(
        dex_info: &DEXInfo<T::AssetId>,
        synthetic_assets: &BTreeSet<T::AssetId>,
        asset_id: T::AssetId,
    ) -> Self {
        if asset_id == dex_info.base_asset_id {
            AssetType::Base
        } else if asset_id == dex_info.synthetic_base_asset_id {
            AssetType::SyntheticBase
        } else if synthetic_assets.contains(&asset_id) {
            AssetType::Synthetic
        } else {
            AssetType::Basic
        }
    }
}

macro_rules! forward_or_backward {
    ($ex1:tt, $ex2:tt) => {
        ($ex1, $ex2) | ($ex2, $ex1)
    };
}

impl<T: Config> ExchangePath<T> {
    pub fn new_trivial(
        dex_info: &DEXInfo<T::AssetId>,
        input_asset_id: T::AssetId,
        output_asset_id: T::AssetId,
    ) -> Option<Vec<Self>> {
        use AssetType::*;

        let synthetic_assets = T::PrimaryMarketXST::enabled_target_assets();
        let input_type = AssetType::determine::<T>(dex_info, &synthetic_assets, input_asset_id);
        let output_type = AssetType::determine::<T>(dex_info, &synthetic_assets, output_asset_id);

        match (input_type, output_type) {
            forward_or_backward!(Base, Basic) | forward_or_backward!(Base, SyntheticBase) => {
                Some(vec![Self(vec![input_asset_id, output_asset_id])])
            }
            forward_or_backward!(SyntheticBase, Synthetic) => Some(vec![
                Self(vec![input_asset_id, output_asset_id]),
                Self(vec![
                    input_asset_id,
                    dex_info.base_asset_id,
                    output_asset_id,
                ]),
            ]),
            (Basic, Basic) | forward_or_backward!(SyntheticBase, Basic) => Some(vec![Self(vec![
                input_asset_id,
                dex_info.base_asset_id,
                output_asset_id,
            ])]),
            (Synthetic, Synthetic) => Some(vec![
                Self(vec![
                    input_asset_id,
                    dex_info.synthetic_base_asset_id,
                    output_asset_id,
                ]),
                Self(vec![
                    input_asset_id,
                    dex_info.base_asset_id,
                    output_asset_id,
                ]),
            ]),
            forward_or_backward!(Base, Synthetic) => Some(vec![
                Self(vec![input_asset_id, output_asset_id]),
                Self(vec![
                    input_asset_id,
                    dex_info.synthetic_base_asset_id,
                    output_asset_id,
                ]),
            ]),
            (Basic, Synthetic) => Some(vec![
                Self(vec![
                    input_asset_id,
                    dex_info.base_asset_id,
                    dex_info.synthetic_base_asset_id,
                    output_asset_id,
                ]),
                Self(vec![
                    input_asset_id,
                    dex_info.base_asset_id,
                    output_asset_id,
                ]),
            ]),
            (Synthetic, Basic) => Some(vec![
                Self(vec![
                    input_asset_id,
                    dex_info.synthetic_base_asset_id,
                    dex_info.base_asset_id,
                    output_asset_id,
                ]),
                Self(vec![
                    input_asset_id,
                    dex_info.base_asset_id,
                    output_asset_id,
                ]),
            ]),
            (Base, Base) | (SyntheticBase, SyntheticBase) => None,
        }
    }
}

#[derive(Eq, PartialEq, Encode, Decode)]
pub struct QuoteInfo<AssetId: Ord, LiquiditySource> {
    pub outcome: SwapOutcome<Balance, AssetId>,
    pub amount_without_impact: Option<Balance>,
    pub rewards: Rewards<AssetId>,
    pub liquidity_sources: Vec<LiquiditySource>,
    pub path: Vec<AssetId>,
}

fn merge_two_vectors_unique<T: PartialEq>(vec_1: &mut Vec<T>, vec_2: Vec<T>) {
    for el in vec_2 {
        if !vec_1.contains(&el) {
            vec_1.push(el);
        }
    }
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
            && !selected_source_types.contains(&LiquiditySourceType::OrderBook)
            && filter_mode == &FilterMode::AllowSelected
            || selected_source_types
                .contains(&LiquiditySourceType::MulticollateralBondingCurvePool)
                && selected_source_types.contains(&LiquiditySourceType::XSTPool)
                && !selected_source_types.contains(&LiquiditySourceType::XYKPool)
                && selected_source_types.contains(&LiquiditySourceType::OrderBook)
                && filter_mode == &FilterMode::ForbidSelected;
        // check if either of tbc reserve assets is present
        let reserve_asset_present = tbc_reserve_assets.contains(input_asset_id)
            || tbc_reserve_assets.contains(output_asset_id);

        is_xyk_only && reserve_asset_present
    }

    // TODO: #395 use AssetInfoProvider instead of assets pallet
    pub fn check_indivisible_assets(
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
    ) -> Result<(), DispatchError> {
        ensure!(
            !assets::Pallet::<T>::is_non_divisible(input_asset_id)
                && !assets::Pallet::<T>::is_non_divisible(output_asset_id),
            Error::<T>::UnableToSwapIndivisibleAssets
        );
        Ok(())
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
    ) -> Result<Weight, DispatchError> {
        Self::check_indivisible_assets(&input_asset_id, &output_asset_id)?;
        let mut total_weight = <T as Config>::WeightInfo::check_indivisible_assets();

        if Self::is_forbidden_filter(
            &input_asset_id,
            &output_asset_id,
            &selected_source_types,
            &filter_mode,
        ) {
            fail!(Error::<T>::ForbiddenFilter);
        }
        total_weight =
            total_weight.saturating_add(<T as Config>::WeightInfo::is_forbidden_filter());

        let (outcome, sources, weight) = Self::inner_exchange(
            dex_id,
            &sender,
            &receiver,
            &input_asset_id,
            &output_asset_id,
            swap_amount,
            LiquiditySourceFilter::with_mode(dex_id, filter_mode, selected_source_types),
        )?;
        total_weight = total_weight.saturating_add(weight);

        let (input_amount, output_amount, fee) = match swap_amount {
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
            fee,
            sources,
        ));

        Ok(total_weight)
    }

    /// Applies trivial routing (via Base Asset), resulting in a poly-swap which may contain several individual swaps.
    /// Those individual swaps are subject to liquidity aggregation algorithm.
    ///
    /// This a wrapper for `exchange_single`.
    pub fn inner_exchange(
        dex_id: T::DEXId,
        sender: &T::AccountId,
        receiver: &T::AccountId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<
        (
            SwapOutcome<Balance, T::AssetId>,
            Vec<LiquiditySourceIdOf<T>>,
            Weight,
        ),
        DispatchError,
    > {
        ensure!(
            input_asset_id != output_asset_id,
            Error::<T>::UnavailableExchangePath
        );

        common::with_transaction(|| {
            let dex_info = T::DexInfoProvider::get_dex_info(&dex_id)?;
            let maybe_paths =
                ExchangePath::<T>::new_trivial(&dex_info, *input_asset_id, *output_asset_id);
            let total_weight = <T as Config>::WeightInfo::new_trivial();
            maybe_paths
                .map_or(Err(Error::<T>::UnavailableExchangePath.into()), |paths| {
                    Self::exchange_sequence(&dex_info, sender, receiver, paths, amount, &filter)
                })
                .map(|(outcome, sources, weight)| {
                    (outcome, sources, total_weight.saturating_add(weight))
                })
        })
    }

    /// Exchange sequence of assets, where each pair is a direct exchange.
    /// The swaps path is selected via `select_best_path`
    fn exchange_sequence(
        dex_info: &DEXInfo<T::AssetId>,
        sender: &T::AccountId,
        receiver: &T::AccountId,
        asset_paths: Vec<ExchangePath<T>>,
        amount: SwapAmount<Balance>,
        filter: &LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<
        (
            SwapOutcome<Balance, T::AssetId>,
            Vec<LiquiditySourceIdOf<T>>,
            Weight,
        ),
        DispatchError,
    > {
        let (_, route, quote_weight) = Self::select_best_path(
            dex_info,
            asset_paths,
            amount.variant(),
            amount.amount(),
            filter,
            true,
            true,
        )
        .map(|(info, swaps, weight)| (info.path, swaps, weight))?;

        Self::exchange_sequence_with_desired_amount(dex_info, sender, receiver, &route, filter)
            .and_then(|(mut swap, sources, weight)| {
                match amount {
                    SwapAmount::WithDesiredInput { min_amount_out, .. } => {
                        ensure!(
                            swap.amount >= min_amount_out,
                            Error::<T>::SlippageNotTolerated
                        );
                        Ok((swap, sources, quote_weight.saturating_add(weight)))
                    }
                    SwapAmount::WithDesiredOutput { max_amount_in, .. } => {
                        // The input limit on the first exchange is an input amount for the whole exchange
                        let input_amount = route
                            .first()
                            .ok_or(Error::<T>::UnavailableExchangePath)?
                            .2
                            .limit();
                        swap.amount = input_amount;
                        ensure!(
                            swap.amount <= max_amount_in,
                            Error::<T>::SlippageNotTolerated
                        );
                        Ok((swap, sources, quote_weight.saturating_add(weight)))
                    }
                }
            })
    }

    /// Performs the sequence of assets exchanges.
    ///
    /// Performs [`Self::exchange_single()`] for each pair of assets and aggregates the results.
    ///
    /// # Parameters
    /// - `dex_info` - information about DEX
    /// - `sender` - address that sends amount
    /// - `receiver` - swap beneficiary
    /// - `swaps` - exchange route with amounts
    /// - `filter` - filter for liquidity sources
    fn exchange_sequence_with_desired_amount(
        dex_info: &DEXInfo<T::AssetId>,
        sender: &T::AccountId,
        receiver: &T::AccountId,
        route: &ExchangeRoute<T::AssetId>,
        filter: &LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<
        (
            SwapOutcome<Balance, T::AssetId>,
            Vec<LiquiditySourceIdOf<T>>,
            Weight,
        ),
        DispatchError,
    > {
        use itertools::EitherOrBoth::*;

        let transit_account = T::GetTechnicalAccountId::get();
        let exchange_count = route.len();

        let sender_iter = sp_std::iter::once(sender)
            .chain(sp_std::iter::repeat(&transit_account).take(exchange_count - 1));
        let receiver_iter = sp_std::iter::repeat(&transit_account)
            .take(exchange_count - 1)
            .chain(sp_std::iter::once(receiver));

        fallible_iterator::convert(
            route
                .iter()
                .zip_longest(sender_iter)
                .zip_longest(receiver_iter)
                .map(|zip| match zip {
                    Both(Both((from, to, swap_amount), cur_sender), cur_receiver) => {
                        (from, to, swap_amount, cur_sender, cur_receiver)
                    }
                    // Sanity check. Should never happen
                    _ => panic!(
                        "Exchanging failed, iterator invariants are broken - \
                                 this is a programmer error"
                    ),
                })
                // Exchange
                .map(|(from, to, swap_amount, cur_sender, cur_receiver)| -> Result<_, DispatchError> {
                    let (swap_outcome, sources, weight) = Self::exchange_single(
                        cur_sender,
                        cur_receiver,
                        &dex_info.base_asset_id,
                        &from,
                        &to,
                        swap_amount.clone(),
                        filter.clone(),
                    )?;
                    Ok((swap_outcome, sources, weight))
                }),
        )
            // Exchange aggregation
            .fold(
                (
                    SwapOutcome::new(balance!(0), OutcomeFee::new()),
                    Vec::new(),
                    Weight::zero(),
                ),
                |(mut outcome, mut sources, mut total_weight),
                 (swap_outcome, swap_sources, swap_weight)| {
                    outcome.amount = swap_outcome.amount;
                    outcome.fee = outcome.fee.merge(swap_outcome.fee);
                    merge_two_vectors_unique(&mut sources, swap_sources);
                    total_weight = total_weight.saturating_add(swap_weight);
                    Ok((outcome, sources, total_weight))
                },
            )
    }

    /// Performs a swap given a number of liquidity sources and a distribution of the swap amount across the sources.
    fn exchange_single(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        base_asset_id: &T::AssetId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<
        (
            SwapOutcome<Balance, T::AssetId>,
            Vec<LiquiditySourceIdOf<T>>,
            Weight,
        ),
        DispatchError,
    > {
        common::with_transaction(|| {
            let mut total_weight = Weight::zero();
            let (outcome, _, sources, weight) = Self::quote_single(
                base_asset_id,
                input_asset_id,
                output_asset_id,
                amount.into(),
                filter,
                true,
                true,
            )?;
            total_weight = total_weight.saturating_add(weight);

            let res = outcome
                .distribution
                .into_iter()
                .filter(|(_src, part_amount)| part_amount.amount() > balance!(0))
                .map(|(src, part_amount)| {
                    T::LiquidityRegistry::exchange(
                        sender,
                        receiver,
                        &src,
                        input_asset_id,
                        output_asset_id,
                        part_amount,
                    )
                    .map(|(outcome, weight)| {
                        total_weight = total_weight.saturating_add(weight);
                        outcome
                    })
                })
                .collect::<Result<Vec<SwapOutcome<Balance, T::AssetId>>, DispatchError>>()?;

            let (amount, fee) = res.into_iter().fold(
                (fixed_wrapper!(0), OutcomeFee::new()),
                |(amount_acc, fee_acc), x| {
                    (
                        amount_acc + FixedWrapper::from(x.amount),
                        fee_acc.merge(x.fee),
                    )
                },
            );
            let amount = amount
                .try_into_balance()
                .map_err(|_| Error::CalculationError::<T>)?;
            Ok((SwapOutcome::new(amount, fee), sources, total_weight))
        })
    }

    /// Applies trivial routing (via Base Asset), resulting in a poly-swap which may contain several individual swaps.
    /// Those individual swaps are subject to liquidity aggregation algorithm.
    ///
    /// This a wrapper for `quote_single`.
    pub fn inner_quote(
        dex_id: T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
        skip_info: bool,
        deduce_fee: bool,
    ) -> Result<
        (
            QuoteInfo<T::AssetId, LiquiditySourceIdOf<T>>,
            ExchangeRoute<T::AssetId>,
            Weight,
        ),
        DispatchError,
    > {
        ensure!(
            input_asset_id != output_asset_id,
            Error::<T>::UnavailableExchangePath
        );
        let dex_info = T::DexInfoProvider::get_dex_info(&dex_id)?;
        let maybe_path =
            ExchangePath::<T>::new_trivial(&dex_info, *input_asset_id, *output_asset_id);
        maybe_path.map_or_else(
            || Err(Error::<T>::UnavailableExchangePath.into()),
            |paths| {
                Self::select_best_path(
                    &dex_info,
                    paths,
                    amount.variant(),
                    amount.amount(),
                    &filter,
                    skip_info,
                    deduce_fee,
                )
            },
        )
    }

    /// Selects the best path between two swap paths
    ///
    /// Returns Result containing a quote result and the selected path
    fn select_best_path(
        dex_info: &DEXInfo<T::AssetId>,
        asset_paths: Vec<ExchangePath<T>>,
        swap_variant: SwapVariant,
        amount: Balance,
        filter: &LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
        skip_info: bool,
        deduce_fee: bool,
    ) -> Result<
        (
            QuoteInfo<T::AssetId, LiquiditySourceIdOf<T>>,
            ExchangeRoute<T::AssetId>,
            Weight,
        ),
        DispatchError,
    > {
        let mut weight = Weight::zero();
        let mut swaps = ExchangeRoute::<T::AssetId>::new();
        let mut path_quote_iter = asset_paths.into_iter().map(|ExchangePath(atomic_path)| {
            let quote = match swap_variant {
                SwapVariant::WithDesiredInput => Self::quote_pairs_with_flexible_amount(
                    dex_info,
                    atomic_path.iter().tuple_windows(),
                    QuoteAmount::with_desired_input,
                    amount,
                    filter,
                    skip_info,
                    deduce_fee,
                    swap_variant,
                ),
                SwapVariant::WithDesiredOutput => Self::quote_pairs_with_flexible_amount(
                    dex_info,
                    atomic_path
                        .iter()
                        .rev()
                        .tuple_windows()
                        .map(|(to, from)| (from, to)),
                    QuoteAmount::with_desired_output,
                    amount,
                    filter,
                    skip_info,
                    deduce_fee,
                    swap_variant,
                ),
            };
            quote.map(|x| {
                swaps = x.4;
                weight = weight.saturating_add(x.5);
                QuoteInfo {
                    outcome: x.0,
                    amount_without_impact: x.1,
                    rewards: x.2,
                    liquidity_sources: x.3,
                    path: atomic_path,
                }
            })
        });

        let primary_path = path_quote_iter
            .next()
            .ok_or(Error::<T>::UnavailableExchangePath)?;

        path_quote_iter
            .fold(primary_path, |acc, path| match (&acc, &path) {
                (Ok(_), Err(_)) => acc,
                (Err(_), Ok(_)) => path,
                (Ok(acc_quote_info), Ok(quote_info)) => {
                    match (
                        swap_variant,
                        acc_quote_info.outcome.cmp(&quote_info.outcome),
                    ) {
                        (SwapVariant::WithDesiredInput, Ordering::Less) => path,
                        (SwapVariant::WithDesiredInput, _) => acc,
                        (_, Ordering::Less) => acc,
                        _ => path,
                    }
                }
                _ => acc,
            })
            .map(|quote| (quote, swaps, weight))
    }

    /// Quote given pairs of assets using `amount_ctr` to construct [`QuoteAmount`] for each pair.
    ///
    /// Performs [`Self::quote_single()`] for each pair and aggregates the results.
    fn quote_pairs_with_flexible_amount<'asset, F: Fn(Balance) -> QuoteAmount<Balance>>(
        dex_info: &DEXInfo<T::AssetId>,
        asset_pairs: impl Iterator<Item = (&'asset T::AssetId, &'asset T::AssetId)>,
        amount_ctr: F,
        amount: Balance,
        filter: &LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
        skip_info: bool,
        deduce_fee: bool,
        swap_variant: SwapVariant,
    ) -> Result<
        (
            SwapOutcome<Balance, T::AssetId>,
            Option<Balance>,
            Rewards<T::AssetId>,
            Vec<LiquiditySourceIdOf<T>>,
            ExchangeRoute<T::AssetId>,
            Weight,
        ),
        DispatchError,
    > {
        let mut current_amount = amount;
        let init_outcome_without_impact = (!skip_info).then(|| balance!(0));
        fallible_iterator::convert(asset_pairs.map(|(from_asset_id, to_asset_id)| {
            let (quote, rewards, liquidity_sources, weight) = Self::quote_single(
                &dex_info.base_asset_id,
                from_asset_id,
                to_asset_id,
                amount_ctr(current_amount),
                filter.clone(),
                skip_info,
                deduce_fee,
            )?;
            let amount_sent = current_amount;
            current_amount = quote.amount;
            Ok((
                amount_sent,
                quote,
                rewards,
                liquidity_sources,
                (from_asset_id, to_asset_id),
                weight,
            ))
        }))
        .fold(
            (
                SwapOutcome::new(balance!(0), OutcomeFee::new()),
                init_outcome_without_impact,
                Rewards::new(),
                Vec::new(),
                Vec::new(),
                Weight::zero(),
            ),
            |(
                mut outcome,
                mut outcome_without_impact,
                mut rewards,
                mut liquidity_sources,
                mut vec_swaps,
                mut weight,
            ),
             (
                amount_sent,
                quote,
                mut quote_rewards,
                quote_liquidity_sources,
                (from_asset, to_asset),
                quote_weight,
            )| {
                outcome_without_impact = outcome_without_impact
                    .map(|without_impact| {
                        Self::calculate_amount_without_impact(
                            from_asset,
                            to_asset,
                            &quote.distribution,
                            outcome.amount,
                            without_impact,
                            deduce_fee,
                        )
                    })
                    .transpose()?;
                outcome.amount = quote.amount;
                outcome.fee = outcome.fee.merge(quote.fee);
                rewards.append(&mut quote_rewards);
                weight = weight.saturating_add(quote_weight);
                match swap_variant {
                    SwapVariant::WithDesiredInput => vec_swaps.push((
                        *from_asset,
                        *to_asset,
                        SwapAmount::with_desired_input(amount_sent, quote.amount),
                    )),
                    SwapVariant::WithDesiredOutput => vec_swaps.insert(
                        0,
                        (
                            *from_asset,
                            *to_asset,
                            SwapAmount::with_desired_output(amount_sent, quote.amount),
                        ),
                    ),
                }
                merge_two_vectors_unique(&mut liquidity_sources, quote_liquidity_sources);
                Ok((
                    outcome,
                    outcome_without_impact,
                    rewards,
                    liquidity_sources,
                    vec_swaps,
                    weight,
                ))
            },
        )
    }

    // Would likely to fail if operating near the limits,
    // because it uses i128 for fixed-point arithmetics.
    // TODO: switch to unsigned internal representation
    fn calculate_amount_without_impact(
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        distribution: &Vec<(
            LiquiditySourceId<T::DEXId, LiquiditySourceType>,
            SwapAmount<Balance>,
        )>,
        outcome_amount: u128,
        outcome_without_impact: u128,
        deduce_fee: bool,
    ) -> Result<Balance, DispatchError> {
        use common::fixnum;
        use fixnum::ops::{One, RoundMode, RoundingDiv, RoundingMul};

        let ratio_to_actual = if outcome_amount != 0 {
            // TODO: switch to unsigned internal representation (`FixedPoint<u128, U18>`)
            // for now lib `fixnum` doesn't implement operations for such types, so
            // we just use `i128` repr
            let outcome_without_impact = Fixed::from_bits(
                outcome_without_impact
                    .try_into()
                    .map_err(|_| Error::<T>::FailedToCalculatePriceWithoutImpact)?,
            );
            let outcome_amount = Fixed::from_bits(
                outcome_amount
                    .try_into()
                    .map_err(|_| Error::<T>::FailedToCalculatePriceWithoutImpact)?,
            );
            // Same RoundMode as was used in frontend
            outcome_without_impact
                .rdiv(outcome_amount, RoundMode::Floor)
                .unwrap_or(Fixed::ONE)
        } else {
            <Fixed as One>::ONE
        };

        // multiply all amounts in distribution to adjust prev quote without impact:
        let distribution = distribution
            .into_iter()
            .filter(|(_, part_amount)| part_amount.amount() > balance!(0))
            .map(|(market, amount)| {
                // Should not overflow unless the amounts are comparable to 10^38 .
                // For reference, a trillion is 10^12.
                //
                // same as mul by ratioToActual, just without floating point ops
                let adjusted_amount: u128 = Fixed::from_bits(
                    amount
                        .amount()
                        .try_into()
                        .map_err(|_| Error::<T>::FailedToCalculatePriceWithoutImpact)?,
                )
                .rmul(ratio_to_actual, RoundMode::Floor)
                .map_err(|_| Error::<T>::FailedToCalculatePriceWithoutImpact)?
                .into_bits()
                .try_into()
                .map_err(|_| Error::<T>::FailedToCalculatePriceWithoutImpact)?;
                Ok::<_, Error<T>>((
                    market,
                    QuoteAmount::with_variant(amount.variant(), adjusted_amount),
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut accumulated_without_impact: Balance = 0;
        for (src, part_amount) in distribution.into_iter() {
            let part_outcome = T::LiquidityRegistry::quote_without_impact(
                src,
                input_asset_id,
                output_asset_id,
                part_amount,
                deduce_fee,
            )?;
            accumulated_without_impact = accumulated_without_impact
                .checked_add(part_outcome.amount)
                .ok_or(Error::<T>::FailedToCalculatePriceWithoutImpact)?;
        }
        Ok(accumulated_without_impact)
    }

    /// Obtains only sources available for `quote`
    fn list_quote_liquidity_sources(
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        filter: &LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<Vec<LiquiditySourceIdOf<T>>, DispatchError> {
        let mut sources =
            T::LiquidityRegistry::list_liquidity_sources(input_asset_id, output_asset_id, filter)?;
        let locked = T::LockedLiquiditySourcesManager::get();
        sources.retain(|x| !locked.contains(&x.liquidity_source_index));

        // the old mechanism cannot combine Order Book source with others
        #[cfg(not(feature = "wip"))] // ALT
        if sources.len() > 1 {
            sources.retain(|x| x.liquidity_source_index != LiquiditySourceType::OrderBook);
        }

        Ok(sources)
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
        base_asset_id: &T::AssetId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
        skip_info: bool,
        deduce_fee: bool,
    ) -> Result<
        (
            AggregatedSwapOutcome<T::AssetId, LiquiditySourceIdOf<T>, Balance>,
            Rewards<T::AssetId>,
            Vec<LiquiditySourceIdOf<T>>,
            Weight,
        ),
        DispatchError,
    > {
        let sources = Self::list_quote_liquidity_sources(input_asset_id, output_asset_id, &filter)?;
        let mut total_weight = <T as Config>::WeightInfo::list_liquidity_sources();
        ensure!(!sources.is_empty(), Error::<T>::UnavailableExchangePath);

        // Check if we have exactly one source => no split required
        if sources.len() == 1 {
            let src = sources.first().unwrap();
            let (outcome, weight) = T::LiquidityRegistry::quote(
                src,
                input_asset_id,
                output_asset_id,
                amount.into(),
                deduce_fee,
            )?;
            total_weight = total_weight.saturating_add(weight);
            let rewards = if skip_info {
                Vec::new()
            } else {
                let (input_amount, output_amount) = amount.place_input_and_output(outcome.clone());
                let (rewards, weight) = T::LiquidityRegistry::check_rewards(
                    src,
                    input_asset_id,
                    output_asset_id,
                    input_amount,
                    output_amount,
                )
                .unwrap_or((Vec::new(), Weight::zero()));
                total_weight = total_weight.saturating_add(weight);
                rewards
            };
            return Ok((
                AggregatedSwapOutcome::new(
                    vec![(
                        src.clone(),
                        SwapAmount::with_variant(amount.variant(), amount.amount(), outcome.amount),
                    )],
                    outcome.amount,
                    outcome.fee,
                ),
                rewards,
                sources,
                total_weight,
            ));
        }

        #[cfg(not(feature = "wip"))] // ALT
        {
            // Check if we have exactly two sources: the primary market and the secondary market
            // Do the "smart" swap split (with fallback)
            // NOTE: we assume here that XST tokens are not added to TBC reserves. If they are in the future, this
            // logic should be redone!
            if sources.len() == 2 {
                let mut primary_market: Option<LiquiditySourceIdOf<T>> = None;
                let mut secondary_market: Option<LiquiditySourceIdOf<T>> = None;

                for src in &sources {
                    match src.liquidity_source_index {
                        // We can't use XST as primary market for smart split, because it use XST asset as base
                        // and does not support DEXes except Polkaswap
                        LiquiditySourceType::MulticollateralBondingCurvePool => {
                            primary_market = Some(src.clone())
                        }
                        LiquiditySourceType::XYKPool | LiquiditySourceType::MockPool => {
                            secondary_market = Some(src.clone())
                        }
                        _ => (),
                    }
                }

                if let (Some(primary_mkt), Some(xyk)) = (primary_market, secondary_market) {
                    let outcome = Self::smart_split(
                        &primary_mkt,
                        &xyk,
                        base_asset_id,
                        input_asset_id,
                        output_asset_id,
                        amount.clone(),
                        skip_info,
                        deduce_fee,
                    )?;
                    total_weight = total_weight.saturating_add(outcome.2);
                    return Ok((outcome.0, outcome.1, sources, total_weight));
                }
            }
        }

        #[cfg(feature = "wip")] // ALT
        {
            let (outcome, rewards, weight) = Self::new_smart_split(
                &sources,
                base_asset_id,
                input_asset_id,
                output_asset_id,
                amount.clone(),
                skip_info,
                deduce_fee,
            )?;

            total_weight = total_weight.saturating_add(weight);
            Ok((outcome, rewards, sources, total_weight))
        }

        #[cfg(not(feature = "wip"))] // ALT
        fail!(Error::<T>::UnavailableExchangePath);
    }

    /// Check if given two arbitrary tokens can be used to perform an exchange via any available sources.
    pub fn is_path_available(
        dex_id: T::DEXId,
        input_asset_id: T::AssetId,
        output_asset_id: T::AssetId,
    ) -> Result<bool, DispatchError> {
        let dex_info = T::DexInfoProvider::get_dex_info(&dex_id)?;
        let maybe_path = ExchangePath::<T>::new_trivial(&dex_info, input_asset_id, output_asset_id);
        maybe_path.map_or(Ok(false), |paths| {
            let paths_flag = paths
                .into_iter()
                .map(|ExchangePath(atomic_path)| {
                    Self::check_asset_path(&dex_id, &dex_info, &atomic_path)
                })
                .any(|x| x);
            Ok(paths_flag)
        })
    }

    /// Checks if the path, consisting of sequential swaps of assets in `path`, is
    /// available and if it is, then returns Ok(true)
    pub fn check_asset_path(
        dex_id: &T::DEXId,
        dex_info: &DEXInfo<T::AssetId>,
        path: &[T::AssetId],
    ) -> bool {
        path.iter()
            .tuple_windows()
            .filter_map(|(from, to)| {
                let pair = Self::weak_sort_pair(&dex_info, *from, *to);
                T::TradingPairSourceManager::list_enabled_sources_for_trading_pair(
                    dex_id,
                    &pair.base_asset_id,
                    &pair.target_asset_id,
                )
                .ok()
            })
            .all(|sources| !sources.is_empty())
    }

    /// Returns a BTreeSet with all LiquiditySourceTypes, which will be used for swap
    pub fn get_asset_path_sources(
        dex_id: &T::DEXId,
        dex_info: &DEXInfo<T::AssetId>,
        path: &[T::AssetId],
    ) -> Result<BTreeSet<LiquiditySourceType>, DispatchError> {
        let sources_set = fallible_iterator::convert(path.to_vec().iter().tuple_windows().map(
            |(from, to)| -> Result<_, DispatchError> {
                let pair = Self::weak_sort_pair(&dex_info, *from, *to);
                let sources = T::TradingPairSourceManager::list_enabled_sources_for_trading_pair(
                    &dex_id,
                    &pair.base_asset_id,
                    &pair.target_asset_id,
                )?;
                ensure!(!sources.is_empty(), Error::<T>::UnavailableExchangePath);
                Ok(sources)
            },
        ))
        .fold(None, |acc: Option<BTreeSet<_>>, sources| match acc {
            Some(mut set) => {
                set.retain(|x| sources.contains(x));
                Ok(Some(set))
            }
            None => Ok(Some(sources)),
        })?
        .unwrap_or_default();
        Ok(sources_set)
    }

    /// Calculates the max potential weight of smart_split / new_smart_split
    ///
    /// This function should cover the current code map and all possible calls of some functions that can take a weight.
    /// The current code map:
    ///
    /// smart_split()
    ///     quote()
    ///     quote()
    ///     check_rewards()
    ///     quote()
    ///     check_rewards()
    ///
    /// The new approach code map:
    ///
    /// new_smart_split()
    ///     step_quote() - max 4 times, because there are 4 liquidity sources
    ///     check_rewards() - max 4 times, because there are 4 liquidity sources
    ///
    /// Dev NOTE: if you change the logic of liquidity proxy, please sustain inner_exchange_weight() and code map above.
    pub fn smart_split_weight() -> Weight {
        // Only TBC has rewards weight, all others are zero.
        // In this case the max value of the sum of rewards weights is TBC weight,
        // because it could be only one TBC source in the list.
        // The rewards weight is added once, no matter how many times it was called in the code.
        T::LiquidityRegistry::check_rewards_weight().saturating_add(
            T::LiquidityRegistry::step_quote_weight(T::GetNumSamples::get()).saturating_mul(4),
        )
    }

    /// Calculates the max potential weight of inner_exchange
    ///
    /// This function should cover the current code map and all possible calls of some functions that can take a weight.
    /// The current code map:
    ///
    /// inner_exchange()
    ///     new_trivial()
    ///     exchange_sequence()
    ///         select_best_path()
    ///             quote_pairs_with_flexible_amount() - call M times, where M is a count of paths
    ///                 quote_single()
    ///                     list_liquidity_sources()
    ///                     quote()
    ///                     smart_split()
    ///         exchange_sequence_with_desired_amount()
    ///             exchange_single()
    ///                 quote_single()
    ///                     list_liquidity_sources()
    ///                     quote()
    ///                     smart_split()
    ///                 exchange() - call N times, where N is a count of assets in the path
    ///
    /// Dev NOTE: if you change the logic of liquidity proxy, please sustain inner_exchange_weight() and code map above.
    pub fn inner_exchange_weight(
        dex_id: &T::DEXId,
        input: &T::AssetId,
        output: &T::AssetId,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Weight {
        // Get DEX info or return weight that will be rejected
        let Ok(dex_info) = T::DexInfoProvider::get_dex_info(dex_id) else {
            return REJECTION_WEIGHT;
        };

        // Get trivial path or return weight that will be rejected
        let Some(trivial_paths) = ExchangePath::<T>::new_trivial(&dex_info, *input, *output) else {
            return REJECTION_WEIGHT;
        };

        #[cfg(not(feature = "wip"))] // ALT
        let quote_single_weight = <T as Config>::WeightInfo::list_liquidity_sources()
            .saturating_add(T::LiquidityRegistry::quote_weight().saturating_mul(4))
            .saturating_add(T::LiquidityRegistry::check_rewards_weight().saturating_mul(2));

        #[cfg(feature = "wip")] // ALT
        let quote_single_weight = <T as Config>::WeightInfo::list_liquidity_sources()
            .saturating_add(Self::smart_split_weight());

        let mut weight = <T as Config>::WeightInfo::new_trivial();

        // in quote_pairs_with_flexible_amount()
        weight =
            weight.saturating_add(quote_single_weight.saturating_mul(trivial_paths.len() as u64));

        let mut weights = Vec::new();

        for path in trivial_paths {
            if path.0.len() > 0 {
                let path_weights =
                    path.0
                        .iter()
                        .tuple_windows()
                        .map(|(input_asset_id, output_asset_id)| {
                            let exchange_sources = Self::list_quote_liquidity_sources(
                                input_asset_id,
                                output_asset_id,
                                &filter,
                            )
                            .unwrap_or(Vec::new()); // no sources -> no exchanges -> no weight
                            let single_exchange_weight =
                                T::LiquidityRegistry::exchange_weight_filtered(
                                    exchange_sources.iter().map(|s| s.liquidity_source_index),
                                );
                            single_exchange_weight
                        });
                let total_exchange_weight = path_weights
                    .fold(Weight::zero(), |acc, next_exchange_weight| {
                        acc.saturating_add(next_exchange_weight)
                    });
                weights.push(
                    weight
                        .saturating_add(quote_single_weight)
                        .saturating_add(total_exchange_weight),
                );
            }
        }

        assert!(!weights.is_empty());
        weights.iter().fold(weights[0], |max, &x| max.max(x))
    }

    /// Calculates the max potential weight of swap
    ///
    /// This function should cover the current code map and all possible calls of some functions that can take a weight.
    /// The current code map:
    ///
    /// swap()
    ///     inner_swap()
    ///         check_indivisible_assets()
    ///         is_forbidden_filter()
    ///         inner_exchange()
    ///
    /// Dev NOTE: if you change the logic of liquidity proxy, please sustain swap_weight() and code map above.
    pub fn swap_weight(
        dex_id: &T::DEXId,
        input: &T::AssetId,
        output: &T::AssetId,
        selected_source_types: &Vec<LiquiditySourceType>,
        filter_mode: &FilterMode,
    ) -> Weight {
        let filter = LiquiditySourceFilter::with_mode(
            *dex_id,
            filter_mode.clone(),
            selected_source_types.clone(),
        );
        let inner_exchange_weight = Self::inner_exchange_weight(dex_id, input, output, filter);

        let weight = <T as Config>::WeightInfo::check_indivisible_assets()
            .saturating_add(<T as Config>::WeightInfo::is_forbidden_filter())
            .saturating_add(inner_exchange_weight);

        weight
    }

    /// Calculates the max potential weight of swap_transfer_batch
    ///
    /// This function should cover the current code map and all possible calls of some functions that can take a weight.
    /// The current code map:
    ///
    /// swap_transfer_batch
    ///     inner_swap_batch_transfer
    ///         loop - call swap_batches.len() times
    ///             exchange_batch_tokens
    ///                 check_indivisible_assets
    ///                 is_forbidden_filter
    ///                 inner_exchange
    ///             transfer_batch_tokens_unchecked
    ///                 loop - call swap_batch_info.receivers.len() times
    ///                     transfer_from
    ///     transfer_from
    ///
    /// Dev NOTE: if you change the logic of liquidity proxy, please sustain swap_transfer_batch_weight() and code map above.
    pub fn swap_transfer_batch_weight(
        swap_batches: &Vec<SwapBatchInfo<T::AssetId, T::DEXId, T::AccountId>>,
        input: &T::AssetId,
        selected_source_types: &Vec<LiquiditySourceType>,
        filter_mode: &FilterMode,
    ) -> Weight {
        let mut weight = Weight::zero();

        for swap_batch_info in swap_batches {
            if input != &swap_batch_info.outcome_asset_id {
                let filter = LiquiditySourceFilter::with_mode(
                    swap_batch_info.dex_id,
                    filter_mode.clone(),
                    selected_source_types.clone(),
                );

                let inner_exchange_weight = Self::inner_exchange_weight(
                    &swap_batch_info.dex_id,
                    input,
                    &swap_batch_info.outcome_asset_id,
                    filter,
                );

                weight = weight
                    .saturating_add(<T as Config>::WeightInfo::check_indivisible_assets())
                    .saturating_add(<T as Config>::WeightInfo::is_forbidden_filter())
                    .saturating_add(<T as assets::Config>::WeightInfo::transfer()) // ADAR fee
                    .saturating_add(inner_exchange_weight);
            }

            // ADAR fee withdraw
            if swap_batch_info.outcome_asset_reuse > 0 {
                weight = weight.saturating_add(<T as assets::Config>::WeightInfo::transfer());
            }

            weight = weight.saturating_add(
                <T as assets::Config>::WeightInfo::transfer()
                    .saturating_mul(swap_batch_info.receivers.len() as u64),
            );
        }
        weight = weight.saturating_add(<T as assets::Config>::WeightInfo::transfer());

        weight
    }

    /// Given two arbitrary tokens return sources that can be used to cover full path.
    /// If there are two possible swap paths, then returns a union of used liquidity sources
    pub fn list_enabled_sources_for_path(
        dex_id: T::DEXId,
        input_asset_id: T::AssetId,
        output_asset_id: T::AssetId,
    ) -> Result<Vec<LiquiditySourceType>, DispatchError> {
        let dex_info = T::DexInfoProvider::get_dex_info(&dex_id)?;
        let maybe_path = ExchangePath::<T>::new_trivial(&dex_info, input_asset_id, output_asset_id);
        maybe_path.map_or_else(
            || Err(Error::<T>::UnavailableExchangePath.into()),
            |paths| {
                let mut paths_sources_iter = paths.into_iter().map(|ExchangePath(atomic_path)| {
                    Self::get_asset_path_sources(&dex_id, &dex_info, &atomic_path)
                });

                let primary_set: Result<BTreeSet<LiquiditySourceType>, DispatchError> =
                    paths_sources_iter
                        .next()
                        .ok_or(Error::<T>::UnavailableExchangePath)?;

                paths_sources_iter
                    .fold(primary_set, |acc: Result<_, DispatchError>, set| {
                        match (acc, set) {
                            (Ok(acc_unwrapped), Err(_)) => Ok(acc_unwrapped),
                            (Err(_), Ok(set_unwrapped)) => Ok(set_unwrapped),
                            (Ok(mut acc_unwrapped), Ok(mut set_unwrapped)) => {
                                acc_unwrapped.append(&mut set_unwrapped);
                                Ok(acc_unwrapped)
                            }
                            (Err(e), _) => Err(e),
                        }
                    })
                    .map(|set| Vec::from_iter(set.into_iter()))
            },
        )
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
    fn weak_sort_pair(
        dex_info: &DEXInfo<T::AssetId>,
        asset_a: T::AssetId,
        asset_b: T::AssetId,
    ) -> TradingPair<T::AssetId> {
        use AssetType::*;

        let synthetic_assets = T::PrimaryMarketXST::enabled_target_assets();
        let a_type = AssetType::determine::<T>(dex_info, &synthetic_assets, asset_a);
        let b_type = AssetType::determine::<T>(dex_info, &synthetic_assets, asset_b);

        match (a_type, b_type) {
            (Base, _) => TradingPair {
                base_asset_id: asset_a,
                target_asset_id: asset_b,
            },
            (_, Base) => TradingPair {
                base_asset_id: asset_b,
                target_asset_id: asset_a,
            },
            (SyntheticBase, _) => TradingPair {
                base_asset_id: asset_a,
                target_asset_id: asset_b,
            },
            (_, SyntheticBase) => TradingPair {
                base_asset_id: asset_b,
                target_asset_id: asset_a,
            },
            (_, _) => TradingPair {
                base_asset_id: asset_a,
                target_asset_id: asset_b,
            },
        }
    }

    #[cfg(feature = "wip")] // ALT
    fn new_smart_split(
        sources: &Vec<LiquiditySourceIdOf<T>>,
        base_asset_id: &T::AssetId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        skip_info: bool,
        deduce_fee: bool,
    ) -> Result<
        (
            AggregatedSwapOutcome<T::AssetId, LiquiditySourceIdOf<T>, Balance>,
            Rewards<T::AssetId>,
            Weight,
        ),
        DispatchError,
    > {
        ensure!(
            input_asset_id != output_asset_id,
            Error::<T>::UnavailableExchangePath
        );

        ensure!(
            input_asset_id == base_asset_id || output_asset_id == base_asset_id,
            Error::<T>::UnavailableExchangePath
        );

        let mut aggregator = LiquidityAggregator::new(amount.variant());

        let mut total_weight = Weight::zero();

        for source in sources {
            if let Ok((discrete_quotation, weight)) = T::LiquidityRegistry::step_quote(
                source,
                input_asset_id,
                output_asset_id,
                amount,
                T::GetNumSamples::get(),
                deduce_fee,
            ) {
                aggregator.add_source(source.clone(), discrete_quotation);
                total_weight = total_weight.saturating_add(weight);
            } else {
                // skip the source if it returns an error
                continue;
            }
        }

        let (swap_info, aggregate_swap_outcome) = aggregator
            .aggregate_swap_outcome(amount.amount())
            .ok_or(Error::<T>::UnavailableExchangePath)?;

        let mut rewards = Rewards::new();

        if !skip_info {
            for (source, (input, output)) in swap_info {
                let (mut reward, weight) = T::LiquidityRegistry::check_rewards(
                    &source,
                    input_asset_id,
                    output_asset_id,
                    input,
                    output,
                )
                .unwrap_or((Vec::new(), Weight::zero()));

                rewards.append(&mut reward);
                total_weight = total_weight.saturating_add(weight);
            }
        }

        Ok((aggregate_swap_outcome, rewards, total_weight))
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
    #[cfg(not(feature = "wip"))] // ALT
    fn smart_split(
        primary_source_id: &LiquiditySourceIdOf<T>,
        secondary_source_id: &LiquiditySourceIdOf<T>,
        base_asset_id: &T::AssetId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        skip_info: bool,
        deduce_fee: bool,
    ) -> Result<
        (
            AggregatedSwapOutcome<T::AssetId, LiquiditySourceIdOf<T>, Balance>,
            Rewards<T::AssetId>,
            Weight,
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

        ensure!(
            input_asset_id == base_asset_id || output_asset_id == base_asset_id,
            Error::<T>::UnavailableExchangePath
        );
        let other_asset = if base_asset_id == input_asset_id {
            output_asset_id
        } else {
            input_asset_id
        };

        let (reserves_base, reserves_other) =
            T::SecondaryMarket::reserves(base_asset_id, other_asset);

        let amount_primary = if output_asset_id == base_asset_id {
            // XOR is being bought
            Self::decide_primary_market_amount_buying_base_asset(
                base_asset_id,
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
                base_asset_id,
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
        let mut total_fee = OutcomeFee::new();
        let mut rewards = Vec::new();
        let mut distr = Vec::new();
        let mut maybe_error: Option<DispatchError> = None;
        let mut total_weight = Weight::zero();

        if amount_primary.amount() > Balance::zero() {
            // Attempting to quote according to the default sources weights
            let intermediary_result = T::LiquidityRegistry::quote(
                primary_source_id,
                input_asset_id,
                output_asset_id,
                amount_primary.clone(),
                deduce_fee,
            )
            .and_then(|(outcome_primary, weight)| {
                total_weight = total_weight.saturating_add(weight);
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
                    .and_then(|(outcome_secondary, weight)| {
                        total_weight = total_weight.saturating_add(weight);
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
                                let (mut reward, reward_weight) =
                                    T::LiquidityRegistry::check_rewards(
                                        info.0,
                                        input_asset_id,
                                        output_asset_id,
                                        input_amount,
                                        output_amount,
                                    )
                                    .unwrap_or((Vec::new(), Weight::zero()));
                                total_weight = total_weight.saturating_add(reward_weight);
                                rewards.append(&mut reward);
                            }
                        };
                        best = outcome_primary.amount + outcome_secondary.amount;
                        total_fee = outcome_primary.fee.merge(outcome_secondary.fee);
                        distr = vec![
                            (
                                primary_source_id.clone(),
                                SwapAmount::with_variant(
                                    amount_primary.variant(),
                                    amount_primary.amount(),
                                    outcome_primary.amount,
                                ),
                            ),
                            (
                                secondary_source_id.clone(),
                                SwapAmount::with_variant(
                                    amount_secondary.variant(),
                                    amount_secondary.amount(),
                                    outcome_secondary.amount,
                                ),
                            ),
                        ];
                        Ok(())
                    })
                } else {
                    best = outcome_primary.amount;
                    total_fee = outcome_primary.fee;
                    distr = vec![(
                        primary_source_id.clone(),
                        SwapAmount::with_variant(
                            amount_primary.variant(),
                            amount_primary.amount(),
                            outcome_primary.amount,
                        ),
                    )];
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
        .and_then(|(outcome, weight)| {
            total_weight = total_weight.saturating_add(weight);
            if is_better(outcome.amount, best) {
                best = outcome.amount;
                total_fee = outcome.fee.clone();
                distr = vec![(
                    secondary_source_id.clone(),
                    SwapAmount::with_variant(amount.variant(), amount.amount(), outcome.amount),
                )];
                if !skip_info {
                    let (input_amount, output_amount) =
                        amount.place_input_and_output(outcome.clone());
                    let reward_weight;
                    (rewards, reward_weight) = T::LiquidityRegistry::check_rewards(
                        secondary_source_id,
                        input_asset_id,
                        output_asset_id,
                        input_amount,
                        output_amount,
                    )
                    .unwrap_or((Vec::new(), Weight::zero()));
                    total_weight = total_weight.saturating_add(reward_weight);
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

        Ok((
            AggregatedSwapOutcome::new(distr, best, total_fee),
            rewards,
            total_weight,
        ))
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
    #[cfg(not(feature = "wip"))] // ALT
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
    #[cfg(not(feature = "wip"))] // ALT
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

    /// Swaps tokens for the following batch distribution and calculates a remainder.
    /// Remainder is used due to inaccuracy of the quote calculation.
    fn exchange_batch_tokens(
        sender: &T::AccountId,
        num_of_receivers: u128,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        max_input_amount: Balance,
        selected_source_types: &Vec<LiquiditySourceType>,
        dex_id: T::DEXId,
        filter_mode: &FilterMode,
        out_amount: Balance,
    ) -> Result<(Balance, Balance, Weight), DispatchError> {
        Self::check_indivisible_assets(input_asset_id, output_asset_id)?;
        let mut total_weight = <T as Config>::WeightInfo::check_indivisible_assets();

        let filter = LiquiditySourceFilter::with_mode(
            dex_id,
            filter_mode.clone(),
            selected_source_types.clone(),
        );

        if Self::is_forbidden_filter(
            &input_asset_id,
            &output_asset_id,
            &selected_source_types,
            &filter_mode,
        ) {
            fail!(Error::<T>::ForbiddenFilter);
        }
        total_weight =
            total_weight.saturating_add(<T as Config>::WeightInfo::is_forbidden_filter());

        let (
            SwapOutcome {
                amount: executed_input_amount,
                fee,
            },
            sources,
            weights,
        ) = Self::inner_exchange(
            dex_id,
            &sender,
            &sender,
            &input_asset_id,
            &output_asset_id,
            SwapAmount::WithDesiredOutput {
                desired_amount_out: out_amount,
                max_amount_in: max_input_amount,
            },
            filter.clone(),
        )?;
        total_weight = total_weight.saturating_add(weights);

        Self::deposit_event(Event::<T>::Exchange(
            sender.clone(),
            dex_id,
            input_asset_id.clone(),
            output_asset_id.clone(),
            executed_input_amount,
            out_amount,
            fee,
            sources,
        ));

        let caller_output_asset_balance =
            assets::Pallet::<T>::total_balance(&output_asset_id, &sender)?;
        let remainder_per_receiver: Balance = if caller_output_asset_balance < out_amount {
            let remainder = out_amount.saturating_sub(caller_output_asset_balance);
            remainder / num_of_receivers + remainder % num_of_receivers
        } else {
            0
        };
        Ok((executed_input_amount, remainder_per_receiver, total_weight))
    }

    fn transfer_batch_tokens_unchecked(
        sender: &T::AccountId,
        output_asset_id: &T::AssetId,
        receivers: Vec<BatchReceiverInfo<T::AccountId>>,
        remainder_per_receiver: Balance,
    ) -> Result<Weight, DispatchError> {
        let len = receivers.len();
        fallible_iterator::convert(receivers.into_iter().map(|val| Ok(val))).for_each(
            |receiver| {
                assets::Pallet::<T>::transfer_from(
                    &output_asset_id,
                    &sender,
                    &receiver.account_id,
                    receiver
                        .target_amount
                        .saturating_sub(remainder_per_receiver),
                )
            },
        )?;
        Ok(<T as assets::Config>::WeightInfo::transfer().saturating_mul(len as u64))
    }

    fn withdraw_adar_commission(
        who: &AccountIdOf<T>,
        asset_id: &AssetIdOf<T>,
        fee_ratio: Balance,
        amount: Balance,
        max_fee_amount: Balance,
    ) -> Result<Balance, DispatchError> {
        if amount.is_zero() {
            return Ok(Zero::zero());
        }

        let adar_commission_ratio = FixedWrapper::from(fee_ratio);

        let adar_commission = (FixedWrapper::from(amount) * adar_commission_ratio)
            .try_into_balance()
            .map_err(|_| Error::<T>::CalculationError)?;

        ensure!(
            adar_commission <= max_fee_amount,
            Error::<T>::SlippageNotTolerated
        );

        if adar_commission > 0 {
            assets::Pallet::<T>::transfer_from(
                &asset_id,
                &who,
                &T::GetADARAccountId::get(),
                adar_commission,
            )
            .map_err(|_| Error::<T>::FailedToTransferAdarCommission)?;
            Self::deposit_event(Event::<T>::ADARFeeWithdrawn(
                asset_id.clone(),
                adar_commission,
            ));
        }
        Ok(adar_commission)
    }

    fn inner_swap_batch_transfer(
        sender: &T::AccountId,
        input_asset_id: &T::AssetId,
        swap_batches: Vec<SwapBatchInfo<T::AssetId, T::DEXId, T::AccountId>>,
        mut max_input_amount: Balance,
        selected_source_types: &Vec<LiquiditySourceType>,
        filter_mode: &FilterMode,
    ) -> Result<(Balance, Balance, Weight), DispatchError> {
        let mut unique_asset_ids: BTreeSet<T::AssetId> = BTreeSet::new();

        let mut executed_batch_input_amount = balance!(0);

        let mut total_weight = Weight::zero();

        let adar_fee_ratio = Self::adar_commission_ratio();

        fallible_iterator::convert(swap_batches.into_iter().map(|val| Ok(val))).for_each(
            |swap_batch_info| {
                let SwapBatchInfo {
                    outcome_asset_id: asset_id,
                    dex_id,
                    receivers,
                    outcome_asset_reuse,
                } = swap_batch_info;

                let balance = assets::Pallet::<T>::free_balance(&asset_id, &sender)?;

                if balance < outcome_asset_reuse {
                    fail!(Error::<T>::InsufficientBalance);
                }

                // extrinsic fails if there are duplicate output asset ids
                if !unique_asset_ids.insert(asset_id.clone()) {
                    fail!(Error::<T>::AggregationError);
                }

                if receivers.len() == 0 {
                    fail!(Error::<T>::InvalidReceiversInfo);
                }

                let out_amount = receivers
                    .iter()
                    .map(|recv| recv.target_amount)
                    .try_fold(Balance::zero(), |acc, val| acc.checked_add(val))
                    .ok_or(Error::<T>::CalculationError)?;

                let (executed_input_amount, remainder_per_receiver, weight): (
                    Balance,
                    Balance,
                    Weight,
                ) = if &asset_id != input_asset_id {
                    let withdrawn_fee = Self::withdraw_adar_commission(
                        &sender,
                        &asset_id,
                        adar_fee_ratio,
                        outcome_asset_reuse.min(out_amount),
                        outcome_asset_reuse,
                    )?;

                    let outcome_asset_reuse = outcome_asset_reuse.saturating_sub(withdrawn_fee);

                    let desired_exchange_amount = out_amount.saturating_sub(outcome_asset_reuse);

                    if !desired_exchange_amount.is_zero() {
                        Self::exchange_batch_tokens(
                            &sender,
                            receivers.len() as u128,
                            &input_asset_id,
                            &asset_id,
                            max_input_amount,
                            &selected_source_types,
                            dex_id,
                            &filter_mode,
                            desired_exchange_amount,
                        )?
                    } else {
                        (0, 0, Weight::zero())
                    }
                } else {
                    (out_amount, 0, Weight::zero())
                };
                total_weight = total_weight.saturating_add(weight);

                executed_batch_input_amount = executed_batch_input_amount
                    .checked_add(executed_input_amount)
                    .ok_or(Error::<T>::CalculationError)?;

                max_input_amount = max_input_amount
                    .checked_sub(executed_input_amount)
                    .ok_or(Error::<T>::SlippageNotTolerated)?;

                let transfer_weight = Self::transfer_batch_tokens_unchecked(
                    &sender,
                    &asset_id,
                    receivers,
                    remainder_per_receiver,
                )?;
                total_weight = total_weight.saturating_add(transfer_weight);
                Result::<_, DispatchError>::Ok(())
            },
        )?;
        let adar_commission = Self::withdraw_adar_commission(
            &sender,
            &input_asset_id,
            adar_fee_ratio,
            executed_batch_input_amount,
            max_input_amount,
        )?;
        Ok((adar_commission, executed_batch_input_amount, total_weight))
    }

    /// Wrapper for `quote_single` to make possible call it from tests.
    pub fn test_quote(
        dex_id: T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
        deduce_fee: bool,
    ) -> Result<AggregatedSwapOutcome<T::AssetId, LiquiditySourceIdOf<T>, Balance>, DispatchError>
    {
        let dex_info = T::DexInfoProvider::get_dex_info(&dex_id)?;
        Pallet::<T>::quote_single(
            &dex_info.base_asset_id,
            input_asset_id,
            output_asset_id,
            amount,
            filter,
            true,
            deduce_fee,
        )
        .map(|(aggregated_swap_outcome, _, _, _)| aggregated_swap_outcome)
    }
}

impl<T: Config> LiquidityProxyTrait<T::DEXId, T::AccountId, T::AssetId> for Pallet<T> {
    /// Applies trivial routing (via Base Asset), resulting in a poly-swap which may contain several individual swaps.
    /// Those individual swaps are subject to liquidity aggregation algorithm.
    ///
    /// This is a wrapper for `quote_single`.
    fn quote(
        dex_id: T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
        deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance, T::AssetId>, DispatchError> {
        Pallet::<T>::inner_quote(
            dex_id,
            input_asset_id,
            output_asset_id,
            amount,
            filter,
            true,
            deduce_fee,
        )
        .map(|(quote_info, _, _)| quote_info.outcome)
    }

    /// Applies trivial routing (via Base Asset), resulting in a poly-swap which may contain several individual swaps.
    /// Those individual swaps are subject to liquidity aggregation algorithm.
    ///
    /// This is a wrapper for `exchange_single`.
    fn exchange(
        dex_id: T::DEXId,
        sender: &T::AccountId,
        receiver: &T::AccountId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance, T::AssetId>, DispatchError> {
        let (outcome, _, _) = Pallet::<T>::inner_exchange(
            dex_id,
            sender,
            receiver,
            input_asset_id,
            output_asset_id,
            amount,
            filter,
        )?;
        Ok(outcome)
    }
}

#[derive(
    Encode, Decode, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, RuntimeDebug, scale_info::TypeInfo,
)]
#[scale_info(skip_type_params(T))]
pub struct BatchReceiverInfo<AccountId> {
    pub account_id: AccountId,
    pub target_amount: Balance,
}

impl<AccountId> BatchReceiverInfo<AccountId> {
    pub fn new(account_id: AccountId, amount: Balance) -> Self {
        BatchReceiverInfo {
            account_id,
            target_amount: amount,
        }
    }
}

#[derive(
    Encode, Decode, Clone, PartialEq, Eq, PartialOrd, Ord, RuntimeDebug, scale_info::TypeInfo,
)]
#[scale_info(skip_type_params(T))]
pub struct SwapBatchInfo<AssetId, DEXId, AccountId> {
    pub outcome_asset_id: AssetId,
    pub outcome_asset_reuse: Balance,
    pub dex_id: DEXId,
    pub receivers: Vec<BatchReceiverInfo<AccountId>>,
}

impl<AssetId, DEXId, AccountId> SwapBatchInfo<AssetId, DEXId, AccountId> {
    pub fn len(&self) -> usize {
        self.receivers.len()
    }
}

pub struct LiquidityProxyBuyBackHandler<T, GetDEXId>(PhantomData<(T, GetDEXId)>);

impl<T: Config, GetDEXId: Get<T::DEXId>> BuyBackHandler<T::AccountId, T::AssetId>
    for LiquidityProxyBuyBackHandler<T, GetDEXId>
{
    fn mint_buy_back_and_burn(
        mint_asset_id: &T::AssetId,
        buy_back_asset_id: &T::AssetId,
        amount: Balance,
    ) -> Result<Balance, DispatchError> {
        let owner = assets::Pallet::<T>::asset_owner(&mint_asset_id)
            .ok_or(assets::Error::<T>::AssetIdNotExists)?;
        let transit = T::GetTechnicalAccountId::get();
        assets::Pallet::<T>::mint_to(mint_asset_id, &owner, &transit, amount)?;
        let amount = Self::buy_back_and_burn(&transit, mint_asset_id, buy_back_asset_id, amount)?;
        Ok(amount)
    }

    fn buy_back_and_burn(
        account_id: &T::AccountId,
        asset_id: &T::AssetId,
        buy_back_asset_id: &T::AssetId,
        amount: Balance,
    ) -> Result<Balance, DispatchError> {
        let dex_id = GetDEXId::get();
        let outcome = Pallet::<T>::exchange(
            dex_id,
            account_id,
            account_id,
            asset_id,
            buy_back_asset_id,
            SwapAmount::with_desired_input(amount, 0),
            LiquiditySourceFilter::with_forbidden(
                dex_id,
                vec![LiquiditySourceType::MulticollateralBondingCurvePool],
            ),
        )?;
        assets::Pallet::<T>::burn_from(buy_back_asset_id, account_id, account_id, outcome.amount)?;
        Ok(outcome.amount)
    }
}

pub struct ReferencePriceProvider<T, GetDEXId, GetReferenceAssetId>(
    PhantomData<(T, GetDEXId, GetReferenceAssetId)>,
);

impl<T: Config, GetDEXId: Get<T::DEXId>, GetReferenceAssetId: Get<T::AssetId>>
    common::ReferencePriceProvider<T::AssetId, Balance>
    for ReferencePriceProvider<T, GetDEXId, GetReferenceAssetId>
{
    fn get_reference_price(asset_id: &T::AssetId) -> Result<Balance, DispatchError> {
        let dex_id = GetDEXId::get();
        let reference_asset_id = GetReferenceAssetId::get();
        if asset_id == &reference_asset_id {
            return Ok(balance!(1));
        }
        let outcome = Pallet::<T>::quote(
            dex_id,
            asset_id,
            &reference_asset_id,
            QuoteAmount::with_desired_input(balance!(1)),
            LiquiditySourceFilter::with_forbidden(
                dex_id,
                vec![LiquiditySourceType::MulticollateralBondingCurvePool],
            ),
            false,
        )?;
        Ok(outcome.amount)
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::prelude::OutcomeFee;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::EnsureOrigin;
    use frame_support::{traits::StorageVersion, transactional};
    use frame_system::pallet_prelude::*;

    // TODO: #395 use AssetInfoProvider instead of assets pallet
    #[pallet::config]
    pub trait Config: frame_system::Config + common::Config + assets::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
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
        type VestedRewardsPallet: Vesting<Self::AccountId, Self::AssetId>;
        type TradingPairSourceManager: TradingPairSourceManager<Self::DEXId, Self::AssetId>;
        type LockedLiquiditySourcesManager: LockedLiquiditySourcesManager<LiquiditySourceType>;
        type GetADARAccountId: Get<Self::AccountId>;
        type ADARCommissionRatioUpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        type MaxAdditionalDataLengthXorlessTransfer: Get<u32>;
        type MaxAdditionalDataLengthSwapTransferBatch: Get<u32>;
        type DexInfoProvider: DexInfoProvider<Self::DEXId, DEXInfo<Self::AssetId>>;
        /// Weight information for the extrinsics in this Pallet.
        type WeightInfo: WeightInfo;
    }

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
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
        #[pallet::call_index(0)]
        #[pallet::weight(Pallet::<T>::swap_weight(dex_id, input_asset_id, output_asset_id, selected_source_types, filter_mode))]
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
            let weight = Self::inner_swap(
                who.clone(),
                who,
                dex_id,
                input_asset_id,
                output_asset_id,
                swap_amount,
                selected_source_types,
                filter_mode,
            )?;
            Ok(PostDispatchInfo {
                actual_weight: Some(weight),
                pays_fee: Pays::Yes,
            })
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
        #[pallet::call_index(1)]
        #[pallet::weight(Pallet::<T>::swap_weight(dex_id, input_asset_id, output_asset_id, selected_source_types, filter_mode))]
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

            let weight = Self::inner_swap(
                who,
                receiver,
                dex_id,
                input_asset_id,
                output_asset_id,
                swap_amount,
                selected_source_types,
                filter_mode,
            )?;
            Ok(PostDispatchInfo {
                actual_weight: Some(weight),
                pays_fee: Pays::Yes,
            })
        }

        /// Dispatches multiple swap & transfer operations. `swap_batches` contains vector of
        /// SwapBatchInfo structs, where each batch specifies which asset ID and DEX ID should
        /// be used for swapping, receiver accounts and their desired outcome amount in asset,
        /// specified for the current batch.
        ///
        /// - `origin`: the account on whose behalf the transaction is being executed,
        /// - `swap_batches`: the vector containing the SwapBatchInfo structs,
        /// - `input_asset_id`: ID of the asset being sold,
        /// - `max_input_amount`: the maximum amount to be sold in input_asset_id,
        /// - `selected_source_types`: list of selected LiquiditySource types, selection effect is
        ///                            determined by filter_mode,
        /// - `filter_mode`: indicate either to allow or forbid selected types only, or disable filtering.
        /// - `additional_data`: data to include in swap success event.
        #[transactional]
        #[pallet::call_index(2)]
        #[pallet::weight(Pallet::<T>::swap_transfer_batch_weight(swap_batches, input_asset_id, selected_source_types, filter_mode))]
        pub fn swap_transfer_batch(
            origin: OriginFor<T>,
            swap_batches: Vec<SwapBatchInfo<T::AssetId, T::DEXId, T::AccountId>>,
            input_asset_id: T::AssetId,
            max_input_amount: Balance,
            selected_source_types: Vec<LiquiditySourceType>,
            filter_mode: FilterMode,
            additional_data: Option<BoundedVec<u8, T::MaxAdditionalDataLengthSwapTransferBatch>>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let (adar_commission, executed_input_amount, mut weight) =
                Self::inner_swap_batch_transfer(
                    &who,
                    &input_asset_id,
                    swap_batches,
                    max_input_amount,
                    &selected_source_types,
                    &filter_mode,
                )?;

            Self::deposit_event(Event::<T>::BatchSwapExecuted(
                adar_commission,
                executed_input_amount,
                additional_data,
            ));

            weight = weight.saturating_add(<T as assets::Config>::WeightInfo::transfer());

            Ok(PostDispatchInfo {
                actual_weight: Some(weight),
                pays_fee: Pays::Yes,
            })
        }

        /// Enables XST or TBC liquidity source.
        ///
        /// - `liquidity_source`: the liquidity source to be enabled.
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::enable_liquidity_source())]
        pub fn enable_liquidity_source(
            origin: OriginFor<T>,
            liquidity_source: LiquiditySourceType,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            ensure!(
                liquidity_source == LiquiditySourceType::XSTPool
                    || liquidity_source == LiquiditySourceType::MulticollateralBondingCurvePool,
                Error::<T>::UnableToEnableLiquiditySource
            );

            let mut locked = T::LockedLiquiditySourcesManager::get();

            ensure!(
                locked.contains(&liquidity_source),
                Error::<T>::LiquiditySourceAlreadyEnabled
            );

            locked.retain(|x| *x != liquidity_source);
            T::LockedLiquiditySourcesManager::set(locked);
            Self::deposit_event(Event::<T>::LiquiditySourceEnabled(liquidity_source));
            Ok(().into())
        }

        /// Disables XST or TBC liquidity source. The liquidity source becomes unavailable for swap.
        ///
        /// - `liquidity_source`: the liquidity source to be disabled.
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::disable_liquidity_source())]
        pub fn disable_liquidity_source(
            origin: OriginFor<T>,
            liquidity_source: LiquiditySourceType,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            ensure!(
                liquidity_source == LiquiditySourceType::XSTPool
                    || liquidity_source == LiquiditySourceType::MulticollateralBondingCurvePool,
                Error::<T>::UnableToDisableLiquiditySource
            );
            ensure!(
                !T::LockedLiquiditySourcesManager::get().contains(&liquidity_source),
                Error::<T>::LiquiditySourceAlreadyDisabled
            );
            T::LockedLiquiditySourcesManager::append(liquidity_source);
            Self::deposit_event(Event::<T>::LiquiditySourceDisabled(liquidity_source));
            Ok(().into())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::set_adar_commission_ratio())]
        pub fn set_adar_commission_ratio(
            origin: OriginFor<T>,
            commission_ratio: Balance,
        ) -> DispatchResultWithPostInfo {
            T::ADARCommissionRatioUpdateOrigin::ensure_origin(origin)?;
            ensure!(
                commission_ratio < balance!(1),
                Error::<T>::InvalidADARCommissionRatio
            );
            ADARCommissionRatio::<T>::put(commission_ratio);
            Ok(().into())
        }

        /// Extrinsic which is enable XORless transfers.
        /// Internally it's swaps `asset_id` to `desired_xor_amount` of `XOR` and transfers remaining amount of `asset_id` to `receiver`.
        /// Client apps should specify the XOR amount which should be paid as a fee in `desired_xor_amount` parameter.
        /// If sender will not have enough XOR to pay fees after execution, transaction will be rejected.
        /// This extrinsic is done as temporary solution for XORless transfers, in future it would be removed
        /// and logic for XORless extrinsics should be moved to xor-fee pallet.
        #[pallet::call_index(6)]
        #[pallet::weight({
            let mut weight = <T as assets::Config>::WeightInfo::transfer();
            if asset_id != &common::XOR.into()
                && max_amount_in > &Balance::zero()
                && desired_xor_amount > &Balance::zero()
            {
                weight = weight.saturating_add(Pallet::<T>::swap_weight(dex_id, asset_id, &common::XOR.into(), selected_source_types, filter_mode));
            }
            weight
        })]
        pub fn xorless_transfer(
            origin: OriginFor<T>,
            dex_id: T::DEXId,
            asset_id: T::AssetId,
            receiver: T::AccountId,
            amount: Balance,
            desired_xor_amount: Balance,
            max_amount_in: Balance,
            selected_source_types: Vec<LiquiditySourceType>,
            filter_mode: FilterMode,
            additional_data: Option<BoundedVec<u8, T::MaxAdditionalDataLengthXorlessTransfer>>,
        ) -> DispatchResultWithPostInfo {
            let sender = ensure_signed(origin)?;
            ensure!(sender != receiver, Error::<T>::TheSameSenderAndReceiver);

            let mut weight = Weight::default();
            if asset_id != common::XOR.into()
                && max_amount_in > Balance::zero()
                && desired_xor_amount > Balance::zero()
            {
                weight = weight.saturating_add(Self::inner_swap(
                    sender.clone(),
                    sender.clone(),
                    dex_id,
                    asset_id,
                    common::XOR.into(),
                    SwapAmount::with_desired_output(desired_xor_amount, max_amount_in),
                    selected_source_types,
                    filter_mode,
                )?);
            }

            assets::Pallet::<T>::transfer_from(&asset_id, &sender, &receiver, amount)?;
            weight = weight.saturating_add(<T as assets::Config>::WeightInfo::transfer());

            Self::deposit_event(Event::<T>::XorlessTransfer(
                asset_id,
                sender,
                receiver,
                amount,
                additional_data,
            ));

            Ok(PostDispatchInfo {
                actual_weight: Some(weight),
                pays_fee: Pays::Yes,
            })
        }
    }

    #[pallet::event]
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
            OutcomeFee<AssetIdOf<T>, Balance>,
            Vec<LiquiditySourceIdOf<T>>,
        ),
        /// Liquidity source was enabled
        LiquiditySourceEnabled(LiquiditySourceType),
        /// Liquidity source was disabled
        LiquiditySourceDisabled(LiquiditySourceType),
        /// Batch of swap transfers has been performed
        /// [Input asset ADAR Fee, Input amount, Additional Data]
        BatchSwapExecuted(
            Balance,
            Balance,
            Option<BoundedVec<u8, T::MaxAdditionalDataLengthSwapTransferBatch>>,
        ),
        /// XORless transfer has been performed
        /// [Asset Id, Caller Account, Receiver Account, Amount, Additional Data]
        XorlessTransfer(
            AssetIdOf<T>,
            AccountIdOf<T>,
            AccountIdOf<T>,
            Balance,
            Option<BoundedVec<u8, T::MaxAdditionalDataLengthXorlessTransfer>>,
        ),
        /// ADAR fee which is withdrawn from reused outcome asset amount
        /// [Asset Id, ADAR Fee]
        ADARFeeWithdrawn(AssetIdOf<T>, Balance),
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
        /// Unable to swap indivisible assets
        UnableToSwapIndivisibleAssets,
        /// Unable to enable liquidity source
        UnableToEnableLiquiditySource,
        /// Liquidity source is already enabled
        LiquiditySourceAlreadyEnabled,
        /// Unable to disable liquidity source
        UnableToDisableLiquiditySource,
        /// Liquidity source is already disabled
        LiquiditySourceAlreadyDisabled,
        // Information about swap batch receivers is invalid
        InvalidReceiversInfo,
        // Failure while transferring commission to ADAR account
        FailedToTransferAdarCommission,
        // ADAR commission ratio exceeds 1
        InvalidADARCommissionRatio,
        // Sender don't have enough asset balance
        InsufficientBalance,
        // Sender and receiver should not be the same
        TheSameSenderAndReceiver,
    }

    #[pallet::type_value]
    pub fn DefaultADARCommissionRatio() -> Balance {
        balance!(0.0025)
    }

    /// ADAR commission ratio
    #[pallet::storage]
    #[pallet::getter(fn adar_commission_ratio)]
    pub type ADARCommissionRatio<T: Config> =
        StorageValue<_, Balance, ValueQuery, DefaultADARCommissionRatio>;
}
