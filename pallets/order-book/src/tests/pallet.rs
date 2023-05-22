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

#![cfg(feature = "wip")] // order-book

use crate::tests::test_utils::*;
use assets::AssetIdOf;
use common::test_utils::assert_last_event;
use common::{balance, AssetInfoProvider, AssetName, AssetSymbol, Balance, PriceVariant, VAL, XOR};
use frame_support::traits::Get;
use frame_support::{assert_err, assert_ok};
use frame_system::RawOrigin;
use framenode_chain_spec::ext;
use framenode_runtime::order_book::{
    self, Config, CurrencyLocker, CurrencyUnlocker, ExpirationScheduler, LimitOrder, OrderBookId,
};
use framenode_runtime::{Runtime, RuntimeOrigin};

#[test]
fn should_register_technical_account() {
    ext().execute_with(|| {
        framenode_runtime::frame_system::Pallet::<Runtime>::inc_providers(&alice());
        let nft = assets::Pallet::<Runtime>::register_from(
            &alice(),
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            balance!(1),
            false,
            None,
            None,
        )
        .unwrap();

        let accounts = [
            (
                DEX,
                OrderBookId::<AssetIdOf<Runtime>> {
                    base: VAL.into(),
                    quote: XOR.into(),
                },
            ),
            (
                DEX,
                OrderBookId::<AssetIdOf<Runtime>> {
                    base: nft,
                    quote: XOR.into(),
                },
            ),
        ];

        // register (on order book creation)
        for (dex_id, order_book_id) in accounts {
            OrderBookPallet::register_tech_account(dex_id.into(), order_book_id).expect(&format!(
                "Could not register account for dex_id: {:?}, pair: {:?}",
                dex_id, order_book_id,
            ));
        }

        // deregister (on order book removal)
        for (dex_id, order_book_id) in accounts {
            OrderBookPallet::deregister_tech_account(dex_id.into(), order_book_id).expect(
                &format!(
                    "Could not deregister account for dex_id: {:?}, pair: {:?}",
                    dex_id, order_book_id,
                ),
            );
        }
    });
}

fn test_lock_unlock_same_account(
    dex_id: common::DEXId,
    order_book_id: OrderBookId<AssetIdOf<Runtime>>,
    asset_id: &AssetIdOf<Runtime>,
    amount_to_lock: Balance,
    account: &<Runtime as frame_system::Config>::AccountId,
) {
    let balance_before =
        assets::Pallet::<Runtime>::free_balance(asset_id, account).expect("Asset must exist");

    assert_ok!(OrderBookPallet::lock_liquidity(
        dex_id.into(),
        account,
        order_book_id,
        asset_id,
        amount_to_lock
    ));

    let balance_after_lock =
        assets::Pallet::<Runtime>::free_balance(asset_id, account).expect("Asset must exist");
    assert_eq!(balance_after_lock, balance_before - amount_to_lock);

    assert_ok!(OrderBookPallet::unlock_liquidity(
        dex_id.into(),
        account,
        order_book_id,
        asset_id,
        amount_to_lock
    ));

    let balance_after_unlock =
        assets::Pallet::<Runtime>::free_balance(asset_id, account).expect("Asset must exist");
    assert_eq!(balance_before, balance_after_unlock);
}

fn test_lock_unlock_other_account(
    dex_id: common::DEXId,
    order_book_id: OrderBookId<AssetIdOf<Runtime>>,
    asset_id: &AssetIdOf<Runtime>,
    amount_to_lock: Balance,
    lock_account: &<Runtime as frame_system::Config>::AccountId,
    unlock_account: &<Runtime as frame_system::Config>::AccountId,
) {
    let lock_account_balance_before =
        assets::Pallet::<Runtime>::free_balance(asset_id, lock_account).expect("Asset must exist");
    let unlock_account_balance_before =
        assets::Pallet::<Runtime>::free_balance(asset_id, unlock_account)
            .expect("Asset must exist");

    assert_ok!(OrderBookPallet::lock_liquidity(
        dex_id.into(),
        lock_account,
        order_book_id,
        asset_id,
        amount_to_lock
    ));

    let lock_account_balance_after_lock =
        assets::Pallet::<Runtime>::free_balance(asset_id, lock_account).expect("Asset must exist");
    assert_eq!(
        lock_account_balance_after_lock,
        lock_account_balance_before - amount_to_lock
    );

    assert_ok!(OrderBookPallet::unlock_liquidity(
        dex_id.into(),
        unlock_account,
        order_book_id,
        asset_id,
        amount_to_lock
    ));

    let unlock_account_balance_after_unlock =
        assets::Pallet::<Runtime>::free_balance(asset_id, unlock_account)
            .expect("Asset must exist");
    assert_eq!(
        unlock_account_balance_after_unlock,
        unlock_account_balance_before + amount_to_lock
    );
}

#[test]
fn should_lock_unlock_base_asset() {
    ext().execute_with(|| {
        let amount_to_lock = balance!(10);
        let amount_to_mint = amount_to_lock;
        assert_ok!(assets::Pallet::<Runtime>::update_balance(
            RuntimeOrigin::root(),
            alice(),
            XOR,
            amount_to_mint.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };
        OrderBookPallet::register_tech_account(DEX.into(), order_book_id).unwrap();

        // Alice -> Alice (expected on order cancellation)
        test_lock_unlock_same_account(DEX, order_book_id, &XOR, amount_to_lock, &alice());

        // Alice -> Bob (expected exchange mechanism)
        test_lock_unlock_other_account(DEX, order_book_id, &XOR, amount_to_lock, &alice(), &bob());
    });
}

#[test]
fn should_lock_unlock_other_asset() {
    ext().execute_with(|| {
        let amount_to_lock = balance!(10);
        let amount_to_mint = amount_to_lock;
        assert_ok!(assets::Pallet::<Runtime>::update_balance(
            RuntimeOrigin::root(),
            alice(),
            VAL,
            amount_to_mint.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };
        OrderBookPallet::register_tech_account(DEX.into(), order_book_id).unwrap();

        // Alice -> Alice (expected on order cancellation)
        test_lock_unlock_same_account(DEX, order_book_id, &VAL, amount_to_lock, &alice());

        // Alice -> Bob (expected exchange mechanism)
        test_lock_unlock_other_account(DEX, order_book_id, &VAL, amount_to_lock, &alice(), &bob());
    });
}

#[test]
fn should_lock_unlock_indivisible_nft() {
    ext().execute_with(|| {
        framenode_runtime::frame_system::Pallet::<Runtime>::inc_providers(&alice());

        let nft = assets::Pallet::<Runtime>::register_from(
            &alice(),
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            balance!(1),
            false,
            None,
            None,
        )
        .unwrap();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: nft.clone(),
            quote: XOR.into(),
        };
        OrderBookPallet::register_tech_account(DEX.into(), order_book_id).unwrap();

        // Alice -> Alice (expected on order cancellation)
        test_lock_unlock_same_account(DEX, order_book_id, &nft, balance!(1), &alice());

        // Alice -> Bob (expected exchange mechanism)
        test_lock_unlock_other_account(DEX, order_book_id, &nft, balance!(1), &alice(), &bob());
    });
}

#[test]
fn should_not_lock_insufficient_base_asset() {
    ext().execute_with(|| {
        let amount_to_lock = balance!(10);
        let amount_to_mint = balance!(9.9);
        assert_ok!(assets::Pallet::<Runtime>::update_balance(
            RuntimeOrigin::root(),
            alice(),
            XOR,
            amount_to_mint.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };
        OrderBookPallet::register_tech_account(DEX.into(), order_book_id).unwrap();

        assert_err!(
            OrderBookPallet::lock_liquidity(
                DEX.into(),
                &alice(),
                order_book_id,
                &XOR,
                amount_to_lock
            ),
            pallet_balances::Error::<Runtime>::InsufficientBalance
        );
    });
}

#[test]
fn should_not_lock_insufficient_other_asset() {
    ext().execute_with(|| {
        let amount_to_lock = balance!(10);
        let amount_to_mint = balance!(9.9);
        assert_ok!(assets::Pallet::<Runtime>::update_balance(
            RuntimeOrigin::root(),
            alice(),
            VAL,
            amount_to_mint.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };
        OrderBookPallet::register_tech_account(DEX.into(), order_book_id).unwrap();

        assert_err!(
            OrderBookPallet::lock_liquidity(
                DEX.into(),
                &alice(),
                order_book_id,
                &VAL,
                amount_to_lock
            ),
            tokens::Error::<Runtime>::BalanceTooLow
        );
    });
}

#[test]
fn should_not_lock_insufficient_nft() {
    ext().execute_with(|| {
        let caller = alice();
        let creator = bob();
        framenode_runtime::frame_system::Pallet::<Runtime>::inc_providers(&creator);

        let nft = assets::Pallet::<Runtime>::register_from(
            &creator,
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            balance!(1),
            false,
            None,
            None,
        )
        .unwrap();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: nft.clone(),
            quote: XOR.into(),
        };
        OrderBookPallet::register_tech_account(DEX.into(), order_book_id).unwrap();

        assert_err!(
            OrderBookPallet::lock_liquidity(DEX.into(), &caller, order_book_id, &nft, balance!(1)),
            tokens::Error::<Runtime>::BalanceTooLow
        );
    });
}

#[test]
fn should_not_unlock_more_base_that_tech_account_has() {
    ext().execute_with(|| {
        let amount_to_lock = balance!(10);
        let amount_to_mint = amount_to_lock;
        let amount_to_try_unlock = balance!(10.1);
        assert_ok!(assets::Pallet::<Runtime>::update_balance(
            RuntimeOrigin::root(),
            alice(),
            XOR,
            amount_to_mint.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };
        OrderBookPallet::register_tech_account(DEX.into(), order_book_id).unwrap();

        assert_ok!(OrderBookPallet::lock_liquidity(
            DEX.into(),
            &alice(),
            order_book_id,
            &XOR,
            amount_to_lock
        ));

        assert_err!(
            OrderBookPallet::unlock_liquidity(
                DEX.into(),
                &alice(),
                order_book_id,
                &XOR,
                amount_to_try_unlock
            ),
            pallet_balances::Error::<Runtime>::InsufficientBalance
        );
    });
}

#[test]
fn should_not_unlock_more_other_that_tech_account_has() {
    ext().execute_with(|| {
        let amount_to_lock = balance!(10);
        let amount_to_mint = amount_to_lock;
        let amount_to_try_unlock = balance!(10.1);
        assert_ok!(assets::Pallet::<Runtime>::update_balance(
            RuntimeOrigin::root(),
            alice(),
            VAL,
            amount_to_mint.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };
        OrderBookPallet::register_tech_account(DEX.into(), order_book_id).unwrap();

        assert_ok!(OrderBookPallet::lock_liquidity(
            DEX.into(),
            &alice(),
            order_book_id,
            &VAL,
            amount_to_lock
        ));

        assert_err!(
            OrderBookPallet::unlock_liquidity(
                DEX.into(),
                &alice(),
                order_book_id,
                &VAL,
                amount_to_try_unlock
            ),
            tokens::Error::<Runtime>::BalanceTooLow
        );
    });
}

#[test]
fn should_not_unlock_more_nft_that_tech_account_has() {
    ext().execute_with(|| {
        framenode_runtime::frame_system::Pallet::<Runtime>::inc_providers(&alice());

        let nft = assets::Pallet::<Runtime>::register_from(
            &alice(),
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            balance!(1),
            false,
            None,
            None,
        )
        .unwrap();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: nft.clone(),
            quote: XOR.into(),
        };
        OrderBookPallet::register_tech_account(DEX.into(), order_book_id).unwrap();

        assert_err!(
            OrderBookPallet::unlock_liquidity(
                DEX.into(),
                &alice(),
                order_book_id,
                &nft,
                balance!(1)
            ),
            tokens::Error::<Runtime>::BalanceTooLow
        );
    });
}

#[test]
fn should_expire_order() {
    ext().execute_with(|| {
        let caller = alice();
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        assert_ok!(OrderBookPallet::create_orderbook(
            RawOrigin::Signed(bob()).into(),
            DEX.into(),
            order_book_id
        ));
        fill_balance(caller.clone(), order_book_id);

        let price = balance!(10);
        let amount = balance!(100);
        let lifespan = 10000;
        let now = 1234;
        let now_block = frame_system::Pallet::<Runtime>::block_number();
        // the lifespan of 10000 ms corresponds to at least
        // ceil(10000 / 6000) = 2 blocks of the order lifespan;
        // at this block the order should still be available
        let end_of_lifespan_block = now_block + 2;

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(now);

        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(caller.clone()).into(),
            order_book_id,
            price,
            amount,
            PriceVariant::Buy,
            Some(lifespan)
        ));

        // verify state

        let order_id = get_last_order_id(order_book_id).unwrap();

        // check
        let expected_order = LimitOrder::<Runtime>::new(
            order_id,
            caller.clone(),
            PriceVariant::Buy,
            price,
            amount,
            now,
            lifespan,
            now_block,
        );

        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_id).unwrap(),
            expected_order
        );
        // Run to the last block the order should still be available at
        run_to_block(end_of_lifespan_block);

        // The order is still there
        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_id).unwrap(),
            expected_order
        );

        // Check a bit after the expected expiration because it's ok to remove
        // it 1-2 blocks later
        run_to_block(end_of_lifespan_block + 2);

        // The order is removed
        assert!(OrderBookPallet::limit_orders(order_book_id, order_id).is_none());
    })
}

#[test]
fn should_cleanup_on_expiring() {
    ext().execute_with(|| {
        let caller = alice();
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        assert_ok!(OrderBookPallet::create_orderbook(
            RawOrigin::Signed(bob()).into(),
            DEX.into(),
            order_book_id
        ));
        fill_balance(caller.clone(), order_book_id);

        let price = balance!(10);
        let amount = balance!(100);
        let lifespan = 10000;
        let now = 1234;
        let now_block = frame_system::Pallet::<Runtime>::block_number();
        // the lifespan of 10000 ms corresponds to at least
        // ceil(10000 / 6000) = 2 blocks of the order lifespan;
        // at this block the order should still be available
        let end_of_lifespan_block = now_block + 2;

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(now);

        // fix state before
        let bids_before = OrderBookPallet::bids(&order_book_id, &price).unwrap_or_default();
        let agg_bids_before = OrderBookPallet::aggregated_bids(&order_book_id);
        let price_volume_before = agg_bids_before.get(&price).cloned().unwrap_or_default();
        let user_orders_before =
            OrderBookPallet::user_limit_orders(&caller, &order_book_id).unwrap_or_default();
        let balance_before =
            <Runtime as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &caller)
                .unwrap();

        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(caller.clone()).into(),
            order_book_id,
            price,
            amount,
            PriceVariant::Buy,
            Some(lifespan)
        ));

        // verify state

        let order_id = get_last_order_id(order_book_id).unwrap();

        // check
        let expected_order = LimitOrder::<Runtime>::new(
            order_id,
            caller.clone(),
            PriceVariant::Buy,
            price,
            amount,
            now,
            lifespan,
            now_block,
        );

        let appropriate_amount = expected_order.appropriate_amount().unwrap();

        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_id).unwrap(),
            expected_order
        );

        let mut bids_with_order = bids_before.clone();
        assert_ok!(bids_with_order.try_push(order_id));
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &price).unwrap(),
            bids_with_order
        );

        let price_volume_with_order = price_volume_before + amount;
        let mut agg_bids_with_order = agg_bids_before.clone();
        assert_ok!(agg_bids_with_order.try_insert(price, price_volume_with_order));
        assert_eq!(
            OrderBookPallet::aggregated_bids(&order_book_id),
            agg_bids_with_order
        );

        let mut user_orders_with_order = user_orders_before.clone();
        assert_ok!(user_orders_with_order.try_push(order_id));
        assert_eq!(
            OrderBookPallet::user_limit_orders(&caller, &order_book_id).unwrap(),
            user_orders_with_order
        );

        let balance =
            <Runtime as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &caller)
                .unwrap();
        let balance_with_order = balance_before - appropriate_amount;
        assert_eq!(balance, balance_with_order);

        // Run to the last block the order should still be available at
        run_to_block(end_of_lifespan_block);

        let order_id = get_last_order_id(order_book_id).unwrap();

        // The order is still there
        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_id).unwrap(),
            expected_order
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &price).unwrap(),
            bids_with_order
        );
        assert_eq!(
            OrderBookPallet::aggregated_bids(&order_book_id),
            agg_bids_with_order
        );
        assert_eq!(
            OrderBookPallet::user_limit_orders(&caller, &order_book_id).unwrap(),
            user_orders_with_order
        );
        assert_eq!(
            <Runtime as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &caller)
                .unwrap(),
            balance_with_order
        );

        // Check a bit after the expected expiration because it's ok to remove
        // it 1-2 blocks later
        run_to_block(end_of_lifespan_block + 2);

        // The order is removed, state returned to original
        assert!(OrderBookPallet::limit_orders(order_book_id, order_id).is_none());
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &price).unwrap_or_default(),
            bids_before
        );
        assert_eq!(
            OrderBookPallet::aggregated_bids(&order_book_id),
            agg_bids_before
        );
        assert_eq!(
            OrderBookPallet::user_limit_orders(&caller, &order_book_id).unwrap_or_default(),
            user_orders_before
        );
        assert_eq!(
            <Runtime as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &caller)
                .unwrap(),
            balance_before
        );
    })
}

#[test]
#[ignore] // it works, but takes a lot of time
fn should_enforce_expiration_and_weight_limits() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };
        assert_ok!(OrderBookPallet::create_orderbook(
            RawOrigin::Signed(bob()).into(),
            DEX.into(),
            order_book_id
        ));

        let price = balance!(10);
        let amount = balance!(100);
        let lifespan = 10000;
        let now = 1234;
        let now_block = frame_system::Pallet::<Runtime>::block_number();
        // the lifespan of 10000 ms corresponds to at least
        // ceil(10000 / 6000) = 2 blocks of the order lifespan;
        // at this block the order should still be available
        let end_of_lifespan_block = now_block + 2;

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(now);

        let max_orders_expire_at_block = <Runtime as Config>::MaxExpiringOrdersPerBlock::get();
        let mut placed_orders = vec![];

        for i in 0..max_orders_expire_at_block {
            // in order to avoid cap on orders from single account
            let caller = generate_account(i);
            fill_balance(caller.clone(), order_book_id);
            assert_ok!(OrderBookPallet::place_limit_order(
                RawOrigin::Signed(caller.clone()).into(),
                order_book_id,
                price,
                amount,
                PriceVariant::Buy,
                Some(lifespan)
            ));
            placed_orders.push(get_last_order_id(order_book_id).unwrap());
        }
        let caller = generate_account(max_orders_expire_at_block);
        fill_balance(caller.clone(), order_book_id);
        assert_err!(
            OrderBookPallet::place_limit_order(
                RawOrigin::Signed(caller.clone()).into(),
                order_book_id,
                price,
                amount,
                PriceVariant::Buy,
                Some(lifespan)
            ),
            order_book::Error::<Runtime>::BlockScheduleFull
        );

        // All orders are indeed placed
        for order_id in &placed_orders {
            assert!(OrderBookPallet::limit_orders(order_book_id, order_id).is_some());
        }

        // Check a bit after the expected expiration because it's ok to remove
        // it a few blocks later (e.g. in case weight limit is reached, for example)
        for i in 0..=10 {
            // Weight spent must not exceed the limit
            let init_weight_consumed = run_to_block(end_of_lifespan_block + i);
            // Weight does not have partial ordering, so we check for overflow this way:
            assert!(<Runtime as Config>::MaxExpirationWeightPerBlock::get()
                .checked_sub(&init_weight_consumed)
                .is_some());
        }

        // All orders are removed
        // reverse because they're expired in the order of placement
        for (i, order_id) in placed_orders.iter().rev().enumerate() {
            assert!(
                OrderBookPallet::limit_orders(order_book_id, order_id).is_none(),
                "Limit order {}/{} is not expired (removed). Maybe the test should pass even more blocks \
                to have enough weight for all expirations or there is some bug.", i, placed_orders.len()
            );
        }
    })
}

#[test]
fn should_emit_event_on_expiration_failure() {
    ext().execute_with(|| {
        // To be able to assert events
        frame_system::Pallet::<Runtime>::set_block_number(1);

        let non_existent_order_book_id = OrderBookId {
            base: XOR,
            quote: VAL,
        };
        let non_existent_order_id = 1;
        let expiration_block = 2u32.into();
        assert_ok!(OrderBookPallet::schedule(
            expiration_block,
            non_existent_order_book_id,
            non_existent_order_id
        ));
        run_to_block(expiration_block);
        assert_last_event::<Runtime>(
            order_book::Event::ExpirationFailure {
                order_book_id: non_existent_order_book_id,
                order_id: non_existent_order_id,
                error: order_book::Error::<Runtime>::UnknownOrderBook.into(),
            }
            .into(),
        );
    })
}
