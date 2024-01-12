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
    use crate::{Config, Error, OrderBookFillSettings};
    use assets::AssetIdOf;
    use codec::{Decode, Encode};
    use common::prelude::BalanceUnit;
    use common::{
        balance, AssetInfoProvider, AssetName, AssetSymbol, Balance, DEXInfo, DexIdOf,
        DexInfoProvider, Oracle, PriceToolsPallet, PriceVariant, TradingPair, XOR,
    };
    use frame_support::dispatch::{
        DispatchError, DispatchResult, DispatchResultWithPostInfo, RawOrigin,
    };
    use frame_support::ensure;
    use frame_support::traits::Get;
    use frame_system::pallet_prelude::BlockNumberFor;
    use order_book::{MomentOf, OrderBookId};
    use sp_arithmetic::traits::CheckedMul;
    use sp_std::fmt::Debug;
    use sp_std::vec::Vec;

    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    #[scale_info(skip_type_params(T))]
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

    pub fn xyk<T: Config + pool_xyk::Config>(
        caller: T::AccountId,
        pairs: Vec<XYKPair<DexIdOf<T>, AssetIdOf<T>>>,
    ) -> DispatchResult {
        for XYKPair {
            dex_id,
            asset_a,
            asset_b,
            price,
        } in pairs
        {
            if <T as Config>::AssetInfoProvider::is_non_divisible(&asset_a)
                || <T as Config>::AssetInfoProvider::is_non_divisible(&asset_b)
            {
                return Err(Error::<T>::AssetsMustBeDivisible.into());
            }

            let dex_info = <T as Config>::DexInfoProvider::get_dex_info(&dex_id)?;
            let trading_pair = trading_pair_from_asset_ids::<T>(dex_info, asset_a, asset_b)
                .ok_or(pool_xyk::Error::<T>::BaseAssetIsNotMatchedWithAnyAssetArguments)?;

            if !trading_pair::Pallet::<T>::is_trading_pair_enabled(
                &dex_id,
                &trading_pair.base_asset_id,
                &trading_pair.target_asset_id,
            )? {
                trading_pair::Pallet::<T>::register_pair(
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

            let value_a: BalanceUnit = if asset_a == XOR.into() {
                balance!(1000000).into()
            } else {
                balance!(10000).into()
            };
            let price = BalanceUnit::divisible(price);
            let value_b = value_a
                .checked_mul(&price)
                .ok_or(Error::<T>::ArithmeticError)?;

            assets::Pallet::<T>::mint_unchecked(&asset_a, &caller, *value_a.balance())?;
            assets::Pallet::<T>::mint_unchecked(&asset_b, &caller, *value_b.balance())?;

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
        Ok(())
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

    /// Prices with 10^18 precision. Amount of the asset per 1 XOR. The same format as used
    /// in price tools.
    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    #[scale_info(skip_type_params(T))]
    pub struct XSTBaseXorSidePrices {
        /// Amount of synthetic base asset per XOR
        pub synthetic_base: Balance,
        /// Amount of reference asset per XOR
        pub reference: Balance,
    }

    /// Price initialization parameters of `xst`'s synthetic base asset (in terms of reference asset)
    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    #[scale_info(skip_type_params(T))]
    pub struct XSTBaseXorPrices {
        pub buy: XSTBaseXorSidePrices,
        pub sell: XSTBaseXorSidePrices,
    }

    /// Input for setting prices for xst base assets
    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    #[scale_info(skip_type_params(T))]
    pub struct XSTBaseSideInput {
        pub reference_per_synthetic_base: Balance,
        /// `None` - get existing price
        pub reference_per_xor: Option<Balance>,
    }

    impl XSTBaseSideInput {
        fn should_update_reference(&self) -> bool {
            self.reference_per_xor.is_some()
        }
    }

    /// Price initialization parameters of `xst`'s synthetic base asset (in terms of reference asset)
    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    #[scale_info(skip_type_params(T))]
    pub struct XSTBaseInput {
        pub buy: XSTBaseSideInput,
        pub sell: XSTBaseSideInput,
    }

    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    #[scale_info(skip_type_params(T))]
    pub enum XSTSyntheticExistence<Symbol> {
        AlreadyExists,
        RegisterNewAsset {
            symbol: AssetSymbol,
            name: AssetName,
            reference_symbol: Symbol,
            fee_ratio: common::Fixed,
        },
    }

    /// Buy/sell price discrepancy is determined for all synthetics in `xst` pallet by synthetic
    /// base (XST) asset prices;
    ///
    /// We can't control it granularly for each asset, so we just deduce it from the existing
    /// pricing and price provided for the given variant
    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    #[scale_info(skip_type_params(T))]
    pub struct XSTSyntheticInput<AssetId, Symbol> {
        pub asset_id: AssetId,
        /// how much DAI per unit of `asset_id`. Note that precision is 10^9, as used by band team.
        pub price_reference_per_asset: u64,
        // pub variant: PriceVariant,
        pub existence: XSTSyntheticExistence<Symbol>,
    }

    fn set_prices_in_price_tools<T: Config>(
        asset_id: &T::AssetId,
        price: Balance,
        variant: PriceVariant,
    ) -> DispatchResult {
        let _ = price_tools::Pallet::<T>::register_asset(asset_id);

        for _ in 0..price_tools::AVG_BLOCK_SPAN {
            price_tools::Pallet::<T>::incoming_spot_price_failure(asset_id, variant);
        }
        for _ in 0..31 {
            price_tools::Pallet::<T>::incoming_spot_price(asset_id, price, variant)?;
        }
        Ok(())
    }

    /// Returns prices for setting in `price_tools`
    fn calculate_xor_prices<T: Config>(
        input_prices: XSTBaseInput,
    ) -> Result<XSTBaseXorPrices, DispatchError> {
        // To obtain xor prices, these formulae should be followed:
        //
        // B:
        // (synthetic -buy-> reference) = (synthetic -buy-> xor) * (xor -buy-> reference) =
        // = (1 / (xor -sell-> synthetic)) * (xor -buy-> reference)
        //
        // S:
        // (synthetic -sell-> reference) = (synthetic -sell-> xor) * (xor -sell-> reference) =
        // = (1 / (xor -buy-> synthetic)) * (xor -sell-> reference)

        // Get known values from the formula:
        let synthetic_buy_reference = input_prices.buy.reference_per_synthetic_base;
        let xor_buy_reference = match input_prices.buy.reference_per_xor {
            Some(p) => p,
            None => price_tools::Pallet::<T>::get_average_price(
                &XOR.into(),
                &xst::ReferenceAssetId::<T>::get(),
                PriceVariant::Buy,
            )
            .map_err(|_| Error::<T>::ReferenceAssetPriceNotFound)?,
        };
        let synthetic_sell_reference = input_prices.sell.reference_per_synthetic_base;
        let xor_sell_reference = match input_prices.sell.reference_per_xor {
            Some(p) => p,
            None => price_tools::Pallet::<T>::get_average_price(
                &XOR.into(),
                &xst::ReferenceAssetId::<T>::get(),
                PriceVariant::Sell,
            )
            .map_err(|_| Error::<T>::ReferenceAssetPriceNotFound)?,
        };

        // B:
        // (synthetic -buy-> reference) = (xor -buy-> reference) / (xor -sell-> synthetic)
        //
        // known:
        // (synthetic -buy-> reference), (xor -buy-> reference)
        //
        // solving for unknown:
        // (xor -sell-> synthetic) = (xor -buy-> reference) / (synthetic -buy-> reference)
        let xor_sell_synthetic = xor_buy_reference / synthetic_buy_reference;

        // S:
        // (synthetic -sell-> reference) = (xor -sell-> reference) / (xor -buy-> synthetic)
        //
        // known:
        // (synthetic -sell-> reference), (xor -sell-> reference)
        //
        // solving for unknown:
        // (xor -buy-> synthetic) = (xor -sell-> reference) / (synthetic -sell-> reference)
        let xor_buy_synthetic = xor_sell_reference / synthetic_sell_reference;
        Ok(XSTBaseXorPrices {
            buy: XSTBaseXorSidePrices {
                synthetic_base: xor_buy_synthetic,
                reference: xor_buy_reference,
            },
            sell: XSTBaseXorSidePrices {
                synthetic_base: xor_sell_synthetic,
                reference: xor_sell_reference,
            },
        })
    }

    fn relay_symbol<T: Config>(
        symbol: <T as Config>::Symbol,
        relayer: T::AccountId,
        price_band: u64,
    ) -> DispatchResultWithPostInfo {
        let symbol = symbol.into();
        let latest_rate = band::Pallet::<T>::rates(&symbol);
        let resolve_time = latest_rate.map_or(0, |rate| rate.last_updated + 1);
        let request_id = latest_rate.map_or(0, |rate| rate.request_id + 1);
        band::Pallet::<T>::relay(
            RawOrigin::Signed(relayer).into(),
            vec![(symbol, price_band)].try_into().unwrap(),
            resolve_time,
            request_id,
        )
    }

    pub fn xst<T: Config + price_tools::Config>(
        base: Option<XSTBaseInput>,
        synthetics: Vec<XSTSyntheticInput<T::AssetId, <T as Config>::Symbol>>,
        relayer: T::AccountId,
    ) -> DispatchResult {
        if let Some(base_prices) = base {
            let synthetic_base_asset_id = <T as xst::Config>::GetSyntheticBaseAssetId::get();
            let reference_asset_id = xst::ReferenceAssetId::<T>::get();

            let should_update_reference_buy = base_prices.buy.should_update_reference();
            let should_update_reference_sell = base_prices.sell.should_update_reference();
            let xor_prices = calculate_xor_prices::<T>(base_prices)?;
            ensure!(
                xor_prices.buy.synthetic_base >= xor_prices.sell.synthetic_base
                    && xor_prices.buy.reference >= xor_prices.sell.reference,
                Error::<T>::BuyLessThanSell
            );
            set_prices_in_price_tools::<T>(
                &synthetic_base_asset_id,
                xor_prices.buy.synthetic_base,
                PriceVariant::Buy,
            )?;
            set_prices_in_price_tools::<T>(
                &synthetic_base_asset_id,
                xor_prices.sell.synthetic_base,
                PriceVariant::Sell,
            )?;
            if should_update_reference_buy {
                set_prices_in_price_tools::<T>(
                    &reference_asset_id,
                    xor_prices.buy.reference,
                    PriceVariant::Buy,
                )?;
            }
            if should_update_reference_sell {
                set_prices_in_price_tools::<T>(
                    &reference_asset_id,
                    xor_prices.sell.reference,
                    PriceVariant::Sell,
                )?;
            }
        }

        if !band::Pallet::<T>::trusted_relayers().is_some_and(|t| t.contains(&relayer)) {
            band::Pallet::<T>::add_relayers(RawOrigin::Root.into(), vec![relayer.clone()])
                .map_err(|e| e.error)?;
        };
        if !oracle_proxy::Pallet::<T>::enabled_oracles().contains(&Oracle::BandChainFeed) {
            oracle_proxy::Pallet::<T>::enable_oracle(RawOrigin::Root.into(), Oracle::BandChainFeed)
                .map_err(|e| e.error)?;
        }
        for synthetic in synthetics {
            match (
                xst::Pallet::<T>::enabled_synthetics(synthetic.asset_id),
                synthetic.existence,
            ) {
                (Some(info), XSTSyntheticExistence::AlreadyExists) => {
                    relay_symbol::<T>(
                        info.reference_symbol.into(),
                        relayer.clone(),
                        synthetic.price_reference_per_asset,
                    )
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
                    relay_symbol::<T>(
                        reference_symbol.clone(),
                        relayer.clone(),
                        synthetic.price_reference_per_asset,
                    )
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
        }
        Ok(())
    }
}
