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

/// Working with different liquidity sources
pub mod liquidity_sources {
    use crate::pallet_tools;
    use crate::pallet_tools::mcbc::TbcdCollateralInput;
    use crate::pallet_tools::price_tools::AssetPrices;
    use crate::Config;
    use assets::AssetIdOf;
    use common::DexIdOf;
    use frame_support::dispatch::{DispatchError, DispatchResult};
    use frame_support::ensure;
    use frame_system::pallet_prelude::BlockNumberFor;
    use order_book::{MomentOf, OrderBookId};
    use pallet_tools::mcbc::{BaseSupply, OtherCollateralInput};
    use pallet_tools::pool_xyk::AssetPairInput;
    use pallet_tools::xst::{BaseInput, SyntheticInput, SyntheticOutput};
    use sp_std::vec::Vec;
    use std::collections::BTreeMap;

    pub fn initialize_xyk<T: Config + pool_xyk::Config>(
        caller: T::AccountId,
        pairs: Vec<AssetPairInput<DexIdOf<T>, AssetIdOf<T>>>,
    ) -> Result<Vec<AssetPairInput<DexIdOf<T>, AssetIdOf<T>>>, DispatchError> {
        pallet_tools::pool_xyk::initialize::<T>(caller, pairs)
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
    pub fn create_and_fill_order_book_batch<T: Config>(
        bids_owner: T::AccountId,
        asks_owner: T::AccountId,
        settings: Vec<(
            OrderBookId<T::AssetId, T::DEXId>,
            pallet_tools::order_book::OrderBookAttributes,
            pallet_tools::order_book::FillInput<MomentOf<T>, BlockNumberFor<T>>,
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
        pallet_tools::order_book::create_empty_batch_unchecked::<T>(creation_settings)?;

        let orders_settings: Vec<_> = settings
            .into_iter()
            .map(|(id, _, fill_settings)| (id, fill_settings))
            .collect();
        pallet_tools::order_book::fill_batch_unchecked::<T>(
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
    pub fn fill_order_book<T: Config>(
        bids_owner: T::AccountId,
        asks_owner: T::AccountId,
        settings: Vec<(
            OrderBookId<T::AssetId, T::DEXId>,
            pallet_tools::order_book::FillInput<MomentOf<T>, BlockNumberFor<T>>,
        )>,
    ) -> DispatchResult {
        for (order_book_id, _) in settings.iter() {
            ensure!(
                order_book::OrderBooks::<T>::contains_key(order_book_id),
                crate::Error::<T>::CannotFillUnknownOrderBook
            );
        }
        pallet_tools::order_book::fill_batch_unchecked::<T>(bids_owner, asks_owner, settings)?;
        Ok(())
    }

    /// Initialize xst liquidity source. Can both update prices of base assets and synthetics.
    ///
    /// ## Return
    ///
    /// Due to limited precision of fixed-point numbers, the requested price might not be precisely
    /// obtainable. Therefore, actual resulting price of synthetics is returned.
    pub fn initialize_xst<T: Config>(
        base: Option<BaseInput>,
        synthetics: Vec<SyntheticInput<T::AssetId, <T as Config>::Symbol>>,
        relayer: T::AccountId,
    ) -> Result<Vec<SyntheticOutput<T::AssetId>>, DispatchError> {
        if let Some(base_prices) = base {
            pallet_tools::xst::initialize_base_assets::<T>(base_prices)?;
        }
        pallet_tools::xst::initialize_synthetics::<T>(synthetics, relayer)
    }

    /// Initialize mcbc liquidity source.
    ///
    /// Parameters:
    /// - `base_supply`: Control supply of XOR,
    /// - `other_collaterals`: Variables related to arbitrary collateral-specific pricing,
    /// - `tbcd_collateral`: TBCD-specific pricing variables.
    pub fn initialize_mcbc<T: Config>(
        base_supply: Option<BaseSupply<T::AccountId>>,
        other_collaterals: Vec<OtherCollateralInput<T::AssetId>>,
        tbcd_collateral: Option<TbcdCollateralInput>,
    ) -> Result<BTreeMap<T::AssetId, AssetPrices>, DispatchError> {
        if let Some(base_supply) = base_supply {
            pallet_tools::mcbc::initialize_base_supply::<T>(base_supply)?;
        }

        // handle tbcd collateral first as it may initialize reference asset xor prices
        // (initialization of all collateral is dependant on these prices)
        let mut collateral_ref_prices = BTreeMap::new();
        if let Some(tbcd_collateral) = tbcd_collateral {
            let actual_ref_prices =
                pallet_tools::mcbc::initialize_tbcd_collateral::<T>(tbcd_collateral)?;
            if let Some(actual_ref_prices) = actual_ref_prices {
                collateral_ref_prices.insert(common::TBCD.into(), actual_ref_prices);
            }
        }
        for collateral_input in other_collaterals {
            let collateral_asset_id = collateral_input.asset;
            let actual_ref_prices =
                pallet_tools::mcbc::initialize_single_collateral::<T>(collateral_input)?;
            if let Some(actual_ref_prices) = actual_ref_prices {
                collateral_ref_prices.insert(collateral_asset_id, actual_ref_prices);
            }
        }
        Ok(collateral_ref_prices)
    }
}
