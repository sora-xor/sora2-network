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

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod test_utils;

pub mod migrations;

use core::convert::TryInto;

use assets::AssetIdOf;
use codec::{Decode, Encode};
use common::fixnum::ops::Zero as _;
use common::prelude::{
    Balance, EnsureDEXManager, Fixed, FixedWrapper, OutcomeFee, PriceToolsProvider, QuoteAmount,
    SwapAmount, SwapOutcome, DEFAULT_BALANCE_PRECISION,
};
use common::{
    balance, fixed, fixed_wrapper, AssetId32, AssetInfoProvider, AssetName, AssetSymbol, DEXId,
    DataFeed, GetMarketInfo, LiquiditySource, LiquiditySourceType, OnSymbolDisabled, PriceVariant,
    Rate, RewardReason, SwapChunk, SyntheticInfoProvider, TradingPairSourceManager, XSTUSD,
};
use frame_support::pallet_prelude::DispatchResult;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use frame_support::{ensure, fail, RuntimeDebug};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::Zero;
use sp_runtime::DispatchError;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::collections::vec_deque::VecDeque;
use sp_std::vec;
use sp_std::vec::Vec;

pub use weights::WeightInfo;

type Assets<T> = assets::Pallet<T>;
type Technical<T> = technical::Pallet<T>;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"xst-pool";
pub const TECH_ACCOUNT_PERMISSIONED: &[u8] = b"permissioned";

pub use pallet::*;

#[derive(Debug, Encode, Decode, Clone, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum DistributionAccount<AccountId, TechAccountId> {
    Account(AccountId),
    TechAccount(TechAccountId),
}

impl<AccountId, TechAccountId: Default> Default for DistributionAccount<AccountId, TechAccountId> {
    fn default() -> Self {
        Self::TechAccount(TechAccountId::default())
    }
}

#[derive(Debug, Encode, Decode, Clone, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct DistributionAccountData<DistributionAccount> {
    pub account: DistributionAccount,
    pub coefficient: Fixed,
}

impl<DistributionAccount: Default> Default for DistributionAccountData<DistributionAccount> {
    fn default() -> Self {
        Self {
            account: Default::default(),
            coefficient: Default::default(),
        }
    }
}

impl<DistributionAccount> DistributionAccountData<DistributionAccount> {
    pub fn new(account: DistributionAccount, coefficient: Fixed) -> Self {
        DistributionAccountData {
            account,
            coefficient,
        }
    }
}

#[derive(RuntimeDebug, Clone, Encode, Decode, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct SyntheticInfo<Symbol> {
    pub reference_symbol: Symbol,
    /// Fee ratio. 1 = 100%
    pub fee_ratio: Fixed,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::traits::StorageVersion;
    use frame_support::{pallet_prelude::*, Parameter};
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + technical::Config + common::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// AssetId which is convertible to/from XSTUSD
        type GetSyntheticBaseAssetId: Get<Self::AssetId>;
        type GetXSTPoolPermissionedTechAccountId: Get<Self::TechAccountId>;
        type EnsureDEXManager: EnsureDEXManager<Self::DEXId, Self::AccountId, DispatchError>;
        type PriceToolsPallet: PriceToolsProvider<Self::AssetId>;
        type Oracle: DataFeed<Self::Symbol, Rate, u64>;
        /// Type of symbol received from oracles
        type Symbol: Parameter + From<common::SymbolName> + MaybeSerializeDeserialize;
        /// Maximum tradable amount of XST
        #[pallet::constant]
        type GetSyntheticBaseBuySellLimit: Get<Balance>;
        type TradingPairSourceManager: TradingPairSourceManager<Self::DEXId, Self::AssetId>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Change reference asset which is used to determine collateral assets value.
        /// Intended to be e.g., stablecoin DAI.
        ///
        /// - `origin`: the sudo account on whose behalf the transaction is being executed,
        /// - `reference_asset_id`: asset id of the new reference asset.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::set_reference_asset())]
        pub fn set_reference_asset(
            origin: OriginFor<T>,
            reference_asset_id: T::AssetId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            Assets::<T>::ensure_asset_exists(&reference_asset_id)?;
            ensure!(
                !Assets::<T>::is_non_divisible(&reference_asset_id),
                Error::<T>::IndivisibleReferenceAsset
            );

            ReferenceAssetId::<T>::put(reference_asset_id.clone());
            Self::deposit_event(Event::ReferenceAssetChanged(reference_asset_id));
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::enable_synthetic_asset())]
        pub fn enable_synthetic_asset(
            origin: OriginFor<T>,
            asset_id: T::AssetId,
            reference_symbol: T::Symbol,
            fee_ratio: Fixed,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            Self::enable_synthetic_asset_unchecked(asset_id, reference_symbol, fee_ratio, true)?;
            Ok(().into())
        }

        /// Register and enable new synthetic asset with `reference_symbol` price binding
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::register_synthetic_asset())]
        pub fn register_synthetic_asset(
            origin: OriginFor<T>,
            asset_symbol: AssetSymbol,
            asset_name: AssetName,
            reference_symbol: T::Symbol,
            fee_ratio: Fixed,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let synthetic_asset_id: T::AssetId =
                AssetId32::<common::PredefinedAssetId>::from_synthetic_reference_symbol(
                    &reference_symbol,
                )
                .into();

            Self::register_synthetic_asset_unchecked(synthetic_asset_id, asset_symbol, asset_name)?;
            Self::enable_synthetic_asset_unchecked(
                synthetic_asset_id,
                reference_symbol,
                fee_ratio,
                true,
            )?;

            Ok(().into())
        }

        /// Disable synthetic asset.
        ///
        /// Removes synthetic from exchanging
        /// and removes XSTPool liquidity source for corresponding trading pair.
        ///
        /// - `origin`: the sudo account on whose behalf the transaction is being executed,
        /// - `synthetic_asset`: synthetic asset id to disable.
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::disable_synthetic_asset())]
        pub fn disable_synthetic_asset(
            origin: OriginFor<T>,
            synthetic_asset: T::AssetId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            ensure!(
                Self::enabled_synthetics(synthetic_asset).is_some(),
                Error::<T>::SyntheticIsNotEnabled
            );
            Self::disable_synthetic_asset_unchecked(synthetic_asset)?;
            Ok(().into())
        }

        /// Entirely remove synthetic asset (including linked symbol info)
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::remove_synthetic_asset())]
        pub fn remove_synthetic_asset(
            origin: OriginFor<T>,
            synthetic_asset: T::AssetId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let reference_symbol = EnabledSynthetics::<T>::get(synthetic_asset)
                .ok_or_else(|| Error::<T>::SyntheticIsNotEnabled)?
                .reference_symbol;

            EnabledSynthetics::<T>::remove(synthetic_asset);
            EnabledSymbols::<T>::remove(&reference_symbol);

            Self::deposit_event(Event::SyntheticAssetRemoved(
                synthetic_asset,
                reference_symbol,
            ));
            Ok(().into())
        }

        /// Set synthetic asset fee.
        ///
        /// This fee will be used to determine the amount of synthetic base asset (e.g. XST) to be
        /// burned when user buys synthetic asset.
        ///
        /// - `origin`: the sudo account on whose behalf the transaction is being executed,
        /// - `synthetic_asset`: synthetic asset id to set fee for,
        /// - `fee_ratio`: fee ratio with precision = 18, so 1000000000000000000 = 1 = 100% fee.
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::set_synthetic_asset_fee())]
        pub fn set_synthetic_asset_fee(
            origin: OriginFor<T>,
            synthetic_asset: T::AssetId,
            fee_ratio: Fixed,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            ensure!(
                fee_ratio >= fixed!(0) && fee_ratio < fixed!(1),
                Error::<T>::InvalidFeeRatio
            );

            EnabledSynthetics::<T>::try_mutate(
                &synthetic_asset,
                |option_info| -> DispatchResult {
                    let info = option_info
                        .as_mut()
                        .ok_or(Error::<T>::SyntheticIsNotEnabled)?;
                    info.fee_ratio = fee_ratio;
                    Ok(())
                },
            )?;

            Self::deposit_event(Event::SyntheticAssetFeeChanged(synthetic_asset, fee_ratio));
            Ok(().into())
        }

        /// Set floor price for the synthetic base asset
        ///
        /// - `origin`: root account
        /// - `floor_price`: floor price for the synthetic base asset
        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::set_synthetic_base_asset_floor_price())]
        pub fn set_synthetic_base_asset_floor_price(
            origin: OriginFor<T>,
            floor_price: Balance,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            SyntheticBaseAssetFloorPrice::<T>::put(floor_price);
            Self::deposit_event(Event::SyntheticBaseAssetFloorPriceChanged(floor_price));
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Reference Asset has been changed for pool. [New Reference Asset Id]
        ReferenceAssetChanged(AssetIdOf<T>),
        /// Synthetic asset has been enabled. [Synthetic Asset Id, Reference Symbol]
        SyntheticAssetEnabled(AssetIdOf<T>, T::Symbol),
        /// Synthetic asset has been disabled. [Synthetic Asset Id]
        SyntheticAssetDisabled(AssetIdOf<T>),
        /// Synthetic asset fee has been changed. [Synthetic Asset Id, New Fee]
        SyntheticAssetFeeChanged(AssetIdOf<T>, Fixed),
        /// Floor price of the synthetic base asset has been changed. [New Floor Price]
        SyntheticBaseAssetFloorPriceChanged(Balance),
        /// Synthetic asset has been removed. [Synthetic Asset Id, Reference Symbol]
        SyntheticAssetRemoved(AssetIdOf<T>, T::Symbol),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// An error occurred while calculating the price.
        PriceCalculationFailed,
        /// Indicated limits for slippage has not been met during transaction execution.
        SlippageLimitExceeded,
        /// Liquidity source can't exchange assets with the given IDs on the given DEXId.
        CantExchange,
        /// Synthetic asset does not exist.
        SyntheticDoesNotExist,
        /// Attempt to enable synthetic asset with inexistent symbol.
        SymbolDoesNotExist,
        /// Attempt to enable synthetic asset with symbol
        /// that is already referenced to another synthetic.
        SymbolAlreadyReferencedToSynthetic,
        /// Attempt to disable synthetic asset that is not enabled.
        SyntheticIsNotEnabled,
        /// Error quoting price from oracle.
        OracleQuoteError,
        /// Invalid fee ratio value.
        InvalidFeeRatio,
        /// Reference asset must be divisible
        IndivisibleReferenceAsset,
        /// Synthetic asset must be divisible
        CantEnableIndivisibleAsset,
        /// Input/output amount of synthetic base asset exceeds the limit
        SyntheticBaseBuySellLimitExceeded,
    }

    /// Synthetic assets and their reference symbols.
    ///
    /// It's a programmer responsibility to keep this collection consistent with [`EnabledSymbols`].
    #[pallet::storage]
    #[pallet::getter(fn enabled_synthetics)]
    pub type EnabledSynthetics<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AssetId, SyntheticInfo<T::Symbol>, OptionQuery>;

    /// Reference symbols and their synthetic assets.
    ///
    /// It's a programmer responsibility to keep this collection consistent with [`EnabledSynthetics`].
    #[pallet::storage]
    #[pallet::getter(fn enabled_symbols)]
    pub type EnabledSymbols<T: Config> =
        StorageMap<_, Blake2_128Concat, T::Symbol, T::AssetId, OptionQuery>;

    /// Asset that is used to compare collateral assets by value, e.g., DAI.
    #[pallet::storage]
    #[pallet::getter(fn reference_asset_id)]
    pub type ReferenceAssetId<T: Config> = StorageValue<_, T::AssetId, ValueQuery>;

    /// Current reserves balance for collateral tokens, used for client usability.
    #[pallet::storage]
    pub(super) type CollateralReserves<T: Config> =
        StorageMap<_, Twox64Concat, T::AssetId, Balance, ValueQuery>;

    #[pallet::type_value]
    pub fn SyntheticBaseAssetDefaultFloorPrice() -> Balance {
        balance!(0.0001)
    }

    /// Floor price for the synthetic base asset.
    #[pallet::storage]
    #[pallet::getter(fn synthetic_base_asset_floor_price)]
    pub type SyntheticBaseAssetFloorPrice<T: Config> =
        StorageValue<_, Balance, ValueQuery, SyntheticBaseAssetDefaultFloorPrice>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        /// Asset that is used to compare collateral assets by value, e.g., DAI.
        pub reference_asset_id: T::AssetId,
        /// List of tokens enabled as collaterals initially.
        /// TODO: replace with Vec<T::AssetId> and make corresponding changes to build() function
        pub initial_synthetic_assets: Vec<(T::AssetId, T::Symbol, Fixed)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                reference_asset_id: common::DAI.into(),
                initial_synthetic_assets: [(
                    XSTUSD.into(),
                    common::SymbolName::usd().into(),
                    common::fixed!(0.00666),
                )]
                .into(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            ReferenceAssetId::<T>::put(&self.reference_asset_id);

            self.initial_synthetic_assets.iter().cloned().for_each(
                |(asset_id, reference_symbol, fee_ratio)| {
                    Pallet::<T>::enable_synthetic_asset_unchecked(
                        asset_id,
                        reference_symbol,
                        fee_ratio,
                        false,
                    )
                    .expect("Failed to initialize XST synthetics.")
                },
            );
        }
    }
}

impl<T: Config> Pallet<T> {
    fn enable_synthetic_asset_unchecked(
        synthetic_asset_id: T::AssetId,
        reference_symbol: T::Symbol,
        fee_ratio: Fixed,
        transactional: bool,
    ) -> sp_runtime::DispatchResult {
        let code = || {
            ensure!(
                fee_ratio >= fixed!(0) && fee_ratio < fixed!(1),
                Error::<T>::InvalidFeeRatio
            );
            Self::ensure_symbol_exists(&reference_symbol)?;

            Assets::<T>::ensure_asset_exists(&synthetic_asset_id)?;
            ensure!(
                Assets::<T>::get_asset_info(&synthetic_asset_id).2 != 0,
                Error::<T>::CantEnableIndivisibleAsset
            );

            Self::enable_synthetic_trading_pair(synthetic_asset_id)?;

            EnabledSynthetics::<T>::insert(
                synthetic_asset_id,
                SyntheticInfo {
                    reference_symbol: reference_symbol.clone(),
                    fee_ratio,
                },
            );

            match Self::enabled_symbols(&reference_symbol) {
                Some(asset_id) => {
                    if asset_id != synthetic_asset_id {
                        Err(Error::<T>::SymbolAlreadyReferencedToSynthetic)
                    } else {
                        Ok(())
                    }
                }
                None => {
                    EnabledSymbols::<T>::insert(reference_symbol.clone(), synthetic_asset_id);
                    Ok(())
                }
            }?;

            Self::deposit_event(Event::SyntheticAssetEnabled(
                synthetic_asset_id,
                reference_symbol,
            ));
            Ok(().into())
        };

        if transactional {
            common::with_transaction(|| code())
        } else {
            code()
        }
    }

    fn enable_synthetic_trading_pair(synthetic_asset_id: T::AssetId) -> sp_runtime::DispatchResult {
        if T::TradingPairSourceManager::is_trading_pair_enabled(
            &DEXId::Polkaswap.into(),
            &T::GetSyntheticBaseAssetId::get(),
            &synthetic_asset_id,
        )? {
            return Ok(());
        }

        T::TradingPairSourceManager::register_pair(
            DEXId::Polkaswap.into(),
            T::GetSyntheticBaseAssetId::get(),
            synthetic_asset_id,
        )?;

        T::TradingPairSourceManager::enable_source_for_trading_pair(
            &DEXId::Polkaswap.into(),
            &T::GetSyntheticBaseAssetId::get(),
            &synthetic_asset_id,
            LiquiditySourceType::XSTPool,
        )?;

        Ok(())
    }

    /// Calculates and returns the buying amount of synthetic main asset if the selling amount of synthetic asset is provided.
    /// In case if buying amount of synthetic main asset is provided, returns selling amount of synthetic asset.
    ///
    /// ## Amount calculation
    /// ### Main asset price calculation
    /// main_asset_price is calculated by [`Self::reference_price`] supplied with Buy variant.
    ///
    /// amount_in is the amount of the synthetic asset we want to sell
    /// amount_out is calculated and it is the amount of the synthetic main asset we want to buy
    ///
    /// ### desired_amount_in variant
    /// returns amount_out = amount_in * synthetic_asset_price / main_asset_price
    ///
    /// ### desired_amount_out variant
    /// returns amount_in = amount_out * main_asset_price / synthetic_asset_price
    pub fn buy_price(
        main_asset_id: &T::AssetId,
        synthetic_asset_id: &T::AssetId,
        quantity: QuoteAmount<Balance>,
    ) -> Result<Fixed, DispatchError> {
        let main_asset_price: FixedWrapper =
            Self::reference_price(main_asset_id, PriceVariant::Buy)?.into();
        let synthetic_asset_price: FixedWrapper =
            Self::reference_price(synthetic_asset_id, PriceVariant::Sell)?.into();

        match quantity {
            // Input target amount of synthetic asset (e.g. XSTUSD) to get some synthetic base asset (e.g. XST)
            QuoteAmount::WithDesiredInput {
                desired_amount_in: synthetic_quantity,
            } => {
                let main_out = synthetic_quantity * synthetic_asset_price / main_asset_price;
                main_out
                    .get()
                    .map_err(|_| Error::<T>::PriceCalculationFailed.into())
                    .map(|value| value.max(Fixed::ZERO))
            }
            // Input some synthetic asset (e.g. XSTUSD) to get a target amount of synthetic base asset (e.g. XST)
            QuoteAmount::WithDesiredOutput {
                desired_amount_out: main_quantity,
            } => {
                let synthetic_quantity = main_quantity * main_asset_price / synthetic_asset_price;
                synthetic_quantity
                    .get()
                    .map_err(|_| Error::<T>::PriceCalculationFailed.into())
                    .map(|value| value.max(Fixed::ZERO))
            }
        }
    }

    /// Calculates and returns the selling amount of synthetic main asset if the buying amount of synthetic asset is provided.
    /// In case if selling amount of synthetic main asset is provided, returns buying amount of synthetic asset.
    ///
    /// ## Amount calculation
    /// ### Main asset price calculation
    /// main_asset_price is calculated by [`Self::reference_price`] supplied with Sell variant.
    ///
    /// amount_in is the amount of the synthetic asset we want to buy
    /// amount_out is calculated and it is the amount of the synthetic main asset we want to sell
    ///
    /// ### desired_amount_in variant
    /// returns amount_out = amount_in * main_asset_price / synthetic_asset_price
    ///
    /// ### desired_amount_out variant
    /// returns amount_in = amount_out * synthetic_asset_price / main_asset_price
    pub fn sell_price(
        main_asset_id: &T::AssetId,
        synthetic_asset_id: &T::AssetId,
        quantity: QuoteAmount<Balance>,
    ) -> Result<Fixed, DispatchError> {
        // Get reference prices for base and synthetic to understand token value.
        let main_asset_price: FixedWrapper =
            Self::reference_price(main_asset_id, PriceVariant::Sell)?.into();
        let synthetic_asset_price: FixedWrapper =
            Self::reference_price(synthetic_asset_id, PriceVariant::Buy)?.into();

        match quantity {
            // Sell desired amount of synthetic base asset (e.g. XST) for some synthetic asset (e.g. XSTUSD)
            QuoteAmount::WithDesiredInput {
                desired_amount_in: quantity_main,
            } => {
                let output_synthetic = quantity_main * main_asset_price / synthetic_asset_price;
                output_synthetic
                    .get()
                    .map_err(|_| Error::<T>::PriceCalculationFailed.into())
            }
            // Sell some amount of synthetic base asset (e.g. XST) for desired amount of synthetic asset (e.g. XSTUSD)
            QuoteAmount::WithDesiredOutput {
                desired_amount_out: quantity_synthetic,
            } => {
                let output_main = quantity_synthetic * synthetic_asset_price / main_asset_price;
                output_main
                    .get()
                    .map_err(|_| Error::<T>::PriceCalculationFailed.into())
            }
        }
    }

    /// Decompose SwapAmount into particular buy quotation query.
    ///
    /// "Buy quotation" means that we give `synthetic_asset_id` to buy/get
    /// `main_asset_id`. It means that `input_amount` is in `synthetic_asset_id`
    /// and `output_amount` is in main currency.
    ///
    /// In other words, swap direction is
    /// `synthetic_asset_id -> main_asset_id`
    ///
    /// Returns ordered pair: (input_amount, output_amount, fee_amount).
    fn decide_buy_amounts(
        main_asset_id: &T::AssetId,
        synthetic_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        deduce_fee: bool,
        check_limits: bool,
    ) -> Result<(Balance, Balance, Balance), DispatchError> {
        let fee_ratio = Self::get_aggregated_fee(synthetic_asset_id)?;

        Ok(match amount {
            QuoteAmount::WithDesiredInput { desired_amount_in } => {
                // Calculate how much `main_asset_id` we will buy (get)
                // if we give `desired_amount_in` of `synthetic_asset_id`
                ensure!(desired_amount_in != 0, Error::<T>::PriceCalculationFailed);
                let mut output_amount: Balance = FixedWrapper::from(Self::buy_price(
                    main_asset_id,
                    synthetic_asset_id,
                    QuoteAmount::with_desired_input(desired_amount_in),
                )?)
                .try_into_balance()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?;

                let fee_amount = if deduce_fee {
                    let fee_amount = (fee_ratio * output_amount)
                        .try_into_balance()
                        .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                    output_amount = output_amount.saturating_sub(fee_amount);
                    fee_amount
                } else {
                    0
                };
                Self::ensure_base_asset_amount_within_limit(output_amount, check_limits)?;

                (desired_amount_in, output_amount, fee_amount)
            }

            QuoteAmount::WithDesiredOutput { desired_amount_out } => {
                // Calculate how much `synthetic_asset_id` we need to give to buy (get)
                // `desired_amount_out` of `main_asset_id`
                ensure!(desired_amount_out != 0, Error::<T>::PriceCalculationFailed);
                Self::ensure_base_asset_amount_within_limit(desired_amount_out, check_limits)?;
                let desired_amount_out_with_fee = if deduce_fee {
                    (FixedWrapper::from(desired_amount_out) / (fixed_wrapper!(1) - fee_ratio))
                        .try_into_balance()
                        .map_err(|_| Error::<T>::PriceCalculationFailed)?
                } else {
                    desired_amount_out
                };
                let input_amount = Self::buy_price(
                    main_asset_id,
                    synthetic_asset_id,
                    QuoteAmount::with_desired_output(desired_amount_out_with_fee.clone()),
                )?;
                let input_amount = input_amount
                    .into_bits()
                    .try_into()
                    .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                (
                    input_amount,
                    desired_amount_out,
                    desired_amount_out_with_fee.saturating_sub(desired_amount_out),
                )
            }
        })
    }

    /// Decompose SwapAmount into particular sell quotation query.
    ///
    /// "Sell quotation" means that we sell/give `main_asset_id` to get
    /// `synthetic_asset_id`. It means that `input_amount` is in main
    /// currency and `output_amount` is in `synthetic_asset_id`.
    ///
    /// In other words, swap direction is
    /// `main_asset_id -> synthetic_asset_id`
    ///
    /// Returns ordered pair: `(input_amount, output_amount, fee_amount)`.
    fn decide_sell_amounts(
        main_asset_id: &T::AssetId,
        synthetic_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        deduce_fee: bool,
        check_limits: bool,
    ) -> Result<(Balance, Balance, Balance), DispatchError> {
        let fee_ratio = Self::get_aggregated_fee(synthetic_asset_id)?;

        Ok(match amount {
            QuoteAmount::WithDesiredInput { desired_amount_in } => {
                // Calculate how much `synthetic_asset_id` we will get
                // if we sell `desired_amount_in` of `main_asset_id`
                ensure!(desired_amount_in != 0, Error::<T>::PriceCalculationFailed);
                Self::ensure_base_asset_amount_within_limit(desired_amount_in, check_limits)?;
                let fee_amount = if deduce_fee {
                    (fee_ratio * FixedWrapper::from(desired_amount_in))
                        .try_into_balance()
                        .map_err(|_| Error::<T>::PriceCalculationFailed)?
                } else {
                    0
                };
                let output_amount = Self::sell_price(
                    main_asset_id,
                    synthetic_asset_id,
                    QuoteAmount::with_desired_input(
                        desired_amount_in.saturating_sub(fee_amount.clone()),
                    ),
                )?;
                let output_amount = output_amount
                    .into_bits()
                    .try_into()
                    .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                (desired_amount_in, output_amount, fee_amount)
            }
            QuoteAmount::WithDesiredOutput { desired_amount_out } => {
                // Calculate how much `main_asset_id` we need to sell to get
                // `desired_amount_out` of `synthetic_asset_id`
                ensure!(desired_amount_out != 0, Error::<T>::PriceCalculationFailed);
                let input_amount: Balance = FixedWrapper::from(Self::sell_price(
                    main_asset_id,
                    synthetic_asset_id,
                    QuoteAmount::with_desired_output(desired_amount_out),
                )?)
                .try_into_balance()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                let (input_amount_with_fee, fee) = if deduce_fee {
                    let input_amount_with_fee =
                        FixedWrapper::from(input_amount) / (fixed_wrapper!(1) - fee_ratio);
                    let input_amount_with_fee = input_amount_with_fee
                        .try_into_balance()
                        .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                    (
                        input_amount_with_fee,
                        input_amount_with_fee.saturating_sub(input_amount),
                    )
                } else {
                    (input_amount, 0)
                };
                Self::ensure_base_asset_amount_within_limit(input_amount_with_fee, check_limits)?;
                (input_amount_with_fee, desired_amount_out, fee)
            }
        })
    }

    /// This function is used by `exchange` function to burn `input_amount` derived from `amount` of `main_asset_id`
    /// and mint calculated amount of `synthetic_asset_id` to the receiver.
    fn swap_mint_burn_assets(
        _dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        swap_amount: SwapAmount<Balance>,
        from_account_id: &T::AccountId,
        to_account_id: &T::AccountId,
    ) -> Result<SwapOutcome<Balance, T::AssetId>, DispatchError> {
        common::with_transaction(|| {
            let permissioned_tech_account_id = T::GetXSTPoolPermissionedTechAccountId::get();
            let permissioned_account_id =
                Technical::<T>::tech_account_id_to_account_id(&permissioned_tech_account_id)?;

            let synthetic_base_asset_id = &T::GetSyntheticBaseAssetId::get();
            let (input_amount, output_amount, fee_amount) =
                if input_asset_id == synthetic_base_asset_id {
                    Self::decide_sell_amounts(
                        &input_asset_id,
                        &output_asset_id,
                        swap_amount.into(),
                        true,
                        true,
                    )?
                } else {
                    Self::decide_buy_amounts(
                        &output_asset_id,
                        &input_asset_id,
                        swap_amount.into(),
                        true,
                        true,
                    )?
                };

            // in XST
            let fee = OutcomeFee::from_asset(T::GetSyntheticBaseAssetId::get(), fee_amount);

            let result = match swap_amount {
                SwapAmount::WithDesiredInput { min_amount_out, .. } => {
                    ensure!(
                        output_amount >= min_amount_out,
                        Error::<T>::SlippageLimitExceeded
                    );
                    SwapOutcome::new(output_amount, fee)
                }
                SwapAmount::WithDesiredOutput { max_amount_in, .. } => {
                    ensure!(
                        input_amount <= max_amount_in,
                        Error::<T>::SlippageLimitExceeded
                    );
                    SwapOutcome::new(input_amount, fee)
                }
            };

            Assets::<T>::burn_from(
                input_asset_id,
                &permissioned_account_id,
                &from_account_id,
                input_amount,
            )?;

            Assets::<T>::mint_to(
                output_asset_id,
                &permissioned_account_id,
                &to_account_id,
                output_amount,
            )?;

            Ok(result)
        })
    }

    fn get_aggregated_fee(synthetic_asset_id: &T::AssetId) -> Result<FixedWrapper, DispatchError> {
        let SyntheticInfo {
            reference_symbol,
            fee_ratio,
        } = EnabledSynthetics::<T>::get(synthetic_asset_id)
            .ok_or(Error::<T>::SyntheticDoesNotExist)?;

        let dynamic_fee_ratio: FixedWrapper = T::Oracle::quote_unchecked(&reference_symbol)
            .map_or(fixed_wrapper!(0), |rate| rate.dynamic_fee.into());
        let fee_ratio: FixedWrapper = fee_ratio.into();
        let resulting_fee_ratio = fee_ratio + dynamic_fee_ratio;

        ensure!(
            resulting_fee_ratio < fixed_wrapper!(1),
            Error::<T>::InvalidFeeRatio
        );

        return Ok(resulting_fee_ratio);
    }

    fn ensure_base_asset_amount_within_limit(
        amount: Balance,
        check_limits: bool,
    ) -> Result<(), DispatchError> {
        if check_limits && amount > T::GetSyntheticBaseBuySellLimit::get() {
            fail!(Error::<T>::SyntheticBaseBuySellLimitExceeded)
        } else {
            Ok(())
        }
    }

    /// This function is used to determine particular synthetic asset price in terms of a reference asset.
    /// The price for synthetics is calculated only for synthetic main asset. For other synthetics it is either
    /// hardcoded or fetched via OracleProxy.
    ///
    /// The reference token here is expected to be DAI (or any other USD stablecoin).
    ///
    /// Example use: understand actual value of two tokens in terms of USD.
    ///
    /// ## Synthetic main asset price calculation
    /// REF - reference asset
    /// MAIN - synthetic main asset
    ///
    /// ### Buy synthetic main asset variant
    /// Equivalent to REF -> XOR -> MAIN swap quote (buying XOR with REF and selling XOR to MAIN).
    /// Therefore the price is calculated as REF_buy_price_tools_price / MAIN_sell_price_tools_price
    ///
    /// ### Sell synthetic main asset variant
    /// Equivalent to MAIN -> XOR -> REF swap quote (buying XOR with MAIN and selling XOR to REF).
    /// Therefore the price is calculated as REF_sell_price_tools_price / MAIN_buy_price_tools_price
    ///
    /// Refer to price-tools pallet documentation for clarification.
    pub fn reference_price(
        asset_id: &T::AssetId,
        price_variant: PriceVariant,
    ) -> Result<Balance, DispatchError> {
        let reference_asset_id = ReferenceAssetId::<T>::get();
        let synthetic_base_asset_id = T::GetSyntheticBaseAssetId::get();

        match asset_id {
            // XSTUSD is a special case because it is equal to the reference asset, DAI
            id if id == &XSTUSD.into() || id == &reference_asset_id => Ok(balance!(1)),
            id if id == &synthetic_base_asset_id => {
                // We don't let the price of XST w.r.t. reference asset go under $3, to prevent manipulation attacks
                T::PriceToolsPallet::get_average_price(id, &reference_asset_id, price_variant)
                    .map(|avg| avg.max(SyntheticBaseAssetFloorPrice::<T>::get()))
            }
            id => {
                let symbol = EnabledSynthetics::<T>::get(id)
                    .ok_or(Error::<T>::SyntheticDoesNotExist)?
                    .reference_symbol;
                T::Oracle::quote_unchecked(&symbol)
                    .map(|rate| rate.value)
                    .ok_or(Error::<T>::OracleQuoteError.into())
            }
        }
    }

    /// Check if any symbol rate is present in OracleProxy
    fn ensure_symbol_exists(reference_symbol: &T::Symbol) -> Result<(), DispatchError> {
        if *reference_symbol == common::SymbolName::usd().into() {
            return Ok(());
        }

        let all_symbols = T::Oracle::list_enabled_symbols()?;
        all_symbols
            .into_iter()
            .find(|(symbol, _rate)| symbol == reference_symbol)
            .map(|_| ())
            .ok_or_else(|| Error::<T>::SymbolDoesNotExist.into())
    }

    fn register_synthetic_asset_unchecked(
        synthetic_asset: T::AssetId,
        asset_symbol: AssetSymbol,
        asset_name: AssetName,
    ) -> Result<(), DispatchError> {
        let permissioned_tech_account_id = T::GetXSTPoolPermissionedTechAccountId::get();
        let permissioned_account_id =
            Technical::<T>::tech_account_id_to_account_id(&permissioned_tech_account_id)?;
        Assets::<T>::register_asset_id(
            permissioned_account_id,
            synthetic_asset,
            asset_symbol,
            asset_name,
            DEFAULT_BALANCE_PRECISION,
            balance!(0),
            true,
            None,
            None,
        )
    }

    fn inner_quote(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        deduce_fee: bool,
        check_limits: bool,
    ) -> Result<(SwapOutcome<Balance, T::AssetId>, Weight), DispatchError> {
        if !Self::can_exchange(dex_id, input_asset_id, output_asset_id) {
            fail!(Error::<T>::CantExchange);
        }
        let synthetic_base_asset_id = &T::GetSyntheticBaseAssetId::get();
        let (input_amount, output_amount, fee_amount) = if input_asset_id == synthetic_base_asset_id
        {
            Self::decide_sell_amounts(
                &input_asset_id,
                &output_asset_id,
                amount,
                deduce_fee,
                check_limits,
            )?
        } else {
            Self::decide_buy_amounts(
                &output_asset_id,
                &input_asset_id,
                amount,
                deduce_fee,
                check_limits,
            )?
        };

        // in XST
        let fee = OutcomeFee::from_asset(T::GetSyntheticBaseAssetId::get(), fee_amount);

        match amount {
            QuoteAmount::WithDesiredInput { .. } => {
                Ok((SwapOutcome::new(output_amount, fee), Self::quote_weight()))
            }
            QuoteAmount::WithDesiredOutput { .. } => {
                Ok((SwapOutcome::new(input_amount, fee), Self::quote_weight()))
            }
        }
    }

    fn disable_synthetic_asset_unchecked(synthetic_asset: AssetIdOf<T>) -> DispatchResult {
        EnabledSynthetics::<T>::remove(synthetic_asset);
        T::TradingPairSourceManager::disable_source_for_trading_pair(
            &DEXId::Polkaswap.into(),
            &T::GetSyntheticBaseAssetId::get(),
            &synthetic_asset,
            LiquiditySourceType::XSTPool,
        )?;
        Self::deposit_event(Event::SyntheticAssetDisabled(synthetic_asset));
        Ok(())
    }
}

impl<T: Config> LiquiditySource<T::DEXId, T::AccountId, T::AssetId, Balance, DispatchError>
    for Pallet<T>
{
    fn can_exchange(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
    ) -> bool {
        if *dex_id != DEXId::Polkaswap.into() {
            return false;
        }
        if input_asset_id == &T::GetSyntheticBaseAssetId::get() {
            Self::is_synthetic(&output_asset_id)
        } else if output_asset_id == &T::GetSyntheticBaseAssetId::get() {
            Self::is_synthetic(&input_asset_id)
        } else {
            false
        }
    }

    /// Get spot price of synthetic tokens based on desired amount.
    fn quote(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        deduce_fee: bool,
    ) -> Result<(SwapOutcome<Balance, T::AssetId>, Weight), DispatchError> {
        Self::inner_quote(
            dex_id,
            input_asset_id,
            output_asset_id,
            amount,
            deduce_fee,
            true,
        )
    }

    fn step_quote(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        recommended_samples_count: usize,
        deduce_fee: bool,
    ) -> Result<(VecDeque<SwapChunk<Balance>>, Weight), DispatchError> {
        if !Self::can_exchange(dex_id, input_asset_id, output_asset_id) {
            fail!(Error::<T>::CantExchange);
        }
        if amount.amount().is_zero() {
            return Ok((VecDeque::new(), Weight::zero()));
        }

        let samples_count = if recommended_samples_count < 1 {
            1
        } else {
            recommended_samples_count
        };

        let synthetic_base_asset_id = &T::GetSyntheticBaseAssetId::get();

        // Get the price without checking the limit, because even if it exceeds the limit it will be rounded below.
        // It is necessary to use as much liquidity from the source as we can.
        let (input_amount, output_amount, fee_amount) = if input_asset_id == synthetic_base_asset_id
        {
            Self::decide_sell_amounts(&input_asset_id, &output_asset_id, amount, deduce_fee, false)?
        } else {
            Self::decide_buy_amounts(&output_asset_id, &input_asset_id, amount, deduce_fee, false)?
        };

        // in XST
        let fee = OutcomeFee::from_asset(T::GetSyntheticBaseAssetId::get(), fee_amount);

        // todo fix (m.tagirov)
        let mut monolith = SwapChunk::new(input_amount, output_amount, fee.get_xst());

        let limit = T::GetSyntheticBaseBuySellLimit::get();

        // If amount exceeds the limit, it is necessary to round the amount to the limit.
        if input_asset_id == synthetic_base_asset_id {
            if input_amount > limit {
                monolith = monolith
                    .rescale_by_input(limit)
                    .ok_or(Error::<T>::PriceCalculationFailed)?;
            }
        } else {
            if output_amount > limit {
                monolith = monolith
                    .rescale_by_output(limit)
                    .ok_or(Error::<T>::PriceCalculationFailed)?;
            }
        }

        let ratio = (FixedWrapper::from(1) / FixedWrapper::from(samples_count))
            .get()
            .map_err(|_| Error::<T>::PriceCalculationFailed)?;

        let chunk = monolith
            .rescale_by_ratio(ratio)
            .ok_or(Error::<T>::PriceCalculationFailed)?;

        let mut chunks: VecDeque<SwapChunk<Balance>> = vec![chunk; samples_count - 1].into();

        // add remaining values as the last chunk to not loss the liquidity on the rounding
        chunks.push_back(SwapChunk::new(
            monolith.input.saturating_sub(
                chunk
                    .input
                    .checked_mul(samples_count as Balance - 1)
                    .ok_or(Error::<T>::PriceCalculationFailed)?,
            ),
            monolith.output.saturating_sub(
                chunk
                    .output
                    .checked_mul(samples_count as Balance - 1)
                    .ok_or(Error::<T>::PriceCalculationFailed)?,
            ),
            monolith.fee.saturating_sub(
                chunk
                    .fee
                    .checked_mul(samples_count as Balance - 1)
                    .ok_or(Error::<T>::PriceCalculationFailed)?,
            ),
        ));

        Ok((chunks, Self::step_quote_weight(samples_count)))
    }

    fn exchange(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        desired_amount: SwapAmount<Balance>,
    ) -> Result<(SwapOutcome<Balance, T::AssetId>, Weight), DispatchError> {
        if !Self::can_exchange(dex_id, input_asset_id, output_asset_id) {
            fail!(Error::<T>::CantExchange);
        }

        let outcome = Self::swap_mint_burn_assets(
            dex_id,
            input_asset_id,
            output_asset_id,
            desired_amount,
            sender,
            receiver,
        )?;
        Ok((outcome, Self::exchange_weight()))
    }

    fn check_rewards(
        _dex_id: &T::DEXId,
        _input_asset_id: &T::AssetId,
        _output_asset_id: &T::AssetId,
        _input_amount: Balance,
        _output_amount: Balance,
    ) -> Result<(Vec<(Balance, T::AssetId, RewardReason)>, Weight), DispatchError> {
        Ok((Vec::new(), Weight::zero())) // no rewards for XST
    }

    fn quote_without_impact(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance, T::AssetId>, DispatchError> {
        // no impact, because price is linear
        // TODO: consider optimizing additional call by introducing NoImpact enum variant
        Self::inner_quote(
            dex_id,
            input_asset_id,
            output_asset_id,
            amount,
            deduce_fee,
            false,
        )
        .map(|(outcome, _)| outcome)
    }

    fn quote_weight() -> Weight {
        <T as Config>::WeightInfo::quote()
    }

    fn step_quote_weight(_samples_count: usize) -> Weight {
        <T as Config>::WeightInfo::step_quote()
    }

    fn exchange_weight() -> Weight {
        <T as Config>::WeightInfo::exchange()
    }

    fn check_rewards_weight() -> Weight {
        Weight::zero()
    }
}

impl<T: Config> GetMarketInfo<T::AssetId> for Pallet<T> {
    fn buy_price(
        synthetic_base_asset: &T::AssetId,
        synthetic_asset: &T::AssetId,
    ) -> Result<Fixed, DispatchError> {
        let base_price_wrt_ref: FixedWrapper =
            Self::reference_price(synthetic_base_asset, PriceVariant::Buy)?.into();
        let synthetic_price_per_reference_unit: FixedWrapper =
            Self::reference_price(synthetic_asset, PriceVariant::Sell)?.into();
        let output = (base_price_wrt_ref / synthetic_price_per_reference_unit)
            .get()
            .map_err(|_| Error::<T>::PriceCalculationFailed)?;
        Ok(output)
    }

    fn sell_price(
        synthetic_base_asset: &T::AssetId,
        synthetic_asset: &T::AssetId,
    ) -> Result<Fixed, DispatchError> {
        let base_price_wrt_ref: FixedWrapper =
            Self::reference_price(synthetic_base_asset, PriceVariant::Sell)?.into();
        let synthetic_price_per_reference_unit: FixedWrapper =
            Self::reference_price(synthetic_asset, PriceVariant::Buy)?.into();
        let output = (base_price_wrt_ref / synthetic_price_per_reference_unit)
            .get()
            .map_err(|_| Error::<T>::PriceCalculationFailed)?;
        Ok(output)
    }

    /// `target_assets` refer to synthetic assets
    fn enabled_target_assets() -> BTreeSet<T::AssetId> {
        EnabledSynthetics::<T>::iter()
            .map(|(asset_id, _)| asset_id)
            .collect()
    }
}

impl<T: Config> SyntheticInfoProvider<T::AssetId> for Pallet<T> {
    fn is_synthetic(asset_id: &T::AssetId) -> bool {
        EnabledSynthetics::<T>::contains_key(asset_id)
    }

    fn get_synthetic_assets() -> Vec<T::AssetId> {
        EnabledSynthetics::<T>::iter_keys().collect()
    }
}

impl<T: Config> OnSymbolDisabled<T::Symbol> for Pallet<T> {
    fn disable_symbol(symbol: &T::Symbol) {
        // error doesn't matter since we don't
        // priorly know whether the symbol exists
        if let Some(asset_id) = Self::enabled_symbols(symbol) {
            if Self::enabled_synthetics(asset_id).is_some() {
                _ = Self::disable_synthetic_asset_unchecked(asset_id);
            }
        } else {
            ()
        }
    }
}
