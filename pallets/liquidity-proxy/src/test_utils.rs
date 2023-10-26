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

use crate::mock::{adar, AccountId, Assets, DEXId, LiquidityProxy};
use crate::{BatchReceiverInfo, SwapBatchInfo};
use common::prelude::{QuoteAmount, SwapOutcome};
use common::{
    assert_approx_eq, balance, AssetId32, AssetInfoProvider, Balance, LiquidityProxyTrait,
    LiquiditySourceFilter, LiquiditySourceType, PredefinedAssetId, XOR,
};
use std::collections::HashMap;

#[inline]
pub fn mcbc_excluding_filter(dex: DEXId) -> LiquiditySourceFilter<DEXId, LiquiditySourceType> {
    LiquiditySourceFilter::with_forbidden(
        dex,
        [LiquiditySourceType::MulticollateralBondingCurvePool].into(),
    )
}

pub fn check_swap_batch_executed_amount(
    swap_batches: Vec<SwapBatchInfo<AssetId32<PredefinedAssetId>, DEXId, AccountId>>,
) {
    swap_batches.into_iter().for_each(|batch| {
        let asset_id = batch.outcome_asset_id;

        // used for aggregating info about receivers and their desired amounts, since
        // there are possible duplicate accounts under the same asset_id
        let mut account_desired_amount: HashMap<AccountId, Balance> = HashMap::new();
        batch.receivers.into_iter().for_each(|receiver_info| {
            let BatchReceiverInfo {
                account_id,
                target_amount,
            } = receiver_info;

            account_desired_amount
                .entry(account_id)
                .and_modify(|balance| *balance += target_amount)
                .or_insert(target_amount);
        });
        account_desired_amount
            .into_iter()
            .for_each(|(account_id, desired_amount)| {
                assert_approx_eq!(
                    desired_amount,
                    Assets::free_balance(&asset_id, &account_id).unwrap(),
                    balance!(0.00001)
                )
            })
    });
}

pub fn check_adar_commission(
    swap_batches: &[SwapBatchInfo<AssetId32<PredefinedAssetId>, DEXId, AccountId>],
    sources: Vec<LiquiditySourceType>,
) {
    let actual_input_amount = calculate_swap_batch_input_amount(swap_batches, sources);

    let adar_fee = LiquidityProxy::calculate_adar_commission(actual_input_amount).unwrap();

    assert_approx_eq!(
        Assets::free_balance(&XOR, &adar()).unwrap(),
        adar_fee,
        balance!(0.02)
    );
}

pub fn calculate_swap_batch_input_amount(
    swap_batches: &[SwapBatchInfo<AssetId32<PredefinedAssetId>, DEXId, AccountId>],
    sources: Vec<LiquiditySourceType>,
) -> Balance {
    let actual_input_amount: Balance = swap_batches
        .iter()
        .cloned()
        .map(|batch| {
            let SwapBatchInfo {
                outcome_asset_id,
                dex_id,
                outcome_asset_reuse,
                ..
            } = batch;
            let target_amount = batch
                .receivers
                .into_iter()
                .map(|receiver_info| receiver_info.target_amount)
                .sum::<Balance>()
                .saturating_sub(outcome_asset_reuse);
            let filter = LiquiditySourceFilter::new(dex_id, sources.clone(), false);
            let SwapOutcome { amount, .. } = LiquidityProxy::quote(
                dex_id,
                &XOR,
                &outcome_asset_id,
                QuoteAmount::WithDesiredOutput {
                    desired_amount_out: target_amount,
                },
                filter,
                true,
            )
            .expect("Expected to quote the outcome of batch swap");
            amount
        })
        .sum();
    actual_input_amount
}

pub fn calculate_swap_batch_input_amount_with_adar_commission(
    swap_batches: &[SwapBatchInfo<AssetId32<PredefinedAssetId>, DEXId, AccountId>],
    sources: Vec<LiquiditySourceType>,
) -> Balance {
    let amount_in = calculate_swap_batch_input_amount(swap_batches, sources);
    let adar_fee = LiquidityProxy::calculate_adar_commission(amount_in).unwrap();

    amount_in
        .checked_add(adar_fee)
        .expect("Expected to calculate swap batch input amount with included adar fee")
}
