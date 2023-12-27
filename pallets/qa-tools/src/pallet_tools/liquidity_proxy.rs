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
        balance, AssetInfoProvider, Balance, DEXInfo, DexIdOf, DexInfoProvider, PriceToolsPallet,
        PriceVariant, TradingPair, XOR,
    };
    use frame_support::dispatch::{DispatchError, DispatchResult, RawOrigin};
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
                base_asset_id: asset_a,
                target_asset_id: asset_b,
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
            let price = BalanceUnit::new(price, true);
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

    /// Prices with 10^18 precision in terms of XOR
    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    #[scale_info(skip_type_params(T))]
    pub struct XSTBaseXorPrices {
        /// Price of synthetic base asset in XOR
        pub synthetic_base: Balance,
        /// Price of reference asset in XOR
        pub reference: Balance,
    }

    /// Only buy or sell prices; with 10^18 precision
    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    #[scale_info(skip_type_params(T))]
    pub enum XSTBasePrices {
        /// Synthetic base asset - set price w.r.t. XOR
        ///
        /// Reference asset - set price w.r.t. XOR
        SetBoth(XSTBaseXorPrices),
        /// Synthetic base asset - set price w.r.t. reference asset
        ///
        /// Reference asset - set price w.r.t. XOR
        SetReferenceDeduceSyntheticBase {
            /// Price in reference asset
            synthetic_base: Balance,
            /// Price in XOR
            reference: Balance,
        },
        /// Synthetic base asset - set price w.r.t. reference asset
        ///
        /// Reference asset - do not touch; should have price in `price_tools` beforehand
        OnlyDeduceSyntheticBase {
            /// Price in reference asset
            synthetic_base: Balance,
        },
    }

    impl XSTBasePrices {
        pub fn synthetic_base_price(&self) -> Balance {
            match *self {
                XSTBasePrices::SetBoth(XSTBaseXorPrices { synthetic_base, .. }) => synthetic_base,
                XSTBasePrices::SetReferenceDeduceSyntheticBase { synthetic_base, .. } => {
                    synthetic_base
                }

                XSTBasePrices::OnlyDeduceSyntheticBase { synthetic_base } => synthetic_base,
            }
        }

        pub fn reference_price(&self) -> Option<Balance> {
            match *self {
                XSTBasePrices::SetBoth(XSTBaseXorPrices { reference, .. }) => Some(reference),
                XSTBasePrices::SetReferenceDeduceSyntheticBase { reference, .. } => Some(reference),

                XSTBasePrices::OnlyDeduceSyntheticBase { .. } => None,
            }
        }
    }

    /// Price initialization parameters of `xst`'s synthetic base asset (in terms of reference asset)
    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    #[scale_info(skip_type_params(T))]
    pub struct XSTBaseBuySellPrices {
        pub buy: XSTBasePrices,
        pub sell: XSTBasePrices,
    }

    /// Buy/sell price discrepancy is determined for all synthetics in `xst` pallet by synthetic
    /// base (XST) asset prices;
    ///
    /// We can't control it granularly for each asset, so we just deduce it from the existing
    /// pricing and price provided for the given variant
    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    #[scale_info(skip_type_params(T))]
    pub struct XSTSyntheticPrice {
        pub price: Balance,
        pub variant: PriceVariant,
    }

    fn set_prices_in_price_tools<T: Config>(
        asset_id: &T::AssetId,
        buy_price: Balance,
        sell_price: Balance,
    ) -> DispatchResult {
        if buy_price < sell_price {
            return Err(Error::<T>::BuyLessThanSell.into());
        }
        let _ = price_tools::Pallet::<T>::register_asset(asset_id);

        for _ in 0..price_tools::AVG_BLOCK_SPAN {
            price_tools::Pallet::<T>::incoming_spot_price_failure(asset_id, PriceVariant::Buy);
            price_tools::Pallet::<T>::incoming_spot_price_failure(asset_id, PriceVariant::Sell);
        }
        for _ in 0..31 {
            price_tools::Pallet::<T>::incoming_spot_price(asset_id, buy_price, PriceVariant::Buy)?;
            price_tools::Pallet::<T>::incoming_spot_price(
                asset_id,
                sell_price,
                PriceVariant::Sell,
            )?;
        }
        Ok(())
    }

    /// Returns resulting prices `(synthetic base, reference)` in XOR.
    fn calculate_xor_price<T: Config>(
        input_price: XSTBasePrices,
        variant: PriceVariant,
    ) -> Result<XSTBaseXorPrices, DispatchError> {
        let (synthetic_base_price, reference_price) = match input_price {
            XSTBasePrices::SetBoth(xor_price) => return Ok(xor_price),
            XSTBasePrices::SetReferenceDeduceSyntheticBase {
                synthetic_base,
                reference,
            } => (synthetic_base, reference),
            XSTBasePrices::OnlyDeduceSyntheticBase { synthetic_base } => {
                let reference = price_tools::Pallet::<T>::get_average_price(
                    &xst::ReferenceAssetId::<T>::get(),
                    &XOR.into(),
                    variant,
                )
                .map_err(|_| Error::<T>::ReferenceAssetPriceNotFound)?;
                (synthetic_base, reference)
            }
        };
        let synthetic_base_in_xor =
            BalanceUnit::new(synthetic_base_price, true) * BalanceUnit::new(reference_price, true);
        Ok(XSTBaseXorPrices {
            synthetic_base: *synthetic_base_in_xor.balance(),
            reference: reference_price,
        })
    }

    pub fn xst<T: Config + price_tools::Config>(
        base: Option<XSTBaseBuySellPrices>,
        synthetics: Vec<XSTSyntheticPrice>,
    ) -> DispatchResult {
        if let Some(base_prices) = base {
            let synthetic_base_asset_id = <T as xst::Config>::GetSyntheticBaseAssetId::get();
            let reference_asset_id = xst::ReferenceAssetId::<T>::get();

            let buy_prices = calculate_xor_price::<T>(base_prices.buy, PriceVariant::Buy)?;
            let sell_prices = calculate_xor_price::<T>(base_prices.sell, PriceVariant::Sell)?;
            ensure!(
                buy_prices.synthetic_base >= sell_prices.synthetic_base
                    && buy_prices.reference >= sell_prices.reference,
                Error::<T>::BuyLessThanSell
            );
            set_prices_in_price_tools::<T>(
                &synthetic_base_asset_id,
                buy_prices.synthetic_base,
                sell_prices.synthetic_base,
            )?;
            set_prices_in_price_tools::<T>(
                &reference_asset_id,
                buy_prices.reference,
                sell_prices.reference,
            )?;
        }
        Ok(())
    }
}
