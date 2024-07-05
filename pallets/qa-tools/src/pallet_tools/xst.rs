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

use crate::pallet_tools;
use crate::{Config, Error};
use codec::{Decode, Encode};
use common::fixnum::ops::CheckedSub;
use common::prelude::{BalanceUnit, QuoteAmount};
use common::{fixed, AssetIdOf, AssetName, AssetSymbol, Balance, Fixed, Oracle, PriceVariant};
use frame_support::dispatch::{
    DispatchResult, DispatchResultWithPostInfo, RawOrigin,
};
use frame_support::ensure;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use pallet_tools::price_tools::AssetPrices;
use sp_runtime::DispatchError;
use sp_std::fmt::Debug;
use sp_std::vec;
use sp_std::vec::Vec;

/// Prices with 10^18 precision. Amount of the asset per 1 XOR. The same format as used
/// in price tools.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BaseXorPrices {
    pub synthetic_base: AssetPrices,
    pub reference: AssetPrices,
}

/// Price initialization parameters of `xst`'s synthetic base asset (in terms of reference asset)
#[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
pub struct BaseInput {
    pub reference_per_synthetic_base_buy: Balance,
    pub reference_per_synthetic_base_sell: Balance,
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
pub enum SyntheticExistence<Symbol> {
    AlreadyExists,
    RegisterNewAsset {
        symbol: AssetSymbol,
        name: AssetName,
        reference_symbol: Symbol,
        fee_ratio: common::Fixed,
    },
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
pub enum SyntheticQuoteDirection {
    SyntheticBaseToSynthetic,
    SyntheticToSyntheticBase,
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
pub struct SyntheticQuote {
    pub direction: SyntheticQuoteDirection,
    pub amount: QuoteAmount<Balance>,
    pub result: Balance,
}

/// Buy/sell price discrepancy is determined for all synthetics in `xst` pallet by synthetic
/// base (XST) asset prices;
///
/// We can't control it granularly for each asset, so we just deduce it from the existing
/// pricing and price provided for the given variant
#[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
pub struct SyntheticInput<AssetId, Symbol> {
    pub asset_id: AssetId,
    /// Quote call with expected output.
    /// The initialization tries to set up pallets to achieve these values
    pub expected_quote: SyntheticQuote,
    pub existence: SyntheticExistence<Symbol>,
}

/// Resulting of initialization for `asset_id`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
pub struct SyntheticOutput<AssetId> {
    pub asset_id: AssetId,
    /// Quote call with output.
    /// Sometimes, due to fixed-point precision limitations the exact value cannot be
    /// reproduced exactly. This provides a way to get the actual result for further usage.
    pub quote_achieved: SyntheticQuote,
}

/// Adapter for [`pallet_tools::price_tools::calculate_xor_prices`] to avoid confusion with
/// assets.
fn calculate_xor_prices<T: Config>(
    input_prices: BaseInput,
    synthetic_base_asset_id: &AssetIdOf<T>,
    reference_asset_id: &AssetIdOf<T>,
) -> Result<BaseXorPrices, DispatchError> {
    // B = reference
    // A = synthetic base
    let xor_prices = pallet_tools::price_tools::calculate_xor_prices::<T>(
        synthetic_base_asset_id,
        reference_asset_id,
        input_prices.reference_per_synthetic_base_buy,
        input_prices.reference_per_synthetic_base_sell,
    )?;
    Ok(BaseXorPrices {
        synthetic_base: xor_prices.asset_a,
        reference: xor_prices.asset_b,
    })
}

/// Feed `band` pallet the price for the symbol.
///
/// Tries to remove (decay) the dynamic fee occurring from the price change.
fn relay_symbol_band<T: Config>(
    symbol: <T as Config>::Symbol,
    relayer: T::AccountId,
    price_band: u64,
) -> DispatchResultWithPostInfo {
    let symbol: <T as band::Config>::Symbol = symbol.into();
    let latest_rate = band::Pallet::<T>::rates(&symbol);
    let mut resolve_time = latest_rate.map_or(0, |rate| rate.last_updated + 1);
    let mut request_id = latest_rate.map_or(0, |rate| rate.request_id + 1);
    let mut post_info = band::Pallet::<T>::relay(
        RawOrigin::Signed(relayer.clone()).into(),
        vec![(symbol.clone(), price_band)].try_into().unwrap(),
        resolve_time,
        request_id,
    )?;
    resolve_time += 1;
    request_id += 1;
    let mut previous_fee: Fixed = fixed!(2);
    for _ in 0..30 {
        if let Some(new_rate) = band::Pallet::<T>::rates(&symbol) {
            if previous_fee.saturating_sub(new_rate.dynamic_fee) == fixed!(0) {
                break;
            }
            previous_fee = new_rate.dynamic_fee;
            if new_rate.dynamic_fee > fixed!(0) {
                let next_post_info = band::Pallet::<T>::relay(
                    RawOrigin::Signed(relayer.clone()).into(),
                    vec![(symbol.clone(), price_band)].try_into().unwrap(),
                    resolve_time,
                    request_id,
                )?;
                resolve_time += 1;
                request_id += 1;
                post_info.actual_weight = post_info
                    .actual_weight
                    .map(|w| {
                        w.saturating_add(next_post_info.actual_weight.unwrap_or(Weight::zero()))
                    })
                    .or(next_post_info.actual_weight);
            } else {
                break;
            }
        }
    }
    Ok(post_info)
}

/// Calculate the band price needed to achieve the expected quote values (closely enough).
fn calculate_band_price<T: Config>(
    target_quote: &SyntheticQuote,
    ref_per_synthetic_base: &AssetPrices,
) -> Result<u64, DispatchError> {
    // band price is `ref_per_synthetic`.
    // we need to get it from formulae in xst pallet.
    let ref_per_synthetic: BalanceUnit = match (
        &target_quote.direction,
        target_quote.amount,
        target_quote.result,
    ) {
        // sell:
        // synthetic base (xst) -> synthetic (xst***)
        // synthetic base (also called main) - sell price, synthetic - no diff between buy/sell
        // (all prices in reference assets per this asset)
        (
            SyntheticQuoteDirection::SyntheticBaseToSynthetic,
            QuoteAmount::WithDesiredInput {
                desired_amount_in: amount_in,
            },
            amount_out,
        )
        | (
            SyntheticQuoteDirection::SyntheticBaseToSynthetic,
            QuoteAmount::WithDesiredOutput {
                desired_amount_out: amount_out,
            },
            amount_in,
        ) => {
            // equivalent formulae for desired input/output:
            //
            // amount_out = amount_in * ref_per_synthetic_base (sell) / ref_per_synthetic
            // amount_in = amount_out * ref_per_synthetic / ref_per_synthetic_base (sell)

            // from this,
            // ref_per_synthetic = ref_per_synthetic_base (sell) * amount_in / amount_out
            let ref_per_synthetic_base_sell = BalanceUnit::divisible(ref_per_synthetic_base.sell);
            ref_per_synthetic_base_sell * BalanceUnit::divisible(amount_in)
                / BalanceUnit::divisible(amount_out)
        }
        // buy
        // synthetic (xst***) -> synthetic base (xst)
        // synthetic base (also called main) - buy price, synthetic - no diff between buy/sell
        // (all prices in reference assets per this asset)
        (
            SyntheticQuoteDirection::SyntheticToSyntheticBase,
            QuoteAmount::WithDesiredInput {
                desired_amount_in: amount_in,
            },
            amount_out,
        )
        | (
            SyntheticQuoteDirection::SyntheticToSyntheticBase,
            QuoteAmount::WithDesiredOutput {
                desired_amount_out: amount_out,
            },
            amount_in,
        ) => {
            // equivalent formulae for desired input/output:
            //
            // amount_out = amount_in * ref_per_synthetic / ref_per_synthetic_base (buy)
            // amount_in = amount_out * ref_per_synthetic_base (buy) / ref_per_synthetic

            // from this,
            // ref_per_synthetic = ref_per_synthetic_base (buy) * amount_out / amount_in
            let ref_per_synthetic_base_buy = BalanceUnit::divisible(ref_per_synthetic_base.buy);
            ref_per_synthetic_base_buy * BalanceUnit::divisible(amount_out)
                / BalanceUnit::divisible(amount_in)
        }
    };
    // band price
    (*ref_per_synthetic.balance() / 10u128.pow(9))
        .try_into()
        .map_err(|_| Error::<T>::ArithmeticError.into())
}

fn calculate_actual_quote<T: Config>(
    asset_id: AssetIdOf<T>,
    expected_quote: SyntheticQuote,
    synthetic_band_price: u64,
    ref_per_synthetic_base: &AssetPrices,
) -> SyntheticOutput<AssetIdOf<T>> {
    let ref_per_synthetic = synthetic_band_price as Balance * 10_u128.pow(9);
    let actual_quote_result = match (&expected_quote.direction, &expected_quote.amount) {
        // sell:
        // synthetic base (xst) -> synthetic (xst***)
        // synthetic base (also called main) - sell price, synthetic - no diff between buy/sell
        // (all prices in reference assets per this asset)
        (
            SyntheticQuoteDirection::SyntheticBaseToSynthetic,
            QuoteAmount::WithDesiredInput {
                desired_amount_in: amount_in,
            },
        ) => {
            // amount_out = amount_in * ref_per_synthetic_base (sell) / ref_per_synthetic
            BalanceUnit::divisible(*amount_in) * BalanceUnit::divisible(ref_per_synthetic_base.sell)
                / BalanceUnit::divisible(ref_per_synthetic)
        }
        (
            SyntheticQuoteDirection::SyntheticBaseToSynthetic,
            QuoteAmount::WithDesiredOutput {
                desired_amount_out: amount_out,
            },
        ) => {
            // amount_in = amount_out * ref_per_synthetic / ref_per_synthetic_base (sell)
            BalanceUnit::divisible(*amount_out) * BalanceUnit::divisible(ref_per_synthetic)
                / BalanceUnit::divisible(ref_per_synthetic_base.sell)
        }
        // buy
        // synthetic (xst***) -> synthetic base (xst)
        // synthetic base (also called main) - buy price, synthetic - no diff between buy/sell
        // (all prices in reference assets per this asset)
        (
            SyntheticQuoteDirection::SyntheticToSyntheticBase,
            QuoteAmount::WithDesiredInput {
                desired_amount_in: amount_in,
            },
        ) => {
            // amount_out = amount_in * ref_per_synthetic / ref_per_synthetic_base (buy)
            BalanceUnit::divisible(*amount_in) * BalanceUnit::divisible(ref_per_synthetic)
                / BalanceUnit::divisible(ref_per_synthetic_base.buy)
        }
        (
            SyntheticQuoteDirection::SyntheticToSyntheticBase,
            QuoteAmount::WithDesiredOutput {
                desired_amount_out: amount_out,
            },
        ) => {
            // amount_in = amount_out * ref_per_synthetic_base (buy) / ref_per_synthetic
            BalanceUnit::divisible(*amount_out) * BalanceUnit::divisible(ref_per_synthetic_base.buy)
                / BalanceUnit::divisible(ref_per_synthetic)
        }
    };
    let actual_quote = SyntheticQuote {
        result: *actual_quote_result.balance(),
        ..expected_quote
    };
    SyntheticOutput {
        asset_id,
        quote_achieved: actual_quote,
    }
}

pub(crate) fn initialize_base_assets<T: Config>(input: BaseInput) -> DispatchResult {
    let synthetic_base_asset_id = <T as xst::Config>::GetSyntheticBaseAssetId::get();
    let reference_asset_id = xst::ReferenceAssetId::<T>::get();

    let xor_prices =
        calculate_xor_prices::<T>(input, &synthetic_base_asset_id, &reference_asset_id)?;
    // check user input correctness as well as calculation sanity
    ensure!(
        xor_prices.synthetic_base.buy >= xor_prices.synthetic_base.sell
            && xor_prices.reference.buy >= xor_prices.reference.sell,
        Error::<T>::BuyLessThanSell
    );
    pallet_tools::price_tools::set_xor_prices::<T>(
        &synthetic_base_asset_id,
        xor_prices.synthetic_base,
    )?;
    // reference asset prices are expected to be set via `price_tools` tools separately
    Ok(())
}

fn initialize_single_synthetic<T: Config>(
    input: SyntheticInput<AssetIdOf<T>, <T as Config>::Symbol>,
    relayer: T::AccountId,
) -> Result<SyntheticOutput<AssetIdOf<T>>, DispatchError> {
    let synthetic_base_asset_id = <T as xst::Config>::GetSyntheticBaseAssetId::get();
    let ref_per_synthetic_base = AssetPrices {
        buy: xst::Pallet::<T>::reference_price(&synthetic_base_asset_id, PriceVariant::Buy)
            .unwrap(),
        sell: xst::Pallet::<T>::reference_price(&synthetic_base_asset_id, PriceVariant::Sell)
            .unwrap(),
    };
    let band_price = calculate_band_price::<T>(&input.expected_quote, &ref_per_synthetic_base)?;
    let resulting_quote = calculate_actual_quote::<T>(
        input.asset_id,
        input.expected_quote,
        band_price,
        &ref_per_synthetic_base,
    );
    match (
        xst::Pallet::<T>::enabled_synthetics(input.asset_id),
        input.existence,
    ) {
        (Some(info), SyntheticExistence::AlreadyExists) => {
            relay_symbol_band::<T>(info.reference_symbol.into(), relayer, band_price)
                .map_err(|e| e.error)?;
        }
        (
            None,
            SyntheticExistence::RegisterNewAsset {
                symbol,
                name,
                reference_symbol,
                fee_ratio,
            },
        ) => {
            relay_symbol_band::<T>(reference_symbol.clone(), relayer, band_price)
                .map_err(|e| e.error)?;
            xst::Pallet::<T>::register_synthetic_asset(
                RawOrigin::Root.into(),
                symbol,
                name,
                reference_symbol.into(),
                fee_ratio,
            )
            .map_err(|e| e.error)?;
        }
        (Some(_), SyntheticExistence::RegisterNewAsset { .. }) => {
            return Err(Error::<T>::AssetAlreadyExists.into())
        }
        (None, SyntheticExistence::AlreadyExists) => {
            return Err(Error::<T>::UnknownSynthetic.into())
        }
    }
    Ok(resulting_quote)
}

pub(crate) fn initialize_synthetics<T: Config>(
    inputs: Vec<SyntheticInput<AssetIdOf<T>, <T as Config>::Symbol>>,
    relayer: T::AccountId,
) -> Result<Vec<SyntheticOutput<AssetIdOf<T>>>, DispatchError> {
    if !inputs.is_empty() {
        if !band::Pallet::<T>::trusted_relayers().is_some_and(|t| t.contains(&relayer)) {
            band::Pallet::<T>::add_relayers(RawOrigin::Root.into(), vec![relayer.clone()])
                .map_err(|e| e.error)?;
        };
        if !oracle_proxy::Pallet::<T>::enabled_oracles().contains(&Oracle::BandChainFeed) {
            oracle_proxy::Pallet::<T>::enable_oracle(RawOrigin::Root.into(), Oracle::BandChainFeed)
                .map_err(|e| e.error)?;
        }
    }
    let mut synthetic_init_results = vec![];
    for synthetic in inputs {
        synthetic_init_results.push(initialize_single_synthetic::<T>(
            synthetic,
            relayer.clone(),
        )?)
    }
    Ok(synthetic_init_results)
}
