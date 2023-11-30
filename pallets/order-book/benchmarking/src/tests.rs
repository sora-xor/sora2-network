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

#![cfg(feature = "ready-to-test")] // order-book

use super::*;

use common::balance;
use common::prelude::SwapAmount;
use frame_system::RawOrigin;
use framenode_chain_spec::ext;
use framenode_runtime::Runtime;
use order_book::test_utils::fill_tools::FillSettings;
#[allow(unused)]
use periphery::presets::*;
use sp_std::vec::Vec;

#[test]
fn test_benchmark_delete_orderbook() {
    ext().execute_with(|| {
        let settings = FillSettings::<Runtime>::max();
        let order_book_id = periphery::delete_orderbook::init(settings.clone());

        OrderBookPallet::<Runtime>::delete_orderbook(RawOrigin::Root.into(), order_book_id)
            .unwrap();

        periphery::delete_orderbook::verify(settings, order_book_id);
    })
}

#[test]
fn test_benchmark_place() {
    ext().execute_with(|| {
        let settings = FillSettings::<Runtime>::max();
        let context = periphery::place_limit_order::init(settings.clone());

        OrderBookPallet::<Runtime>::place_limit_order(
            RawOrigin::Signed(context.caller.clone()).into(),
            context.order_book_id,
            *context.price.balance(),
            *context.amount.balance(),
            context.side,
            Some(context.lifespan),
        )
        .unwrap();

        periphery::place_limit_order::verify(settings, context);
    })
}

#[test]
fn test_benchmark_cancel() {
    ext().execute_with(|| {
        let settings = FillSettings::<Runtime>::max();
        let context = periphery::cancel_limit_order::init(settings.clone(), false);

        OrderBookPallet::<Runtime>::cancel_limit_order(
            RawOrigin::Signed(context.caller.clone()).into(),
            context.order_book_id,
            context.order_id,
        )
        .unwrap();

        periphery::cancel_limit_order::verify(settings, context);
    })
}

fn test_benchmark_execute_market_order(executed_orders_limit: u32) {
    ext().execute_with(|| {
        let mut settings = FillSettings::<Runtime>::max();
        settings.executed_orders_limit = executed_orders_limit;
        let context = periphery::execute_market_order::init(settings.clone());

        OrderBookPallet::<Runtime>::execute_market_order(
            RawOrigin::Signed(context.caller.clone()).into(),
            context.order_book_id,
            context.side,
            *context.amount.balance(),
        )
        .unwrap();

        periphery::execute_market_order::verify(context);
    })
}

#[test]
fn test_benchmark_execute_market_order_max_orders() {
    test_benchmark_execute_market_order(
        <Runtime as order_book_imported::Config>::HARD_MIN_MAX_RATIO
            .try_into()
            .unwrap(),
    );
}

#[test]
fn test_benchmark_execute_market_order_one_order() {
    test_benchmark_execute_market_order(1);
}

#[test]
fn test_benchmark_execute_market_order_scattered() {
    ext().execute_with(|| {
        let settings = FillSettings::<Runtime>::max();
        let context = periphery::execute_market_order_scattered::init(settings.clone());

        OrderBookPallet::<Runtime>::execute_market_order(
            RawOrigin::Signed(context.caller.clone()).into(),
            context.order_book_id,
            context.side,
            *context.amount.balance(),
        )
        .unwrap();

        periphery::execute_market_order_scattered::verify(context);
    })
}

#[test]
fn test_benchmark_quote() {
    ext().execute_with(|| {
        use common::LiquiditySource;

        let settings = FillSettings::<Runtime>::max();
        let context = periphery::quote::init(settings);

        let _ = OrderBookPallet::<Runtime>::quote(
            &context.dex_id,
            &context.input_asset_id,
            &context.output_asset_id,
            context.amount,
            context.deduce_fee,
        )
        .unwrap();
    })
}

fn test_benchmark_exchange_dense(executed_orders_limit: u32) {
    ext().execute_with(|| {
        use common::LiquiditySource;

        let mut settings = FillSettings::<Runtime>::max();
        settings.executed_orders_limit = executed_orders_limit;
        let context = periphery::exchange::init(settings.clone());

        let (_outcome, _) = OrderBookPallet::<Runtime>::exchange(
            &context.caller,
            &context.caller,
            &context.order_book_id.dex_id,
            &context.order_book_id.base,
            &context.order_book_id.quote,
            SwapAmount::with_desired_output(
                context.expected_out,
                context.expected_in + balance!(5),
            ),
        )
        .unwrap();

        periphery::exchange::verify(context);
    })
}

#[test]
fn test_benchmark_exchange_dense_max_orders() {
    test_benchmark_exchange_dense(
        <Runtime as order_book_imported::Config>::HARD_MIN_MAX_RATIO
            .try_into()
            .unwrap(),
    );
}

#[test]
fn test_benchmark_exchange_dense_one_order() {
    test_benchmark_exchange_dense(1);
}

fn test_benchmark_exchange(executed_orders_limit: u32) {
    ext().execute_with(|| {
        use common::LiquiditySource;

        let mut settings = FillSettings::<Runtime>::max();
        settings.executed_orders_limit = executed_orders_limit;
        let context = periphery::exchange_scattered::init(settings.clone());

        let (_outcome, _) = OrderBookPallet::<Runtime>::exchange(
            &context.caller,
            &context.caller,
            &context.order_book_id.dex_id,
            &context.order_book_id.base,
            &context.order_book_id.quote,
            SwapAmount::with_desired_output(
                context.expected_out,
                context.expected_in + balance!(5),
            ),
        )
        .unwrap();

        periphery::exchange_scattered::verify(context);
    })
}

#[test]
fn test_benchmark_exchange_max_orders() {
    test_benchmark_exchange(
        <Runtime as order_book_imported::Config>::HARD_MIN_MAX_RATIO
            .try_into()
            .unwrap(),
    );
}
#[test]
fn test_benchmark_exchange_scattered_one_order() {
    test_benchmark_exchange(1);
}

#[test]
fn test_benchmark_align_single_order() {
    ext().execute_with(|| {
        let settings = FillSettings::<Runtime>::max();
        let context = periphery::align_single_order::init(settings);

        let mut data =
            framenode_runtime::order_book::storage_data_layer::StorageDataLayer::<Runtime>::new();

        context
            .order_book
            .align_limit_orders(Vec::from([context.order_to_align.clone()]), &mut data)
            .unwrap();

        periphery::align_single_order::verify(context);
    });
}
