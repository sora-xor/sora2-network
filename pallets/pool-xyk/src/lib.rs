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

use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::storage::PrefixIterator;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use frame_support::{ensure, fail, Parameter};
use frame_system::ensure_signed;
use sp_std::vec::Vec;

use common::alt::{DiscreteQuotation, SideAmount, SwapChunk};
use common::prelude::{
    Balance, EnsureDEXManager, FixedWrapper, OutcomeFee, QuoteAmount, SwapAmount, SwapOutcome,
    SwapVariant,
};
use common::{
    fixed_wrapper, AssetIdOf, AssetInfoProvider, AssetRegulator, DEXInfo, DexInfoProvider,
    EnsureTradingPairExists, GetPoolReserves, LiquiditySource, LiquiditySourceType, ManagementMode,
    OnPoolReservesChanged, RewardReason, TechAccountId, TechPurpose, ToFeeAccount, TradingPair,
    TradingPairSourceManager, XykPool,
};

mod aliases;
use aliases::{
    AccountIdOf, DEXIdOf, DepositLiquidityActionOf, PairSwapActionOf, PolySwapActionStructOf,
    TechAccountIdOf, TechAssetIdOf, WithdrawLiquidityActionOf,
};
use sp_std::collections::btree_set::BTreeSet;

pub mod migrations;
pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[macro_use]
mod macros;

mod math;
mod utils;

mod bounds;
use bounds::*;

mod action_deposit_liquidity;
mod action_pair_swap;
mod action_poly_swap;
mod action_withdraw_liquidity;

mod operations;
pub use operations::*;

const MIN_LIQUIDITY: u128 = 1000;

pub use weights::WeightInfo;

impl<T: Config> XykPool<T::AccountId, AssetIdOf<T>> for Pallet<T> {
    type PoolProvidersOutput = PrefixIterator<(AccountIdOf<T>, Balance)>;
    type PoolPropertiesOutput =
        PrefixIterator<(AssetIdOf<T>, AssetIdOf<T>, (AccountIdOf<T>, AccountIdOf<T>))>;

    fn pool_providers(pool_account: &T::AccountId) -> Self::PoolProvidersOutput {
        PoolProviders::<T>::iter_prefix(pool_account)
    }

    fn total_issuance(pool_account: &T::AccountId) -> Result<Balance, DispatchError> {
        TotalIssuances::<T>::get(pool_account).ok_or(Error::<T>::PoolIsInvalid.into())
    }

    fn all_properties() -> Self::PoolPropertiesOutput {
        Properties::<T>::iter()
    }

    fn properties_of_pool(
        base_asset_id: AssetIdOf<T>,
        target_asset_id: AssetIdOf<T>,
    ) -> Option<(T::AccountId, T::AccountId)> {
        Properties::<T>::get(base_asset_id, target_asset_id)
    }

    fn balance_of_pool_provider(
        pool_account: T::AccountId,
        liquidity_provider_account: T::AccountId,
    ) -> Option<Balance> {
        PoolProviders::<T>::get(pool_account, liquidity_provider_account)
    }

    fn transfer_lp_tokens(
        pool_account: T::AccountId,
        asset_a: AssetIdOf<T>,
        asset_b: AssetIdOf<T>,
        base_account_id: T::AccountId,
        target_account_id: T::AccountId,
        pool_tokens: Balance,
    ) -> Result<(), DispatchError> {
        // Subtract lp_tokens from base_account
        let mut result: Result<_, Error<T>> =
            PoolProviders::<T>::mutate_exists(pool_account.clone(), base_account_id, |balance| {
                let old_balance = balance.ok_or(Error::<T>::AccountBalanceIsInvalid)?;
                let new_balance = old_balance
                    .checked_sub(pool_tokens)
                    .ok_or(Error::<T>::AccountBalanceIsInvalid)?;
                *balance = (new_balance != 0).then(|| new_balance);
                Ok(())
            });
        result?;

        // Pool tokens balance is zero while minted amount will be non-zero.
        if PoolProviders::<T>::get(&pool_account, target_account_id.clone())
            .unwrap_or(0)
            .is_zero()
            && !pool_tokens.is_zero()
        {
            let pair = Pallet::<T>::get_trading_pair(&asset_a.clone(), &asset_a, &asset_b)?;
            AccountPools::<T>::mutate(target_account_id.clone(), &pair.base_asset_id, |set| {
                set.insert(pair.target_asset_id)
            });
        }

        // Add lp_tokens to target_account
        result = PoolProviders::<T>::mutate(pool_account.clone(), target_account_id, |balance| {
            *balance = Some(balance.unwrap_or(0) + pool_tokens);
            Ok(())
        });
        result?;

        Ok(())
    }
}

impl<T: Config> Pallet<T> {
    fn initialize_pool_properties(
        dex_id: &T::DEXId,
        asset_a: &AssetIdOf<T>,
        asset_b: &AssetIdOf<T>,
        reserves_account_id: &T::AccountId,
        fees_account_id: &T::AccountId,
    ) -> DispatchResult {
        let dex_info = T::DexInfoProvider::get_dex_info(dex_id)?;
        let (sorted_asset_a, sorted_asset_b) = if dex_info.base_asset_id == *asset_a {
            (asset_a, asset_b)
        } else if dex_info.base_asset_id == *asset_b {
            (asset_b, asset_a)
        } else {
            let hash_key = common::comm_merkle_op(asset_a, asset_b);
            let (asset_a_pair, asset_b_pair) =
                common::sort_with_hash_key(hash_key, (asset_a, &()), (asset_b, &()));
            (asset_a_pair.0, asset_b_pair.0)
        };

        T::TradingPairSourceManager::enable_source_for_trading_pair(
            dex_id,
            sorted_asset_a,
            sorted_asset_b,
            LiquiditySourceType::XYKPool,
        )?;
        Properties::<T>::insert(
            sorted_asset_a,
            sorted_asset_b,
            (reserves_account_id.clone(), fees_account_id.clone()),
        );
        Ok(())
    }

    fn update_reserves(
        base_asset_id: &AssetIdOf<T>,
        asset_a: &AssetIdOf<T>,
        asset_b: &AssetIdOf<T>,
        balance_pair: (&Balance, &Balance),
    ) {
        let tpair = Pallet::<T>::get_trading_pair(base_asset_id, asset_a, asset_b);
        if let Ok(tpair) = tpair {
            if asset_a == &tpair.target_asset_id {
                Reserves::<T>::insert(
                    &tpair.base_asset_id,
                    &tpair.target_asset_id,
                    (balance_pair.1, balance_pair.0),
                );
            } else if asset_b == &tpair.target_asset_id {
                Reserves::<T>::insert(
                    &tpair.base_asset_id,
                    &tpair.target_asset_id,
                    (balance_pair.0, balance_pair.1),
                );
            } else {
                // Do nothing, should never happen
                return;
            }
            T::OnPoolReservesChanged::reserves_changed(&tpair.target_asset_id);
        } else {
            let hash_key = common::comm_merkle_op(asset_a, asset_b);
            let (pair_u, pair_v) = common::sort_with_hash_key(
                hash_key,
                (asset_a, balance_pair.0),
                (asset_b, balance_pair.1),
            );
            Reserves::<T>::insert(pair_u.0, pair_v.0, (pair_u.1, pair_v.1));
            T::OnPoolReservesChanged::reserves_changed(asset_a);
            T::OnPoolReservesChanged::reserves_changed(asset_b);
        }
    }

    pub fn initialize_pool_unchecked(
        _source: AccountIdOf<T>,
        dex_id: DEXIdOf<T>,
        asset_a: AssetIdOf<T>,
        asset_b: AssetIdOf<T>,
    ) -> Result<
        (
            common::TradingPair<TechAssetIdOf<T>>,
            TechAccountIdOf<T>,
            TechAccountIdOf<T>,
        ),
        DispatchError,
    > {
        let (trading_pair, tech_acc_id) =
            Pallet::<T>::tech_account_from_dex_and_asset_pair(dex_id, asset_a, asset_b)?;
        let fee_acc_id = tech_acc_id.to_fee_account().unwrap();
        // Function initialize_pools is usually called once, just quick check if tech
        // account is not registered is enough to do the job.
        // If function is called second time, than this is not usual case and additional checks
        // can be done, check every condition for `PoolIsAlreadyInitialized`.
        if technical::Pallet::<T>::ensure_tech_account_registered(&tech_acc_id).is_ok() {
            if technical::Pallet::<T>::ensure_tech_account_registered(&fee_acc_id).is_ok()
                && T::EnsureTradingPairExists::ensure_trading_pair_exists(
                    &dex_id,
                    &trading_pair.base_asset_id.into(),
                    &trading_pair.target_asset_id.into(),
                )
                .is_ok()
            {
                Err(Error::<T>::PoolIsAlreadyInitialized)?;
            } else {
                Err(Error::<T>::PoolInitializationIsInvalid)?;
            }
        }
        technical::Pallet::<T>::register_tech_account_id(tech_acc_id.clone())?;
        technical::Pallet::<T>::register_tech_account_id(fee_acc_id.clone())?;
        Ok((trading_pair, tech_acc_id, fee_acc_id))
    }

    pub fn deposit_liquidity_unchecked(
        source: AccountIdOf<T>,
        dex_id: DEXIdOf<T>,
        input_asset_a: AssetIdOf<T>,
        input_asset_b: AssetIdOf<T>,
        input_a_desired: Balance,
        input_b_desired: Balance,
        input_a_min: Balance,
        input_b_min: Balance,
    ) -> DispatchResult {
        let dex_info = T::DexInfoProvider::get_dex_info(&dex_id)?;
        let (_, tech_acc_id) = Pallet::<T>::tech_account_from_dex_and_asset_pair(
            dex_id,
            input_asset_a,
            input_asset_b,
        )?;
        let action = PolySwapActionStructOf::<T>::DepositLiquidity(DepositLiquidityActionOf::<T> {
            client_account: None,
            receiver_account: None,
            pool_account: tech_acc_id,
            source: ResourcePair(
                Resource {
                    asset: input_asset_a,
                    amount: Bounds::<Balance>::RangeFromDesiredToMin(input_a_desired, input_a_min),
                },
                Resource {
                    asset: input_asset_b,
                    amount: Bounds::<Balance>::RangeFromDesiredToMin(input_b_desired, input_b_min),
                },
            ),
            pool_tokens: 0,
            min_liquidity: None,
        });
        let action = T::PolySwapAction::from(action);
        let mut action = action.into();
        technical::Pallet::<T>::create_swap(source, &mut action, &dex_info.base_asset_id)?;
        Ok(())
    }

    fn withdraw_liquidity_unchecked(
        source: AccountIdOf<T>,
        dex_id: DEXIdOf<T>,
        output_asset_a: AssetIdOf<T>,
        output_asset_b: AssetIdOf<T>,
        marker_asset_desired: Balance,
        output_a_min: Balance,
        output_b_min: Balance,
    ) -> DispatchResult {
        let dex_info = T::DexInfoProvider::get_dex_info(&dex_id)?;
        let (_, tech_acc_id) = Pallet::<T>::tech_account_from_dex_and_asset_pair(
            dex_id,
            output_asset_a,
            output_asset_b,
        )?;
        let action =
            PolySwapActionStructOf::<T>::WithdrawLiquidity(WithdrawLiquidityActionOf::<T> {
                client_account: None,
                receiver_account_a: None,
                receiver_account_b: None,
                pool_account: tech_acc_id,
                pool_tokens: marker_asset_desired,
                destination: ResourcePair(
                    Resource {
                        asset: output_asset_a,
                        amount: Bounds::Min(output_a_min),
                    },
                    Resource {
                        asset: output_asset_b,
                        amount: Bounds::Min(output_b_min),
                    },
                ),
            });
        let action = T::PolySwapAction::from(action);
        let mut action = action.into();
        technical::Pallet::<T>::create_swap(source, &mut action, &dex_info.base_asset_id)?;
        Ok(())
    }

    pub fn get_pool_trading_pair(
        pool_account: &T::AccountId,
    ) -> Result<TradingPair<AssetIdOf<T>>, DispatchError> {
        let tech_acc = technical::Pallet::<T>::lookup_tech_account_id(pool_account)?;
        match tech_acc.into() {
            TechAccountId::Pure(_, TechPurpose::XykLiquidityKeeper(trading_pair)) => {
                Ok(trading_pair.map(|a| a.into()))
            }
            _ => Err(Error::<T>::PoolIsInvalid.into()),
        }
    }

    /// Returns (input reserves, output reserves, max output amount)
    /// Output reserves could only be greater than max output amount if it's chameleon pool
    pub fn get_actual_reserves(
        pool_acc_id: &T::AccountId,
        base_asset_id: &AssetIdOf<T>,
        input_asset_id: &AssetIdOf<T>,
        output_asset_id: &AssetIdOf<T>,
    ) -> Result<(Balance, Balance, Balance), DispatchError> {
        let (tpair, base_chameleon_asset_id, is_chameleon_pool) =
            Self::get_pair_info(base_asset_id, input_asset_id, output_asset_id)?;
        let reserve_base =
            <T as Config>::AssetInfoProvider::free_balance(&tpair.base_asset_id, &pool_acc_id)?;
        let reserve_target =
            <T as Config>::AssetInfoProvider::free_balance(&tpair.target_asset_id, &pool_acc_id)?;
        let reserve_chameleon = if let Some(base_chameleon_asset_id) = base_chameleon_asset_id {
            if is_chameleon_pool {
                <T as Config>::AssetInfoProvider::free_balance(
                    &base_chameleon_asset_id,
                    &pool_acc_id,
                )?
            } else {
                0
            }
        } else {
            0
        };
        let reserve_base_chameleon = reserve_base
            .checked_add(reserve_chameleon)
            .ok_or(Error::<T>::PoolTokenSupplyOverflow)?;
        let max_output = if *output_asset_id == tpair.target_asset_id {
            reserve_target
        } else if *output_asset_id == tpair.base_asset_id {
            reserve_base
        } else {
            reserve_chameleon
        };
        if tpair.target_asset_id == *input_asset_id {
            Ok((reserve_target, reserve_base_chameleon, max_output))
        } else {
            Ok((reserve_base_chameleon, reserve_target, max_output))
        }
    }
}

impl<T: Config> LiquiditySource<T::DEXId, T::AccountId, AssetIdOf<T>, Balance, DispatchError>
    for Pallet<T>
{
    fn can_exchange(
        dex_id: &T::DEXId,
        input_asset_id: &AssetIdOf<T>,
        output_asset_id: &AssetIdOf<T>,
    ) -> bool {
        if let Ok(dex_info) = T::DexInfoProvider::get_dex_info(dex_id) {
            if let Ok(tpair) =
                Self::get_trading_pair(&dex_info.base_asset_id, input_asset_id, output_asset_id)
            {
                Properties::<T>::contains_key(&tpair.base_asset_id, &tpair.target_asset_id)
            } else {
                false
            }
        } else {
            false
        }
    }

    fn quote(
        dex_id: &T::DEXId,
        input_asset_id: &AssetIdOf<T>,
        output_asset_id: &AssetIdOf<T>,
        amount: QuoteAmount<Balance>,
        deduce_fee: bool,
    ) -> Result<(SwapOutcome<Balance, AssetIdOf<T>>, Weight), DispatchError> {
        let dex_info = T::DexInfoProvider::get_dex_info(dex_id)?;
        // Get pool account.
        let (_, tech_acc_id) = Pallet::<T>::tech_account_from_dex_and_asset_pair(
            *dex_id,
            *input_asset_id,
            *output_asset_id,
        )?;
        let pool_acc_id = technical::Pallet::<T>::tech_account_id_to_account_id(&tech_acc_id)?;

        // Get actual pool reserves.
        let (reserve_input, reserve_output, max_output_available) = Self::get_actual_reserves(
            &pool_acc_id,
            &dex_info.base_asset_id,
            &input_asset_id,
            &output_asset_id,
        )?;

        // Check reserves validity.
        if reserve_input == 0 && reserve_output == 0 {
            fail!(Error::<T>::PoolIsEmpty);
        } else if reserve_input <= 0 || reserve_output <= 0 {
            fail!(Error::<T>::PoolIsInvalid);
        }

        // Decide which side should be used for fee.
        let get_fee_from_destination = Pallet::<T>::decide_is_fee_from_destination(
            &dex_info.base_asset_id,
            input_asset_id,
            output_asset_id,
        )?;

        // Calculate quote.
        let (calculated, fee_amount) = match amount {
            QuoteAmount::WithDesiredInput { desired_amount_in } => {
                let (calculated, fee_amount) = Pallet::<T>::calc_output_for_exact_input(
                    T::GetFee::get(),
                    get_fee_from_destination,
                    &reserve_input,
                    &reserve_output,
                    &desired_amount_in,
                    deduce_fee,
                )?;
                if max_output_available != reserve_output {
                    let required_output = if get_fee_from_destination {
                        calculated + fee_amount
                    } else {
                        calculated
                    };
                    ensure!(
                        required_output <= max_output_available,
                        Error::<T>::NotEnoughOutputReserves
                    );
                }
                (calculated, fee_amount)
            }
            QuoteAmount::WithDesiredOutput { desired_amount_out } => {
                if max_output_available != reserve_output {
                    ensure!(
                        desired_amount_out <= max_output_available,
                        Error::<T>::NotEnoughOutputReserves
                    );
                }
                Pallet::<T>::calc_input_for_exact_output(
                    T::GetFee::get(),
                    get_fee_from_destination,
                    &reserve_input,
                    &reserve_output,
                    &desired_amount_out,
                    deduce_fee,
                )?
            }
        };

        // in XOR for dex_id = 0
        // in XSTUSD for dex_id = 1
        let fee = OutcomeFee::from_asset(dex_info.base_asset_id, fee_amount);

        Ok((SwapOutcome::new(calculated, fee), Self::quote_weight()))
    }

    fn step_quote(
        dex_id: &T::DEXId,
        input_asset_id: &AssetIdOf<T>,
        output_asset_id: &AssetIdOf<T>,
        amount: QuoteAmount<Balance>,
        recommended_samples_count: usize,
        deduce_fee: bool,
    ) -> Result<(DiscreteQuotation<AssetIdOf<T>, Balance>, Weight), DispatchError> {
        let mut quotation = DiscreteQuotation::new();

        if amount.amount().is_zero() {
            return Ok((quotation, Weight::zero()));
        }

        let samples_count = if recommended_samples_count < 1 {
            1
        } else {
            recommended_samples_count
        };

        let dex_info = T::DexInfoProvider::get_dex_info(dex_id)?;
        // Get pool account.
        let (_, tech_acc_id) = Pallet::<T>::tech_account_from_dex_and_asset_pair(
            *dex_id,
            *input_asset_id,
            *output_asset_id,
        )?;
        let pool_acc_id = technical::Pallet::<T>::tech_account_id_to_account_id(&tech_acc_id)?;

        // Get actual pool reserves.
        let (reserve_input, reserve_output, max_output_available) = Self::get_actual_reserves(
            &pool_acc_id,
            &dex_info.base_asset_id,
            &input_asset_id,
            &output_asset_id,
        )?;

        // Check reserves validity.
        if reserve_input == 0 && reserve_output == 0 {
            return Ok((quotation, Weight::zero()));
        }

        // Decide which side should be used for fee.
        let get_fee_from_destination = Pallet::<T>::decide_is_fee_from_destination(
            &dex_info.base_asset_id,
            input_asset_id,
            output_asset_id,
        )?;

        let variant = amount.variant();
        let amount = match amount {
            QuoteAmount::WithDesiredInput { desired_amount_in }
                if max_output_available == reserve_output =>
            {
                desired_amount_in
            }
            QuoteAmount::WithDesiredInput { desired_amount_in } => {
                let max_amount = Pallet::<T>::calc_input_for_exact_output(
                    T::GetFee::get(),
                    get_fee_from_destination,
                    &reserve_input,
                    &reserve_output,
                    &max_output_available,
                    deduce_fee,
                )
                .ok();
                quotation.limits.max_amount =
                    max_amount.map(|(calculated, _fee)| SideAmount::Input(calculated));
                if let Some((calculated, _fee)) = max_amount {
                    desired_amount_in.min(calculated)
                } else {
                    desired_amount_in
                }
            }
            QuoteAmount::WithDesiredOutput { desired_amount_out } => {
                let max_output = Pallet::<T>::calc_max_output(
                    T::GetFee::get(),
                    get_fee_from_destination,
                    reserve_output,
                    deduce_fee,
                )?;
                quotation.limits.max_amount =
                    Some(SideAmount::Output(max_output.min(max_output_available)));

                max_output.min(desired_amount_out).min(max_output_available)
            }
        };

        let common_step = amount
            .checked_div(samples_count as Balance)
            .ok_or(Error::<T>::FixedWrapperCalculationFailed)?;

        // volume & step
        let mut volumes = Vec::new();

        let mut remaining = amount;
        for i in 1..=samples_count - 1 {
            let volume = common_step
                .checked_mul(i as Balance)
                .ok_or(Error::<T>::FixedWrapperCalculationFailed)?;
            volumes.push((volume, common_step));
            remaining = remaining.saturating_sub(common_step);
        }
        volumes.push((amount, remaining));

        let mut sub_sum = Balance::zero();
        let mut sub_fee = Balance::zero();

        match variant {
            SwapVariant::WithDesiredInput => {
                for (volume, step) in volumes {
                    let (calculated, fee) = Pallet::<T>::calc_output_for_exact_input(
                        T::GetFee::get(),
                        get_fee_from_destination,
                        &reserve_input,
                        &reserve_output,
                        &volume,
                        deduce_fee,
                    )?;

                    let output = calculated.saturating_sub(sub_sum);

                    // in XOR for dex_id = 0
                    // in XSTUSD for dex_id = 1
                    let fee_chunk =
                        OutcomeFee::from_asset(dex_info.base_asset_id, fee.saturating_sub(sub_fee));

                    sub_sum = calculated;
                    sub_fee = fee;
                    quotation
                        .chunks
                        .push_back(SwapChunk::new(step, output, fee_chunk));
                }
            }
            SwapVariant::WithDesiredOutput => {
                for (volume, step) in volumes {
                    let (calculated, fee) = Pallet::<T>::calc_input_for_exact_output(
                        T::GetFee::get(),
                        get_fee_from_destination,
                        &reserve_input,
                        &reserve_output,
                        &volume,
                        deduce_fee,
                    )?;

                    let input = calculated.saturating_sub(sub_sum);

                    // in XOR for dex_id = 0
                    // in XSTUSD for dex_id = 1
                    let fee_chunk =
                        OutcomeFee::from_asset(dex_info.base_asset_id, fee.saturating_sub(sub_fee));

                    sub_sum = calculated;
                    sub_fee = fee;
                    quotation
                        .chunks
                        .push_back(SwapChunk::new(input, step, fee_chunk));
                }
            }
        }

        Ok((quotation, Self::step_quote_weight(samples_count)))
    }

    fn exchange(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        dex_id: &T::DEXId,
        input_asset_id: &AssetIdOf<T>,
        output_asset_id: &AssetIdOf<T>,
        swap_amount: SwapAmount<Balance>,
    ) -> Result<(SwapOutcome<Balance, AssetIdOf<T>>, Weight), DispatchError> {
        T::AssetRegulator::check_permission(
            &sender,
            &receiver,
            &input_asset_id,
            &common::permissions::TRANSFER,
        )
        .map_err(|_| Error::<T>::AssetRegulationsCheckFailed)?;

        T::AssetRegulator::check_permission(
            &sender,
            &receiver,
            &output_asset_id,
            &common::permissions::TRANSFER,
        )
        .map_err(|_| Error::<T>::AssetRegulationsCheckFailed)?;

        let dex_info = T::DexInfoProvider::get_dex_info(&dex_id)?;
        let (_, tech_acc_id) = Pallet::<T>::tech_account_from_dex_and_asset_pair(
            *dex_id,
            *input_asset_id,
            *output_asset_id,
        )?;
        let (source_amount, destination_amount) =
            Pallet::<T>::get_bounds_from_swap_amount(swap_amount.clone())?;
        let mut action = PolySwapActionStructOf::<T>::PairSwap(PairSwapActionOf::<T> {
            client_account: None,
            receiver_account: Some(receiver.clone()),
            pool_account: tech_acc_id,
            source: Resource {
                asset: *input_asset_id,
                amount: source_amount,
            },
            destination: Resource {
                asset: *output_asset_id,
                amount: destination_amount,
            },
            fee: Default::default(),
            fee_account: None,
            get_fee_from_destination: None,
            base_chameleon_asset: None,
            is_chameleon_pool: None,
            dex_id: *dex_id,
        });
        common::SwapRulesValidation::<AccountIdOf<T>, TechAccountIdOf<T>, AssetIdOf<T>, T>::prepare_and_validate(
            &mut action,
            Some(sender),
            &dex_info.base_asset_id,
        )?;

        // It is guarantee that unwrap is always ok.
        // Clone is used here because action is used for create_swap_unchecked.
        let retval = match action.clone() {
            PolySwapAction::PairSwap(a) => {
                let (fee, amount) = match swap_amount {
                    SwapAmount::WithDesiredInput { .. } => (a.fee, a.destination.amount.unwrap()),
                    SwapAmount::WithDesiredOutput { .. } => (a.fee, a.source.amount.unwrap()),
                };
                Ok((
                    common::prelude::SwapOutcome::new(amount, fee),
                    Self::exchange_weight(),
                ))
            }
            _ => unreachable!("we know that always PairSwap is used"),
        };

        let action = T::PolySwapAction::from(action);
        let mut action = action.into();
        technical::Pallet::<T>::create_swap_unchecked(
            sender.clone(),
            &mut action,
            &dex_info.base_asset_id,
        )?;

        retval
    }

    fn check_rewards(
        _target_id: &T::DEXId,
        _input_asset_id: &AssetIdOf<T>,
        _output_asset_id: &AssetIdOf<T>,
        _input_amount: Balance,
        _output_amount: Balance,
    ) -> Result<(Vec<(Balance, AssetIdOf<T>, RewardReason)>, Weight), DispatchError> {
        // XYK Pool has no rewards currently
        Ok((Vec::new(), Weight::zero()))
    }

    fn quote_without_impact(
        dex_id: &T::DEXId,
        input_asset_id: &AssetIdOf<T>,
        output_asset_id: &AssetIdOf<T>,
        amount: QuoteAmount<Balance>,
        deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance, AssetIdOf<T>>, DispatchError> {
        let dex_info = T::DexInfoProvider::get_dex_info(dex_id)?;
        // Get pool account.
        let (_, tech_acc_id) = Pallet::<T>::tech_account_from_dex_and_asset_pair(
            *dex_id,
            *input_asset_id,
            *output_asset_id,
        )?;
        let pool_acc_id = technical::Pallet::<T>::tech_account_id_to_account_id(&tech_acc_id)?;

        // Get actual pool reserves.
        let (reserve_input, reserve_output, _max_output_available) = Self::get_actual_reserves(
            &pool_acc_id,
            &dex_info.base_asset_id,
            &input_asset_id,
            &output_asset_id,
        )?;

        // Check reserves validity.
        if reserve_input == 0 && reserve_output == 0 {
            fail!(Error::<T>::PoolIsEmpty);
        } else if reserve_input <= 0 || reserve_output <= 0 {
            fail!(Error::<T>::PoolIsInvalid);
        }

        // Decide which side should be used for fee.
        let get_fee_from_destination = Pallet::<T>::decide_is_fee_from_destination(
            &dex_info.base_asset_id,
            input_asset_id,
            output_asset_id,
        )?;

        let input_price_wrt_output = FixedWrapper::from(reserve_output) / reserve_input;
        let fee_fraction = if deduce_fee {
            T::GetFee::get()
        } else {
            common::Fixed::default()
        };
        let (calculated, fee_amount) = match amount {
            QuoteAmount::WithDesiredInput { desired_amount_in } => {
                let (output, fee_amount) = if get_fee_from_destination {
                    // output token is xor, user indicates desired input amount
                    // y_1 = x_in * y / x
                    // y_out = y_1 * (1 - fee)
                    let out_with_fee =
                        FixedWrapper::from(desired_amount_in) * input_price_wrt_output;
                    let output = FixedWrapper::from(out_with_fee.clone())
                        * (fixed_wrapper!(1) - fee_fraction);
                    let fee_amount = out_with_fee - output.clone();
                    (output, fee_amount)
                } else {
                    // input token is xor, user indicates desired input amount
                    // x_1 = x_in * (1 - fee)
                    // y_out = x_1 * y / x
                    let input_without_fee = FixedWrapper::from(desired_amount_in.clone())
                        * (fixed_wrapper!(1) - fee_fraction);
                    let output = input_without_fee.clone() * input_price_wrt_output;
                    let fee_amount = FixedWrapper::from(desired_amount_in) - input_without_fee;
                    (output, fee_amount)
                };

                let calculated = output
                    .try_into_balance()
                    .map_err(|_| Error::<T>::FailedToCalculatePriceWithoutImpact)?;
                let fee_amount = fee_amount
                    .try_into_balance()
                    .map_err(|_| Error::<T>::FailedToCalculatePriceWithoutImpact)?;

                (calculated, fee_amount)
            }
            QuoteAmount::WithDesiredOutput { desired_amount_out } => {
                let (input, fee_amount) = if get_fee_from_destination {
                    // output token is xor, user indicates desired output amount:
                    // y_1 = y_out / (1 - fee)
                    // x_in = y_1 / y / x
                    let output_with_fee = FixedWrapper::from(desired_amount_out.clone())
                        / (fixed_wrapper!(1) - fee_fraction);
                    let fee_amount =
                        output_with_fee.clone() - FixedWrapper::from(desired_amount_out);
                    let input = output_with_fee / input_price_wrt_output;
                    (input, fee_amount)
                } else {
                    // input token is xor, user indicates desired output amount:
                    // x_in = (y_out / y / x) / (1 - fee)
                    let input_without_fee =
                        FixedWrapper::from(desired_amount_out) / input_price_wrt_output;
                    let input = input_without_fee.clone() / (fixed_wrapper!(1) - fee_fraction);
                    let fee_amount = input.clone() - input_without_fee;
                    (input, fee_amount)
                };

                let calculated = input
                    .try_into_balance()
                    .map_err(|_| Error::<T>::FailedToCalculatePriceWithoutImpact)?;
                let fee_amount = fee_amount
                    .try_into_balance()
                    .map_err(|_| Error::<T>::FailedToCalculatePriceWithoutImpact)?;

                (calculated, fee_amount)
            }
        };

        // in XOR for dex_id = 0
        // in XSTUSD for dex_id = 1
        let fee = OutcomeFee::from_asset(dex_info.base_asset_id, fee_amount);
        Ok(SwapOutcome::new(calculated, fee))
    }

    fn quote_weight() -> Weight {
        <T as Config>::WeightInfo::quote()
    }

    fn step_quote_weight(samples_count: usize) -> Weight {
        <T as Config>::WeightInfo::step_quote(samples_count as u32)
    }

    fn exchange_weight() -> Weight {
        <T as Config>::WeightInfo::swap_pair()
    }

    fn check_rewards_weight() -> Weight {
        Weight::zero()
    }
}

impl<T: Config> GetPoolReserves<AssetIdOf<T>> for Pallet<T> {
    fn reserves(base_asset: &AssetIdOf<T>, other_asset: &AssetIdOf<T>) -> (Balance, Balance) {
        Reserves::<T>::get(base_asset, other_asset)
    }
}

pub use pallet::*;
use sp_runtime::traits::Zero;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::{
        AccountIdOf, AssetName, AssetRegulator, AssetSymbol, BalancePrecision, ContentSource,
        Description, EnabledSourcesManager, Fixed, GetMarketInfo, OnPoolCreated,
    };
    use frame_support::pallet_prelude::*;
    use frame_support::sp_runtime::Percent;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;
    use orml_traits::GetByKey;

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + technical::Config
        + ceres_liquidity_locker::Config
        + demeter_farming_platform::Config
        + permissions::Config
        + orml_tokens::Config
    {
        /// The minimum amount of XOR to deposit as liquidity
        const MIN_XOR: Balance;

        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        //TODO: implement and use + Into<SwapActionOf<T> for this types.
        type PairSwapAction: common::SwapAction<AccountIdOf<Self>, TechAccountIdOf<Self>, AssetIdOf<Self>, Self>
            + Parameter;
        type DepositLiquidityAction: common::SwapAction<AccountIdOf<Self>, TechAccountIdOf<Self>, AssetIdOf<Self>, Self>
            + Parameter;
        type WithdrawLiquidityAction: common::SwapAction<AccountIdOf<Self>, TechAccountIdOf<Self>, AssetIdOf<Self>, Self>
            + Parameter;
        type PolySwapAction: common::SwapAction<AccountIdOf<Self>, TechAccountIdOf<Self>, AssetIdOf<Self>, Self>
            + Parameter
            + Into<<Self as technical::Config>::SwapAction>
            + From<PolySwapActionStructOf<Self>>;
        type EnsureDEXManager: EnsureDEXManager<Self::DEXId, Self::AccountId, DispatchError>;
        type TradingPairSourceManager: TradingPairSourceManager<Self::DEXId, AssetIdOf<Self>>;
        type DexInfoProvider: DexInfoProvider<Self::DEXId, DEXInfo<AssetIdOf<Self>>>;
        type EnabledSourcesManager: EnabledSourcesManager<Self::DEXId, AssetIdOf<Self>>;
        type EnsureTradingPairExists: EnsureTradingPairExists<
            Self::DEXId,
            AssetIdOf<Self>,
            DispatchError,
        >;
        type XSTMarketInfo: GetMarketInfo<AssetIdOf<Self>>;
        type GetFee: Get<Fixed>;
        /// Maximum allowed ratio between real and current issuance in pool
        type GetMaxIssuanceRatio: Get<Fixed>;
        type OnPoolCreated: OnPoolCreated<AccountId = AccountIdOf<Self>, DEXId = DEXIdOf<Self>>;
        type OnPoolReservesChanged: OnPoolReservesChanged<AssetIdOf<Self>>;
        type GetTradingPairRestrictedFlag: GetByKey<TradingPair<AssetIdOf<Self>>, bool>;
        /// base_asset_id => (chameleon_base_asset_id, target_assets)
        type GetChameleonPools: GetByKey<
            AssetIdOf<Self>,
            Option<(AssetIdOf<Self>, BTreeSet<AssetIdOf<Self>>)>,
        >;
        /// To retrieve asset info
        type AssetInfoProvider: AssetInfoProvider<
            AssetIdOf<Self>,
            Self::AccountId,
            AssetSymbol,
            AssetName,
            BalancePrecision,
            ContentSource,
            Description,
        >;

        /// Regulator of asset operations
        type AssetRegulator: AssetRegulator<Self::AccountId, AssetIdOf<Self>>;

        /// Percent of reserve which is not involved in swap
        #[pallet::constant]
        type IrreducibleReserve: Get<Percent>;
        /// How often to check and adjust Chameleon pool issuance
        #[pallet::constant]
        type PoolAdjustPeriod: Get<BlockNumberFor<Self>>;
        /// Weight information for extrinsics in this pallet.
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
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_runtime_upgrade() -> Weight {
            migrations::migrate::<T>()
        }

        fn on_initialize(now: BlockNumberFor<T>) -> Weight {
            if now % T::PoolAdjustPeriod::get() != Zero::zero() {
                return Default::default();
            }
            let mut weight = Weight::default();
            for dex_id in T::DexInfoProvider::list_dex_ids() {
                weight = weight.saturating_add(T::DbWeight::get().reads(2));
                let Ok(dex_info) = T::DexInfoProvider::get_dex_info(&dex_id) else {
                    frame_support::log::warn!("Failed to get DEX info for {:?}", dex_id);
                    continue;
                };
                let Some((_, targets)) = <T::GetChameleonPools as orml_traits::GetByKey<_, _>>::get(&dex_info.base_asset_id) else {
                    continue;
                };
                for target in targets {
                    if let Err(err) = Self::adjust_liquidity_in_pool(
                        dex_id,
                        &dex_info.base_asset_id,
                        &target,
                        &mut weight,
                    ) {
                        frame_support::log::warn!(
                            "Failed to adjust liquidity for [{:?}] {:?} -> {:?}: {:?}",
                            dex_id,
                            dex_info.base_asset_id,
                            target,
                            err
                        );
                    }
                }
            }
            weight
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::deposit_liquidity())]
        pub fn deposit_liquidity(
            origin: OriginFor<T>,
            dex_id: DEXIdOf<T>,
            input_asset_a: AssetIdOf<T>,
            input_asset_b: AssetIdOf<T>,
            input_a_desired: Balance,
            input_b_desired: Balance,
            input_a_min: Balance,
            input_b_min: Balance,
        ) -> DispatchResultWithPostInfo {
            let source = ensure_signed(origin)?;

            ensure!(
                !<T as Config>::AssetInfoProvider::is_non_divisible(&input_asset_a)
                    && !<T as Config>::AssetInfoProvider::is_non_divisible(&input_asset_b),
                Error::<T>::UnableToOperateWithIndivisibleAssets
            );
            ensure!(
                input_a_desired > 0 && input_a_min > 0,
                Error::<T>::InvalidDepositLiquidityBasicAssetAmount
            );
            ensure!(
                input_b_desired > 0 && input_b_min > 0,
                Error::<T>::InvalidDepositLiquidityTargetAssetAmount
            );
            ensure!(
                input_a_desired >= input_a_min && input_b_desired >= input_b_min,
                Error::<T>::InvalidMinimumBoundValueOfBalance
            );

            T::AssetRegulator::check_permission(
                &source,
                &source,
                &input_asset_a,
                &common::permissions::TRANSFER,
            )
            .map_err(|_| Error::<T>::AssetRegulationsCheckFailed)?;

            T::AssetRegulator::check_permission(
                &source,
                &source,
                &input_asset_b,
                &common::permissions::TRANSFER,
            )
            .map_err(|_| Error::<T>::AssetRegulationsCheckFailed)?;

            Pallet::<T>::deposit_liquidity_unchecked(
                source,
                dex_id,
                input_asset_a,
                input_asset_b,
                input_a_desired,
                input_b_desired,
                input_a_min,
                input_b_min,
            )?;
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::withdraw_liquidity())]
        pub fn withdraw_liquidity(
            origin: OriginFor<T>,
            dex_id: DEXIdOf<T>,
            output_asset_a: AssetIdOf<T>,
            output_asset_b: AssetIdOf<T>,
            marker_asset_desired: Balance,
            output_a_min: Balance,
            output_b_min: Balance,
        ) -> DispatchResultWithPostInfo {
            let source = ensure_signed(origin)?;

            ensure!(
                !<T as Config>::AssetInfoProvider::is_non_divisible(&output_asset_a)
                    && !<T as Config>::AssetInfoProvider::is_non_divisible(&output_asset_b),
                Error::<T>::UnableToOperateWithIndivisibleAssets
            );
            ensure!(
                output_a_min > 0,
                Error::<T>::InvalidWithdrawLiquidityBasicAssetAmount
            );
            ensure!(
                output_b_min > 0,
                Error::<T>::InvalidWithdrawLiquidityTargetAssetAmount
            );

            T::AssetRegulator::check_permission(
                &source,
                &source,
                &output_asset_a,
                &common::permissions::TRANSFER,
            )
            .map_err(|_| Error::<T>::AssetRegulationsCheckFailed)?;

            T::AssetRegulator::check_permission(
                &source,
                &source,
                &output_asset_b,
                &common::permissions::TRANSFER,
            )
            .map_err(|_| Error::<T>::AssetRegulationsCheckFailed)?;

            Pallet::<T>::withdraw_liquidity_unchecked(
                source,
                dex_id,
                output_asset_a,
                output_asset_b,
                marker_asset_desired,
                output_a_min,
                output_b_min,
            )?;
            Ok(().into())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::initialize_pool())]
        pub fn initialize_pool(
            origin: OriginFor<T>,
            dex_id: DEXIdOf<T>,
            asset_a: AssetIdOf<T>,
            asset_b: AssetIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            common::with_transaction(|| {
                let source = ensure_signed(origin.clone())?;
                <T as Config>::EnsureDEXManager::ensure_can_manage(
                    &dex_id,
                    origin.clone(),
                    ManagementMode::Public,
                )?;

                ensure!(
                    !<T as Config>::AssetInfoProvider::is_non_divisible(&asset_a)
                        && !<T as Config>::AssetInfoProvider::is_non_divisible(&asset_b),
                    Error::<T>::UnableToCreatePoolWithIndivisibleAssets
                );

                let (trading_pair, tech_account_id, fees_account_id) =
                    Pallet::<T>::initialize_pool_unchecked(
                        source.clone(),
                        dex_id,
                        asset_a,
                        asset_b,
                    )?;

                Pallet::<T>::ensure_trading_pair_is_not_restricted(
                    &trading_pair.map(|a| Into::<AssetIdOf<T>>::into(a)),
                )?;

                let ta_repr =
                    technical::Pallet::<T>::tech_account_id_to_account_id(&tech_account_id)?;
                let fees_ta_repr =
                    technical::Pallet::<T>::tech_account_id_to_account_id(&fees_account_id)?;
                Pallet::<T>::initialize_pool_properties(
                    &dex_id,
                    &asset_a,
                    &asset_b,
                    &ta_repr,
                    &fees_ta_repr,
                )?;
                let (_, pool_account) =
                    Pallet::<T>::tech_account_from_dex_and_asset_pair(dex_id, asset_a, asset_b)?;
                let pool_account =
                    technical::Pallet::<T>::tech_account_id_to_account_id(&pool_account)?;
                T::OnPoolCreated::on_pool_created(fees_ta_repr, dex_id, pool_account)?;
                Self::deposit_event(Event::PoolIsInitialized(ta_repr));
                Ok(().into())
            })
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        // New pool for particular pair was initialized. [Reserves Account Id]
        PoolIsInitialized(AccountIdOf<T>),
        PoolAdjusted {
            /// Pool account
            pool: AccountIdOf<T>,
            /// Issuance before adjustment
            old_issuance: Balance,
            /// Issuance after adjustment
            new_issuance: Balance,
            /// Amount of processed pool providers
            providers: u32,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// It is impossible to calculate fee for some pair swap operation, or other operation.
        UnableToCalculateFee,
        /// Failure while calculating price ignoring non-linearity of liquidity source.
        FailedToCalculatePriceWithoutImpact,
        /// Is is impossible to get balance.
        UnableToGetBalance,
        /// Impossible to decide asset pair amounts.
        ImpossibleToDecideAssetPairAmounts,
        /// Pool pair ratio and pair swap ratio are different.
        PoolPairRatioAndPairSwapRatioIsDifferent,
        /// Pair swap action fee is smaller than recommended.
        PairSwapActionFeeIsSmallerThanRecommended,
        /// Source balance is not large enough.
        SourceBalanceIsNotLargeEnough,
        /// Target balance is not large enough.
        TargetBalanceIsNotLargeEnough,
        /// It is not possible to derive fee account.
        UnableToDeriveFeeAccount,
        /// The fee account is invalid.
        FeeAccountIsInvalid,
        /// Source and client accounts do not match as equal.
        SourceAndClientAccountDoNotMatchAsEqual,
        /// In this case assets must not be same.
        AssetsMustNotBeSame,
        /// Impossible to decide deposit liquidity amounts.
        ImpossibleToDecideDepositLiquidityAmounts,
        /// Invalid deposit liquidity base asset amount.
        InvalidDepositLiquidityBasicAssetAmount,
        /// Invalid deposit liquidity target asset amount.
        InvalidDepositLiquidityTargetAssetAmount,
        /// Pair swap action minimum liquidity is smaller than recommended.
        PairSwapActionMinimumLiquidityIsSmallerThanRecommended,
        /// Destination amount of liquidity is not large enough.
        DestinationAmountOfLiquidityIsNotLargeEnough,
        /// Source base amount is not large enough.
        SourceBaseAmountIsNotLargeEnough,
        /// Target base amount is not large enough.
        TargetBaseAmountIsNotLargeEnough,
        /// The balance structure of pool is invalid.
        PoolIsInvalid,
        /// The pool has empty liquidity.
        PoolIsEmpty,
        /// Amount parameter has zero value, it is invalid.
        ZeroValueInAmountParameter,
        /// The account balance is invalid.
        AccountBalanceIsInvalid,
        /// Invalid deposit liquidity destination amount.
        InvalidDepositLiquidityDestinationAmount,
        /// Initial liquidity deposit ratio must be defined.
        InitialLiqudityDepositRatioMustBeDefined,
        /// Technical asset is not representable.
        TechAssetIsNotRepresentable,
        /// Unable or impossible to decide marker asset.
        UnableToDecideMarkerAsset,
        /// Unable or impossible to get asset representation.
        UnableToGetAssetRepr,
        /// Impossible to decide withdraw liquidity amounts.
        ImpossibleToDecideWithdrawLiquidityAmounts,
        /// Invalid withdraw liquidity base asset amount.
        InvalidWithdrawLiquidityBasicAssetAmount,
        /// Invalid withdraw liquidity target asset amount.
        InvalidWithdrawLiquidityTargetAssetAmount,
        /// Source base amount is too large.
        SourceBaseAmountIsTooLarge,
        /// Source balance of liquidity is not large enough.
        SourceBalanceOfLiquidityTokensIsNotLargeEnough,
        /// Destination base balance is not large enough.
        DestinationBaseBalanceIsNotLargeEnough,
        /// Destination base balance is not large enough.
        DestinationTargetBalanceIsNotLargeEnough,
        /// Asset for liquidity marking is invalid.
        InvalidAssetForLiquidityMarking,
        /// Error in asset decoding.
        AssetDecodingError,
        /// Calculated value is out of desired bounds.
        CalculatedValueIsOutOfDesiredBounds,
        /// The base asset is not matched with any asset arguments.
        BaseAssetIsNotMatchedWithAnyAssetArguments,
        /// Some values need to be same, the destination amount must be same.
        DestinationAmountMustBeSame,
        /// Some values need to be same, the source amount must be same.
        SourceAmountMustBeSame,
        /// The pool initialization is invalid and has failed.
        PoolInitializationIsInvalid,
        /// The pool is already initialized.
        PoolIsAlreadyInitialized,
        /// The minimum bound values of balance are invalid.
        InvalidMinimumBoundValueOfBalance,
        /// It is impossible to decide valid pair values from range for this pool.
        ImpossibleToDecideValidPairValuesFromRangeForThisPool,
        /// This range values is not validy by rules of correct range.
        RangeValuesIsInvalid,
        /// The values that is calculated is out out of required bounds.
        CalculatedValueIsNotMeetsRequiredBoundaries,
        /// In this case getting fee from destination is impossible.
        GettingFeeFromDestinationIsImpossible,
        /// Math calculation with fixed number has failed to complete.
        FixedWrapperCalculationFailed,
        /// This case if not supported by logic of pool of validation code.
        ThisCaseIsNotSupported,
        /// Pool becomes invalid after operation.
        PoolBecameInvalidAfterOperation,
        /// Unable to convert asset to tech asset id.
        UnableToConvertAssetToTechAssetId,
        /// Unable to get XOR part from marker asset.
        UnableToGetXORPartFromMarkerAsset,
        /// Pool token supply has reached limit of data type.
        PoolTokenSupplyOverflow,
        /// Couldn't increase reference counter for the account that adds liquidity.
        /// It is expected to never happen because if the account has funds to add liquidity, it has a provider from balances.
        IncRefError,
        /// Unable to provide liquidity because its XOR part is lesser than the minimum value (0.007)
        UnableToDepositXorLessThanMinimum,
        /// Attempt to quote via unsupported path, i.e. both output and input tokens are not XOR.
        UnsupportedQuotePath,
        /// Not enough unlocked liquidity to withdraw
        NotEnoughUnlockedLiquidity,
        /// Cannot create a pool with indivisible assets
        UnableToCreatePoolWithIndivisibleAssets,
        /// Unable to proceed operation with indivisible assets
        UnableToOperateWithIndivisibleAssets,
        /// Not enough liquidity out of farming to withdraw
        NotEnoughLiquidityOutOfFarming,
        /// Cannot create a pool with restricted target asset
        TargetAssetIsRestricted,
        /// Restricted Chameleon pool
        RestrictedChameleonPool,
        /// Output asset reserves is not enough
        NotEnoughOutputReserves,
        /// Asset Regulations Check failed
        AssetRegulationsCheckFailed,
    }

    /// Updated after last liquidity change operation.
    /// [Base Asset Id (XOR) -> Target Asset Id => (Base Balance, Target Balance)].
    /// This storage records is not used as source of information, but used as quick cache for
    /// information that comes from balances for assets from technical accounts.
    /// For example, communication with technical accounts and their storage is not needed, and this
    /// pair to balance cache can be used quickly.
    #[pallet::storage]
    #[pallet::getter(fn reserves)]
    pub type Reserves<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        AssetIdOf<T>,
        Blake2_128Concat,
        AssetIdOf<T>,
        (Balance, Balance),
        ValueQuery,
    >;

    /// Liquidity providers of particular pool.
    /// Pool account => Liquidity provider account => Pool token balance
    #[pallet::storage]
    #[pallet::getter(fn pool_providers)]
    pub type PoolProviders<T: Config> =
        StorageDoubleMap<_, Identity, AccountIdOf<T>, Identity, AccountIdOf<T>, Balance>;

    /// Set of pools in which accounts have some share.
    /// Liquidity provider account => Target Asset of pair (assuming base asset is XOR)
    #[pallet::storage]
    #[pallet::getter(fn account_pools)]
    pub type AccountPools<T: Config> = StorageDoubleMap<
        _,
        Identity,
        AccountIdOf<T>,
        Blake2_128Concat,
        AssetIdOf<T>,
        BTreeSet<AssetIdOf<T>>,
        ValueQuery,
    >;

    /// Total issuance of particular pool.
    /// Pool account => Total issuance
    #[pallet::storage]
    pub type TotalIssuances<T: Config> = StorageMap<_, Identity, AccountIdOf<T>, Balance>;

    /// Properties of particular pool. Base Asset => Target Asset => (Reserves Account Id, Fees Account Id)
    #[pallet::storage]
    #[pallet::getter(fn properties)]
    pub type Properties<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        AssetIdOf<T>,
        Blake2_128Concat,
        AssetIdOf<T>,
        (T::AccountId, T::AccountId),
    >;
}
