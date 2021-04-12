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

use frame_support::dispatch::DispatchResult;
use frame_support::weights::Weight;
use frame_support::{dispatch, ensure};

use common::prelude::{Balance, FixedWrapper};

use crate::to_balance;

use crate::aliases::{AccountIdOf, AssetIdOf, TechAccountIdOf, TechAssetIdOf};
use crate::{Config, Error, Module, MIN_LIQUIDITY};

use crate::bounds::*;
use crate::operations::*;

impl<T: Config> common::SwapRulesValidation<AccountIdOf<T>, TechAccountIdOf<T>, T>
    for WithdrawLiquidityAction<
        AssetIdOf<T>,
        TechAssetIdOf<T>,
        Balance,
        AccountIdOf<T>,
        TechAccountIdOf<T>,
    >
{
    fn is_abstract_checking(&self) -> bool {
        (self.destination.0).amount == Bounds::Dummy
            || (self.destination.1).amount == Bounds::Dummy
            || self.source.amount == Bounds::Dummy
    }

    fn prepare_and_validate(&mut self, source_opt: Option<&AccountIdOf<T>>) -> DispatchResult {
        //TODO: replace unwrap.
        let source = source_opt.unwrap();
        // Check that client account is same as source, because signature is checked for source.
        // Signature checking is used in extrinsics for example, and source is derived from origin.
        // TODO: In general case it is possible to use different client account, for example if
        // signature of source is legal for some source accounts.
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
        match &self.receiver_account_a {
            // Just use `client_account` as same account, swapping to self.
            None => {
                self.receiver_account_a = self.client_account.clone();
            }
            _ => (),
        }
        match &self.receiver_account_b {
            // Just use `client_account` as same account, swapping to self.
            None => {
                self.receiver_account_b = self.client_account.clone();
            }
            _ => (),
        }
        let pool_account_repr_sys =
            technical::Module::<T>::tech_account_id_to_account_id(&self.pool_account)?;
        // Check that pool account is valid.
        Module::<T>::is_pool_account_valid_for(self.destination.0.asset, &self.pool_account)?;

        let mark_asset = Module::<T>::get_marking_asset(&self.pool_account)?;
        ensure!(
            self.source.asset == mark_asset,
            Error::<T>::InvalidAssetForLiquidityMarking
        );

        let repr_k_asset_id = self.source.asset.into();

        // Balance of source account for k value.
        let balance_ks = <assets::Module<T>>::free_balance(&repr_k_asset_id, &source)?;
        if balance_ks <= 0 {
            Err(Error::<T>::AccountBalanceIsInvalid)?;
        }

        // Balance of pool account for asset pair basic asset.
        let balance_bp =
            <assets::Module<T>>::free_balance(&(self.destination.0).asset, &pool_account_repr_sys)?;
        // Balance of pool account for asset pair target asset.
        let balance_tp =
            <assets::Module<T>>::free_balance(&(self.destination.1).asset, &pool_account_repr_sys)?;

        if balance_bp == 0 && balance_tp == 0 {
            Err(Error::<T>::PoolIsEmpty)?;
        } else if balance_bp <= 0 {
            Err(Error::<T>::PoolIsInvalid)?;
        } else if balance_tp <= 0 {
            Err(Error::<T>::PoolIsInvalid)?;
        }

        let fxw_balance_bp = FixedWrapper::from(balance_bp);
        let fxw_balance_tp = FixedWrapper::from(balance_tp);

        let total_iss = assets::Module::<T>::total_issuance(&repr_k_asset_id)?;
        // Adding min liquidity to pretend that initial provider has locked amount, which actually is not reflected in total supply.
        let fxw_total_iss = FixedWrapper::from(total_iss) + MIN_LIQUIDITY;

        match (
            self.source.amount,
            (self.destination.0).amount,
            (self.destination.1).amount,
        ) {
            (Bounds::Desired(source_k), ox, oy) => {
                ensure!(source_k > 0, Error::<T>::ZeroValueInAmountParameter);
                let fxw_source_k = FixedWrapper::from(source_k);
                // let fxw_piece_to_take = fxw_total_iss / fxw_source_k;
                let fxw_recom_x = fxw_balance_bp * fxw_source_k.clone() / fxw_total_iss.clone();
                let fxw_recom_y = fxw_balance_tp * fxw_source_k / fxw_total_iss;
                let recom_x: Balance = to_balance!(fxw_recom_x);
                let recom_y = to_balance!(fxw_recom_y);

                match ox {
                    Bounds::Desired(x) => {
                        if x != recom_x {
                            Err(Error::<T>::InvalidWithdrawLiquidityBasicAssetAmount)?;
                        }
                    }
                    bounds => {
                        let calc = Bounds::Calculated(recom_x);
                        ensure!(
                            bounds.meets_the_boundaries(&calc),
                            Error::<T>::CalculatedValueIsNotMeetsRequiredBoundaries
                        );
                        (self.destination.0).amount = calc;
                    }
                }

                match oy {
                    Bounds::Desired(y) => {
                        if y != recom_y {
                            Err(Error::<T>::InvalidWithdrawLiquidityTargetAssetAmount)?;
                        }
                    }
                    bounds => {
                        let calc = Bounds::Calculated(recom_y);
                        ensure!(
                            bounds.meets_the_boundaries(&calc),
                            Error::<T>::CalculatedValueIsNotMeetsRequiredBoundaries
                        );
                        (self.destination.1).amount = calc;
                    }
                }
            }

            _ => {
                Err(Error::<T>::ImpossibleToDecideDepositLiquidityAmounts)?;
            }
        }

        // Get required values, now it is always Some, it is safe to unwrap().
        let _base_amount = (self.destination.1).amount.unwrap();
        let _target_amount = (self.destination.0).amount.unwrap();
        let source_amount = self.source.amount.unwrap();

        if balance_ks < source_amount {
            Err(Error::<T>::SourceBalanceOfLiquidityTokensIsNotLargeEnough)?;
        }

        //TODO: Debug why in this place checking is failed, but in transfer checks is success.
        /*
        // Checking that balances if correct and large enough for amounts.
        if balance_bp < base_amount {
            Err(Error::<T>::DestinationBaseBalanceIsNotLargeEnough)?;
        }
        if balance_tp < target_amount {
            Err(Error::<T>::DestinationTargetBalanceIsNotLargeEnough)?;
        }
        */
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

impl<T: Config> common::SwapAction<AccountIdOf<T>, TechAccountIdOf<T>, T>
    for WithdrawLiquidityAction<
        AssetIdOf<T>,
        TechAssetIdOf<T>,
        Balance,
        AccountIdOf<T>,
        TechAccountIdOf<T>,
    >
{
    fn reserve(&self, source: &AccountIdOf<T>) -> dispatch::DispatchResult {
        ensure!(
            Some(source) == self.client_account.as_ref(),
            Error::<T>::SourceAndClientAccountDoNotMatchAsEqual
        );
        let asset_repr = Into::<AssetIdOf<T>>::into(self.source.asset);
        let pool_account_repr_sys =
            technical::Module::<T>::tech_account_id_to_account_id(&self.pool_account)?;
        technical::Module::<T>::transfer_out(
            &(self.destination.0).asset,
            &self.pool_account,
            self.receiver_account_a.as_ref().unwrap(),
            self.destination.0.amount.unwrap(),
        )?;
        technical::Module::<T>::transfer_out(
            &(self.destination.1).asset,
            &self.pool_account,
            self.receiver_account_b.as_ref().unwrap(),
            self.destination.1.amount.unwrap(),
        )?;
        assets::Module::<T>::burn_from(
            &asset_repr,
            &pool_account_repr_sys,
            &source,
            self.source.amount.unwrap(),
        )?;
        let pool_account_repr_sys =
            technical::Module::<T>::tech_account_id_to_account_id(&self.pool_account)?;
        let balance_a =
            <assets::Module<T>>::free_balance(&(self.destination.0).asset, &pool_account_repr_sys)?;
        let balance_b =
            <assets::Module<T>>::free_balance(&(self.destination.1).asset, &pool_account_repr_sys)?;
        Module::<T>::update_reserves(
            &(self.destination.0).asset,
            &(self.destination.1).asset,
            (&balance_a, &balance_b),
        );
        Ok(())
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
