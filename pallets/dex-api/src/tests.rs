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

use crate::mock::*;
use crate::Pallet;
use common::prelude::QuoteAmount;
use common::DEXId::Polkaswap;
use common::{
    balance, DexIdOf, LiquidityRegistry, LiquiditySource, LiquiditySourceFilter, LiquiditySourceId,
    LiquiditySourceType, DOT, XOR,
};
use frame_support::weights::Weight;

type DexApi = Pallet<Runtime>;

#[test]
fn test_filter_empty_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let list =
            DexApi::list_liquidity_sources(&XOR, &DOT, &LiquiditySourceFilter::empty(DEX_A_ID))
                .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool),
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool2),
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool3),
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool4),
            ]
        );
    })
}

#[test]
fn test_filter_with_forbidden_existing_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let list = DexApi::list_liquidity_sources(
            &XOR,
            &DOT,
            &LiquiditySourceFilter::with_forbidden(
                DEX_A_ID,
                [
                    LiquiditySourceType::MockPool,
                    LiquiditySourceType::MockPool3,
                ]
                .into(),
            ),
        )
        .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool2),
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool4),
            ]
        );
    })
}

#[test]
fn test_filter_with_allowed_existing_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let list = DexApi::list_liquidity_sources(
            &XOR,
            &DOT,
            &LiquiditySourceFilter::with_allowed(
                DEX_A_ID,
                [
                    LiquiditySourceType::MockPool,
                    LiquiditySourceType::MockPool2,
                ]
                .into(),
            ),
        )
        .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool),
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool2),
            ]
        );
    })
}

#[test]
fn test_different_reserves_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let res1 = crate::Pallet::<Runtime>::quote(
            &LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool),
            &XOR,
            &DOT,
            QuoteAmount::with_desired_input(balance!(100)),
            true,
        );
        assert_eq!(
            res1.unwrap().0.amount,
            balance!(136.851187324744592819) // for reserves: 5000 XOR, 7000 DOT, 30bp fee
        );
        let res2 = crate::Pallet::<Runtime>::quote(
            &LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool2),
            &XOR,
            &DOT,
            QuoteAmount::with_desired_input(balance!(100)),
            true,
        );
        assert_eq!(
            res2.unwrap().0.amount,
            balance!(114.415463055560109513) // for reserves: 6000 XOR, 7000 DOT, 30bp fee
        );
    })
}

#[test]
fn test_exchange_weight_filtered_calculates() {
    framenode_chain_spec::ext().execute_with(|| {
        let dex_id = Polkaswap.into();
        let xyk_weight =
            <<framenode_runtime::Runtime as framenode_runtime::dex_api::Config>::XYKPool>::exchange_weight();
        let multicollateral_weight =
            <<framenode_runtime::Runtime as framenode_runtime::dex_api::Config>::MulticollateralBondingCurvePool>::exchange_weight();
        let xst_weight =
            <<framenode_runtime::Runtime as framenode_runtime::dex_api::Config>::XYKPool>::exchange_weight();
        let order_book_weight =
            <<framenode_runtime::Runtime as framenode_runtime::dex_api::Config>::OrderBook>::exchange_weight();

        fn exchange_weight_filtered(enabled_sources: Vec<&LiquiditySourceId<DexIdOf<framenode_runtime::Runtime>, LiquiditySourceType>>) -> Weight {
            framenode_runtime::dex_api::Pallet::<framenode_runtime::Runtime>::exchange_weight_filtered(enabled_sources.into_iter().cloned().collect())
        }

        let xyk_source = LiquiditySourceId::new(dex_id, LiquiditySourceType::XYKPool);
        let multicollateral_source = LiquiditySourceId::new(dex_id, LiquiditySourceType::MulticollateralBondingCurvePool);
        let xst_source = LiquiditySourceId::new(dex_id, LiquiditySourceType::XYKPool);
        #[cfg(feature = "wip")] // order-book
        let order_book_source = LiquiditySourceId::new(dex_id, LiquiditySourceType::OrderBook);

        assert_eq!(exchange_weight_filtered(vec![]), Weight::zero());
        assert_eq!(exchange_weight_filtered(vec![&xyk_source]), xyk_weight);
        assert_eq!(exchange_weight_filtered(vec![&multicollateral_source]), multicollateral_weight);
        assert_eq!(exchange_weight_filtered(vec![&xst_source]), xst_weight);
        #[cfg(feature = "wip")] // order-book
        assert_eq!(exchange_weight_filtered(vec![&order_book_source]), order_book_weight);
        assert_eq!(
            exchange_weight_filtered(vec![&xyk_source, &xst_source]),
            xyk_weight
                .max(xst_weight)
        );
        assert_eq!(
            exchange_weight_filtered(
                vec![&xyk_source, &xst_source, &multicollateral_source]
            ),
            xyk_weight
                .max(xst_weight)
                .max(multicollateral_weight)
        );
        #[cfg(feature = "wip")] // order-book
        assert_eq!(
            exchange_weight_filtered(
                vec![&xyk_source, &xst_source, &multicollateral_source, &order_book_source]
            ),
            xyk_weight
                .max(xst_weight)
                .max(multicollateral_weight)
                .max(order_book_weight)
        );
    })
}

#[test]
fn test_exchange_weight_filtered_matches_exchange_weight() {
    framenode_chain_spec::ext().execute_with(|| {
        let dex_id = Polkaswap.into();
        let all_sources = vec![
            LiquiditySourceType::XYKPool,
            LiquiditySourceType::BondingCurvePool,
            LiquiditySourceType::MulticollateralBondingCurvePool,
            LiquiditySourceType::MockPool,
            LiquiditySourceType::MockPool2,
            LiquiditySourceType::MockPool3,
            LiquiditySourceType::MockPool4,
            LiquiditySourceType::XSTPool,
            #[cfg(feature = "wip")] // order-book
            LiquiditySourceType::OrderBook,
        ];
        // add new source to `all_sources` if new enum variant is created.
        // enum is solely for detecting new variants and making compile errors :)
        match all_sources[0] {
            LiquiditySourceType::XYKPool
            | LiquiditySourceType::BondingCurvePool
            | LiquiditySourceType::MulticollateralBondingCurvePool
            | LiquiditySourceType::MockPool
            | LiquiditySourceType::MockPool2
            | LiquiditySourceType::MockPool3
            | LiquiditySourceType::MockPool4
            | LiquiditySourceType::XSTPool => (),
            #[cfg(feature = "wip")] // order-book
            LiquiditySourceType::OrderBook => (),
        }
        let all_sources: Vec<_> = all_sources
            .into_iter()
            .map(|source| LiquiditySourceId::new(dex_id, source))
            .collect();
        assert_eq!(
            framenode_runtime::dex_api::Pallet::<
                framenode_runtime::Runtime,
            >::exchange_weight_filtered(all_sources),
            framenode_runtime::dex_api::Pallet::<
                framenode_runtime::Runtime,
            >::exchange_weight(),
        )
    })
}
