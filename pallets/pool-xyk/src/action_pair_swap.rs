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

use frame_support::dispatch::DispatchResult;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use frame_support::{dispatch, ensure};

use common::prelude::{Balance, FixedWrapper};
use common::{balance, AssetInfoProvider, DexInfoProvider};
use sp_runtime::traits::Zero;

use crate::to_fixed_wrapper;

use crate::bounds::*;

use crate::aliases::{AccountIdOf, AssetIdOf, DEXIdOf, TechAccountIdOf};
use crate::operations::*;
use crate::{Config, Error, Pallet};

impl<T: Config> common::SwapRulesValidation<AccountIdOf<T>, TechAccountIdOf<T>, AssetIdOf<T>, T>
    for PairSwapAction<DEXIdOf<T>, AssetIdOf<T>, AccountIdOf<T>, TechAccountIdOf<T>>
{
    fn is_abstract_checking(&self) -> bool {
        self.source.amount == Bounds::Dummy || self.destination.amount == Bounds::Dummy
    }

    fn prepare_and_validate(
        &mut self,
        source_opt: Option<&AccountIdOf<T>>,
        base_asset_id: &AssetIdOf<T>,
    ) -> DispatchResult {
        let abstract_checking_from_method = common::SwapRulesValidation::<
            AccountIdOf<T>,
            TechAccountIdOf<T>,
            AssetIdOf<T>,
            T,
        >::is_abstract_checking(self);
        let abstract_checking = source_opt.is_none() || abstract_checking_from_method;
        let abstract_checking_for_quote = source_opt.is_none() && !abstract_checking_from_method;

        // Check that client account is same as source, because signature is checked for source.
        // Signature checking is used in extrinsics for example, and source is derived from origin.
        // TODO: In general case it is possible to use different client account, for example if
        // signature of source is legal for some source accounts.
        if !abstract_checking {
            let source = source_opt.unwrap();
            match &self.client_account {
                // Just use `client_account` as copy of source.
                None => {
                    self.client_account = Some(source.clone());
                }
                Some(ca) => {
                    if ca != source {
                        Err(Error::<T>::SourceAndClientAccountDoNotMatchAsEqual)?;
                    }
                }
            }

            // Dealing with receiver account, for example case then not swapping to self, but to
            // other account.
            match &self.receiver_account {
                // Just use `client_account` as same account, swapping to self.
                None => {
                    self.receiver_account = self.client_account.clone();
                }
                _ => (),
            }
        }

        let pool_account_repr_sys =
            technical::Pallet::<T>::tech_account_id_to_account_id(&self.pool_account)?;
        // Check that pool account is valid.
        Pallet::<T>::is_pool_account_valid_for(self.source.asset, &self.pool_account)?;

        // Source balance of source account.
        let balance_ss = if abstract_checking {
            None
        } else {
            Some(<T as Config>::AssetInfoProvider::free_balance(
                &self.source.asset,
                &source_opt.unwrap(),
            )?)
        };
        let (balance_st, balance_tt) = Pallet::<T>::get_actual_reserves(
            &pool_account_repr_sys,
            &base_asset_id,
            &self.source.asset,
            &self.destination.asset,
        )?;
        if !abstract_checking {
            ensure!(balance_ss.unwrap() > 0, Error::<T>::AccountBalanceIsInvalid);
        }
        if balance_st == 0 && balance_tt == 0 {
            Err(Error::<T>::PoolIsEmpty)?;
        } else if balance_st <= 0 || balance_tt <= 0 {
            Err(Error::<T>::PoolIsInvalid)?;
        }

        match self.get_fee_from_destination {
            None => {
                let is_fee_from_d = Pallet::<T>::decide_is_fee_from_destination(
                    base_asset_id,
                    &self.source.asset,
                    &self.destination.asset,
                )?;
                self.get_fee_from_destination = Some(is_fee_from_d);
            }
            _ => (),
        }

        // Recommended fee, will be used if fee is not specified or for checking if specified.
        let mut recom_fee = Balance::zero();

        if abstract_checking_for_quote || !abstract_checking {
            match (self.source.amount, self.destination.amount) {
                // Case then both source and destination amounts is specified, just checking it.
                (Bounds::Desired(sa), Bounds::Desired(ta)) => {
                    ensure!(sa > 0, Error::<T>::ZeroValueInAmountParameter);
                    ensure!(ta > 0, Error::<T>::ZeroValueInAmountParameter);
                    let y_out_pair = Pallet::<T>::calc_output_for_exact_input(
                        T::GetFee::get(),
                        self.get_fee_from_destination.unwrap(),
                        &balance_st,
                        &balance_tt,
                        &sa,
                        true,
                    )?;
                    let x_in_pair = Pallet::<T>::calc_input_for_exact_output(
                        T::GetFee::get(),
                        self.get_fee_from_destination.unwrap(),
                        &balance_st,
                        &balance_tt,
                        &ta,
                        true,
                    )?;
                    if y_out_pair.0 != ta || x_in_pair.0 != sa || y_out_pair.1 != x_in_pair.1 {
                        Err(Error::<T>::PoolPairRatioAndPairSwapRatioIsDifferent)?;
                    }
                    recom_fee = y_out_pair.1;
                }
                // Case then source amount is specified but destination is not, it`s possible to decide it.
                (Bounds::Desired(sa), ta_bnd) => {
                    ensure!(sa > 0, Error::<T>::ZeroValueInAmountParameter);
                    match ta_bnd {
                        Bounds::Min(ta_min) => {
                            let (calculated, fee) = Pallet::<T>::calc_output_for_exact_input(
                                T::GetFee::get(),
                                self.get_fee_from_destination.unwrap(),
                                &balance_st,
                                &balance_tt,
                                &sa,
                                true,
                            )?;

                            ensure!(
                                calculated >= ta_min,
                                Error::<T>::CalculatedValueIsOutOfDesiredBounds
                            );
                            self.destination.amount = Bounds::Calculated(calculated);
                            recom_fee = fee;
                        }
                        _ => {
                            Err(Error::<T>::ImpossibleToDecideAssetPairAmounts)?;
                        }
                    }
                }
                // Case then destination amount is specified but source is not, it`s possible to decide it.
                (sa_bnd, Bounds::Desired(ta)) => {
                    ensure!(ta > 0, Error::<T>::ZeroValueInAmountParameter);
                    match sa_bnd {
                        Bounds::Max(sa_max) => {
                            let (calculated, fee) = Pallet::<T>::calc_input_for_exact_output(
                                T::GetFee::get(),
                                self.get_fee_from_destination.unwrap(),
                                &balance_st,
                                &balance_tt,
                                &ta,
                                true,
                            )?;

                            ensure!(
                                calculated <= sa_max,
                                Error::<T>::CalculatedValueIsOutOfDesiredBounds
                            );
                            self.source.amount = Bounds::Calculated(calculated);
                            recom_fee = fee;
                        }
                        _ => {
                            Err(Error::<T>::ImpossibleToDecideAssetPairAmounts)?;
                        }
                    }
                }
                // Case then no amount is specified, impossible to decide any amounts.
                (_, _) => {
                    Err(Error::<T>::ImpossibleToDecideAssetPairAmounts)?;
                }
            }
        }

        // Check fee account if it is specified, or set it if not.
        match self.fee_account {
            Some(ref fa) => {
                // Checking that fee account is valid for this set of parameters.
                Pallet::<T>::is_fee_account_valid_for(self.source.asset, &self.pool_account, fa)?;
            }
            None => {
                let fa = Pallet::<T>::get_fee_account(&self.pool_account)?;
                self.fee_account = Some(fa);
            }
        }

        if abstract_checking_for_quote || !abstract_checking {
            let source_amount = self.source.amount.unwrap();
            let destination_amount = self.destination.amount.unwrap();

            let dex_info = T::DexInfoProvider::get_dex_info(&self.dex_id)?;

            // in XOR for dex_id = 0
            // in XSTUSD for dex_id = 1
            let fee = self.fee.get_by_asset(&dex_info.base_asset_id);

            // Set recommended or check that fee is correct.
            if fee.is_zero() {
                self.fee.add_by_asset(dex_info.base_asset_id, recom_fee);
            } else {
                if fee < recom_fee {
                    Err(Error::<T>::PairSwapActionFeeIsSmallerThanRecommended)?;
                }
            }

            if !abstract_checking {
                // Checking that balances if correct and large enouth for amounts.
                if self.get_fee_from_destination.unwrap() {
                    // For source account balance must be not smaller than required with fee.
                    if balance_ss.unwrap() < source_amount {
                        Err(Error::<T>::SourceBalanceIsNotLargeEnough)?;
                    }

                    /*
                    TODO: find correct solution.
                    // For destination technical account balance must successful large for this swap.
                    if balance_tt - fee < destination_amount {
                        Err(Error::<T>::TargetBalanceIsNotLargeEnough)?;
                    }
                    if (self.destination.amount.unwrap() - self.fee.unwrap()) <= 0 {
                        Err(Error::<T>::GettingFeeFromDestinationIsImpossible)?;
                    }
                    */

                    if balance_tt < destination_amount {
                        Err(Error::<T>::TargetBalanceIsNotLargeEnough)?;
                    }
                } else {
                    /*
                    TODO: find correct solution.
                    // For source account balance must be not smaller than required with fee.
                    if balance_ss.unwrap() - fee < source_amount {
                        Err(Error::<T>::SourceBalanceIsNotLargeEnough)?;
                    }
                    */

                    if balance_ss.unwrap() < source_amount {
                        Err(Error::<T>::SourceBalanceIsNotLargeEnough)?;
                    }

                    // For destination technical account balance must successful large for this swap.
                    if balance_tt < destination_amount {
                        Err(Error::<T>::TargetBalanceIsNotLargeEnough)?;
                    }
                }
            }
        }
        if abstract_checking {
            return Ok(());
        }
        // check if k has not turned to 0
        let pool_is_valid_after_op_test = {
            let fxw_x =
                to_fixed_wrapper!(balance_st) + to_fixed_wrapper!(self.source.amount.unwrap());
            let fxw_y =
                to_fixed_wrapper!(balance_tt) - to_fixed_wrapper!(self.destination.amount.unwrap());
            fxw_x.try_into_balance().unwrap_or(balance!(0)) != balance!(0)
                && fxw_y.try_into_balance().unwrap_or(balance!(0)) != balance!(0)
        };
        ensure!(
            pool_is_valid_after_op_test,
            Error::<T>::PoolBecameInvalidAfterOperation
        );
        Ok(())
    }
    fn instant_auto_claim_used(&self) -> bool {
        true
    }
    fn triggered_auto_claim_used(&self) -> bool {
        false
    }
    fn is_able_to_claim(&self) -> bool {
        true
    }
}

impl<T: Config> common::SwapAction<AccountIdOf<T>, TechAccountIdOf<T>, AssetIdOf<T>, T>
    for PairSwapAction<DEXIdOf<T>, AssetIdOf<T>, AccountIdOf<T>, TechAccountIdOf<T>>
{
    /// This function is called after validation, and every `Option` is `Some`, and it is safe to do
    /// unwrap. `Bounds` is also safe to unwrap.
    fn reserve(
        &self,
        source: &AccountIdOf<T>,
        base_asset_id: &AssetIdOf<T>,
    ) -> dispatch::DispatchResult {
        common::with_transaction(|| {
            if Some(source) != self.client_account.as_ref() {
                let e = Error::<T>::SourceAndClientAccountDoNotMatchAsEqual.into();
                return Err(e);
            }
            ensure!(
                Some(source) == self.client_account.as_ref(),
                Error::<T>::SourceAndClientAccountDoNotMatchAsEqual
            );
            let fee_account_repr_sys = technical::Pallet::<T>::tech_account_id_to_account_id(
                self.fee_account.as_ref().unwrap(),
            )?;

            let dex_info = T::DexInfoProvider::get_dex_info(&self.dex_id)?;

            // in XOR for dex_id = 0
            // in XSTUSD for dex_id = 1
            let fee = self.fee.get_by_asset(&dex_info.base_asset_id);

            if self.get_fee_from_destination.unwrap() {
                technical::Pallet::<T>::transfer_in(
                    &self.source.asset,
                    &source,
                    &self.pool_account,
                    self.source.amount.unwrap(),
                )?;
                technical::Pallet::<T>::transfer_out(
                    &self.destination.asset,
                    &self.pool_account,
                    &fee_account_repr_sys,
                    fee,
                )?;
                technical::Pallet::<T>::transfer_out(
                    &self.destination.asset,
                    &self.pool_account,
                    self.receiver_account.as_ref().unwrap(),
                    self.destination.amount.unwrap(),
                )?;
            } else {
                technical::Pallet::<T>::transfer_in(
                    &self.source.asset,
                    &source,
                    &self.pool_account,
                    self.source.amount.unwrap() - fee,
                )?;
                technical::Pallet::<T>::transfer_in(
                    &self.source.asset,
                    &source,
                    self.fee_account.as_ref().unwrap(),
                    fee,
                )?;
                technical::Pallet::<T>::transfer_out(
                    &self.destination.asset,
                    &self.pool_account,
                    self.receiver_account.as_ref().unwrap(),
                    self.destination.amount.unwrap(),
                )?;
            }

            let pool_account_repr_sys =
                technical::Pallet::<T>::tech_account_id_to_account_id(&self.pool_account)?;
            let (balance_a, balance_b) = Pallet::<T>::get_actual_reserves(
                &pool_account_repr_sys,
                &base_asset_id,
                &self.source.asset,
                &self.destination.asset,
            )?;
            Pallet::<T>::update_reserves(
                base_asset_id,
                &self.source.asset,
                &self.destination.asset,
                (&balance_a, &balance_b),
            );
            Ok(())
        })
    }
    fn claim(&self, _source: &AccountIdOf<T>) -> bool {
        true
    }
    fn weight(&self) -> Weight {
        unimplemented!()
    }
    fn cancel(&self, _source: &AccountIdOf<T>) {
        unimplemented!()
    }
}
