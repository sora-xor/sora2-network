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

pub mod source_initialization {
    use crate::pallet_tools;
    use crate::{Config, Error};
    use assets::AssetIdOf;
    use codec::{Decode, Encode};
    use common::fixnum::ops::CheckedSub;
    use common::prelude::{BalanceUnit, QuoteAmount};
    use common::{
        balance, fixed, AssetInfoProvider, AssetName, AssetSymbol, Balance, DEXInfo, DexIdOf,
        DexInfoProvider, Fixed, Oracle, PriceVariant, TradingPair, TradingPairSourceManager, XOR,
    };
    use frame_support::dispatch::{
        DispatchError, DispatchResult, DispatchResultWithPostInfo, RawOrigin,
    };
    use frame_support::ensure;
    use frame_support::traits::Get;
    use frame_support::weights::Weight;
    use frame_system::pallet_prelude::BlockNumberFor;
    use order_book::{MomentOf, OrderBookId};
    use pallet_tools::price_tools::AssetPrices;
    use sp_arithmetic::traits::CheckedMul;
    use sp_std::fmt::Debug;
    use sp_std::vec;
    use sp_std::vec::Vec;

    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    pub struct XYKPair<DEXId, AssetId> {
        pub dex_id: DEXId,
        pub asset_a: AssetId,
        pub asset_b: AssetId,
        /// Price of `asset_a` in terms of `asset_b` (how much `asset_b` is needed to buy 1 `asset_a`)
        pub price: Balance,
    }

    impl<DEXId, AssetId> XYKPair<DEXId, AssetId> {
        // `price` - Price of `asset_a` in terms of `asset_b` (how much `asset_b` is needed to buy 1
        // `asset_a`)
        pub fn new(dex_id: DEXId, asset_a: AssetId, asset_b: AssetId, price: Balance) -> Self {
            Self {
                dex_id,
                asset_a,
                asset_b,
                price,
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
    pub fn xyk<T: Config + pool_xyk::Config>(
        caller: T::AccountId,
        pairs: Vec<XYKPair<DexIdOf<T>, AssetIdOf<T>>>,
    ) -> Result<Vec<XYKPair<DexIdOf<T>, AssetIdOf<T>>>, DispatchError> {
        let mut actual_prices = pairs.clone();
        for (
            XYKPair {
                dex_id,
                asset_a,
                asset_b,
                price: expected_price,
            },
            XYKPair {
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

            // Some magic numbers taken from existing init code
            // https://github.com/soramitsu/sora2-api-tests/blob/f590995abbd3b191a57b988ba3c10607a89d6f89/tests/testAccount/mintTokensForPairs.test.ts#L136
            let value_a: BalanceUnit = if asset_a == XOR.into() {
                balance!(1000000).into()
            } else {
                balance!(10000).into()
            };
            let price = BalanceUnit::divisible(expected_price);
            let value_b = value_a
                .checked_mul(&price)
                .ok_or(Error::<T>::ArithmeticError)?;

            assets::Pallet::<T>::mint_unchecked(&asset_a, &caller, *value_a.balance())?;
            assets::Pallet::<T>::mint_unchecked(&asset_b, &caller, *value_b.balance())?;

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

    /// Create multiple order books with parameters and fill them according to given parameters.
    ///
    /// Balance for placing the orders is minted automatically, trading pairs are created if needed.
    ///
    /// Parameters:
    /// - `bids_owner`: Creator of the buy orders placed on the order books,
    /// - `asks_owner`: Creator of the sell orders placed on the order books,
    /// - `settings`: Parameters for creation of the order book and placing the orders in each
    /// order book.
    pub fn order_book_create_and_fill<T: Config>(
        bids_owner: T::AccountId,
        asks_owner: T::AccountId,
        settings: Vec<(
            OrderBookId<T::AssetId, T::DEXId>,
            pallet_tools::order_book::settings::OrderBookAttributes,
            pallet_tools::order_book::settings::OrderBookFill<MomentOf<T>, BlockNumberFor<T>>,
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
        pallet_tools::order_book::create_multiple_empty_unchecked::<T>(creation_settings)?;

        let orders_settings: Vec<_> = settings
            .into_iter()
            .map(|(id, _, fill_settings)| (id, fill_settings))
            .collect();
        pallet_tools::order_book::fill_multiple_empty_unchecked::<T>(
            bids_owner,
            asks_owner,
            orders_settings,
        )?;
        Ok(())
    }

    /// Fill the order books according to given parameters.
    ///
    /// Balance for placing the orders is minted automatically.
    ///
    /// Parameters:
    /// - `bids_owner`: Creator of the buy orders placed on the order books,
    /// - `asks_owner`: Creator of the sell orders placed on the order books,
    /// - `settings`: Parameters for placing the orders in each order book.
    pub fn order_book_only_fill<T: Config>(
        bids_owner: T::AccountId,
        asks_owner: T::AccountId,
        settings: Vec<(
            OrderBookId<T::AssetId, T::DEXId>,
            pallet_tools::order_book::settings::OrderBookFill<MomentOf<T>, BlockNumberFor<T>>,
        )>,
    ) -> DispatchResult {
        for (order_book_id, _) in settings.iter() {
            ensure!(
                order_book::OrderBooks::<T>::contains_key(order_book_id),
                crate::Error::<T>::CannotFillUnknownOrderBook
            );
        }
        pallet_tools::order_book::fill_multiple_empty_unchecked::<T>(
            bids_owner, asks_owner, settings,
        )?;
        Ok(())
    }

    /// Prices with 10^18 precision. Amount of the asset per 1 XOR. The same format as used
    /// in price tools.
    #[derive(Clone, PartialEq, Eq, Debug)]
    pub struct XSTBaseXorPrices {
        pub synthetic_base: AssetPrices,
        pub reference: AssetPrices,
    }

    /// Price initialization parameters of `xst`'s synthetic base asset (in terms of reference asset)
    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    pub struct XSTBaseInput {
        pub reference_per_synthetic_base_buy: Balance,
        pub reference_per_synthetic_base_sell: Balance,
    }

    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    pub enum XSTSyntheticExistence<Symbol> {
        AlreadyExists,
        RegisterNewAsset {
            symbol: AssetSymbol,
            name: AssetName,
            reference_symbol: Symbol,
            fee_ratio: common::Fixed,
        },
    }

    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    pub enum XSTSyntheticQuoteDirection {
        SyntheticBaseToSynthetic,
        SyntheticToSyntheticBase,
    }

    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    pub struct XSTSyntheticQuote {
        pub direction: XSTSyntheticQuoteDirection,
        pub amount: QuoteAmount<Balance>,
        pub result: Balance,
    }

    /// Buy/sell price discrepancy is determined for all synthetics in `xst` pallet by synthetic
    /// base (XST) asset prices;
    ///
    /// We can't control it granularly for each asset, so we just deduce it from the existing
    /// pricing and price provided for the given variant
    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    pub struct XSTSyntheticInput<AssetId, Symbol> {
        pub asset_id: AssetId,
        /// Quote call with expected output.
        /// The initialization tries to set up pallets to achieve these values
        pub expected_quote: XSTSyntheticQuote,
        pub existence: XSTSyntheticExistence<Symbol>,
    }

    /// Resulting of initialization for `asset_id`.
    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    pub struct XSTSyntheticOutput<AssetId> {
        pub asset_id: AssetId,
        /// Quote call with output.
        /// Sometimes, due to fixed-point precision limitations the exact value cannot be
        /// reproduced exactly. This provides a way to get the actual result for further usage.
        pub quote_achieved: XSTSyntheticQuote,
    }

    /// Adapter for [`pallet_tools::price_tools::calculate_xor_prices`] to avoid confusion with
    /// assets.
    fn calculate_xor_prices<T: Config>(
        input_prices: XSTBaseInput,
        synthetic_base_asset_id: &T::AssetId,
        reference_asset_id: &T::AssetId,
    ) -> Result<XSTBaseXorPrices, DispatchError> {
        // B = reference
        // A = synthetic base
        let xor_prices = pallet_tools::price_tools::calculate_xor_prices::<T>(
            synthetic_base_asset_id,
            reference_asset_id,
            input_prices.reference_per_synthetic_base_buy,
            input_prices.reference_per_synthetic_base_sell,
        )?;
        Ok(XSTBaseXorPrices {
            synthetic_base: xor_prices.asset_a,
            reference: xor_prices.asset_b,
        })
    }

    fn relay_symbol<T: Config>(
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
        target_quote: &XSTSyntheticQuote,
    ) -> Result<u64, DispatchError> {
        // band price is `ref_per_synthetic`.
        // we need to get it from formulae in xst pallet.
        let synthetic_base_asset_id = <T as xst::Config>::GetSyntheticBaseAssetId::get();

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
                XSTSyntheticQuoteDirection::SyntheticBaseToSynthetic,
                QuoteAmount::WithDesiredInput {
                    desired_amount_in: amount_in,
                },
                amount_out,
            )
            | (
                XSTSyntheticQuoteDirection::SyntheticBaseToSynthetic,
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
                let ref_per_synthetic_base_sell =
                    BalanceUnit::divisible(xst::Pallet::<T>::reference_price(
                        &synthetic_base_asset_id,
                        PriceVariant::Sell,
                    )?);
                ref_per_synthetic_base_sell * BalanceUnit::divisible(amount_in)
                    / BalanceUnit::divisible(amount_out)
            }
            // buy
            // synthetic (xst***) -> synthetic base (xst)
            // synthetic base (also called main) - buy price, synthetic - no diff between buy/sell
            // (all prices in reference assets per this asset)
            (
                XSTSyntheticQuoteDirection::SyntheticToSyntheticBase,
                QuoteAmount::WithDesiredInput {
                    desired_amount_in: amount_in,
                },
                amount_out,
            )
            | (
                XSTSyntheticQuoteDirection::SyntheticToSyntheticBase,
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
                let ref_per_synthetic_base_buy = BalanceUnit::divisible(
                    xst::Pallet::<T>::reference_price(&synthetic_base_asset_id, PriceVariant::Buy)?,
                );
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
        asset_id: T::AssetId,
        expected_quote: XSTSyntheticQuote,
        synthetic_band_price: u64,
    ) -> XSTSyntheticOutput<T::AssetId> {
        let ref_per_synthetic = synthetic_band_price as Balance * 10_u128.pow(9);
        let synthetic_base_asset_id = <T as xst::Config>::GetSyntheticBaseAssetId::get();
        // todo: pass as args
        let ref_per_synthetic_base_sell =
            xst::Pallet::<T>::reference_price(&synthetic_base_asset_id, PriceVariant::Sell)
                .unwrap();
        let ref_per_synthetic_base_buy =
            xst::Pallet::<T>::reference_price(&synthetic_base_asset_id, PriceVariant::Buy).unwrap();
        let actual_quote_result = match (&expected_quote.direction, &expected_quote.amount) {
            // sell:
            // synthetic base (xst) -> synthetic (xst***)
            // synthetic base (also called main) - sell price, synthetic - no diff between buy/sell
            // (all prices in reference assets per this asset)
            (
                XSTSyntheticQuoteDirection::SyntheticBaseToSynthetic,
                QuoteAmount::WithDesiredInput {
                    desired_amount_in: amount_in,
                },
            ) => {
                // amount_out = amount_in * ref_per_synthetic_base (sell) / ref_per_synthetic
                BalanceUnit::divisible(*amount_in)
                    * BalanceUnit::divisible(ref_per_synthetic_base_sell)
                    / BalanceUnit::divisible(ref_per_synthetic)
            }
            (
                XSTSyntheticQuoteDirection::SyntheticBaseToSynthetic,
                QuoteAmount::WithDesiredOutput {
                    desired_amount_out: amount_out,
                },
            ) => {
                // amount_in = amount_out * ref_per_synthetic / ref_per_synthetic_base (sell)
                BalanceUnit::divisible(*amount_out) * BalanceUnit::divisible(ref_per_synthetic)
                    / BalanceUnit::divisible(ref_per_synthetic_base_sell)
            }
            // buy
            // synthetic (xst***) -> synthetic base (xst)
            // synthetic base (also called main) - buy price, synthetic - no diff between buy/sell
            // (all prices in reference assets per this asset)
            (
                XSTSyntheticQuoteDirection::SyntheticToSyntheticBase,
                QuoteAmount::WithDesiredInput {
                    desired_amount_in: amount_in,
                },
            ) => {
                // amount_out = amount_in * ref_per_synthetic / ref_per_synthetic_base (buy)
                BalanceUnit::divisible(*amount_in) * BalanceUnit::divisible(ref_per_synthetic)
                    / BalanceUnit::divisible(ref_per_synthetic_base_buy)
            }
            (
                XSTSyntheticQuoteDirection::SyntheticToSyntheticBase,
                QuoteAmount::WithDesiredOutput {
                    desired_amount_out: amount_out,
                },
            ) => {
                // amount_in = amount_out * ref_per_synthetic_base (buy) / ref_per_synthetic
                BalanceUnit::divisible(*amount_out)
                    * BalanceUnit::divisible(ref_per_synthetic_base_buy)
                    / BalanceUnit::divisible(ref_per_synthetic)
            }
        };
        let actual_quote = XSTSyntheticQuote {
            result: *actual_quote_result.balance(),
            ..expected_quote
        };
        XSTSyntheticOutput {
            asset_id,
            quote_achieved: actual_quote,
        }
    }

    fn xst_base_assets<T: Config>(input: XSTBaseInput) -> DispatchResult {
        let synthetic_base_asset_id = <T as xst::Config>::GetSyntheticBaseAssetId::get();
        let reference_asset_id = xst::ReferenceAssetId::<T>::get();

        let xor_prices =
            calculate_xor_prices::<T>(input, &synthetic_base_asset_id, &reference_asset_id)?;
        ensure!(
            xor_prices.synthetic_base.buy >= xor_prices.synthetic_base.sell
                && xor_prices.reference.buy >= xor_prices.reference.sell,
            Error::<T>::BuyLessThanSell
        );
        pallet_tools::price_tools::set_price::<T>(
            &synthetic_base_asset_id,
            xor_prices.synthetic_base.buy,
            PriceVariant::Buy,
        )?;
        pallet_tools::price_tools::set_price::<T>(
            &synthetic_base_asset_id,
            xor_prices.synthetic_base.sell,
            PriceVariant::Sell,
        )?;
        Ok(())
    }

    fn xst_single_synthetic<T: Config>(
        input: XSTSyntheticInput<T::AssetId, <T as Config>::Symbol>,
        relayer: T::AccountId,
    ) -> Result<XSTSyntheticOutput<T::AssetId>, DispatchError> {
        let band_price = calculate_band_price::<T>(&input.expected_quote)?;
        let resulting_quote =
            calculate_actual_quote::<T>(input.asset_id, input.expected_quote, band_price);
        match (
            xst::Pallet::<T>::enabled_synthetics(input.asset_id),
            input.existence,
        ) {
            (Some(info), XSTSyntheticExistence::AlreadyExists) => {
                relay_symbol::<T>(info.reference_symbol.into(), relayer, band_price)
                    .map_err(|e| e.error)?;
            }
            (
                None,
                XSTSyntheticExistence::RegisterNewAsset {
                    symbol,
                    name,
                    reference_symbol,
                    fee_ratio,
                },
            ) => {
                relay_symbol::<T>(reference_symbol.clone(), relayer, band_price)
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
            (Some(_), XSTSyntheticExistence::RegisterNewAsset { .. }) => {
                return Err(Error::<T>::AssetAlreadyExists.into())
            }
            (None, XSTSyntheticExistence::AlreadyExists) => {
                return Err(Error::<T>::UnknownSynthetic.into())
            }
        }
        Ok(resulting_quote)
    }

    fn xst_synthetics<T: Config>(
        inputs: Vec<XSTSyntheticInput<T::AssetId, <T as Config>::Symbol>>,
        relayer: T::AccountId,
    ) -> Result<Vec<XSTSyntheticOutput<T::AssetId>>, DispatchError> {
        if !inputs.is_empty() {
            if !band::Pallet::<T>::trusted_relayers().is_some_and(|t| t.contains(&relayer)) {
                band::Pallet::<T>::add_relayers(RawOrigin::Root.into(), vec![relayer.clone()])
                    .map_err(|e| e.error)?;
            };
            if !oracle_proxy::Pallet::<T>::enabled_oracles().contains(&Oracle::BandChainFeed) {
                oracle_proxy::Pallet::<T>::enable_oracle(
                    RawOrigin::Root.into(),
                    Oracle::BandChainFeed,
                )
                .map_err(|e| e.error)?;
            }
        }
        let mut synthetic_init_results = vec![];
        for synthetic in inputs {
            synthetic_init_results.push(xst_single_synthetic::<T>(synthetic, relayer.clone())?)
        }
        Ok(synthetic_init_results)
    }

    /// Initialize xst liquidity source. Can both update prices of base assets and synthetics.
    ///
    /// ## Return
    ///
    /// Due to limited precision of fixed-point numbers, the requested price might not be precisely
    /// obtainable. Therefore, actual resulting price of synthetics is returned.
    ///
    /// `quote` in `xst` pallet requires swap to involve synthetic base asset, as well as
    pub fn xst<T: Config>(
        base: Option<XSTBaseInput>,
        synthetics: Vec<XSTSyntheticInput<T::AssetId, <T as Config>::Symbol>>,
        relayer: T::AccountId,
    ) -> Result<Vec<XSTSyntheticOutput<T::AssetId>>, DispatchError> {
        if let Some(base_prices) = base {
            xst_base_assets::<T>(base_prices)?;
        }
        xst_synthetics::<T>(synthetics, relayer)
    }
}
