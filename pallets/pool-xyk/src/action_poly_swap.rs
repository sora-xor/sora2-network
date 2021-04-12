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

use frame_support::dispatch;
use frame_support::dispatch::DispatchResult;
use frame_support::weights::Weight;

use common::prelude::Balance;
use common::SwapRulesValidation;

use crate::aliases::{
    AccountIdOf, AssetIdOf, PolySwapActionStructOf, TechAccountIdOf, TechAssetIdOf,
};
use crate::Config;

use crate::operations::*;

impl<T: Config> common::SwapRulesValidation<AccountIdOf<T>, TechAccountIdOf<T>, T>
    for PolySwapActionStructOf<T>
where
    PairSwapAction<AssetIdOf<T>, Balance, AccountIdOf<T>, TechAccountIdOf<T>>:
        SwapRulesValidation<AccountIdOf<T>, TechAccountIdOf<T>, T>,
    DepositLiquidityAction<
        AssetIdOf<T>,
        TechAssetIdOf<T>,
        Balance,
        AccountIdOf<T>,
        TechAccountIdOf<T>,
    >: SwapRulesValidation<AccountIdOf<T>, TechAccountIdOf<T>, T>,
    WithdrawLiquidityAction<
        AssetIdOf<T>,
        TechAssetIdOf<T>,
        Balance,
        AccountIdOf<T>,
        TechAccountIdOf<T>,
    >: SwapRulesValidation<AccountIdOf<T>, TechAccountIdOf<T>, T>,
{
    fn is_abstract_checking(&self) -> bool {
        match self {
            PolySwapAction::PairSwap(a) => a.is_abstract_checking(),
            PolySwapAction::DepositLiquidity(a) => a.is_abstract_checking(),
            PolySwapAction::WithdrawLiquidity(a) => a.is_abstract_checking(),
        }
    }
    fn prepare_and_validate(&mut self, source: Option<&AccountIdOf<T>>) -> DispatchResult {
        match self {
            PolySwapAction::PairSwap(a) => a.prepare_and_validate(source),
            PolySwapAction::DepositLiquidity(a) => a.prepare_and_validate(source),
            PolySwapAction::WithdrawLiquidity(a) => a.prepare_and_validate(source),
        }
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
    for PolySwapActionStructOf<T>
where
    PairSwapAction<AssetIdOf<T>, Balance, AccountIdOf<T>, TechAccountIdOf<T>>:
        common::SwapAction<AccountIdOf<T>, TechAccountIdOf<T>, T>,
    DepositLiquidityAction<
        AssetIdOf<T>,
        TechAssetIdOf<T>,
        Balance,
        AccountIdOf<T>,
        TechAccountIdOf<T>,
    >: common::SwapAction<AccountIdOf<T>, TechAccountIdOf<T>, T>,
    WithdrawLiquidityAction<
        AssetIdOf<T>,
        TechAssetIdOf<T>,
        Balance,
        AccountIdOf<T>,
        TechAccountIdOf<T>,
    >: common::SwapAction<AccountIdOf<T>, TechAccountIdOf<T>, T>,
{
    fn reserve(&self, source: &AccountIdOf<T>) -> dispatch::DispatchResult {
        match self {
            PolySwapAction::PairSwap(a) => a.reserve(source),
            PolySwapAction::DepositLiquidity(a) => a.reserve(source),
            PolySwapAction::WithdrawLiquidity(a) => a.reserve(source),
        }
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
