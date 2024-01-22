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

use common::fixnum::ops::One;
use common::prelude::{FixedWrapper, QuoteAmount, SwapAmount, SwapOutcome};
use common::{
    balance, fixed, Balance, DexInfoProvider, Fixed, GetPoolReserves, LiquiditySource,
    LiquiditySourceQuoteError, RewardReason,
};
use core::convert::TryInto;
use frame_support::dispatch::DispatchError;
use frame_support::ensure;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use frame_system::ensure_signed;
use permissions::{Scope, BURN, MINT};
use sp_std::vec::Vec;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[allow(non_snake_case)]
impl<T: Config<I>, I: 'static> Pallet<T, I> {
    #[cfg(feature = "std")]
    fn initialize_reserves(reserves: &[(T::DEXId, T::AssetId, (Fixed, Fixed))]) {
        reserves
            .iter()
            .for_each(|(dex_id, target_asset_id, reserve_pair)| {
                <Reserves<T, I>>::insert(dex_id, target_asset_id, reserve_pair);
            })
    }

    fn get_base_amount_out(
        target_amount_in: Fixed,
        base_reserve: Fixed,
        target_reserve: Fixed,
        deduce_fee: bool,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        let zero = fixed!(0);
        ensure!(
            target_amount_in > zero,
            <Error<T, I>>::InsufficientInputAmount
        );
        ensure!(
            base_reserve > zero && target_reserve > zero,
            <Error<T, I>>::InsufficientLiquidity
        );
        let X: FixedWrapper = base_reserve.into();
        let Y: FixedWrapper = target_reserve.into();
        let d_Y: FixedWrapper = target_amount_in.into();

        let amount_out_without_fee = (d_Y.clone() * X / (Y + d_Y))
            .get()
            .map_err(|_| Error::<T, I>::InsufficientLiquidity)?;

        let fee_fraction: FixedWrapper = if deduce_fee {
            T::GetFee::get().into()
        } else {
            0.into()
        };
        let fee_amount = amount_out_without_fee * fee_fraction;
        Ok(SwapOutcome::new(
            (amount_out_without_fee - fee_amount.clone())
                .get()
                .map_err(|_| Error::<T, I>::CalculationError)?,
            fee_amount
                .get()
                .map_err(|_| Error::<T, I>::CalculationError)?,
        ))
    }

    fn get_target_amount_out(
        base_amount_in: Fixed,
        base_reserve: Fixed,
        target_reserve: Fixed,
        deduce_fee: bool,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        let zero = fixed!(0);
        ensure!(
            base_amount_in > zero,
            <Error<T, I>>::InsufficientInputAmount
        );
        ensure!(
            base_reserve > zero && target_reserve > zero,
            <Error<T, I>>::InsufficientLiquidity
        );
        let fee_amount = if deduce_fee {
            let fee_fraction: FixedWrapper = T::GetFee::get().into();
            base_amount_in * fee_fraction
        } else {
            0.into()
        };
        let amount_in_with_fee = base_amount_in - fee_amount.clone();
        let X: FixedWrapper = base_reserve.into();
        let Y: FixedWrapper = target_reserve.into();
        let d_X: FixedWrapper = amount_in_with_fee.into();
        let amount_out = (Y * d_X.clone() / (X + d_X))
            .get()
            .map_err(|_| Error::<T, I>::InsufficientLiquidity)?;
        let fee_amount = fee_amount
            .get()
            .map_err(|_| Error::<T, I>::CalculationError)?;

        Ok(SwapOutcome::new(amount_out, fee_amount))
    }

    fn get_base_amount_in(
        target_amount_out: Fixed,
        base_reserve: Fixed,
        target_reserve: Fixed,
        deduce_fee: bool,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        let zero = fixed!(0);
        ensure!(
            target_amount_out > zero,
            <Error<T, I>>::InsufficientOutputAmount
        );
        ensure!(
            base_reserve > zero && target_reserve > zero,
            <Error<T, I>>::InsufficientLiquidity
        );

        ensure!(
            target_amount_out < target_reserve,
            <Error<T, I>>::InsufficientLiquidity
        );

        let X: FixedWrapper = base_reserve.into();
        let Y: FixedWrapper = target_reserve.into();
        let d_Y: FixedWrapper = target_amount_out.into();

        let base_amount_in_without_fee = (X * d_Y.clone() / (Y - d_Y))
            .get()
            .map_err(|_| Error::<T, I>::InsufficientLiquidity)?;
        let fee_fraction: FixedWrapper = T::GetFee::get().into();
        let base_amount_in_with_fee = FixedWrapper::from(base_amount_in_without_fee)
            / (FixedWrapper::from(Fixed::ONE) - fee_fraction);
        let actual_target_amount_out = Self::get_target_amount_out(
            base_amount_in_with_fee
                .clone()
                .get()
                .map_err(|_| Error::<T, I>::CalculationError)?,
            base_reserve,
            target_reserve,
            deduce_fee,
        )?
        .amount;
        let amount_in = if actual_target_amount_out < target_amount_out {
            base_amount_in_with_fee.clone() + Fixed::from_bits(1)
        } else {
            base_amount_in_with_fee.clone()
        };
        Ok(SwapOutcome::new(
            amount_in
                .get()
                .map_err(|_| Error::<T, I>::CalculationError)?,
            (base_amount_in_with_fee - base_amount_in_without_fee)
                .get()
                .map_err(|_| Error::<T, I>::CalculationError)?,
        ))
    }

    fn get_target_amount_in(
        base_amount_out: Fixed,
        base_reserve: Fixed,
        target_reserve: Fixed,
        deduce_fee: bool,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        let zero = fixed!(0);
        ensure!(
            base_amount_out > zero,
            <Error<T, I>>::InsufficientOutputAmount
        );
        ensure!(
            base_reserve > zero && target_reserve > zero,
            <Error<T, I>>::InsufficientLiquidity
        );

        let one: FixedWrapper = fixed!(1);
        let base_amount_out_wrapper: FixedWrapper = base_amount_out.into();
        let base_amount_out_with_fee = base_amount_out_wrapper / (one - T::GetFee::get());

        let X: FixedWrapper = base_reserve.into();
        let Y: FixedWrapper = target_reserve.into();
        let d_X = base_amount_out_with_fee.clone();

        let target_amount_in: Fixed = (Y * d_X.clone() / (X - d_X))
            .get()
            .map_err(|_| Error::<T, I>::InsufficientLiquidity)?;
        let actual_base_amount_out =
            Self::get_base_amount_out(target_amount_in, base_reserve, target_reserve, deduce_fee)?
                .amount;

        let amount_in = if actual_base_amount_out < base_amount_out {
            target_amount_in + Fixed::from_bits(1).into()
        } else {
            target_amount_in.into()
        };
        let amount_in = amount_in
            .get()
            .map_err(|_| Error::<T, I>::CalculationError)?;
        let fee = (base_amount_out_with_fee - base_amount_out)
            .get()
            .map_err(|_| Error::<T, I>::CalculationError)?;
        Ok(SwapOutcome::new(amount_in, fee))
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    pub fn set_reserves_account_id(account: T::TechAccountId) -> Result<(), DispatchError> {
        let account_id = technical::Pallet::<T>::tech_account_id_to_account_id(&account)?;
        frame_system::Pallet::<T>::inc_consumers(&account_id)
            .map_err(|_| Error::<T, I>::IncRefError)?;
        ReservesAcc::<T, I>::set(account.clone());
        let permissions = [BURN, MINT];
        for permission in &permissions {
            permissions::Pallet::<T>::assign_permission(
                account_id.clone(),
                &account_id,
                *permission,
                Scope::Unlimited,
            )?;
        }
        Ok(())
    }

    pub fn add_reward(entry: (Balance, T::AssetId, RewardReason)) {
        Rewards::<T, I>::mutate(|vec| vec.push(entry));
    }
}

impl<T: Config<I>, I: 'static>
    LiquiditySource<T::DEXId, T::AccountId, T::AssetId, Balance, DispatchError> for Pallet<T, I>
{
    fn can_exchange(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
    ) -> bool {
        if let Ok(dex_info) = T::DexInfoProvider::get_dex_info(dex_id) {
            if input_asset_id == &dex_info.base_asset_id {
                <Reserves<T, I>>::contains_key(dex_id, output_asset_id)
            } else if output_asset_id == &dex_info.base_asset_id {
                <Reserves<T, I>>::contains_key(dex_id, input_asset_id)
            } else {
                <Reserves<T, I>>::contains_key(dex_id, output_asset_id)
                    && <Reserves<T, I>>::contains_key(dex_id, input_asset_id)
            }
        } else {
            false
        }
    }

    fn quote(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        deduce_fee: bool,
    ) -> Result<(SwapOutcome<Balance>, Weight), LiquiditySourceQuoteError> {
        let dex_info = T::DexInfoProvider::get_dex_info(dex_id)
            .map_err(|error| LiquiditySourceQuoteError::DispatchError(error.into()))?;
        let amount = amount.try_into().map_err(|_| {
            LiquiditySourceQuoteError::DispatchError(Error::<T, I>::CalculationError.into())
        })?;
        let res = if input_asset_id == &dex_info.base_asset_id {
            let (base_reserve, target_reserve) = <Reserves<T, I>>::get(dex_id, output_asset_id);
            Ok(match amount {
                QuoteAmount::WithDesiredInput {
                    desired_amount_in: base_amount_in,
                    ..
                } => Self::get_target_amount_out(
                    base_amount_in,
                    base_reserve,
                    target_reserve,
                    deduce_fee,
                )
                .map_err(|_| {
                    LiquiditySourceQuoteError::DispatchError(Error::<T, I>::CalculationError.into())
                })?
                .try_into()
                .map_err(|_| {
                    LiquiditySourceQuoteError::DispatchError(Error::<T, I>::CalculationError.into())
                })?,
                QuoteAmount::WithDesiredOutput {
                    desired_amount_out: target_amount_out,
                    ..
                } => Self::get_base_amount_in(
                    target_amount_out,
                    base_reserve,
                    target_reserve,
                    deduce_fee,
                )
                .map_err(|_| {
                    LiquiditySourceQuoteError::DispatchError(Error::<T, I>::CalculationError.into())
                })?
                .try_into()
                .map_err(|_| {
                    LiquiditySourceQuoteError::DispatchError(Error::<T, I>::CalculationError.into())
                })?,
            })
        } else if output_asset_id == &dex_info.base_asset_id {
            let (base_reserve, target_reserve) = <Reserves<T, I>>::get(dex_id, input_asset_id);
            Ok(match amount {
                QuoteAmount::WithDesiredInput {
                    desired_amount_in: target_amount_in,
                    ..
                } => Self::get_base_amount_out(
                    target_amount_in,
                    base_reserve,
                    target_reserve,
                    deduce_fee,
                )
                .map_err(|_| {
                    LiquiditySourceQuoteError::DispatchError(Error::<T, I>::CalculationError.into())
                })?
                .try_into()
                .map_err(|_| {
                    LiquiditySourceQuoteError::DispatchError(Error::<T, I>::CalculationError.into())
                })?,
                QuoteAmount::WithDesiredOutput {
                    desired_amount_out: base_amount_out,
                    ..
                } => Self::get_target_amount_in(
                    base_amount_out,
                    base_reserve,
                    target_reserve,
                    deduce_fee,
                )
                .map_err(|_| {
                    LiquiditySourceQuoteError::DispatchError(Error::<T, I>::CalculationError.into())
                })?
                .try_into()
                .map_err(|_| {
                    LiquiditySourceQuoteError::DispatchError(Error::<T, I>::CalculationError.into())
                })?,
            })
        } else {
            let (base_reserve_a, target_reserve_a) = <Reserves<T, I>>::get(dex_id, input_asset_id);
            let (base_reserve_b, target_reserve_b) = <Reserves<T, I>>::get(dex_id, output_asset_id);
            match amount {
                QuoteAmount::WithDesiredInput {
                    desired_amount_in, ..
                } => {
                    let outcome_a: SwapOutcome<Fixed> = Self::get_base_amount_out(
                        desired_amount_in,
                        base_reserve_a,
                        target_reserve_a,
                        deduce_fee,
                    )
                    .map_err(|error| LiquiditySourceQuoteError::DispatchError(error.into()))?;
                    let outcome_b: SwapOutcome<Fixed> = Self::get_target_amount_out(
                        outcome_a.amount,
                        base_reserve_b,
                        target_reserve_b,
                        deduce_fee,
                    )
                    .map_err(|error| LiquiditySourceQuoteError::DispatchError(error.into()))?;
                    let outcome_a_fee: FixedWrapper = outcome_a.fee.into();
                    let outcome_b_fee: FixedWrapper = outcome_b.fee.into();
                    let amount = outcome_b.amount.into_bits().try_into().map_err(|_| {
                        LiquiditySourceQuoteError::DispatchError(
                            Error::<T, I>::CalculationError.into(),
                        )
                    })?;
                    let fee = (outcome_a_fee + outcome_b_fee)
                        .try_into_balance()
                        .map_err(|_| {
                            LiquiditySourceQuoteError::DispatchError(
                                Error::<T, I>::CalculationError.into(),
                            )
                        })?;
                    Ok(SwapOutcome::new(amount, fee))
                }
                QuoteAmount::WithDesiredOutput {
                    desired_amount_out, ..
                } => {
                    let outcome_b: SwapOutcome<Fixed> = Self::get_base_amount_in(
                        desired_amount_out,
                        base_reserve_b,
                        target_reserve_b,
                        deduce_fee,
                    )
                    .map_err(|error| LiquiditySourceQuoteError::DispatchError(error.into()))?;
                    let outcome_a: SwapOutcome<Fixed> = Self::get_target_amount_in(
                        outcome_b.amount,
                        base_reserve_a,
                        target_reserve_a,
                        deduce_fee,
                    )
                    .map_err(|error| LiquiditySourceQuoteError::DispatchError(error.into()))?;
                    let outcome_a_fee: FixedWrapper = outcome_a.fee.into();
                    let outcome_b_fee: FixedWrapper = outcome_b.fee.into();
                    let amount = outcome_a.amount.into_bits().try_into().map_err(|_| {
                        LiquiditySourceQuoteError::DispatchError(
                            Error::<T, I>::CalculationError.into(),
                        )
                    })?;
                    let fee = (outcome_b_fee + outcome_a_fee)
                        .try_into_balance()
                        .map_err(|_| {
                            LiquiditySourceQuoteError::DispatchError(
                                Error::<T, I>::CalculationError.into(),
                            )
                        })?;
                    Ok(SwapOutcome::new(amount, fee))
                }
            }
        };
        res.map(|outcome| (outcome, Self::quote_weight()))
    }

    fn exchange(
        _sender: &T::AccountId,
        _receiver: &T::AccountId,
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        desired_amount: SwapAmount<Balance>,
    ) -> Result<(SwapOutcome<Balance>, Weight), DispatchError> {
        // actual exchange does not happen
        Self::quote(
            dex_id,
            input_asset_id,
            output_asset_id,
            desired_amount.into(),
            true,
        )
        .map_err(|error| match error {
            LiquiditySourceQuoteError::NotEnoughAmountForFee => {
                Error::<T, I>::InsufficientInputAmount.into()
            }
            LiquiditySourceQuoteError::NotEnoughLiquidityForSwap => {
                Error::<T, I>::InsufficientLiquidity.into()
            }
            LiquiditySourceQuoteError::DispatchError(error) => error,
        })
    }

    fn check_rewards(
        _target_id: &T::DEXId,
        _input_asset_id: &T::AssetId,
        _output_asset_id: &T::AssetId,
        _input_amount: Balance,
        _output_amount: Balance,
    ) -> Result<(Vec<(Balance, T::AssetId, RewardReason)>, Weight), DispatchError> {
        Ok((Rewards::<T, I>::get(), Weight::zero()))
    }

    fn quote_without_impact(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        _deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        let dex_info = T::DexInfoProvider::get_dex_info(dex_id)?;
        if input_asset_id == &dex_info.base_asset_id {
            let (base_reserve, target_reserve) = <Reserves<T, I>>::get(dex_id, output_asset_id);
            let base_price_wrt_target = FixedWrapper::from(target_reserve) / base_reserve;
            Ok(match amount {
                QuoteAmount::WithDesiredInput {
                    desired_amount_in: base_amount_in,
                } => SwapOutcome::new(
                    (FixedWrapper::from(base_amount_in) * base_price_wrt_target)
                        .try_into_balance()
                        .map_err(|_| Error::<T, I>::CalculationError)?,
                    0,
                ),
                QuoteAmount::WithDesiredOutput {
                    desired_amount_out: target_amount_out,
                } => SwapOutcome::new(
                    (FixedWrapper::from(target_amount_out) / base_price_wrt_target)
                        .try_into_balance()
                        .map_err(|_| Error::<T, I>::CalculationError)?,
                    0,
                ),
            })
        } else {
            let (base_reserve, target_reserve) = <Reserves<T, I>>::get(dex_id, input_asset_id);
            let target_price_wrt_base = FixedWrapper::from(base_reserve) / target_reserve;
            Ok(match amount {
                QuoteAmount::WithDesiredInput {
                    desired_amount_in: target_amount_in,
                } => SwapOutcome::new(
                    (FixedWrapper::from(target_amount_in) * target_price_wrt_base)
                        .try_into_balance()
                        .map_err(|_| Error::<T, I>::CalculationError)?,
                    0,
                ),
                QuoteAmount::WithDesiredOutput {
                    desired_amount_out: base_amount_out,
                } => SwapOutcome::new(
                    (FixedWrapper::from(base_amount_out) / target_price_wrt_base)
                        .try_into_balance()
                        .map_err(|_| Error::<T, I>::CalculationError)?,
                    0,
                ),
            })
        }
    }

    fn quote_weight() -> Weight {
        Weight::zero()
    }

    fn exchange_weight() -> Weight {
        Weight::from_all(1)
    }

    fn check_rewards_weight() -> Weight {
        Weight::zero()
    }
}

impl<T: Config<I>, I: 'static> GetPoolReserves<T::AssetId> for Pallet<T, I> {
    fn reserves(_base_asset: &T::AssetId, other_asset: &T::AssetId) -> (Balance, Balance) {
        // This will only work for the dex_id being common::DEXId::Polkaswap
        // Letting the dex_id being passed as a parameter by the caller would require changing
        // the trait interface which is not desirable
        let dex_id: T::DEXId = common::DEXId::Polkaswap.into();
        let (base_reserve, target_reserve) = <Reserves<T, I>>::get(dex_id, other_asset);
        (
            base_reserve.into_bits().try_into().unwrap_or(balance!(0)),
            target_reserve.into_bits().try_into().unwrap_or(balance!(0)),
        )
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::{DEXInfo, EnsureDEXManager, EnsureTradingPairExists, ManagementMode};
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config<I: 'static = ()>:
        frame_system::Config + common::Config + assets::Config + technical::Config
    {
        type GetFee: Get<Fixed>;
        type EnsureDEXManager: EnsureDEXManager<Self::DEXId, Self::AccountId, DispatchError>;
        type EnsureTradingPairExists: EnsureTradingPairExists<
            Self::DEXId,
            Self::AssetId,
            DispatchError,
        >;
        type DexInfoProvider: DexInfoProvider<Self::DEXId, DEXInfo<Self::AssetId>>;
    }

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

    #[pallet::hooks]
    impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {}

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        // example, this checks should be called at the beginning of management functions of actual liquidity sources, e.g. register, set_fee
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::zero())]
        pub fn test_access(
            origin: OriginFor<T>,
            dex_id: T::DEXId,
            target_id: T::AssetId,
        ) -> DispatchResultWithPostInfo {
            let _who =
                T::EnsureDEXManager::ensure_can_manage(&dex_id, origin, ManagementMode::Public)?;
            T::EnsureTradingPairExists::ensure_trading_pair_exists(
                &dex_id,
                &T::GetBaseAssetId::get(),
                &target_id,
            )?;
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(Weight::zero())]
        pub fn set_reserve(
            origin: OriginFor<T>,
            dex_id: T::DEXId,
            target_id: T::AssetId,
            base_reserve: Fixed,
            target_reserve: Fixed,
        ) -> DispatchResultWithPostInfo {
            let _who = ensure_signed(origin)?;
            <Reserves<T, I>>::insert(dex_id, target_id, (base_reserve, target_reserve));
            Ok(().into())
        }
    }

    #[pallet::error]
    pub enum Error<T, I = ()> {
        PairDoesNotExist,
        InsufficientInputAmount,
        InsufficientOutputAmount,
        InsufficientLiquidity,
        /// Specified parameters lead to arithmetic error
        CalculationError,
        /// Increment account reference error.
        IncRefError,
    }

    #[pallet::storage]
    #[pallet::getter(fn reserves)]
    pub type Reserves<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::DEXId,
        Blake2_128Concat,
        T::AssetId,
        (Fixed, Fixed),
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn reserves_account_id)]
    pub type ReservesAcc<T: Config<I>, I: 'static = ()> =
        StorageValue<_, T::TechAccountId, ValueQuery>;

    #[pallet::storage]
    pub type Rewards<T: Config<I>, I: 'static = ()> =
        StorageValue<_, Vec<(Balance, T::AssetId, RewardReason)>, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config<I>, I: 'static = ()> {
        pub phantom: sp_std::marker::PhantomData<I>,
        pub reserves: Vec<(T::DEXId, T::AssetId, (Fixed, Fixed))>,
    }

    #[cfg(feature = "std")]
    impl<T: Config<I>, I: 'static> Default for GenesisConfig<T, I> {
        fn default() -> Self {
            Self {
                phantom: Default::default(),
                reserves: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config<I>, I: 'static> GenesisBuild<T, I> for GenesisConfig<T, I> {
        fn build(&self) {
            Pallet::<T, I>::initialize_reserves(&self.reserves)
        }
    }
}
