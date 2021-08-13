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

pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod benchmarking;
mod migration;

use core::convert::TryInto;

use assets::AssetIdOf;
use codec::{Decode, Encode};
use common::fixnum::ops::Zero as _;
use common::prelude::{
    Balance, EnsureDEXManager, EnsureTradingPairExists, Fixed, FixedWrapper, PriceToolsPallet,
    QuoteAmount, SwapAmount, SwapOutcome,
};
use common::{
    balance, fixed, fixed_wrapper, DEXId, DexIdOf, GetMarketInfo, LiquiditySource,
    LiquiditySourceFilter, LiquiditySourceType, ManagementMode, RewardReason, DAI, XSTUSD,
};
use frame_support::traits::Get;
use frame_support::weights::Weight;
use frame_support::{ensure, fail};
use liquidity_proxy::LiquidityProxyTrait;
use permissions::{Scope, BURN, MINT};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::collections::btree_set::BTreeSet;
use sp_std::vec::Vec;

pub trait WeightInfo {
    fn on_initialize(_elems: u32) -> Weight;
    fn initialize_pool() -> Weight;
    fn set_reference_asset() -> Weight;
}

type Assets<T> = assets::Module<T>;
type Technical<T> = technical::Module<T>;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"xst-pool";
pub const TECH_ACCOUNT_PERMISSIONED: &[u8] = b"permissioned";

pub use pallet::*;

#[derive(Debug, Encode, Decode, Clone)]
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

#[derive(Debug, Encode, Decode, Clone)]
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

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + common::Config
        + assets::Config
        + technical::Config
        + trading_pair::Config
        + pool_xyk::Config
        + dex_api::Config
    {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type LiquidityProxy: LiquidityProxyTrait<Self::DEXId, Self::AccountId, Self::AssetId>;
        type EnsureDEXManager: EnsureDEXManager<Self::DEXId, Self::AccountId, DispatchError>;
        type EnsureTradingPairExists: EnsureTradingPairExists<
            Self::DEXId,
            Self::AssetId,
            DispatchError,
        >;
        type PriceToolsPallet: PriceToolsPallet<Self::AssetId>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_block_number: T::BlockNumber) -> Weight {
            <T as Config>::WeightInfo::on_initialize(0)
        }

        fn on_runtime_upgrade() -> Weight {
            migration::migrate::<T>()
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Enable exchange path on the pool for pair BaseAsset-SyntheticAsset.
        #[pallet::weight(<T as Config>::WeightInfo::initialize_pool())]
        pub fn initialize_pool(
            origin: OriginFor<T>,
            synthetic_asset_id: T::AssetId,
        ) -> DispatchResultWithPostInfo {
            let _who = <T as Config>::EnsureDEXManager::ensure_can_manage(
                &DEXId::Polkaswap.into(),
                origin,
                ManagementMode::Private,
            )?;
            Self::initialize_pool_unchecked(synthetic_asset_id, true)?;
            Ok(().into())
        }

        /// Change reference asset which is used to determine collateral assets value. Intended to be e.g., stablecoin DAI.
        #[pallet::weight(<T as Config>::WeightInfo::set_reference_asset())]
        pub fn set_reference_asset(
            origin: OriginFor<T>,
            reference_asset_id: T::AssetId,
        ) -> DispatchResultWithPostInfo {
            let _who = <T as Config>::EnsureDEXManager::ensure_can_manage(
                &DEXId::Polkaswap.into(),
                origin,
                ManagementMode::Private,
            )?;
            ReferenceAssetId::<T>::put(reference_asset_id.clone());
            Self::deposit_event(Event::ReferenceAssetChanged(reference_asset_id));
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(DexIdOf<T> = "DEXId", AssetIdOf<T> = "AssetId")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Pool is initialized for pair. [DEX Id, Synthetic Asset Id]
        PoolInitialized(DexIdOf<T>, AssetIdOf<T>),
        /// Reference Asset has been changed for pool. [New Reference Asset Id]
        ReferenceAssetChanged(AssetIdOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// An error occurred while calculating the price.
        PriceCalculationFailed,
        /// Failure while calculating price ignoring non-linearity of liquidity source.
        FailedToCalculatePriceWithoutImpact,
        /// The pool can't perform exchange on itself.
        CannotExchangeWithSelf,
        /// Attempt to initialize pool for pair that already exists.
        PoolAlreadyInitializedForPair,
        /// Attempt to get info for uninitialized pool.
        PoolNotInitialized,
        /// Indicated limits for slippage has not been met during transaction execution.
        SlippageLimitExceeded,
        /// Indicated collateral asset is not enabled for pool.
        UnsupportedCollateralAssetId,
        /// Could not calculate fee.
        FeeCalculationFailed,
        /// Liquidity source can't exchange assets with the given IDs on the given DEXId.
        CantExchange,
        /// Increment account reference error.
        IncRefError,
    }

    // TODO: should be "created" in migration to avoid incref error
    // TODO: better by replaced with Get<>
    /// Technical account used to store collateral tokens.
    #[pallet::storage]
    #[pallet::getter(fn permissioned_tech_account)]
    pub type PermissionedTechAccount<T: Config> = StorageValue<_, T::TechAccountId, ValueQuery>;

    #[pallet::type_value]
    pub(super) fn DefaultForBaseFee() -> Fixed {
        fixed!(0.007)
    }

    /// Base fee in XOR which is deducted on all trades, currently it's burned: 0.7%.
    #[pallet::storage]
    #[pallet::getter(fn base_fee)]
    pub(super) type BaseFee<T: Config> = StorageValue<_, Fixed, ValueQuery, DefaultForBaseFee>;

    /// XST Assets allowed to be traded using XST.
    #[pallet::storage]
    #[pallet::getter(fn enabled_synthetics)]
    pub type EnabledSynthetics<T: Config> = StorageValue<_, BTreeSet<T::AssetId>, ValueQuery>;

    /// Asset that is used to compare collateral assets by value, e.g., DAI.
    #[pallet::storage]
    #[pallet::getter(fn reference_asset_id)]
    pub type ReferenceAssetId<T: Config> = StorageValue<_, T::AssetId, ValueQuery>;

    /// Current reserves balance for collateral tokens, used for client usability.
    #[pallet::storage]
    pub(super) type CollateralReserves<T: Config> =
        StorageMap<_, Twox64Concat, T::AssetId, Balance, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        /// Technical account used to perform permissioned actions e.g. mint/burn.
        pub tech_account_id: T::TechAccountId,
        /// Asset that is used to compare collateral assets by value, e.g., DAI.
        pub reference_asset_id: T::AssetId,
        /// List of tokens enabled as collaterals initially.
        pub initial_synthetic_assets: Vec<T::AssetId>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                tech_account_id: Default::default(),
                reference_asset_id: DAI.into(),
                initial_synthetic_assets: [XSTUSD.into()].into(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            ReferenceAssetId::<T>::put(&self.reference_asset_id);
            self.initial_synthetic_assets
                .iter()
                .cloned()
                .for_each(|asset_id| {
                    Pallet::<T>::initialize_pool_unchecked(asset_id, false)
                        .expect("Failed to initialize XST synthetics.")
                });
        }
    }
}

#[allow(non_snake_case)]
impl<T: Config> Module<T> {
    #[inline]
    #[allow(unused)]
    fn self_excluding_filter() -> LiquiditySourceFilter<T::DEXId, LiquiditySourceType> {
        LiquiditySourceFilter::with_forbidden(
            DEXId::Polkaswap.into(),
            [LiquiditySourceType::XSTPool].into(),
        )
    }

    fn initialize_pool_unchecked(
        synthetic_asset_id: T::AssetId,
        transactional: bool,
    ) -> DispatchResult {
        let code = || {
            ensure!(
                !EnabledSynthetics::<T>::get().contains(&synthetic_asset_id),
                Error::<T>::PoolAlreadyInitializedForPair
            );
            T::EnsureTradingPairExists::ensure_trading_pair_exists(
                &DEXId::Polkaswap.into(),
                &T::GetBaseAssetId::get(),
                &synthetic_asset_id,
            )?;
            trading_pair::Module::<T>::enable_source_for_trading_pair(
                &DEXId::Polkaswap.into(),
                &T::GetBaseAssetId::get(),
                &synthetic_asset_id,
                LiquiditySourceType::XSTPool,
            )?;

            EnabledSynthetics::<T>::mutate(|set| set.insert(synthetic_asset_id));
            Self::deposit_event(Event::PoolInitialized(
                DEXId::Polkaswap.into(),
                synthetic_asset_id,
            ));
            Ok(())
        };
        if transactional {
            common::with_transaction(|| code())
        } else {
            code()
        }
    }

    /// Buys the main asset.
    /// Calculates and returns the current buy price, assuming that input is the synthetic asset and output is the main asset.
    pub fn buy_price(
        main_asset_id: &T::AssetId,
        _synthetic_asset_id: &T::AssetId, //TODO: we will use this once we have more XST assets
        quantity: QuoteAmount<Balance>,
    ) -> Result<Fixed, DispatchError> {
        let main_asset_price_per_reference_unit: FixedWrapper =
            Self::reference_price(main_asset_id)?.into();

        match quantity {
            // Input target amount of XST(USD) to get some XOR
            QuoteAmount::WithDesiredInput {
                desired_amount_in: synthetic_quantity,
            } => {
                //TODO: here we assume only DAI-pegged XST(USD) synthetics. Need to have a price oracle to handle other synthetics in the future!
                let main_out = synthetic_quantity / main_asset_price_per_reference_unit;
                main_out
                    .get()
                    .map_err(|_| Error::<T>::PriceCalculationFailed.into())
                    .map(|value| value.max(Fixed::ZERO))
            }
            // Input some XST(USD) to get a target amount of XOR
            QuoteAmount::WithDesiredOutput {
                desired_amount_out: main_quantity,
            } => {
                //TODO: here we assume only DAI-pegged XST(USD) synthetics. Need to have a price oracle to handle other synthetics in the future!
                let synthetic_quantity = main_quantity * main_asset_price_per_reference_unit;
                synthetic_quantity
                    .get()
                    .map_err(|_| Error::<T>::PriceCalculationFailed.into())
                    .map(|value| value.max(Fixed::ZERO))
            }
        }
    }

    /// Calculates and returns the current sell price, assuming that input is the main asset and output is the collateral asset.
    ///
    /// To calculate sell price for a specific amount of assets:
    /// 1. Current reserves of collateral token are taken
    /// 2. Same amount by value is assumed for main asset
    ///   2.1 Values are compared via getting prices for both main and collateral tokens with regard to another token
    ///       called reference token which is set for particular pair. This should be e.g. stablecoin DAI.
    ///   2.2 Reference price for base token is taken as 80% of current bonding curve buy price.
    ///   2.3 Reference price for collateral token is taken as current market price, i.e. price for 1 token on liquidity proxy.
    /// 3. Given known reserves for main and collateral, output collateral amount is calculated by applying x*y=k model resulting
    ///    in curve-like dependency.
    pub fn sell_price(
        main_asset_id: &T::AssetId,
        _synthetic_asset_id: &T::AssetId,
        quantity: QuoteAmount<Balance>,
    ) -> Result<Fixed, DispatchError> {
        // Get reference prices for base and synthetic to understand token value.
        let main_price_per_reference_unit: FixedWrapper =
            Self::reference_price(main_asset_id)?.into();

        match quantity {
            // Sell desired amount of XOR for some XST(USD)
            QuoteAmount::WithDesiredInput {
                desired_amount_in: quantity_main,
            } => {
                let output_synthetic = quantity_main * main_price_per_reference_unit;
                let output_synthetic_unwrapped = output_synthetic
                    .get()
                    .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                Ok(output_synthetic_unwrapped)
            }
            // Sell some amount of XOR for desired amount of XST(USD)
            QuoteAmount::WithDesiredOutput {
                desired_amount_out: quantity_synthetic,
            } => {
                let output_main = quantity_synthetic / main_price_per_reference_unit;
                output_main
                    .get()
                    .map_err(|_| Error::<T>::PriceCalculationFailed.into())
            }
        }
    }

    /// Decompose SwapAmount into particular buy quotation query.
    ///
    /// Returns ordered pair: (input_amount, output_amount, fee_amount).
    fn decide_buy_amounts(
        main_asset_id: &T::AssetId,
        synthetic_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
    ) -> Result<(Balance, Balance, Balance), DispatchError> {
        Ok(match amount {
            QuoteAmount::WithDesiredInput { desired_amount_in } => {
                let mut output_amount: Balance = FixedWrapper::from(Self::buy_price(
                    main_asset_id,
                    synthetic_asset_id,
                    QuoteAmount::with_desired_input(desired_amount_in),
                )?)
                .try_into_balance()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                let fee_amount = (FixedWrapper::from(BaseFee::<T>::get()) * output_amount)
                    .try_into_balance()
                    .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                output_amount = output_amount.saturating_sub(fee_amount);
                (desired_amount_in, output_amount, fee_amount)
            }

            QuoteAmount::WithDesiredOutput { desired_amount_out } => {
                let desired_amount_out_with_fee = (FixedWrapper::from(desired_amount_out)
                    / (fixed_wrapper!(1) - BaseFee::<T>::get()))
                .try_into_balance()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?;
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
    /// Returns ordered pair: (input_amount, output_amount, fee_amount).
    fn decide_sell_amounts(
        main_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
    ) -> Result<(Balance, Balance, Balance), DispatchError> {
        Ok(match amount {
            QuoteAmount::WithDesiredInput { desired_amount_in } => {
                let fee_ratio = FixedWrapper::from(BaseFee::<T>::get());
                let fee_amount = (fee_ratio * FixedWrapper::from(desired_amount_in))
                    .try_into_balance()
                    .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                let output_amount = Self::sell_price(
                    main_asset_id,
                    collateral_asset_id,
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
                let input_amount: Balance = FixedWrapper::from(Self::sell_price(
                    main_asset_id,
                    collateral_asset_id,
                    QuoteAmount::with_desired_output(desired_amount_out),
                )?)
                .try_into_balance()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                let fee_ratio = FixedWrapper::from(BaseFee::<T>::get());
                let input_amount_with_fee =
                    FixedWrapper::from(input_amount) / (fixed_wrapper!(1) - fee_ratio);
                let input_amount_with_fee = input_amount_with_fee
                    .try_into_balance()
                    .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                (
                    input_amount_with_fee,
                    desired_amount_out,
                    input_amount_with_fee.saturating_sub(input_amount),
                )
            }
        })
    }

    /// This function is used by `exchange` function to burn `input_amount` derived from `amount` of `main_asset_id`
    /// and mint calculated amount of `synthetic_asset_id` to the receiver from reserves.
    ///
    /// If there's not enough reserves in the pool, `NotEnoughReserves` error will be returned.
    ///
    fn swap_mint_burn_assets(
        _dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        swap_amount: SwapAmount<Balance>,
        from_account_id: &T::AccountId,
        to_account_id: &T::AccountId,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        common::with_transaction(|| {
            let permissioned_tech_account_id = Self::permissioned_tech_account();
            let permissioned_account_id =
                Technical::<T>::tech_account_id_to_account_id(&permissioned_tech_account_id)?;

            let base_asset_id = &T::GetBaseAssetId::get();
            let (input_amount, output_amount, fee_amount) = if input_asset_id == base_asset_id {
                Self::decide_sell_amounts(&input_asset_id, &output_asset_id, swap_amount.into())?
            } else {
                Self::decide_buy_amounts(&output_asset_id, &input_asset_id, swap_amount.into())?
            };

            let result = match swap_amount {
                SwapAmount::WithDesiredInput { min_amount_out, .. } => {
                    ensure!(
                        output_amount >= min_amount_out,
                        Error::<T>::SlippageLimitExceeded
                    );
                    SwapOutcome::new(output_amount, fee_amount)
                }
                SwapAmount::WithDesiredOutput { max_amount_in, .. } => {
                    ensure!(
                        input_amount <= max_amount_in,
                        Error::<T>::SlippageLimitExceeded
                    );
                    SwapOutcome::new(input_amount, fee_amount)
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

    /// Assign account id that is used to burn and mint.
    pub fn set_tech_account_id(account: T::TechAccountId) -> Result<(), DispatchError> {
        common::with_transaction(|| {
            PermissionedTechAccount::<T>::set(account.clone());
            let account_id = Technical::<T>::tech_account_id_to_account_id(&account)?;
            let permissions = [BURN, MINT];
            for permission in &permissions {
                permissions::Module::<T>::assign_permission(
                    account_id.clone(),
                    &account_id,
                    *permission,
                    Scope::Unlimited,
                )?;
            }
            Ok(())
        })
    }

    /// This function is used to determine particular asset price in terms of a reference asset, which is set for
    /// XST quotes (there can be only single token chosen as reference for all comparisons).
    /// The reference token is expected to be a USD-bound stablecoin, e.g. DAI.
    ///
    /// Example use: understand actual value of two tokens in terms of USD.
    fn reference_price(asset_id: &T::AssetId) -> Result<Balance, DispatchError> {
        let reference_asset_id = ReferenceAssetId::<T>::get();
        let price = if asset_id == &reference_asset_id {
            balance!(1)
        } else {
            <T as pallet::Config>::PriceToolsPallet::get_average_price(
                asset_id,
                &reference_asset_id,
            )?
        };
        Ok(price)
    }
}

impl<T: Config> LiquiditySource<T::DEXId, T::AccountId, T::AssetId, Balance, DispatchError>
    for Module<T>
{
    fn can_exchange(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
    ) -> bool {
        if *dex_id != DEXId::Polkaswap.into() {
            return false;
        }
        if input_asset_id == &T::GetBaseAssetId::get() {
            EnabledSynthetics::<T>::get().contains(&output_asset_id)
        } else {
            EnabledSynthetics::<T>::get().contains(&input_asset_id)
        }
    }

    fn quote(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        if !Self::can_exchange(dex_id, input_asset_id, output_asset_id) {
            fail!(Error::<T>::CantExchange);
        }
        let base_asset_id = &T::GetBaseAssetId::get();
        let (input_amount, output_amount, fee_amount) = if input_asset_id == base_asset_id {
            Self::decide_sell_amounts(&input_asset_id, &output_asset_id, amount)?
        } else {
            Self::decide_buy_amounts(&output_asset_id, &input_asset_id, amount)?
        };
        match amount {
            QuoteAmount::WithDesiredInput { .. } => Ok(SwapOutcome::new(output_amount, fee_amount)),
            QuoteAmount::WithDesiredOutput { .. } => Ok(SwapOutcome::new(input_amount, fee_amount)),
        }
    }

    fn exchange(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        desired_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
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
        );
        outcome
    }

    fn check_rewards(
        _dex_id: &T::DEXId,
        _input_asset_id: &T::AssetId,
        _output_asset_id: &T::AssetId,
        _input_amount: Balance,
        _output_amount: Balance,
    ) -> Result<Vec<(Balance, T::AssetId, RewardReason)>, DispatchError> {
        Ok(Vec::new()) // no rewards for XST
    }

    fn quote_without_impact(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        // no impact, because price is linear
        // TODO: consider optimizing additional call by introducing NoImpact enum variant
        Self::quote(dex_id, input_asset_id, output_asset_id, amount)
    }
}

impl<T: Config> GetMarketInfo<T::AssetId> for Module<T> {
    fn buy_price(
        base_asset: &T::AssetId,
        synthetic_asset: &T::AssetId,
    ) -> Result<Fixed, DispatchError> {
        let base_price_wrt_ref: FixedWrapper = Self::reference_price(base_asset)?.into();
        let synthetic_price_per_reference_unit: FixedWrapper =
            Self::reference_price(synthetic_asset)?.into();
        let output = (base_price_wrt_ref / synthetic_price_per_reference_unit)
            .get()
            .map_err(|_| Error::<T>::PriceCalculationFailed)?;
        Ok(output)
    }

    fn sell_price(
        base_asset: &T::AssetId,
        synthetic_asset: &T::AssetId,
    ) -> Result<Fixed, DispatchError> {
        let base_price_wrt_ref: FixedWrapper = Self::reference_price(base_asset)?.into();
        let synthetic_price_per_reference_unit: FixedWrapper =
            Self::reference_price(synthetic_asset)?.into();
        let output = (base_price_wrt_ref / synthetic_price_per_reference_unit)
            .get()
            .map_err(|_| Error::<T>::PriceCalculationFailed)?;
        Ok(output)
    }

    /// `target_assets` refer to synthetic assets
    fn enabled_target_assets() -> BTreeSet<T::AssetId> {
        EnabledSynthetics::<T>::get()
    }
}
