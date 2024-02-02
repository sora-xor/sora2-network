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

//! Tests are not essential for this qa helper pallet,
//! but they make modify-run iterations during development much quicker

use assets::AssetIdOf;
use common::prelude::{err_pays_no, BalanceUnit, QuoteAmount};
use common::{
    assert_approx_eq, balance, fixed, AssetId32, AssetName, AssetSymbol, Balance, DEXId, DexIdOf,
    LiquiditySource, PredefinedAssetId, PriceVariant, SymbolName, DAI, ETH, PSWAP, TBCD, VAL, XOR,
    XST, XSTUSD,
};
use core::str::FromStr;
use frame_support::dispatch::{Pays, PostDispatchInfo};
use frame_support::traits::{Get, Hooks};
use frame_support::{assert_err, assert_ok};
use frame_system::pallet_prelude::BlockNumberFor;
use framenode_chain_spec::ext;
use framenode_runtime::qa_tools;
use framenode_runtime::{Runtime, RuntimeOrigin};
use order_book::{DataLayer, LimitOrder, MomentOf, OrderBookId, OrderPrice, OrderVolume};
use qa_tools::source_initialization::{
    XSTBaseInput, XSTBaseSideInput, XSTSyntheticExistence, XSTSyntheticInput, XSTSyntheticOutput,
    XSTSyntheticQuote, XSTSyntheticQuoteDirection, XYKPair,
};
use qa_tools::{pallet_tools::order_book::settings, Error};
use sp_runtime::traits::BadOrigin;
use sp_runtime::DispatchErrorWithPostInfo;

type FrameSystem = framenode_runtime::frame_system::Pallet<Runtime>;
pub type QAToolsPallet = qa_tools::Pallet<Runtime>;

pub fn alice() -> <Runtime as frame_system::Config>::AccountId {
    <Runtime as frame_system::Config>::AccountId::new([1u8; 32])
}

pub fn bob() -> <Runtime as frame_system::Config>::AccountId {
    <Runtime as frame_system::Config>::AccountId::new([2u8; 32])
}

pub fn charlie() -> <Runtime as frame_system::Config>::AccountId {
    <Runtime as frame_system::Config>::AccountId::new([3u8; 32])
}

pub fn dave() -> <Runtime as frame_system::Config>::AccountId {
    <Runtime as frame_system::Config>::AccountId::new([4u8; 32])
}

fn default_price_step() -> Balance {
    settings::OrderBookAttributes::default().tick_size
}

pub fn run_to_block(n: u32) {
    while FrameSystem::block_number() < n {
        order_book::Pallet::<Runtime>::on_finalize(FrameSystem::block_number());
        qa_tools::Pallet::<Runtime>::on_finalize(FrameSystem::block_number());
        FrameSystem::set_block_number(FrameSystem::block_number() + 1);
        FrameSystem::on_initialize(FrameSystem::block_number());
        order_book::Pallet::<Runtime>::on_initialize(FrameSystem::block_number());
        qa_tools::Pallet::<Runtime>::on_initialize(FrameSystem::block_number());
    }
}

fn test_creates_orderbook(
    base: AssetId32<PredefinedAssetId>,
    quote: AssetId32<PredefinedAssetId>,
    attributes: settings::OrderBookAttributes,
    best_bid_price: Balance,
    best_ask_price: Balance,
    steps: usize,
    amount_range: (Balance, Balance),
) -> OrderBookId<AssetIdOf<Runtime>, DexIdOf<Runtime>> {
    let order_book_id = OrderBookId {
        dex_id: DEXId::Polkaswap.into(),
        base,
        quote,
    };

    assert_err!(
        QAToolsPallet::order_book_create_and_fill_batch(
            RuntimeOrigin::signed(alice()),
            alice(),
            alice(),
            vec![]
        ),
        BadOrigin
    );

    let price_step = default_price_step();
    let orders_per_price = 3;
    let amount_range = settings::RandomAmount::new(amount_range.0, amount_range.1);
    assert_ok!(QAToolsPallet::order_book_create_and_fill_batch(
        RuntimeOrigin::root(),
        alice(),
        alice(),
        vec![(
            order_book_id,
            attributes,
            settings::OrderBookFill {
                bids: Some(settings::SideFill {
                    highest_price: best_bid_price,
                    lowest_price: best_bid_price - (steps - 1) as u128 * price_step,
                    price_step,
                    orders_per_price,
                    lifespan: None,
                    amount_range_inclusive: Some(amount_range)
                }),
                asks: Some(settings::SideFill {
                    highest_price: best_ask_price + (steps - 1) as u128 * price_step,
                    lowest_price: best_ask_price,
                    price_step,
                    orders_per_price,
                    lifespan: None,
                    amount_range_inclusive: Some(amount_range)
                }),
                random_seed: None,
            }
        )]
    ));

    assert!(order_book::Pallet::<Runtime>::order_books(order_book_id).is_some());

    assert_eq!(
        order_book::Pallet::<Runtime>::aggregated_bids(order_book_id).len(),
        steps
    );
    assert_eq!(
        order_book::Pallet::<Runtime>::aggregated_asks(order_book_id).len(),
        steps
    );

    let mut data = order_book::storage_data_layer::StorageDataLayer::<Runtime>::new();

    let limit_orders = data.get_all_limit_orders(&order_book_id);

    assert_eq!(limit_orders.len(), steps * 2 * orders_per_price as usize);
    let amount_range = amount_range
        .as_non_empty_inclusive_range()
        .expect("empty range provided");
    assert!(limit_orders
        .iter()
        .all(|order| amount_range.contains(order.amount.balance())));

    order_book_id
}

#[test]
fn should_create_and_fill_orderbook_fixed_amount() {
    ext().execute_with(|| {
        test_creates_orderbook(
            VAL,
            XOR,
            settings::OrderBookAttributes::default(),
            balance!(10),
            balance!(11),
            4,
            (balance!(1), balance!(1)),
        );

        FrameSystem::inc_providers(&bob());
        let nft = assets::Pallet::<Runtime>::register_from(
            &bob(),
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            1000,
            false,
            None,
            None,
        )
        .unwrap();
        test_creates_orderbook(
            nft,
            XOR,
            settings::OrderBookAttributes {
                tick_size: balance!(0.00001),
                step_lot_size: 1,
                min_lot_size: 1,
                max_lot_size: 1000,
            },
            balance!(10),
            balance!(11),
            4,
            (1, 1),
        );
    });
}

#[test]
fn should_create_and_fill_orderbook_random_amount() {
    ext().execute_with(|| {
        test_creates_orderbook(
            VAL,
            XOR,
            settings::OrderBookAttributes::default(),
            balance!(10),
            balance!(11),
            4,
            (balance!(1), balance!(10)),
        );

        FrameSystem::inc_providers(&bob());
        let nft = assets::Pallet::<Runtime>::register_from(
            &bob(),
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            1000,
            false,
            None,
            None,
        )
        .unwrap();
        test_creates_orderbook(
            nft,
            XOR,
            settings::OrderBookAttributes {
                tick_size: balance!(0.00001),
                step_lot_size: 1,
                min_lot_size: 1,
                max_lot_size: 1000,
            },
            balance!(10),
            balance!(11),
            4,
            (1, 10),
        );
    });
}

#[test]
fn should_respect_orderbook_seed() {
    ext().execute_with(|| {
        let order_book_id_1 = OrderBookId {
            dex_id: DEXId::Polkaswap.into(),
            base: VAL,
            quote: XOR,
        };
        let order_book_id_2 = OrderBookId {
            dex_id: DEXId::Polkaswap.into(),
            base: PSWAP,
            quote: XOR,
        };
        let price_step = default_price_step();
        let orders_per_price = 3;
        let best_bid_price = balance!(10);
        let best_ask_price = balance!(11);
        let steps = 4;
        let amount_range = settings::RandomAmount::new(balance!(1), balance!(10));
        let fill_settings = settings::OrderBookFill {
            bids: Some(settings::SideFill {
                highest_price: best_bid_price,
                lowest_price: best_bid_price - (steps - 1) as u128 * price_step,
                price_step,
                orders_per_price,
                lifespan: None,
                amount_range_inclusive: Some(amount_range),
            }),
            asks: Some(settings::SideFill {
                highest_price: best_ask_price + (steps - 1) as u128 * price_step,
                lowest_price: best_ask_price,
                price_step,
                orders_per_price,
                lifespan: None,
                amount_range_inclusive: Some(amount_range),
            }),
            random_seed: None,
        };
        assert_ok!(QAToolsPallet::order_book_create_and_fill_batch(
            RuntimeOrigin::root(),
            alice(),
            alice(),
            vec![
                (
                    order_book_id_1,
                    settings::OrderBookAttributes::default(),
                    fill_settings.clone()
                ),
                (
                    order_book_id_2,
                    settings::OrderBookAttributes::default(),
                    fill_settings
                )
            ]
        ));

        let mut data = order_book::storage_data_layer::StorageDataLayer::<Runtime>::new();
        let mut limit_orders_1 = data.get_all_limit_orders(&order_book_id_1);
        let mut limit_orders_2 = data.get_all_limit_orders(&order_book_id_2);
        fn cmp_by_id(a: &LimitOrder<Runtime>, b: &LimitOrder<Runtime>) -> sp_std::cmp::Ordering {
            a.id.cmp(&b.id)
        }
        limit_orders_1.sort_by(cmp_by_id);
        limit_orders_2.sort_by(cmp_by_id);

        assert_eq!(limit_orders_1, limit_orders_2);
    })
}

#[test]
fn should_keep_orderbook_randomness_independent() {
    ext().execute_with(|| {
        let order_book_id_1 = OrderBookId {
            dex_id: DEXId::Polkaswap.into(),
            base: VAL,
            quote: XOR,
        };
        let order_book_id_2 = OrderBookId {
            dex_id: DEXId::Polkaswap.into(),
            base: PSWAP,
            quote: XOR,
        };
        let price_step = default_price_step();
        let orders_per_price = 3;
        let best_bid_price = balance!(10);
        let best_ask_price = balance!(11);
        let steps = 4;
        let amount_range = settings::RandomAmount::new(balance!(1), balance!(10));
        let fill_settings_1 = settings::OrderBookFill {
            bids: Some(settings::SideFill {
                highest_price: best_bid_price,
                lowest_price: best_bid_price - (steps - 1) as u128 * price_step,
                price_step,
                orders_per_price,
                lifespan: None,
                amount_range_inclusive: Some(amount_range),
            }),
            asks: Some(settings::SideFill {
                highest_price: best_ask_price + (steps - 1) as u128 * price_step,
                lowest_price: best_ask_price,
                price_step,
                orders_per_price,
                lifespan: None,
                amount_range_inclusive: Some(amount_range),
            }),
            random_seed: None,
        };
        let fill_settings_2 = settings::OrderBookFill {
            bids: None,
            ..fill_settings_1.clone()
        };
        assert_ok!(QAToolsPallet::order_book_create_and_fill_batch(
            RuntimeOrigin::root(),
            alice(),
            alice(),
            vec![
                (
                    order_book_id_1,
                    settings::OrderBookAttributes::default(),
                    fill_settings_1
                ),
                (
                    order_book_id_2,
                    settings::OrderBookAttributes::default(),
                    fill_settings_2
                )
            ]
        ));

        let mut data = order_book::storage_data_layer::StorageDataLayer::<Runtime>::new();
        let mut asks_1: Vec<_> = data
            .get_all_limit_orders(&order_book_id_1)
            .into_iter()
            .filter(|order| order.side == PriceVariant::Sell)
            .collect();
        let mut asks_2: Vec<_> = data
            .get_all_limit_orders(&order_book_id_2)
            .into_iter()
            .filter(|order| order.side == PriceVariant::Sell)
            .collect();
        fn cmp_by_id(a: &LimitOrder<Runtime>, b: &LimitOrder<Runtime>) -> sp_std::cmp::Ordering {
            a.id.cmp(&b.id)
        }
        asks_1.sort_by(cmp_by_id);
        asks_2.sort_by(cmp_by_id);
        fn order_without_id<T: qa_tools::Config>(
            order: LimitOrder<T>,
        ) -> (
            T::AccountId,
            PriceVariant,
            OrderPrice,
            OrderVolume,
            OrderVolume,
            MomentOf<T>,
            MomentOf<T>,
            BlockNumberFor<T>,
        ) {
            (
                order.owner,
                order.side,
                order.price,
                order.original_amount,
                order.amount,
                order.time,
                order.lifespan,
                order.expires_at,
            )
        }
        let asks_1: Vec<_> = asks_1.into_iter().map(order_without_id).collect();
        let asks_2: Vec<_> = asks_2.into_iter().map(order_without_id).collect();

        assert_eq!(asks_1, asks_2);
    })
}

#[test]
fn should_reject_incorrect_orderbook_fill_settings() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId {
            dex_id: DEXId::Polkaswap.into(),
            base: VAL,
            quote: XOR,
        };
        let price_step = default_price_step();
        let orders_per_price = 3;
        let best_bid_price = balance!(10);
        let steps = 4;
        let amount_range = settings::RandomAmount::new(balance!(1), balance!(10));
        let correct_bids_settings = settings::SideFill {
            highest_price: best_bid_price,
            lowest_price: best_bid_price - (steps - 1) as u128 * price_step,
            price_step,
            orders_per_price,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        let mut bids_settings = correct_bids_settings.clone();
        bids_settings.price_step = 1;
        let fill_settings = settings::OrderBookFill {
            bids: Some(bids_settings),
            asks: None,
            random_seed: None,
        };
        let mut err: DispatchErrorWithPostInfo<PostDispatchInfo> =
            Error::<Runtime>::IncorrectPrice.into();
        err.post_info.pays_fee = Pays::No;
        assert_err!(
            QAToolsPallet::order_book_create_and_fill_batch(
                RuntimeOrigin::root(),
                alice(),
                alice(),
                vec![(
                    order_book_id,
                    settings::OrderBookAttributes::default(),
                    fill_settings
                )]
            ),
            err
        );
        let mut bids_settings = correct_bids_settings;
        bids_settings.price_step = 0;
        let fill_settings = settings::OrderBookFill {
            bids: Some(bids_settings),
            asks: None,
            random_seed: None,
        };
        assert_err!(
            QAToolsPallet::order_book_create_and_fill_batch(
                RuntimeOrigin::root(),
                alice(),
                alice(),
                vec![(
                    order_book_id,
                    settings::OrderBookAttributes::default(),
                    fill_settings
                )]
            ),
            err
        );
    });
}

#[test]
fn should_reject_too_many_orders() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId {
            dex_id: DEXId::Polkaswap.into(),
            base: VAL,
            quote: XOR,
        };
        let price_step = default_price_step();
        let amount_range = settings::RandomAmount::new(balance!(1), balance!(10));

        // 100 001 prices by 10 orders = 1 000 010 orders
        let wrong_settings1 = settings::SideFill {
            highest_price: balance!(10),
            lowest_price: balance!(9),
            price_step,
            orders_per_price: 10,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        let fill_settings = settings::OrderBookFill {
            bids: Some(wrong_settings1),
            asks: None,
            random_seed: None,
        };

        let mut err: DispatchErrorWithPostInfo<PostDispatchInfo> =
            Error::<Runtime>::TooManyPrices.into();
        err.post_info.pays_fee = Pays::No;
        assert_err!(
            QAToolsPallet::order_book_create_and_fill_batch(
                RuntimeOrigin::root(),
                alice(),
                alice(),
                vec![(
                    order_book_id,
                    settings::OrderBookAttributes::default(),
                    fill_settings
                )]
            ),
            err
        );

        // 1 price by 10 000 orders = 10 000 orders
        let wrong_settings2 = settings::SideFill {
            highest_price: balance!(10),
            lowest_price: balance!(10),
            price_step,
            orders_per_price: 10_000,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        let fill_settings = settings::OrderBookFill {
            bids: Some(wrong_settings2),
            asks: None,
            random_seed: None,
        };

        let mut err: DispatchErrorWithPostInfo<PostDispatchInfo> =
            Error::<Runtime>::TooManyOrders.into();
        err.post_info.pays_fee = Pays::No;
        assert_err!(
            QAToolsPallet::order_book_create_and_fill_batch(
                RuntimeOrigin::root(),
                alice(),
                alice(),
                vec![(
                    order_book_id,
                    settings::OrderBookAttributes::default(),
                    fill_settings
                )]
            ),
            err
        );

        // 11 prices by 100 orders = 1100 orders
        let wrong_settings3 = settings::SideFill {
            highest_price: balance!(10),
            lowest_price: balance!(10) - 10 * price_step,
            price_step,
            orders_per_price: 100,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        let fill_settings = settings::OrderBookFill {
            bids: Some(wrong_settings3),
            asks: None,
            random_seed: None,
        };

        let mut err: DispatchErrorWithPostInfo<PostDispatchInfo> =
            Error::<Runtime>::TooManyOrders.into();
        err.post_info.pays_fee = Pays::No;
        assert_err!(
            QAToolsPallet::order_book_create_and_fill_batch(
                RuntimeOrigin::root(),
                alice(),
                alice(),
                vec![(
                    order_book_id,
                    settings::OrderBookAttributes::default(),
                    fill_settings
                )]
            ),
            err
        );
    });
}

#[test]
fn should_create_and_fill_orderbook_max_orders_count() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId {
            dex_id: DEXId::Polkaswap.into(),
            base: VAL,
            quote: XOR,
        };
        let price_step = default_price_step();
        let amount_range = settings::RandomAmount::new(balance!(1), balance!(10));

        // 10 prices by 100 orders = 1000 orders
        let bids_settings = settings::SideFill {
            highest_price: balance!(10),
            lowest_price: balance!(10) - 9 * price_step,
            price_step,
            orders_per_price: 100,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        // 100 prices by 10 orders = 1000 orders
        let asks_settings = settings::SideFill {
            highest_price: balance!(11) + 99 * price_step,
            lowest_price: balance!(11),
            price_step,
            orders_per_price: 10,
            lifespan: Some(2_590_000_000),
            amount_range_inclusive: Some(amount_range),
        };
        let fill_settings = settings::OrderBookFill {
            bids: Some(bids_settings),
            asks: Some(asks_settings),
            random_seed: None,
        };

        assert_ok!(QAToolsPallet::order_book_create_and_fill_batch(
            RuntimeOrigin::root(),
            alice(),
            bob(),
            vec![(
                order_book_id,
                settings::OrderBookAttributes::default(),
                fill_settings
            )]
        ));

        assert!(order_book::Pallet::<Runtime>::order_books(order_book_id).is_some());

        assert_eq!(
            order_book::Pallet::<Runtime>::aggregated_bids(order_book_id).len(),
            10
        );
        assert_eq!(
            order_book::Pallet::<Runtime>::aggregated_asks(order_book_id).len(),
            100
        );
        assert_eq!(
            order_book::LimitOrders::<Runtime>::iter_prefix(order_book_id).count(),
            2000
        );
    });
}

#[test]
fn should_not_create_existing_order_book() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId {
            dex_id: DEXId::Polkaswap.into(),
            base: VAL,
            quote: XOR,
        };
        let price_step = default_price_step();
        let amount_range = settings::RandomAmount::new(balance!(1), balance!(10));

        let bids_settings = settings::SideFill {
            highest_price: balance!(10),
            lowest_price: balance!(10) - 9 * price_step,
            price_step,
            orders_per_price: 10,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        let asks_settings = settings::SideFill {
            highest_price: balance!(11) + 9 * price_step,
            lowest_price: balance!(11),
            price_step,
            orders_per_price: 10,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        let fill_settings = settings::OrderBookFill {
            bids: Some(bids_settings),
            asks: Some(asks_settings),
            random_seed: None,
        };

        assert_ok!(QAToolsPallet::order_book_create_and_fill_batch(
            RuntimeOrigin::root(),
            alice(),
            bob(),
            vec![(
                order_book_id,
                settings::OrderBookAttributes::default(),
                fill_settings.clone()
            )]
        ));

        assert!(order_book::Pallet::<Runtime>::order_books(order_book_id).is_some());

        let mut err: DispatchErrorWithPostInfo<PostDispatchInfo> =
            Error::<Runtime>::OrderBookAlreadyExists.into();
        err.post_info.pays_fee = Pays::No;
        assert_err!(
            QAToolsPallet::order_book_create_and_fill_batch(
                RuntimeOrigin::root(),
                alice(),
                bob(),
                vec![(
                    order_book_id,
                    settings::OrderBookAttributes::default(),
                    fill_settings
                )]
            ),
            err
        );
    });
}

#[test]
fn should_not_fill_non_existing_order_book() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId {
            dex_id: DEXId::Polkaswap.into(),
            base: VAL,
            quote: XOR,
        };
        let price_step = default_price_step();
        let amount_range = settings::RandomAmount::new(balance!(1), balance!(10));

        let bids_settings = settings::SideFill {
            highest_price: balance!(10),
            lowest_price: balance!(10) - 9 * price_step,
            price_step,
            orders_per_price: 10,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        let asks_settings = settings::SideFill {
            highest_price: balance!(11) + 9 * price_step,
            lowest_price: balance!(11),
            price_step,
            orders_per_price: 10,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        let fill_settings = settings::OrderBookFill {
            bids: Some(bids_settings),
            asks: Some(asks_settings),
            random_seed: None,
        };

        let mut err: DispatchErrorWithPostInfo<PostDispatchInfo> =
            Error::<Runtime>::CannotFillUnknownOrderBook.into();
        err.post_info.pays_fee = Pays::No;
        assert_err!(
            QAToolsPallet::order_book_fill_batch(
                RuntimeOrigin::root(),
                alice(),
                bob(),
                vec![(order_book_id, fill_settings)]
            ),
            err
        );
    });
}

#[test]
fn should_fill_order_book() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId {
            dex_id: DEXId::Polkaswap.into(),
            base: VAL,
            quote: XOR,
        };
        let price_step = default_price_step();
        let amount_range = settings::RandomAmount::new(balance!(1), balance!(10));

        let bids_settings = settings::SideFill {
            highest_price: balance!(10),
            lowest_price: balance!(10) - 9 * price_step,
            price_step,
            orders_per_price: 10,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        let asks_settings = settings::SideFill {
            highest_price: balance!(11) + 9 * price_step,
            lowest_price: balance!(11),
            price_step,
            orders_per_price: 10,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        let fill_settings = settings::OrderBookFill {
            bids: Some(bids_settings),
            asks: Some(asks_settings),
            random_seed: None,
        };

        assert_ok!(QAToolsPallet::order_book_create_and_fill_batch(
            RuntimeOrigin::root(),
            alice(),
            bob(),
            vec![(
                order_book_id,
                settings::OrderBookAttributes::default(),
                fill_settings.clone()
            )]
        ));

        assert!(order_book::Pallet::<Runtime>::order_books(order_book_id).is_some());

        assert_ok!(QAToolsPallet::order_book_fill_batch(
            RuntimeOrigin::root(),
            alice(),
            bob(),
            vec![(order_book_id, fill_settings)]
        ));
    });
}

#[test]
fn should_fill_orderbook_max_orders_count() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId {
            dex_id: DEXId::Polkaswap.into(),
            base: VAL,
            quote: XOR,
        };
        let price_step = default_price_step();
        let amount_range = settings::RandomAmount::new(balance!(1), balance!(10));

        // 10 prices by 100 orders = 1000 orders
        let bids_settings = settings::SideFill {
            highest_price: balance!(10),
            lowest_price: balance!(10) - 9 * price_step,
            price_step,
            orders_per_price: 100,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        // 100 prices by 10 orders = 1000 orders
        let asks_settings = settings::SideFill {
            highest_price: balance!(11) + 99 * price_step,
            lowest_price: balance!(11),
            price_step,
            orders_per_price: 10,
            lifespan: Some(2_590_000_000),
            amount_range_inclusive: Some(amount_range),
        };
        let fill_settings = settings::OrderBookFill {
            bids: Some(bids_settings),
            asks: Some(asks_settings),
            random_seed: None,
        };

        assert_ok!(QAToolsPallet::order_book_create_and_fill_batch(
            RuntimeOrigin::root(),
            alice(),
            bob(),
            vec![(
                order_book_id,
                settings::OrderBookAttributes::default(),
                fill_settings.clone()
            )]
        ));

        assert!(order_book::Pallet::<Runtime>::order_books(order_book_id).is_some());

        assert_eq!(
            order_book::Pallet::<Runtime>::aggregated_bids(order_book_id).len(),
            10
        );
        assert_eq!(
            order_book::Pallet::<Runtime>::aggregated_asks(order_book_id).len(),
            100
        );
        assert_eq!(
            order_book::LimitOrders::<Runtime>::iter_prefix(order_book_id).count(),
            2000
        );

        let current_block = FrameSystem::block_number();
        run_to_block(current_block + 1);

        assert_ok!(QAToolsPallet::order_book_fill_batch(
            RuntimeOrigin::root(),
            charlie(),
            dave(),
            vec![(order_book_id, fill_settings)]
        ));

        assert_eq!(
            order_book::Pallet::<Runtime>::aggregated_bids(order_book_id).len(),
            10
        );
        assert_eq!(
            order_book::Pallet::<Runtime>::aggregated_asks(order_book_id).len(),
            100
        );
        assert_eq!(
            order_book::LimitOrders::<Runtime>::iter_prefix(order_book_id).count(),
            4000
        );
    });
}

#[test]
fn should_initialize_xyk_pool() {
    ext().execute_with(|| {
        let pairs = vec![
            XYKPair::new(DEXId::Polkaswap.into(), XOR, VAL, balance!(0.5)),
            XYKPair::new(DEXId::Polkaswap.into(), XOR, ETH, balance!(0.1)),
            XYKPair::new(DEXId::Polkaswap.into(), XOR, PSWAP, balance!(1)),
            XYKPair::new(DEXId::Polkaswap.into(), XOR, DAI, balance!(10)),
            XYKPair::new(DEXId::Polkaswap.into(), XOR, XST, balance!(0.5)),
            XYKPair::new(DEXId::Polkaswap.into(), XOR, TBCD, balance!(0.5)),
            XYKPair::new(DEXId::PolkaswapXSTUSD.into(), XSTUSD, VAL, balance!(0.5)),
            XYKPair::new(DEXId::PolkaswapXSTUSD.into(), XSTUSD, PSWAP, balance!(0.5)),
            XYKPair::new(
                DEXId::PolkaswapXSTUSD.into(),
                XSTUSD,
                ETH,
                balance!(0.000000000000000001),
            ),
            XYKPair::new(DEXId::PolkaswapXSTUSD.into(), XSTUSD, DAI, balance!(0.5)),
        ];
        let prices =
            qa_tools::source_initialization::xyk::<Runtime>(alice(), pairs.clone()).unwrap();

        for (expected_pair, actual_pair) in pairs.into_iter().zip(prices.into_iter()) {
            let result = pool_xyk::Pallet::<Runtime>::quote_without_impact(
                &expected_pair.dex_id,
                &expected_pair.asset_a,
                &expected_pair.asset_b,
                QuoteAmount::WithDesiredInput {
                    desired_amount_in: balance!(1),
                },
                false,
            )
            .unwrap();
            // `deduce_fee` was set to false
            assert_eq!(result.fee, 0);
            let price = result.amount;
            assert_eq!(actual_pair.price, price);
            assert_approx_eq!(actual_pair.price, expected_pair.price, 10, 0);
        }
    })
}

#[test]
fn should_not_initialize_existing_xyk_pool() {
    ext().execute_with(|| {
        assert_ok!(QAToolsPallet::initialize_xyk(
            RuntimeOrigin::root(),
            alice(),
            vec![
                XYKPair::new(DEXId::Polkaswap.into(), XOR, VAL, balance!(0.5)),
                XYKPair::new(DEXId::PolkaswapXSTUSD.into(), XSTUSD, VAL, balance!(0.5))
            ],
        ));
        assert_eq!(
            QAToolsPallet::initialize_xyk(
                RuntimeOrigin::root(),
                alice(),
                vec![XYKPair::new(
                    DEXId::Polkaswap.into(),
                    XOR,
                    VAL,
                    balance!(0.5)
                ),],
            ),
            Err(err_pays_no(
                pool_xyk::Error::<Runtime>::PoolIsAlreadyInitialized
            ))
        );
    })
}

fn test_init_xst_synthetic_base_price(prices: XSTBaseInput) {
    ext().execute_with(|| {
        // DAI
        let reference_asset_id = xst::ReferenceAssetId::<Runtime>::get();
        // XST
        let synthetic_base_asset_id = <Runtime as xst::Config>::GetSyntheticBaseAssetId::get();

        assert_ok!(QAToolsPallet::initialize_xst(
            RuntimeOrigin::root(),
            Some(prices.clone()),
            vec![],
            alice(),
        ));
        assert_eq!(
            price_tools::Pallet::<Runtime>::get_average_price(
                &synthetic_base_asset_id,
                &reference_asset_id,
                PriceVariant::Buy
            ),
            Ok(prices.buy.reference_per_synthetic_base)
        );
        assert_eq!(
            price_tools::Pallet::<Runtime>::get_average_price(
                &synthetic_base_asset_id,
                &reference_asset_id,
                PriceVariant::Sell
            ),
            Ok(prices.sell.reference_per_synthetic_base)
        );
    });
}

#[test]
fn should_init_xst_synthetic_base_price() {
    let prices = XSTBaseInput {
        buy: XSTBaseSideInput {
            reference_per_synthetic_base: balance!(1),
            reference_per_xor: Some(balance!(1)),
        },
        sell: XSTBaseSideInput {
            reference_per_synthetic_base: balance!(1),
            reference_per_xor: Some(balance!(1)),
        },
    };
    test_init_xst_synthetic_base_price(prices);
    let prices = XSTBaseInput {
        buy: XSTBaseSideInput {
            reference_per_synthetic_base: balance!(3),
            reference_per_xor: Some(balance!(5)),
        },
        sell: XSTBaseSideInput {
            reference_per_synthetic_base: balance!(1),
            reference_per_xor: Some(balance!(2)),
        },
    };
    test_init_xst_synthetic_base_price(prices);
}

#[test]
fn should_reject_incorrect_xst_synthetic_base_price() {
    ext().execute_with(|| {
        assert_eq!(
            QAToolsPallet::initialize_xst(
                RuntimeOrigin::root(),
                Some(XSTBaseInput {
                    buy: XSTBaseSideInput {
                        reference_per_synthetic_base: balance!(1),
                        reference_per_xor: Some(balance!(1)),
                    },
                    sell: XSTBaseSideInput {
                        reference_per_synthetic_base: balance!(1.1),
                        reference_per_xor: Some(balance!(1)),
                    },
                }),
                vec![],
                alice(),
            ),
            Err(err_pays_no(Error::<Runtime>::BuyLessThanSell))
        );
        assert_eq!(
            QAToolsPallet::initialize_xst(
                RuntimeOrigin::root(),
                Some(XSTBaseInput {
                    buy: XSTBaseSideInput {
                        reference_per_synthetic_base: balance!(1),
                        reference_per_xor: Some(balance!(1)),
                    },
                    sell: XSTBaseSideInput {
                        reference_per_synthetic_base: balance!(1),
                        reference_per_xor: Some(balance!(1.1)),
                    },
                }),
                vec![],
                alice(),
            ),
            Err(err_pays_no(Error::<Runtime>::BuyLessThanSell))
        );
    })
}

#[test]
fn should_reject_deduce_only_with_uninitialized_reference_asset() {
    ext().execute_with(|| {
        // Reject when not initialized
        assert_eq!(
            QAToolsPallet::initialize_xst(
                RuntimeOrigin::root(),
                Some(XSTBaseInput {
                    buy: XSTBaseSideInput {
                        reference_per_synthetic_base: balance!(1),
                        reference_per_xor: None,
                    },
                    sell: XSTBaseSideInput {
                        reference_per_synthetic_base: balance!(1),
                        reference_per_xor: Some(balance!(1)),
                    },
                }),
                vec![],
                alice(),
            ),
            Err(err_pays_no(Error::<Runtime>::ReferenceAssetPriceNotFound))
        );
        assert_eq!(
            QAToolsPallet::initialize_xst(
                RuntimeOrigin::root(),
                Some(XSTBaseInput {
                    buy: XSTBaseSideInput {
                        reference_per_synthetic_base: balance!(1),
                        reference_per_xor: Some(balance!(1)),
                    },
                    sell: XSTBaseSideInput {
                        reference_per_synthetic_base: balance!(1),
                        reference_per_xor: None,
                    },
                }),
                vec![],
                alice(),
            ),
            Err(err_pays_no(Error::<Runtime>::ReferenceAssetPriceNotFound))
        );

        // Initialize the reference asset
        assert_ok!(QAToolsPallet::initialize_xst(
            RuntimeOrigin::root(),
            Some(XSTBaseInput {
                buy: XSTBaseSideInput {
                    reference_per_synthetic_base: balance!(3),
                    reference_per_xor: Some(balance!(5)),
                },
                sell: XSTBaseSideInput {
                    reference_per_synthetic_base: balance!(1),
                    reference_per_xor: Some(balance!(2)),
                },
            }),
            vec![],
            alice(),
        ));

        // Now it should work fine
        let (reference_per_synthetic_base_buy, reference_per_synthetic_base_sell) =
            (balance!(21), balance!(7));
        assert_ok!(QAToolsPallet::initialize_xst(
            RuntimeOrigin::root(),
            Some(XSTBaseInput {
                buy: XSTBaseSideInput {
                    reference_per_synthetic_base: reference_per_synthetic_base_buy,
                    reference_per_xor: None,
                },
                sell: XSTBaseSideInput {
                    reference_per_synthetic_base: reference_per_synthetic_base_sell,
                    reference_per_xor: None,
                },
            }),
            vec![],
            alice(),
        ));
        // check prices
        let reference_per_xor_buy = price_tools::Pallet::<Runtime>::get_average_price(
            &XOR,
            &xst::ReferenceAssetId::<Runtime>::get(),
            PriceVariant::Buy,
        )
        .unwrap();
        let reference_per_xor_sell = price_tools::Pallet::<Runtime>::get_average_price(
            &XOR,
            &xst::ReferenceAssetId::<Runtime>::get(),
            PriceVariant::Sell,
        )
        .unwrap();
        let synthetic_base_per_xor_buy = BalanceUnit::divisible(reference_per_xor_sell)
            / BalanceUnit::divisible(reference_per_synthetic_base_sell);
        assert_eq!(
            price_tools::Pallet::<Runtime>::get_average_price(
                &XOR,
                &<Runtime as xst::Config>::GetSyntheticBaseAssetId::get(),
                PriceVariant::Buy,
            )
            .unwrap(),
            *synthetic_base_per_xor_buy.balance()
        );
        let synthetic_base_per_xor_sell = BalanceUnit::divisible(reference_per_xor_buy)
            / BalanceUnit::divisible(reference_per_synthetic_base_buy);
        assert_eq!(
            price_tools::Pallet::<Runtime>::get_average_price(
                &XOR,
                &<Runtime as xst::Config>::GetSyntheticBaseAssetId::get(),
                PriceVariant::Sell,
            )
            .unwrap(),
            *synthetic_base_per_xor_sell.balance()
        );
    })
}

fn euro_init_input<T: qa_tools::Config>(
    expected_quote: XSTSyntheticQuote,
) -> XSTSyntheticInput<T::AssetId, <T as qa_tools::Config>::Symbol> {
    let symbol_name =
        SymbolName::from_str("EURO").expect("Failed to parse `symbol_name` as a symbol name");
    let asset_id = AssetId32::<PredefinedAssetId>::from_synthetic_reference_symbol(&symbol_name);
    let symbol = AssetSymbol("XSTEUR".into());
    let name = AssetName("XST Euro".into());
    let fee_ratio = fixed!(0);
    XSTSyntheticInput {
        asset_id: asset_id.into(),
        expected_quote,
        existence: XSTSyntheticExistence::RegisterNewAsset {
            symbol,
            name,
            reference_symbol: symbol_name.into(),
            fee_ratio,
        },
    }
}

/// Returns results of initialization
fn test_synthetic_price_set<T: qa_tools::Config>(
    synthetic_input: XSTSyntheticInput<T::AssetId, <T as qa_tools::Config>::Symbol>,
    base_input: Option<XSTBaseInput>,
    relayer: T::AccountId,
) -> Vec<XSTSyntheticOutput<T::AssetId>> {
    let synthetic_base_asset_id = <T as xst::Config>::GetSyntheticBaseAssetId::get();
    let init_result = qa_tools::source_initialization::xst::<T>(
        base_input,
        vec![synthetic_input.clone()],
        relayer,
    )
    .unwrap();
    assert_approx_eq!(
        synthetic_input.expected_quote.result,
        init_result[0].quote_achieved.result,
        10,
        0.0001f64
    );

    let (input_asset_id, output_asset_id) = match synthetic_input.expected_quote.direction {
        XSTSyntheticQuoteDirection::SyntheticBaseToSynthetic => {
            (synthetic_base_asset_id, synthetic_input.asset_id)
        }
        XSTSyntheticQuoteDirection::SyntheticToSyntheticBase => {
            (synthetic_input.asset_id, synthetic_base_asset_id)
        }
    };
    let (quote_result, _) = xst::Pallet::<T>::quote(
        &DEXId::Polkaswap.into(),
        &input_asset_id,
        &output_asset_id,
        synthetic_input.expected_quote.amount,
        false,
    )
    .unwrap();
    assert_eq!(quote_result.amount, init_result[0].quote_achieved.result);
    assert_eq!(quote_result.fee, 0);
    init_result
}

fn test_init_xst_synthetic_price_unit_prices(reversed: bool) {
    // XST
    let synthetic_base_asset_id = <Runtime as xst::Config>::GetSyntheticBaseAssetId::get();

    // simple for calculations, even though quite unrealistic
    let prices = XSTBaseInput {
        buy: XSTBaseSideInput {
            reference_per_synthetic_base: balance!(1),
            reference_per_xor: Some(balance!(1)),
        },
        sell: XSTBaseSideInput {
            reference_per_synthetic_base: balance!(1),
            reference_per_xor: Some(balance!(1)),
        },
    };

    let mut quote = XSTSyntheticQuote {
        direction: XSTSyntheticQuoteDirection::SyntheticBaseToSynthetic,
        amount: QuoteAmount::WithDesiredOutput {
            desired_amount_out: balance!(1),
        },
        result: balance!(1),
    };
    if reversed {
        quote.direction = XSTSyntheticQuoteDirection::SyntheticToSyntheticBase
    }
    let euro_init = euro_init_input::<Runtime>(quote);
    test_synthetic_price_set::<Runtime>(euro_init.clone(), Some(prices), alice());
    // additionally check other directions/variants
    let (quote_result, _) = xst::Pallet::<Runtime>::quote(
        &DEXId::Polkaswap.into(),
        &synthetic_base_asset_id,
        &euro_init.asset_id,
        QuoteAmount::WithDesiredInput {
            desired_amount_in: balance!(1),
        },
        false,
    )
    .unwrap();
    assert_eq!(quote_result.amount, balance!(1));
    assert_eq!(quote_result.fee, 0);
    let (quote_result, _) = xst::Pallet::<Runtime>::quote(
        &DEXId::Polkaswap.into(),
        &euro_init.asset_id,
        &synthetic_base_asset_id,
        QuoteAmount::WithDesiredInput {
            desired_amount_in: balance!(1),
        },
        false,
    )
    .unwrap();
    assert_eq!(quote_result.amount, balance!(1));
    assert_eq!(quote_result.fee, 0);
    let (quote_result, _) = xst::Pallet::<Runtime>::quote(
        &DEXId::Polkaswap.into(),
        &euro_init.asset_id,
        &synthetic_base_asset_id,
        QuoteAmount::WithDesiredOutput {
            desired_amount_out: balance!(1),
        },
        false,
    )
    .unwrap();
    assert_eq!(quote_result.amount, balance!(1));
    assert_eq!(quote_result.fee, 0);
}

#[test]
fn should_init_xst_synthetic_price_unit_prices_forward() {
    ext().execute_with(|| {
        test_init_xst_synthetic_price_unit_prices(false);
    })
}

#[test]
fn should_init_xst_synthetic_price_unit_prices_reverse() {
    ext().execute_with(|| {
        test_init_xst_synthetic_price_unit_prices(true);
    })
}

#[test]
fn should_init_xst_synthetic_price_various_prices() {
    ext().execute_with(|| {
        let prices = XSTBaseInput {
            buy: XSTBaseSideInput {
                reference_per_synthetic_base: balance!(5),
                reference_per_xor: Some(balance!(7)),
            },
            sell: XSTBaseSideInput {
                reference_per_synthetic_base: balance!(2),
                reference_per_xor: Some(balance!(3)),
            },
        };
        let euro_init = euro_init_input::<Runtime>(XSTSyntheticQuote {
            direction: XSTSyntheticQuoteDirection::SyntheticBaseToSynthetic,
            amount: QuoteAmount::WithDesiredOutput {
                desired_amount_out: balance!(1),
            },
            result: balance!(33),
        });
        test_synthetic_price_set::<Runtime>(euro_init, Some(prices), alice());
    })
}

#[test]
fn should_update_xst_synthetic_price() {
    ext().execute_with(|| {
        // XST
        let synthetic_base_asset_id = <Runtime as xst::Config>::GetSyntheticBaseAssetId::get();

        // Some initial values
        let prices = XSTBaseInput {
            buy: XSTBaseSideInput {
                reference_per_synthetic_base: balance!(3),
                reference_per_xor: Some(balance!(5)),
            },
            sell: XSTBaseSideInput {
                reference_per_synthetic_base: balance!(1),
                reference_per_xor: Some(balance!(2)),
            },
        };

        let euro_init = euro_init_input::<Runtime>(XSTSyntheticQuote {
            direction: XSTSyntheticQuoteDirection::SyntheticBaseToSynthetic,
            amount: QuoteAmount::WithDesiredOutput {
                desired_amount_out: balance!(1),
            },
            result: balance!(123),
        });
        let euro_asset_id = euro_init.asset_id;
        test_synthetic_price_set::<Runtime>(euro_init, Some(prices), alice());
        // correctly updates prices
        let prices = XSTBaseInput {
            buy: XSTBaseSideInput {
                reference_per_synthetic_base: balance!(1),
                reference_per_xor: Some(balance!(1)),
            },
            sell: XSTBaseSideInput {
                reference_per_synthetic_base: balance!(1),
                reference_per_xor: Some(balance!(1)),
            },
        };
        let euro_init = XSTSyntheticInput {
            asset_id: euro_asset_id,
            expected_quote: XSTSyntheticQuote {
                direction: XSTSyntheticQuoteDirection::SyntheticBaseToSynthetic,
                amount: QuoteAmount::WithDesiredInput {
                    desired_amount_in: balance!(1),
                },
                result: balance!(33),
            },
            existence: XSTSyntheticExistence::AlreadyExists,
        };
        test_synthetic_price_set::<Runtime>(euro_init, Some(prices), alice());

        // other variants
        let euro_init = XSTSyntheticInput {
            asset_id: euro_asset_id,
            expected_quote: XSTSyntheticQuote {
                direction: XSTSyntheticQuoteDirection::SyntheticBaseToSynthetic,
                amount: QuoteAmount::WithDesiredOutput {
                    desired_amount_out: balance!(1),
                },
                result: balance!(33),
            },
            existence: XSTSyntheticExistence::AlreadyExists,
        };
        test_synthetic_price_set::<Runtime>(euro_init, None, alice());
        let euro_init = XSTSyntheticInput {
            asset_id: euro_asset_id,
            expected_quote: XSTSyntheticQuote {
                direction: XSTSyntheticQuoteDirection::SyntheticToSyntheticBase,
                amount: QuoteAmount::WithDesiredInput {
                    desired_amount_in: balance!(1),
                },
                result: balance!(33),
            },
            existence: XSTSyntheticExistence::AlreadyExists,
        };
        test_synthetic_price_set::<Runtime>(euro_init, None, alice());
        let euro_init = XSTSyntheticInput {
            asset_id: euro_asset_id,
            expected_quote: XSTSyntheticQuote {
                direction: XSTSyntheticQuoteDirection::SyntheticToSyntheticBase,
                amount: QuoteAmount::WithDesiredOutput {
                    desired_amount_out: balance!(1),
                },
                result: balance!(33),
            },
            existence: XSTSyntheticExistence::AlreadyExists,
        };
        let init_result = test_synthetic_price_set::<Runtime>(euro_init.clone(), None, alice());

        // prices actually change
        let prices = XSTBaseInput {
            buy: XSTBaseSideInput {
                reference_per_synthetic_base: balance!(321),
                reference_per_xor: Some(balance!(5)),
            },
            sell: XSTBaseSideInput {
                reference_per_synthetic_base: balance!(123),
                reference_per_xor: Some(balance!(2)),
            },
        };
        assert_ok!(QAToolsPallet::initialize_xst(
            RuntimeOrigin::root(),
            Some(prices),
            vec![],
            alice(),
        ));
        let (quote_result, _) = xst::Pallet::<Runtime>::quote(
            &DEXId::Polkaswap.into(),
            &synthetic_base_asset_id,
            &euro_asset_id,
            euro_init.expected_quote.amount,
            false,
        )
        .unwrap();
        assert_ne!(quote_result.amount, init_result[0].quote_achieved.result);
        assert_eq!(quote_result.fee, 0);

        // Synthetic prices are updated correctly after changes in base assets prices.
        test_synthetic_price_set::<Runtime>(euro_init, None, alice());
    })
}
