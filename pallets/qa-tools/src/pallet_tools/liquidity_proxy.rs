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
    use crate::Config;
    use assets::AssetIdOf;
    use common::DexIdOf;
    use frame_support::dispatch::{DispatchError, DispatchResult};
    use frame_support::ensure;
    use frame_system::pallet_prelude::BlockNumberFor;
    use order_book::{MomentOf, OrderBookId};
    use pallet_tools::pool_xyk::XYKPair;
    use pallet_tools::xst::{XSTBaseInput, XSTSyntheticInput, XSTSyntheticOutput};
    use sp_std::vec::Vec;

    // todo: rename 'CAPS' to 'Caps'
    pub fn initialize_xyk<T: Config + pool_xyk::Config>(
        caller: T::AccountId,
        pairs: Vec<XYKPair<DexIdOf<T>, AssetIdOf<T>>>,
    ) -> Result<Vec<XYKPair<DexIdOf<T>, AssetIdOf<T>>>, DispatchError> {
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
    pub fn create_and_fill_order_book<T: Config>(
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
        pallet_tools::order_book::fill_multiple_unchecked::<T>(
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
            pallet_tools::order_book::settings::OrderBookFill<MomentOf<T>, BlockNumberFor<T>>,
        )>,
    ) -> DispatchResult {
        for (order_book_id, _) in settings.iter() {
            ensure!(
                order_book::OrderBooks::<T>::contains_key(order_book_id),
                crate::Error::<T>::CannotFillUnknownOrderBook
            );
        }
        pallet_tools::order_book::fill_multiple_unchecked::<T>(bids_owner, asks_owner, settings)?;
        Ok(())
    }

    /// Initialize xst liquidity source. Can both update prices of base assets and synthetics.
    ///
    /// ## Return
    ///
    /// Due to limited precision of fixed-point numbers, the requested price might not be precisely
    /// obtainable. Therefore, actual resulting price of synthetics is returned.
    ///
    /// `quote` in `xst` pallet requires swap to involve synthetic base asset, as well as
    pub fn initialize_xst<T: Config>(
        base: Option<XSTBaseInput>,
        synthetics: Vec<XSTSyntheticInput<T::AssetId, <T as Config>::Symbol>>,
        relayer: T::AccountId,
    ) -> Result<Vec<XSTSyntheticOutput<T::AssetId>>, DispatchError> {
        if let Some(base_prices) = base {
            pallet_tools::xst::xst_base_assets::<T>(base_prices)?;
        }
        pallet_tools::xst::xst_synthetics::<T>(synthetics, relayer)
    }

    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    pub struct MCBCPriceToolsPrice {
        pub buy: Option<Balance>,
        pub sell: Option<Balance>,
    }

    /// Input for initializing collateral assets except TBCD.
    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    pub struct MCBCCollateralInput<AssetId> {
        /// Collateral asset id
        pub asset: AssetId,
        /// Price of collateral in terms of reference asset. Linearly affects the exchange amounts.
        /// (if collateral costs 10x more sell output should be 10x smaller)
        pub ref_prices: MCBCPriceToolsPrice,
        /// Desired amount of collateral asset in the MCBC reserve account. Affects actual sell
        /// price according to formulae.
        pub reserves: Balance,
    }

    /// Input for initializing TBCD collateral.
    #[derive(Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, Debug)]
    pub struct MCBCTBCDInput<AssetId> {
        /// Collateral asset id
        pub asset: AssetId,
        /// Price of collateral in terms of reference asset. Linearly affects the exchange amounts.
        /// (if collateral costs 10x more sell output should be 10x smaller)
        pub ref_prices: MCBCPriceToolsPrice,
        /// Desired amount of collateral asset in the MCBC reserve account. Affects actual sell
        /// price according to formulae.
        pub reserves: Balance,
        pub xor_ref_prices: MCBCPriceToolsPrice,
    }

    pub struct MCBCBaseSupply<AccountId> {
        pub base_supply_collector: AccountId,
        pub new_base_supply: Balance,
    }

    fn init_single_mcbc_collateral<T: Config>(
        input: MCBCCollateralInput<T::AssetId>,
    ) -> DispatchResult {
        // initialize price???

        // initialize reserves

        // let base_asset = T::GetBaseAssetId::get();
        // let reference_asset = multicollateral_bonding_curve_pool::Pallet::<T>::reference_asset_id();
        // let total_issuance = assets::Pallet::<T>::total_issuance(&base_asset)?;
        // todo: register TP if not exist
        // TradingPair::register(
        //     RuntimeOrigin::signed(alice()),
        //     DEXId::Polkaswap.into(),
        //     XOR,
        //     VAL,
        // )
        // .expect("Failed to register trading pair.");
        // TradingPair::register(
        //     RuntimeOrigin::signed(alice()),
        //     DEXId::Polkaswap.into(),
        //     XOR,
        //     XSTUSD,
        // )
        // .expect("Failed to register trading pair.");

        // todo: initialize pool if not already
        // MBCPool::initialize_pool_unchecked(VAL, false).expect("Failed to initialize pool.");

        // todo: register account if not present???
        // let bonding_curve_tech_account_id = TechAccountId::Pure(
        //     DEXId::Polkaswap,
        //     TechPurpose::Identifier(b"bonding_curve_tech_account_id".to_vec()),
        // );
        // Technical::register_tech_account_id(bonding_curve_tech_account_id.clone())?;
        // MBCPool::set_reserves_account_id(bonding_curve_tech_account_id.clone())?;

        // set price_tools prices if needed
        if let Some(price) = input.ref_prices.buy {
            set_prices_in_price_tools::<T>(&input.asset, price, PriceVariant::Buy)?;
        }
        if let Some(price) = input.ref_prices.sell {
            set_prices_in_price_tools::<T>(&input.asset, price, PriceVariant::Sell)?;
        }

        // todo: use traits where possible (not only here, in whole pallet)
        // let reserve_amount_expected = FixedWrapper::from(total_issuance)
        //     * multicollateral_bonding_curve_pool::Pallet::<T>::sell_function(
        //         &base_asset,
        //         &input.asset,
        //         Fixed::ZERO,
        //     )?;

        // let pool_reference_amount = reserve_amount_expected * ratio;
        // let pool_reference_amount = pool_reference_amount
        //     .try_into_balance()
        //     .map_err(|_| Error::<T>::ArithmeticError)?;
        // let pool_val_amount = <T as Config>::LiquidityProxy::quote(
        //     DEXId::Polkaswap.into(),
        //     &reference_asset,
        //     &input.asset,
        //     QuoteAmount::with_desired_input(pool_reference_amount),
        //     LiquiditySourceFilter::empty(DEXId::Polkaswap.into()),
        //     true,
        // )?;

        // let reserves_account =
        //     multicollateral_bonding_curve_pool::Pallet::<T>::reserves_account_id();
        // technical::Pallet::<T>::mint(&input.asset, &reserves_account, pool_val_amount.amount)?;

        Ok(())
    }

    fn init_tbcd_mcbc_collateral<T: Config>(input: MCBCTBCDInput<T::AssetId>) -> DispatchResult {
        // handle xor ref price
        // input.xor_ref_prices

        init_single_mcbc_collateral::<T>(MCBCCollateralInput {
            asset: input.asset,
            ref_prices: input.ref_prices,
            reserves: input.reserves,
        })
    }

    fn init_mcbc_base_supply<T: Config>(input: MCBCBaseSupply<T::AccountId>) -> DispatchResult {
        let base_asset_id = &T::GetBaseAssetId::get();
        let current_base_supply: FixedWrapper =
            assets::Pallet::<T>::total_issuance(base_asset_id)?.into();
        let supply_delta = input.new_base_supply - current_base_supply;
        let supply_delta = supply_delta
            .get()
            .map_err(|_| Error::<T>::ArithmeticError)?
            .into_bits();

        // realistically the error should never be triggered
        let owner =
            assets::Pallet::<T>::asset_owner(&base_asset_id).ok_or(Error::<T>::UnknownMCBCAsset)?;
        if supply_delta > 0 {
            let mint_amount = supply_delta
                .try_into()
                .map_err(|_| Error::<T>::ArithmeticError)?;
            assets::Pallet::<T>::mint_to(
                base_asset_id,
                &owner,
                &input.base_supply_collector,
                mint_amount,
            )?;
        } else if supply_delta < 0 {
            let burn_amount = supply_delta
                .abs()
                .try_into()
                .map_err(|_| Error::<T>::ArithmeticError)?;
            assets::Pallet::<T>::burn_from(
                base_asset_id,
                &owner,
                &input.base_supply_collector,
                burn_amount,
            )?;
        }
        Ok(())
    }

    pub fn mcbc<T: Config>(
        base_supply: Option<MCBCBaseSupply<T::AccountId>>,
        other_collaterals: Vec<MCBCCollateralInput<T::AssetId>>,
        tbcd_collateral: Option<MCBCTBCDInput<T::AssetId>>,
    ) -> DispatchResult {
        if let Some(base_supply) = base_supply {
            init_mcbc_base_supply::<T>(base_supply)?;
        }

        for collateral_input in other_collaterals {
            init_single_mcbc_collateral::<T>(collateral_input)?;
        }
        if let Some(tbcd_collateral) = tbcd_collateral {
            init_tbcd_mcbc_collateral::<T>(tbcd_collateral)?;
        }
        Ok(())
    }
}
