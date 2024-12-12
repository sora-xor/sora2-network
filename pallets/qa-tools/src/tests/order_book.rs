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

use super::{alice, bob, charlie, dave, FrameSystem, QaToolsPallet};
use assets::AssetIdOf;
use common::{
    balance, AssetId32, AssetName, AssetSymbol, Balance, DEXId, DexIdOf, OrderBookId,
    PredefinedAssetId, PriceVariant, PSWAP, VAL, XOR,
};
use frame_support::dispatch::{Pays, PostDispatchInfo};
use frame_support::traits::Hooks;
use frame_support::{assert_err, assert_ok};
use frame_system::pallet_prelude::BlockNumberFor;
use framenode_chain_spec::ext;
use framenode_runtime::qa_tools;
use framenode_runtime::{Runtime, RuntimeOrigin};
use order_book::{DataLayer, LimitOrder, MomentOf, OrderPrice, OrderVolume};
use qa_tools::{pallet_tools, Error};
use sp_runtime::traits::BadOrigin;
use sp_runtime::DispatchErrorWithPostInfo;

fn default_price_step() -> Balance {
    pallet_tools::order_book::OrderBookAttributes::default().tick_size
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
    attributes: pallet_tools::order_book::OrderBookAttributes,
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
        QaToolsPallet::order_book_create_and_fill_batch(
            RuntimeOrigin::signed(alice()),
            alice(),
            alice(),
            vec![]
        ),
        BadOrigin
    );

    let price_step = default_price_step();
    let orders_per_price = 3;
    let amount_range = pallet_tools::order_book::RandomAmount::new(amount_range.0, amount_range.1);
    assert_ok!(QaToolsPallet::order_book_create_and_fill_batch(
        RuntimeOrigin::root(),
        alice(),
        alice(),
        vec![(
            order_book_id,
            attributes,
            pallet_tools::order_book::FillInput {
                bids: Some(pallet_tools::order_book::SideFillInput {
                    highest_price: best_bid_price,
                    lowest_price: best_bid_price - (steps - 1) as u128 * price_step,
                    price_step,
                    orders_per_price,
                    lifespan: None,
                    amount_range_inclusive: Some(amount_range)
                }),
                asks: Some(pallet_tools::order_book::SideFillInput {
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
            pallet_tools::order_book::OrderBookAttributes::default(),
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
            common::AssetType::NFT,
            None,
            None,
        )
        .unwrap();
        test_creates_orderbook(
            nft,
            XOR,
            pallet_tools::order_book::OrderBookAttributes {
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
            pallet_tools::order_book::OrderBookAttributes::default(),
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
            common::AssetType::NFT,
            None,
            None,
        )
        .unwrap();
        test_creates_orderbook(
            nft,
            XOR,
            pallet_tools::order_book::OrderBookAttributes {
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
        let amount_range = pallet_tools::order_book::RandomAmount::new(balance!(1), balance!(10));
        let fill_settings = pallet_tools::order_book::FillInput {
            bids: Some(pallet_tools::order_book::SideFillInput {
                highest_price: best_bid_price,
                lowest_price: best_bid_price - (steps - 1) as u128 * price_step,
                price_step,
                orders_per_price,
                lifespan: None,
                amount_range_inclusive: Some(amount_range),
            }),
            asks: Some(pallet_tools::order_book::SideFillInput {
                highest_price: best_ask_price + (steps - 1) as u128 * price_step,
                lowest_price: best_ask_price,
                price_step,
                orders_per_price,
                lifespan: None,
                amount_range_inclusive: Some(amount_range),
            }),
            random_seed: None,
        };
        assert_ok!(QaToolsPallet::order_book_create_and_fill_batch(
            RuntimeOrigin::root(),
            alice(),
            alice(),
            vec![
                (
                    order_book_id_1,
                    pallet_tools::order_book::OrderBookAttributes::default(),
                    fill_settings.clone()
                ),
                (
                    order_book_id_2,
                    pallet_tools::order_book::OrderBookAttributes::default(),
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
        let amount_range = pallet_tools::order_book::RandomAmount::new(balance!(1), balance!(10));
        let fill_settings_1 = pallet_tools::order_book::FillInput {
            bids: Some(pallet_tools::order_book::SideFillInput {
                highest_price: best_bid_price,
                lowest_price: best_bid_price - (steps - 1) as u128 * price_step,
                price_step,
                orders_per_price,
                lifespan: None,
                amount_range_inclusive: Some(amount_range),
            }),
            asks: Some(pallet_tools::order_book::SideFillInput {
                highest_price: best_ask_price + (steps - 1) as u128 * price_step,
                lowest_price: best_ask_price,
                price_step,
                orders_per_price,
                lifespan: None,
                amount_range_inclusive: Some(amount_range),
            }),
            random_seed: None,
        };
        let fill_settings_2 = pallet_tools::order_book::FillInput {
            bids: None,
            ..fill_settings_1.clone()
        };
        assert_ok!(QaToolsPallet::order_book_create_and_fill_batch(
            RuntimeOrigin::root(),
            alice(),
            alice(),
            vec![
                (
                    order_book_id_1,
                    pallet_tools::order_book::OrderBookAttributes::default(),
                    fill_settings_1
                ),
                (
                    order_book_id_2,
                    pallet_tools::order_book::OrderBookAttributes::default(),
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
        let amount_range = pallet_tools::order_book::RandomAmount::new(balance!(1), balance!(10));
        let correct_bids_settings = pallet_tools::order_book::SideFillInput {
            highest_price: best_bid_price,
            lowest_price: best_bid_price - (steps - 1) as u128 * price_step,
            price_step,
            orders_per_price,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        let mut bids_settings = correct_bids_settings.clone();
        bids_settings.price_step = 1;
        let fill_settings = pallet_tools::order_book::FillInput {
            bids: Some(bids_settings),
            asks: None,
            random_seed: None,
        };
        let mut err: DispatchErrorWithPostInfo<PostDispatchInfo> =
            Error::<Runtime>::IncorrectPrice.into();
        err.post_info.pays_fee = Pays::No;
        assert_err!(
            QaToolsPallet::order_book_create_and_fill_batch(
                RuntimeOrigin::root(),
                alice(),
                alice(),
                vec![(
                    order_book_id,
                    pallet_tools::order_book::OrderBookAttributes::default(),
                    fill_settings
                )]
            ),
            err
        );
        let mut bids_settings = correct_bids_settings;
        bids_settings.price_step = 0;
        let fill_settings = pallet_tools::order_book::FillInput {
            bids: Some(bids_settings),
            asks: None,
            random_seed: None,
        };
        assert_err!(
            QaToolsPallet::order_book_create_and_fill_batch(
                RuntimeOrigin::root(),
                alice(),
                alice(),
                vec![(
                    order_book_id,
                    pallet_tools::order_book::OrderBookAttributes::default(),
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
        let amount_range = pallet_tools::order_book::RandomAmount::new(balance!(1), balance!(10));

        // 100 001 prices by 10 orders = 1 000 010 orders
        let wrong_settings1 = pallet_tools::order_book::SideFillInput {
            highest_price: balance!(10),
            lowest_price: balance!(9),
            price_step,
            orders_per_price: 10,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        let fill_settings = pallet_tools::order_book::FillInput {
            bids: Some(wrong_settings1),
            asks: None,
            random_seed: None,
        };

        let mut err: DispatchErrorWithPostInfo<PostDispatchInfo> =
            Error::<Runtime>::TooManyPrices.into();
        err.post_info.pays_fee = Pays::No;
        assert_err!(
            QaToolsPallet::order_book_create_and_fill_batch(
                RuntimeOrigin::root(),
                alice(),
                alice(),
                vec![(
                    order_book_id,
                    pallet_tools::order_book::OrderBookAttributes::default(),
                    fill_settings
                )]
            ),
            err
        );

        // 1 price by 10 000 orders = 10 000 orders
        let wrong_settings2 = pallet_tools::order_book::SideFillInput {
            highest_price: balance!(10),
            lowest_price: balance!(10),
            price_step,
            orders_per_price: 10_000,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        let fill_settings = pallet_tools::order_book::FillInput {
            bids: Some(wrong_settings2),
            asks: None,
            random_seed: None,
        };

        let mut err: DispatchErrorWithPostInfo<PostDispatchInfo> =
            Error::<Runtime>::TooManyOrders.into();
        err.post_info.pays_fee = Pays::No;
        assert_err!(
            QaToolsPallet::order_book_create_and_fill_batch(
                RuntimeOrigin::root(),
                alice(),
                alice(),
                vec![(
                    order_book_id,
                    pallet_tools::order_book::OrderBookAttributes::default(),
                    fill_settings
                )]
            ),
            err
        );

        // 11 prices by 100 orders = 1100 orders
        let wrong_settings3 = pallet_tools::order_book::SideFillInput {
            highest_price: balance!(10),
            lowest_price: balance!(10) - 10 * price_step,
            price_step,
            orders_per_price: 100,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        let fill_settings = pallet_tools::order_book::FillInput {
            bids: Some(wrong_settings3),
            asks: None,
            random_seed: None,
        };

        let mut err: DispatchErrorWithPostInfo<PostDispatchInfo> =
            Error::<Runtime>::TooManyOrders.into();
        err.post_info.pays_fee = Pays::No;
        assert_err!(
            QaToolsPallet::order_book_create_and_fill_batch(
                RuntimeOrigin::root(),
                alice(),
                alice(),
                vec![(
                    order_book_id,
                    pallet_tools::order_book::OrderBookAttributes::default(),
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
        let amount_range = pallet_tools::order_book::RandomAmount::new(balance!(1), balance!(10));

        // 10 prices by 100 orders = 1000 orders
        let bids_settings = pallet_tools::order_book::SideFillInput {
            highest_price: balance!(10),
            lowest_price: balance!(10) - 9 * price_step,
            price_step,
            orders_per_price: 100,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        // 100 prices by 10 orders = 1000 orders
        let asks_settings = pallet_tools::order_book::SideFillInput {
            highest_price: balance!(11) + 99 * price_step,
            lowest_price: balance!(11),
            price_step,
            orders_per_price: 10,
            lifespan: Some(2_590_000_000),
            amount_range_inclusive: Some(amount_range),
        };
        let fill_settings = pallet_tools::order_book::FillInput {
            bids: Some(bids_settings),
            asks: Some(asks_settings),
            random_seed: None,
        };

        assert_ok!(QaToolsPallet::order_book_create_and_fill_batch(
            RuntimeOrigin::root(),
            alice(),
            bob(),
            vec![(
                order_book_id,
                pallet_tools::order_book::OrderBookAttributes::default(),
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
        let amount_range = pallet_tools::order_book::RandomAmount::new(balance!(1), balance!(10));

        let bids_settings = pallet_tools::order_book::SideFillInput {
            highest_price: balance!(10),
            lowest_price: balance!(10) - 9 * price_step,
            price_step,
            orders_per_price: 10,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        let asks_settings = pallet_tools::order_book::SideFillInput {
            highest_price: balance!(11) + 9 * price_step,
            lowest_price: balance!(11),
            price_step,
            orders_per_price: 10,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        let fill_settings = pallet_tools::order_book::FillInput {
            bids: Some(bids_settings),
            asks: Some(asks_settings),
            random_seed: None,
        };

        assert_ok!(QaToolsPallet::order_book_create_and_fill_batch(
            RuntimeOrigin::root(),
            alice(),
            bob(),
            vec![(
                order_book_id,
                pallet_tools::order_book::OrderBookAttributes::default(),
                fill_settings.clone()
            )]
        ));

        assert!(order_book::Pallet::<Runtime>::order_books(order_book_id).is_some());

        let mut err: DispatchErrorWithPostInfo<PostDispatchInfo> =
            Error::<Runtime>::OrderBookAlreadyExists.into();
        err.post_info.pays_fee = Pays::No;
        assert_err!(
            QaToolsPallet::order_book_create_and_fill_batch(
                RuntimeOrigin::root(),
                alice(),
                bob(),
                vec![(
                    order_book_id,
                    pallet_tools::order_book::OrderBookAttributes::default(),
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
        let amount_range = pallet_tools::order_book::RandomAmount::new(balance!(1), balance!(10));

        let bids_settings = pallet_tools::order_book::SideFillInput {
            highest_price: balance!(10),
            lowest_price: balance!(10) - 9 * price_step,
            price_step,
            orders_per_price: 10,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        let asks_settings = pallet_tools::order_book::SideFillInput {
            highest_price: balance!(11) + 9 * price_step,
            lowest_price: balance!(11),
            price_step,
            orders_per_price: 10,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        let fill_settings = pallet_tools::order_book::FillInput {
            bids: Some(bids_settings),
            asks: Some(asks_settings),
            random_seed: None,
        };

        let mut err: DispatchErrorWithPostInfo<PostDispatchInfo> =
            Error::<Runtime>::CannotFillUnknownOrderBook.into();
        err.post_info.pays_fee = Pays::No;
        assert_err!(
            QaToolsPallet::order_book_fill_batch(
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
        let amount_range = pallet_tools::order_book::RandomAmount::new(balance!(1), balance!(10));

        let bids_settings = pallet_tools::order_book::SideFillInput {
            highest_price: balance!(10),
            lowest_price: balance!(10) - 9 * price_step,
            price_step,
            orders_per_price: 10,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        let asks_settings = pallet_tools::order_book::SideFillInput {
            highest_price: balance!(11) + 9 * price_step,
            lowest_price: balance!(11),
            price_step,
            orders_per_price: 10,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        let fill_settings = pallet_tools::order_book::FillInput {
            bids: Some(bids_settings),
            asks: Some(asks_settings),
            random_seed: None,
        };

        assert_ok!(QaToolsPallet::order_book_create_and_fill_batch(
            RuntimeOrigin::root(),
            alice(),
            bob(),
            vec![(
                order_book_id,
                pallet_tools::order_book::OrderBookAttributes::default(),
                fill_settings.clone()
            )]
        ));

        assert!(order_book::Pallet::<Runtime>::order_books(order_book_id).is_some());

        assert_ok!(QaToolsPallet::order_book_fill_batch(
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
        let amount_range = pallet_tools::order_book::RandomAmount::new(balance!(1), balance!(10));

        // 10 prices by 100 orders = 1000 orders
        let bids_settings = pallet_tools::order_book::SideFillInput {
            highest_price: balance!(10),
            lowest_price: balance!(10) - 9 * price_step,
            price_step,
            orders_per_price: 100,
            lifespan: None,
            amount_range_inclusive: Some(amount_range),
        };
        // 100 prices by 10 orders = 1000 orders
        let asks_settings = pallet_tools::order_book::SideFillInput {
            highest_price: balance!(11) + 99 * price_step,
            lowest_price: balance!(11),
            price_step,
            orders_per_price: 10,
            lifespan: Some(2_590_000_000),
            amount_range_inclusive: Some(amount_range),
        };
        let fill_settings = pallet_tools::order_book::FillInput {
            bids: Some(bids_settings),
            asks: Some(asks_settings),
            random_seed: None,
        };

        assert_ok!(QaToolsPallet::order_book_create_and_fill_batch(
            RuntimeOrigin::root(),
            alice(),
            bob(),
            vec![(
                order_book_id,
                pallet_tools::order_book::OrderBookAttributes::default(),
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

        assert_ok!(QaToolsPallet::order_book_fill_batch(
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
