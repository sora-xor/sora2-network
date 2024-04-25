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

#![cfg(feature = "wip")] // ALT

use assets::AssetIdOf;
use codec::Decode;
use common::prelude::{OutcomeFee, QuoteAmount, SwapAmount, SwapOutcome};
use common::{
    balance, DexIdOf, FilterMode, LiquiditySourceFilter, LiquiditySourceId, LiquiditySourceType,
    VAL, XOR,
};
use frame_support::assert_ok;
use frame_system::RawOrigin;
use framenode_chain_spec::ext;
use framenode_runtime::liquidity_proxy::liquidity_aggregator::AggregatedSwapOutcome;
use framenode_runtime::liquidity_proxy::Pallet;
use framenode_runtime::{Runtime, RuntimeOrigin};
use order_book::test_utils::{create_and_fill_order_book, create_empty_order_book, fill_balance};
use order_book::OrderBookId;
use qa_tools::pallet_tools::liquidity_proxy::liquidity_sources;
use qa_tools::pallet_tools::pool_xyk::AssetPairInput;
use sp_std::vec;
use sp_std::vec::Vec;

type DEXId = DexIdOf<Runtime>;
type LiquidityProxyPallet = Pallet<Runtime>;
pub const DEX: common::DEXId = common::DEXId::Polkaswap;

fn alice<T: frame_system::Config>() -> <T as frame_system::Config>::AccountId {
    <T as frame_system::Config>::AccountId::decode(&mut &[1u8; 32][..]).unwrap()
}

fn bob<T: frame_system::Config>() -> <T as frame_system::Config>::AccountId {
    <T as frame_system::Config>::AccountId::decode(&mut &[2u8; 32][..]).unwrap()
}

// todo #750, it is a test just to catch the problem. All tests will be written in #750

#[test]
fn check_alt() {
    ext().execute_with(|| {
        let pair = AssetPairInput::new(DEX.into(), VAL, XOR, balance!(11.1), None);
        assert_ok!(liquidity_sources::initialize_xyk::<Runtime>(
            bob::<Runtime>(),
            vec![pair]
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        create_and_fill_order_book::<Runtime>(order_book_id);

        assert_ok!(assets::Pallet::<Runtime>::update_balance(
            RuntimeOrigin::root(),
            alice::<Runtime>(),
            XOR,
            balance!(100000).try_into().unwrap()
        ));
        let amount = SwapAmount::WithDesiredInput {
            desired_amount_in: balance!(1947),
            min_amount_out: balance!(176),
        };

        let quote = LiquidityProxyPallet::test_quote(
            DEX.into(),
            &XOR,
            &VAL,
            amount.into(),
            LiquiditySourceFilter::empty(DEX.into()),
            true,
        )
        .unwrap();

        assert_eq!(
            quote,
            AggregatedSwapOutcome::new(
                vec![
                    (
                        LiquiditySourceId::new(DEX.into(), LiquiditySourceType::XYKPool),
                        SwapAmount::with_desired_input(
                            balance!(7.7),
                            balance!(0.690405237531098527)
                        )
                    ),
                    (
                        LiquiditySourceId::new(DEX.into(), LiquiditySourceType::OrderBook),
                        SwapAmount::with_desired_input(balance!(1939.3), balance!(176.3))
                    )
                ],
                balance!(176.990405237531098527),
                OutcomeFee::xor(balance!(0.023099999999999999))
            )
        );

        assert_ok!(LiquidityProxyPallet::swap(
            RuntimeOrigin::signed(alice::<Runtime>()),
            DEX.into(),
            XOR,
            VAL,
            amount,
            Vec::new(),
            FilterMode::Disabled
        ));
    });
}

#[test]
fn check_small_reserves() {
    ext().execute_with(|| {
        framenode_runtime::frame_system::Pallet::<Runtime>::inc_providers(&bob::<Runtime>());
        let asset = assets::Pallet::<Runtime>::register_from(
            &bob::<Runtime>(),
            common::AssetSymbol(b"TST".to_vec()),
            common::AssetName(b"Test".to_vec()),
            common::DEFAULT_BALANCE_PRECISION,
            balance!(1000000),
            false,
            None,
            None,
        )
        .unwrap();

        let pair = AssetPairInput::new(DEX.into(), asset, XOR, balance!(10), Some(balance!(100)));

        assert_ok!(liquidity_sources::initialize_xyk::<Runtime>(
            bob::<Runtime>(),
            vec![pair]
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: asset,
            quote: XOR,
        };

        create_empty_order_book::<Runtime>(order_book_id);

        fill_balance::<Runtime>(alice::<Runtime>(), order_book_id);

        assert_ok!(order_book::Pallet::<Runtime>::place_limit_order(
            RawOrigin::Signed(alice::<Runtime>()).into(),
            order_book_id,
            balance!(10),
            balance!(100),
            common::PriceVariant::Sell,
            None
        ));

        let (info, _, _) = LiquidityProxyPallet::inner_quote(
            DEX.into(),
            &XOR,
            &asset,
            QuoteAmount::with_desired_output(balance!(101)),
            LiquiditySourceFilter::empty(DEX.into()),
            true,
            true,
        )
        .unwrap();

        assert_eq!(
            info.outcome,
            SwapOutcome::new(balance!(1500), OutcomeFee::xor(balance!(1.5)))
        );
    });
}
