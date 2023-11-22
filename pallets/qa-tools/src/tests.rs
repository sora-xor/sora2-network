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

//! Tests are not essential for this testing helper pallet,
//! but they make modify-run iterations during development much quicker

use assets::AssetIdOf;
use common::{
    balance, AccountIdOf, AssetId32, AssetInfoProvider, AssetName, AssetSymbol, Balance, DEXId,
    DexIdOf, PredefinedAssetId, PriceVariant, PSWAP, VAL, XOR,
};
use frame_support::pallet_prelude::DispatchResult;
use frame_support::{assert_err, assert_ok};
use frame_system::pallet_prelude::BlockNumberFor;
use framenode_chain_spec::ext;
use framenode_runtime::qa_tools::{self, settings, WhitelistedCallers};
use framenode_runtime::{Runtime, RuntimeOrigin};
use order_book::{DataLayer, LimitOrder, MomentOf, OrderBookId, OrderPrice, OrderVolume};
use sp_runtime::traits::BadOrigin;

type FrameSystem = framenode_runtime::frame_system::Pallet<Runtime>;
pub type QAToolsPallet = qa_tools::Pallet<Runtime>;

pub fn alice() -> <Runtime as frame_system::Config>::AccountId {
    <Runtime as frame_system::Config>::AccountId::new([1u8; 32])
}

pub fn bob() -> <Runtime as frame_system::Config>::AccountId {
    <Runtime as frame_system::Config>::AccountId::new([2u8; 32])
}

fn test_creates_orderbook(
    base: AssetId32<PredefinedAssetId>,
    quote: AssetId32<PredefinedAssetId>,
    best_bid_price: Balance,
    best_ask_price: Balance,
    steps: usize,
    amount_range: (Balance, Balance),
) -> OrderBookId<AssetIdOf<Runtime>, DexIdOf<Runtime>> {
    let mut start_balance_base = assets::Pallet::<Runtime>::total_balance(&base, &alice()).unwrap();
    let start_balance_quote = assets::Pallet::<Runtime>::total_balance(&quote, &alice()).unwrap();
    let order_book_id = OrderBookId {
        dex_id: DEXId::Polkaswap.into(),
        base,
        quote,
    };
    let _ = QAToolsPallet::add_to_whitelist(RuntimeOrigin::root(), alice());
    let price_step = if assets::Pallet::<Runtime>::is_non_divisible(&base) {
        order_book::OrderBook::<Runtime>::default_indivisible(order_book_id).tick_size
    } else {
        order_book::OrderBook::<Runtime>::default(order_book_id).tick_size
    };
    let orders_per_price = 3;
    assert_ok!(QAToolsPallet::order_book_create_and_fill_batch(
        RuntimeOrigin::signed(alice()),
        alice(),
        alice(),
        vec![(
            order_book_id,
            settings::OrderBookFill {
                bids: Some(settings::SideFill {
                    best_price: best_bid_price,
                    worst_price: best_bid_price - (steps - 1) as u128 * *price_step.balance(),
                    price_step: *price_step.balance(),
                    orders_per_price,
                    amount_range_inclusive: Some(amount_range)
                }),
                asks: Some(settings::SideFill {
                    best_price: best_ask_price,
                    worst_price: best_ask_price + (steps - 1) as u128 * *price_step.balance(),
                    price_step: *price_step.balance(),
                    orders_per_price,
                    amount_range_inclusive: Some(amount_range)
                }),
                lifespan: None,
                random_seed: None,
            }
        )]
    ));
    assert_eq!(
        assets::Pallet::<Runtime>::total_balance(&quote, &alice()).unwrap(),
        start_balance_quote
    );
    // 1 nft is minted in case none were owned
    if start_balance_base == 0 && assets::Pallet::<Runtime>::is_non_divisible(&base) {
        start_balance_base += 1;
    }
    assert_eq!(
        assets::Pallet::<Runtime>::total_balance(&base, &alice()).unwrap(),
        start_balance_base
    );

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
    let amount_range = amount_range.0..=amount_range.1;
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
            1,
            false,
            None,
            None,
        )
        .unwrap();
        test_creates_orderbook(nft, XOR, balance!(10), balance!(11), 4, (1, 1));
    });
}

#[test]
fn should_create_and_fill_orderbook_random_amount() {
    ext().execute_with(|| {
        test_creates_orderbook(
            VAL,
            XOR,
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
            1,
            false,
            None,
            None,
        )
        .unwrap();
        test_creates_orderbook(nft, XOR, balance!(10), balance!(11), 4, (1, 10));
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
        let _ = QAToolsPallet::add_to_whitelist(RuntimeOrigin::root(), alice());
        let price_step = order_book::OrderBook::<Runtime>::default(order_book_id_1).tick_size;
        let orders_per_price = 3;
        let best_bid_price = balance!(10);
        let best_ask_price = balance!(11);
        let steps = 4;
        let amount_range = (balance!(1), balance!(10));
        let settings = settings::OrderBookFill {
            bids: Some(settings::SideFill {
                best_price: best_bid_price,
                worst_price: best_bid_price - (steps - 1) as u128 * *price_step.balance(),
                price_step: *price_step.balance(),
                orders_per_price,
                amount_range_inclusive: Some(amount_range),
            }),
            asks: Some(settings::SideFill {
                best_price: best_ask_price,
                worst_price: best_ask_price + (steps - 1) as u128 * *price_step.balance(),
                price_step: *price_step.balance(),
                orders_per_price,
                amount_range_inclusive: Some(amount_range),
            }),
            lifespan: None,
            random_seed: None,
        };
        assert_ok!(QAToolsPallet::order_book_create_and_fill_batch(
            RuntimeOrigin::signed(alice()),
            alice(),
            alice(),
            vec![
                (order_book_id_1, settings.clone()),
                (order_book_id_2, settings)
            ]
        ));

        let mut data = order_book::storage_data_layer::StorageDataLayer::<Runtime>::new();
        let mut limit_orders_1 = data.get_all_limit_orders(&order_book_id_1);
        let mut limit_orders_2 = data.get_all_limit_orders(&order_book_id_2);
        fn cmp_by_id(a: &LimitOrder<Runtime>, b: &LimitOrder<Runtime>) -> sp_std::cmp::Ordering {
            let a = u128::try_from(a.id).unwrap();
            let b = u128::try_from(b.id).unwrap();
            a.cmp(&b)
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
        let _ = QAToolsPallet::add_to_whitelist(RuntimeOrigin::root(), alice());
        let price_step = order_book::OrderBook::<Runtime>::default(order_book_id_1).tick_size;
        let orders_per_price = 3;
        let best_bid_price = balance!(10);
        let best_ask_price = balance!(11);
        let steps = 4;
        let amount_range = (balance!(1), balance!(10));
        let settings_1 = settings::OrderBookFill {
            bids: Some(settings::SideFill {
                best_price: best_bid_price,
                worst_price: best_bid_price - (steps - 1) as u128 * *price_step.balance(),
                price_step: *price_step.balance(),
                orders_per_price,
                amount_range_inclusive: Some(amount_range),
            }),
            asks: Some(settings::SideFill {
                best_price: best_ask_price,
                worst_price: best_ask_price + (steps - 1) as u128 * *price_step.balance(),
                price_step: *price_step.balance(),
                orders_per_price,
                amount_range_inclusive: Some(amount_range),
            }),
            lifespan: None,
            random_seed: None,
        };
        let settings_2 = settings::OrderBookFill {
            bids: None,
            ..settings_1.clone()
        };
        assert_ok!(QAToolsPallet::order_book_create_and_fill_batch(
            RuntimeOrigin::signed(alice()),
            alice(),
            alice(),
            vec![(order_book_id_1, settings_1), (order_book_id_2, settings_2)]
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
            let a = u128::try_from(a.id).unwrap();
            let b = u128::try_from(b.id).unwrap();
            a.cmp(&b)
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

fn test_whitelist<F: Fn(AccountIdOf<Runtime>) -> DispatchResult>(call: F) {
    let whitelist_before = WhitelistedCallers::<Runtime>::get().clone();
    let _ = QAToolsPallet::remove_from_whitelist(RuntimeOrigin::root(), alice());
    assert_err!(call(alice()), BadOrigin);
    QAToolsPallet::add_to_whitelist(RuntimeOrigin::root(), alice())
        .expect("just removed from whitelist");
    assert_ne!(call(alice()), Err(BadOrigin.into()));
    WhitelistedCallers::<Runtime>::set(whitelist_before);
}

#[test]
fn create_empty_batch_whitelist_only() {
    ext().execute_with(|| {
        test_whitelist(|caller| {
            QAToolsPallet::order_book_create_empty_batch(RuntimeOrigin::signed(caller), vec![])
                .map_err(|e| e.error)?;
            Ok(())
        });
    })
}

#[test]
fn create_and_fill_batch_whitelist_only() {
    ext().execute_with(|| {
        test_whitelist(|caller| {
            QAToolsPallet::order_book_create_and_fill_batch(
                RuntimeOrigin::signed(caller),
                alice(),
                alice(),
                vec![],
            )
            .map_err(|e| e.error)?;
            Ok(())
        });
    })
}

#[test]
fn whitelist_modification_is_root_only() {
    ext().execute_with(|| {
        assert_err!(
            QAToolsPallet::add_to_whitelist(RuntimeOrigin::none(), alice()),
            BadOrigin
        );
        assert_err!(
            QAToolsPallet::add_to_whitelist(RuntimeOrigin::signed(alice()), alice()),
            BadOrigin
        );
        assert_err!(
            QAToolsPallet::add_to_whitelist(RuntimeOrigin::signed(bob()), alice()),
            BadOrigin
        );
        assert_ok!(QAToolsPallet::add_to_whitelist(
            RuntimeOrigin::root(),
            alice()
        ));
    })
}
