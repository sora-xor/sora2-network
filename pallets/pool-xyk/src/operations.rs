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

use codec::{Decode, Encode};
use common::prelude::{Balance, OutcomeFee};
use common::AssetId32;
use sp_core::crypto::AccountId32;
use sp_runtime::RuntimeDebug;

type DEXId = common::DEXId;
pub type AssetId = AssetId32<common::PredefinedAssetId>;
type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
pub type AccountId = AccountId32;
type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;

use crate::bounds::*;

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode, scale_info::TypeInfo)]
pub struct Resource<AssetId, Balance> {
    // This is `AssetId` of `Resource`.
    pub asset: AssetId,
    // This is amount of `Resource`.
    pub amount: Bounds<Balance>,
}

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode, scale_info::TypeInfo)]
pub struct ResourcePair<AssetId, Balance>(
    pub Resource<AssetId, Balance>,
    pub Resource<AssetId, Balance>,
);

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode, scale_info::TypeInfo)]
pub struct PairSwapAction<DEXId, AssetId: Ord, AccountId, TechAccountId> {
    pub client_account: Option<AccountId>,
    pub receiver_account: Option<AccountId>,
    pub pool_account: TechAccountId,
    pub source: Resource<AssetId, Balance>,
    pub destination: Resource<AssetId, Balance>,
    pub fee: OutcomeFee<AssetId, Balance>,
    pub fee_account: Option<TechAccountId>,
    pub get_fee_from_destination: Option<bool>,
    pub dex_id: DEXId,
}

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode, scale_info::TypeInfo)]
pub struct DepositLiquidityAction<AssetId, AccountId, TechAccountId> {
    pub client_account: Option<AccountId>,
    pub receiver_account: Option<AccountId>,
    pub pool_account: TechAccountId,
    pub source: ResourcePair<AssetId, Balance>,
    pub pool_tokens: Balance,
    pub min_liquidity: Option<Balance>,
}

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode, scale_info::TypeInfo)]
pub struct WithdrawLiquidityAction<AssetId, AccountId, TechAccountId> {
    pub client_account: Option<AccountId>,
    pub receiver_account_a: Option<AccountId>,
    pub receiver_account_b: Option<AccountId>,
    pub pool_account: TechAccountId,
    pub pool_tokens: Balance,
    pub destination: ResourcePair<AssetId, Balance>,
}

#[derive(Clone, RuntimeDebug, Eq, PartialEq, Encode, Decode, scale_info::TypeInfo)]
pub enum PolySwapAction<DEXId, AssetId: Ord, AccountId, TechAccountId> {
    PairSwap(PairSwapAction<DEXId, AssetId, AccountId, TechAccountId>),
    DepositLiquidity(DepositLiquidityAction<AssetId, AccountId, TechAccountId>),
    WithdrawLiquidity(WithdrawLiquidityAction<AssetId, AccountId, TechAccountId>),
}

impl Into<()> for PolySwapAction<DEXId, AssetId, AccountId, TechAccountId> {
    fn into(self) -> () {}
}
