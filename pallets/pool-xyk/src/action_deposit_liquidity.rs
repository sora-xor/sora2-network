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
use frame_support::weights::Weight;
use frame_support::{dispatch, ensure};
use sp_runtime::traits::Zero;

use common::prelude::{AssetInfoProvider, Balance, FixedWrapper};

use crate::{to_balance, to_fixed_wrapper, AccountPools, PoolProviders, TotalIssuances};

use crate::aliases::{AccountIdOf, AssetIdOf, TechAccountIdOf};
use crate::{Config, Error, Pallet, MIN_LIQUIDITY};

use crate::bounds::*;
use crate::operations::*;

impl<T: Config> common::SwapRulesValidation<AccountIdOf<T>, TechAccountIdOf<T>, AssetIdOf<T>, T>
    for DepositLiquidityAction<AssetIdOf<T>, AccountIdOf<T>, TechAccountIdOf<T>>
{
    fn is_abstract_checking(&self) -> bool {
        self.source.0.amount == Bounds::Dummy || self.source.1.amount == Bounds::Dummy
    }

    fn prepare_and_validate(
        &mut self,
        source_opt: Option<&AccountIdOf<T>>,
        base_asset_id: &AssetIdOf<T>,
    ) -> DispatchResult {
        let abstract_checking = source_opt.is_none() || common::SwapRulesValidation::<AccountIdOf<T>, TechAccountIdOf<T>, AssetIdOf<T>, T>::is_abstract_checking(self);

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
        Pallet::<T>::is_pool_account_valid_for(self.source.0.asset, &self.pool_account)?;

        // Balance of source account for asset pair.
        let (balance_bs, balance_ts) = if abstract_checking {
            (None, None)
        } else {
            let source = source_opt.unwrap();
            (
                Some(<assets::Pallet<T>>::free_balance(
                    &self.source.0.asset,
                    &source,
                )?),
                Some(<assets::Pallet<T>>::free_balance(
                    &self.source.1.asset,
                    &source,
                )?),
            )
        };

        if !abstract_checking && (balance_bs.unwrap() <= 0 || balance_ts.unwrap() <= 0) {
            Err(Error::<T>::AccountBalanceIsInvalid)?;
        }

        let (balance_bp, balance_tp) = Pallet::<T>::get_actual_reserves(
            &pool_account_repr_sys,
            &base_asset_id,
            &self.source.0.asset,
            &self.source.1.asset,
        )?;

        let mut empty_pool = false;
        if balance_bp == 0 && balance_tp == 0 {
            empty_pool = true;
        } else if balance_bp == 0 {
            Err(Error::<T>::PoolIsInvalid)?;
        } else if balance_tp == 0 {
            Err(Error::<T>::PoolIsInvalid)?;
        }

        #[allow(unused_variables)]
        let mut init_x = 0;
        #[allow(unused_variables)]
        let mut init_y = 0;
        if !abstract_checking && empty_pool {
            // Convertation from `Bounds` to `Option` is used here, and it is posible that value
            // None value returned from conversion.
            init_x = Option::<Balance>::from(self.source.0.amount)
                .ok_or(Error::<T>::InitialLiqudityDepositRatioMustBeDefined)?;
            init_y = Option::<Balance>::from(self.source.1.amount)
                .ok_or(Error::<T>::InitialLiqudityDepositRatioMustBeDefined)?;

            if init_x < T::MIN_XOR {
                return Err(Error::<T>::UnableToDepositXorLessThanMinimum.into());
            }
        }

        // FixedWrapper version of variables.
        let fxw_balance_bp = FixedWrapper::from(balance_bp);
        let fxw_balance_tp = FixedWrapper::from(balance_tp);

        // Product of pool pair amounts to get k value.
        let pool_k = {
            if empty_pool {
                if abstract_checking {
                    None
                } else {
                    let fxw_init_x = to_fixed_wrapper!(init_x);
                    let fxw_init_y = to_fixed_wrapper!(init_y);
                    let fxw_value = fxw_init_x.multiply_and_sqrt(&fxw_init_y);
                    ensure!(
                        !((fxw_value.clone() * fxw_init_x.clone())
                            .try_into_balance()
                            .is_err()
                            || (fxw_value.clone() * fxw_init_y).try_into_balance().is_err()),
                        Error::<T>::CalculatedValueIsOutOfDesiredBounds
                    );
                    let value = to_balance!(fxw_value.clone());
                    Some(value)
                }
            } else {
                let fxw_value: FixedWrapper = fxw_balance_bp.multiply_and_sqrt(&fxw_balance_tp);
                let value = to_balance!(fxw_value.clone());
                Some(value)
            }
        };

        if !abstract_checking {
            if empty_pool {
                self.pool_tokens = pool_k.unwrap();
            } else {
                match (self.source.0.amount, self.source.1.amount) {
                    (
                        Bounds::RangeFromDesiredToMin(xdes, xmin),
                        Bounds::RangeFromDesiredToMin(ydes, ymin),
                    ) => {
                        ensure!(
                            xdes >= xmin && ydes >= ymin,
                            Error::<T>::RangeValuesIsInvalid
                        );

                        // Adding min liquidity to pretend that initial provider has locked amount,
                        // which actually is not reflected in total supply.
                        let total_iss = TotalIssuances::<T>::get(&pool_account_repr_sys)
                            .ok_or(Error::<T>::PoolIsInvalid)?;
                        let total_iss = total_iss
                            .checked_add(MIN_LIQUIDITY)
                            .ok_or(Error::<T>::PoolTokenSupplyOverflow)?;

                        let (calc_xdes, calc_ydes, calc_marker) =
                            Pallet::<T>::calc_deposit_liquidity_1(
                                total_iss, balance_bp, balance_tp, xdes, ydes, xmin, ymin,
                            )?;

                        let acc_already_in_pool = PoolProviders::<T>::contains_key(
                            &pool_account_repr_sys,
                            self.receiver_account.as_ref().unwrap(),
                        );
                        if !acc_already_in_pool && calc_xdes < T::MIN_XOR {
                            return Err(Error::<T>::UnableToDepositXorLessThanMinimum.into());
                        }

                        self.source.0.amount = Bounds::Calculated(calc_xdes);
                        self.source.1.amount = Bounds::Calculated(calc_ydes);
                        self.pool_tokens = calc_marker;
                    }
                    // Case then no amount is specified (or something needed is not specified),
                    // impossible to decide any amounts.
                    _ => {
                        Err(Error::<T>::ImpossibleToDecideDepositLiquidityAmounts)?;
                    }
                }
            }
        }

        // Recommended minimum liquidity, will be used if not specified or for checking if specified.
        let recom_min_liquidity = MIN_LIQUIDITY;
        // Set recommended or check that `min_liquidity` is correct.
        match self.min_liquidity {
            // Just set it here if it not specified, this is usual case.
            None => {
                self.min_liquidity = Some(recom_min_liquidity);
            }
            // Case with source user `min_liquidity` is set, checking that it is not smaller.
            Some(min_liquidity) => {
                if min_liquidity < recom_min_liquidity {
                    Err(Error::<T>::PairSwapActionMinimumLiquidityIsSmallerThanRecommended)?
                }
            }
        }

        //TODO: for abstract_checking, check that is enough liquidity in pool.
        if !abstract_checking {
            // Get required values, now it is always Some, it is safe to unwrap().
            let min_liquidity = self.min_liquidity.unwrap();
            let base_amount = self.source.0.amount.unwrap();
            let target_amount = self.source.1.amount.unwrap();
            // Checking by minimum liquidity.
            if min_liquidity > pool_k.unwrap() && self.pool_tokens < min_liquidity - pool_k.unwrap()
            {
                Err(Error::<T>::DestinationAmountOfLiquidityIsNotLargeEnough)?;
            }
            // Checking that balances if correct and large enough for amounts.
            if balance_bs.unwrap() < base_amount {
                Err(Error::<T>::SourceBaseAmountIsNotLargeEnough)?;
            }
            if balance_ts.unwrap() < target_amount {
                Err(Error::<T>::TargetBaseAmountIsNotLargeEnough)?;
            }
        }

        if empty_pool {
            // Previous checks guarantee that unwrap and subtraction are safe.
            self.pool_tokens -= self.min_liquidity.unwrap();
        }

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
    for DepositLiquidityAction<AssetIdOf<T>, AccountIdOf<T>, TechAccountIdOf<T>>
{
    fn reserve(
        &self,
        source: &AccountIdOf<T>,
        base_asset_id: &AssetIdOf<T>,
    ) -> dispatch::DispatchResult {
        ensure!(
            Some(source) == self.client_account.as_ref(),
            Error::<T>::SourceAndClientAccountDoNotMatchAsEqual
        );
        let pool_account_repr_sys =
            technical::Pallet::<T>::tech_account_id_to_account_id(&self.pool_account)?;
        technical::Pallet::<T>::transfer_in(
            &self.source.0.asset,
            &source,
            &self.pool_account,
            self.source.0.amount.unwrap(),
        )?;
        technical::Pallet::<T>::transfer_in(
            &self.source.1.asset,
            &source,
            &self.pool_account,
            self.source.1.amount.unwrap(),
        )?;
        let receiver_account = self.receiver_account.as_ref().unwrap();
        // Pool tokens balance is zero while minted amount will be non-zero.
        if Pallet::<T>::pool_providers(&pool_account_repr_sys, receiver_account)
            .unwrap_or(0)
            .is_zero()
            && !self.pool_tokens.is_zero()
        {
            let pair = Pallet::<T>::strict_sort_pair(
                base_asset_id,
                &self.source.0.asset,
                &self.source.1.asset,
            )?;
            AccountPools::<T>::mutate(receiver_account, &pair.base_asset_id, |set| {
                set.insert(pair.target_asset_id)
            });
        }
        Pallet::<T>::mint(&pool_account_repr_sys, receiver_account, self.pool_tokens)?;
        let (balance_a, balance_b) = Pallet::<T>::get_actual_reserves(
            &pool_account_repr_sys,
            &base_asset_id,
            &self.source.0.asset,
            &self.source.1.asset,
        )?;
        Pallet::<T>::update_reserves(
            base_asset_id,
            &self.source.0.asset,
            &self.source.1.asset,
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
