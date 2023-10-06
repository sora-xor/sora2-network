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

use core::cmp::min;

use crate::test_utils::*;
use assets::AssetIdOf;
use common::prelude::{QuoteAmount, SwapAmount, SwapOutcome};
use common::test_utils::assert_last_event;
use common::{
    balance, AssetName, AssetSymbol, Balance, LiquiditySource, PriceVariant, VAL, XOR, XSTUSD,
};
use frame_support::traits::Get;
use frame_support::{assert_err, assert_ok};
use frame_system::RawOrigin;
use framenode_chain_spec::ext;
use framenode_runtime::order_book::{
    self, Config, CurrencyLocker, CurrencyUnlocker, ExpirationScheduler, LimitOrder, MarketRole,
    OrderBook, OrderBookId, OrderBookStatus, OrderPrice, OrderVolume, WeightInfo,
};
use framenode_runtime::{Runtime, RuntimeOrigin};
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::collections::btree_map::BTreeMap;

fn should_register_technical_account() {
    ext().execute_with(|| {
        framenode_runtime::frame_system::Pallet::<Runtime>::inc_providers(&alice::<Runtime>());
        let nft = assets::Pallet::<Runtime>::register_from(
            &alice::<Runtime>(),
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            1,
            false,
            None,
            None,
        )
        .unwrap();

        let accounts = [
            OrderBookId::<AssetIdOf<Runtime>, DEXId> {
                dex_id: DEX.into(),
                base: VAL.into(),
                quote: XOR.into(),
            },
            OrderBookId::<AssetIdOf<Runtime>, DEXId> {
                dex_id: DEX.into(),
                base: nft,
                quote: XOR.into(),
            },
        ];

        // register (on order book creation)
        for order_book_id in accounts {
            OrderBookPallet::register_tech_account(order_book_id).expect(&format!(
                "Could not register account for order_book_id: {:?}",
                order_book_id,
            ));
        }

        // deregister (on order book removal)
        for order_book_id in accounts {
            OrderBookPallet::deregister_tech_account(order_book_id).expect(&format!(
                "Could not deregister account for order_book_id: {:?}",
                order_book_id,
            ));
        }
    });
}

fn test_lock_unlock_same_account(
    order_book_id: OrderBookId<AssetIdOf<Runtime>, DEXId>,
    asset_id: &AssetIdOf<Runtime>,
    amount_to_lock: Balance,
    account: &<Runtime as frame_system::Config>::AccountId,
) {
    let balance_before = free_balance::<Runtime>(asset_id, account);

    assert_ok!(OrderBookPallet::lock_liquidity(
        account,
        order_book_id,
        asset_id,
        amount_to_lock.into()
    ));

    let balance_after_lock = free_balance::<Runtime>(asset_id, account);
    assert_eq!(balance_after_lock, balance_before - amount_to_lock);

    assert_ok!(OrderBookPallet::unlock_liquidity(
        account,
        order_book_id,
        asset_id,
        amount_to_lock.into()
    ));

    let balance_after_unlock = free_balance::<Runtime>(asset_id, account);
    assert_eq!(balance_before, balance_after_unlock);
}

fn test_lock_unlock_other_account(
    order_book_id: OrderBookId<AssetIdOf<Runtime>, DEXId>,
    asset_id: &AssetIdOf<Runtime>,
    amount_to_lock: Balance,
    lock_account: &<Runtime as frame_system::Config>::AccountId,
    unlock_account: &<Runtime as frame_system::Config>::AccountId,
) {
    let lock_account_balance_before = free_balance::<Runtime>(asset_id, lock_account);
    let unlock_account_balance_before = free_balance::<Runtime>(asset_id, unlock_account);

    assert_ok!(OrderBookPallet::lock_liquidity(
        lock_account,
        order_book_id,
        asset_id,
        amount_to_lock.into()
    ));

    let lock_account_balance_after_lock = free_balance::<Runtime>(asset_id, lock_account);
    assert_eq!(
        lock_account_balance_after_lock,
        lock_account_balance_before - amount_to_lock
    );

    assert_ok!(OrderBookPallet::unlock_liquidity(
        unlock_account,
        order_book_id,
        asset_id,
        amount_to_lock.into()
    ));

    let unlock_account_balance_after_unlock = free_balance::<Runtime>(asset_id, unlock_account);
    assert_eq!(
        unlock_account_balance_after_unlock,
        unlock_account_balance_before + amount_to_lock
    );
}

fn test_lock_unlock_other_accounts(
    order_book_id: OrderBookId<AssetIdOf<Runtime>, DEXId>,
    asset_id: &AssetIdOf<Runtime>,
    amount_to_lock: Balance,
    lock_account: &<Runtime as frame_system::Config>::AccountId,
    unlock_account1: &<Runtime as frame_system::Config>::AccountId,
    unlock_account2: &<Runtime as frame_system::Config>::AccountId,
) {
    let lock_account_balance_before = free_balance::<Runtime>(asset_id, lock_account);
    let unlock_account_balance_before1 = free_balance::<Runtime>(asset_id, unlock_account1);
    let unlock_account_balance_before2 = free_balance::<Runtime>(asset_id, unlock_account2);

    assert_ok!(OrderBookPallet::lock_liquidity(
        lock_account,
        order_book_id,
        asset_id,
        amount_to_lock.into()
    ));

    let lock_account_balance_after_lock = free_balance::<Runtime>(asset_id, lock_account);
    assert_eq!(
        lock_account_balance_after_lock,
        lock_account_balance_before - amount_to_lock
    );

    let unlock_amount1: Balance = (amount_to_lock / 4) * 3;
    let unlock_amount2: Balance = amount_to_lock / 4;

    let unlocks = BTreeMap::from([
        (unlock_account1.clone(), unlock_amount1.into()),
        (unlock_account2.clone(), unlock_amount2.into()),
    ]);

    assert_ok!(OrderBookPallet::unlock_liquidity_batch(
        order_book_id,
        asset_id,
        &unlocks
    ));

    let unlock_account_balance_after_unlock1 = free_balance::<Runtime>(asset_id, unlock_account1);
    assert_eq!(
        unlock_account_balance_after_unlock1,
        unlock_account_balance_before1 + unlock_amount1
    );

    let unlock_account_balance_after_unlock2 = free_balance::<Runtime>(asset_id, unlock_account2);
    assert_eq!(
        unlock_account_balance_after_unlock2,
        unlock_account_balance_before2 + unlock_amount2
    );
}

#[test]
fn should_lock_unlock_base_asset() {
    ext().execute_with(|| {
        let amount_to_lock = balance!(10);
        let amount_to_mint = amount_to_lock * 2;
        assert_ok!(assets::Pallet::<Runtime>::update_balance(
            RuntimeOrigin::root(),
            alice::<Runtime>(),
            XOR,
            amount_to_mint.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        // Alice -> Alice (expected on order cancellation)
        test_lock_unlock_same_account(order_book_id, &XOR, amount_to_lock, &alice::<Runtime>());

        // Alice -> Bob (expected exchange mechanism)
        test_lock_unlock_other_account(
            order_book_id,
            &XOR,
            amount_to_lock,
            &alice::<Runtime>(),
            &bob::<Runtime>(),
        );

        // Alice -> Bob & Charlie
        test_lock_unlock_other_accounts(
            order_book_id,
            &XOR,
            amount_to_lock,
            &alice::<Runtime>(),
            &bob::<Runtime>(),
            &charlie::<Runtime>(),
        );
    });
}

#[test]
fn should_lock_unlock_other_asset() {
    ext().execute_with(|| {
        let amount_to_lock = balance!(10);
        let amount_to_mint = amount_to_lock * 2;
        assert_ok!(assets::Pallet::<Runtime>::update_balance(
            RuntimeOrigin::root(),
            alice::<Runtime>(),
            VAL,
            amount_to_mint.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        // Alice -> Alice (expected on order cancellation)
        test_lock_unlock_same_account(order_book_id, &VAL, amount_to_lock, &alice::<Runtime>());

        // Alice -> Bob (expected exchange mechanism)
        test_lock_unlock_other_account(
            order_book_id,
            &VAL,
            amount_to_lock,
            &alice::<Runtime>(),
            &bob::<Runtime>(),
        );
    });
}

#[test]
fn should_lock_unlock_indivisible_nft() {
    ext().execute_with(|| {
        framenode_runtime::frame_system::Pallet::<Runtime>::inc_providers(&alice::<Runtime>());

        let nft = assets::Pallet::<Runtime>::register_from(
            &alice::<Runtime>(),
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            1,
            false,
            None,
            None,
        )
        .unwrap();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: nft.clone(),
            quote: XOR.into(),
        };
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        // Alice -> Alice (expected on order cancellation)
        test_lock_unlock_same_account(order_book_id, &nft, 1, &alice::<Runtime>());

        // Alice -> Bob (expected exchange mechanism)
        test_lock_unlock_other_account(
            order_book_id,
            &nft,
            1,
            &alice::<Runtime>(),
            &bob::<Runtime>(),
        );
    });
}

#[test]
fn should_lock_unlock_multiple_indivisible_nfts() {
    ext().execute_with(|| {
        framenode_runtime::frame_system::Pallet::<Runtime>::inc_providers(&alice::<Runtime>());

        let nft = assets::Pallet::<Runtime>::register_from(
            &alice::<Runtime>(),
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            4,
            false,
            None,
            None,
        )
        .unwrap();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: nft.clone(),
            quote: XOR.into(),
        };
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        // Alice -> Bob & Charlie
        test_lock_unlock_other_accounts(
            order_book_id,
            &nft,
            4,
            &alice::<Runtime>(),
            &bob::<Runtime>(),
            &charlie::<Runtime>(),
        );
    });
}

#[test]
fn should_not_lock_insufficient_base_asset() {
    ext().execute_with(|| {
        let amount_to_lock = balance!(10);
        let amount_to_mint = balance!(9.9);
        assert_ok!(assets::Pallet::<Runtime>::update_balance(
            RuntimeOrigin::root(),
            alice::<Runtime>(),
            XOR,
            amount_to_mint.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        assert_err!(
            OrderBookPallet::lock_liquidity(
                &alice::<Runtime>(),
                order_book_id,
                &XOR,
                amount_to_lock.into()
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
            alice::<Runtime>(),
            VAL,
            amount_to_mint.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        assert_err!(
            OrderBookPallet::lock_liquidity(
                &alice::<Runtime>(),
                order_book_id,
                &VAL,
                amount_to_lock.into()
            ),
            tokens::Error::<Runtime>::BalanceTooLow
        );
    });
}

#[test]
fn should_not_lock_insufficient_nft() {
    ext().execute_with(|| {
        let caller = alice::<Runtime>();
        let creator = bob::<Runtime>();
        framenode_runtime::frame_system::Pallet::<Runtime>::inc_providers(&creator);

        let nft = assets::Pallet::<Runtime>::register_from(
            &creator,
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            1,
            false,
            None,
            None,
        )
        .unwrap();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: nft.clone(),
            quote: XOR.into(),
        };
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        assert_err!(
            OrderBookPallet::lock_liquidity(
                &caller,
                order_book_id,
                &nft,
                OrderVolume::indivisible(1)
            ),
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
            alice::<Runtime>(),
            XOR,
            amount_to_mint.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        assert_ok!(OrderBookPallet::lock_liquidity(
            &alice::<Runtime>(),
            order_book_id,
            &XOR,
            amount_to_lock.into()
        ));

        assert_err!(
            OrderBookPallet::unlock_liquidity(
                &alice::<Runtime>(),
                order_book_id,
                &XOR,
                amount_to_try_unlock.into()
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
            alice::<Runtime>(),
            VAL,
            amount_to_mint.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        assert_ok!(OrderBookPallet::lock_liquidity(
            &alice::<Runtime>(),
            order_book_id,
            &VAL,
            amount_to_lock.into()
        ));

        assert_err!(
            OrderBookPallet::unlock_liquidity(
                &alice::<Runtime>(),
                order_book_id,
                &VAL,
                amount_to_try_unlock.into()
            ),
            tokens::Error::<Runtime>::BalanceTooLow
        );
    });
}

#[test]
fn should_not_unlock_more_nft_that_tech_account_has() {
    ext().execute_with(|| {
        framenode_runtime::frame_system::Pallet::<Runtime>::inc_providers(&alice::<Runtime>());

        let nft = assets::Pallet::<Runtime>::register_from(
            &alice::<Runtime>(),
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            1,
            false,
            None,
            None,
        )
        .unwrap();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: nft.clone(),
            quote: XOR.into(),
        };
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        assert_err!(
            OrderBookPallet::unlock_liquidity(
                &alice::<Runtime>(),
                order_book_id,
                &nft,
                OrderVolume::indivisible(1)
            ),
            tokens::Error::<Runtime>::BalanceTooLow
        );
    });
}

#[test]
fn should_expire_order() {
    ext().execute_with(|| {
        let caller = alice::<Runtime>();
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_empty_order_book::<Runtime>(order_book_id);
        fill_balance::<Runtime>(caller.clone(), order_book_id);

        let price: OrderPrice = balance!(10).into();
        let amount: OrderVolume = balance!(100).into();
        let lifespan = <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000;
        let now = 1234;
        let current_block = frame_system::Pallet::<Runtime>::block_number();
        // the lifespan of N ms corresponds to at least
        // ceil(N / 6000) blocks of the order being available
        let end_of_lifespan_block = current_block
            + <u32>::try_from(lifespan.div_ceil(<Runtime as Config>::MILLISECS_PER_BLOCK)).unwrap();

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(now);

        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(caller.clone()).into(),
            order_book_id,
            *price.balance(),
            *amount.balance(),
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
            current_block,
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
        let caller = alice::<Runtime>();
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_empty_order_book::<Runtime>(order_book_id);
        fill_balance::<Runtime>(caller.clone(), order_book_id);

        let price: OrderPrice = balance!(10).into();
        let amount: OrderVolume = balance!(100).into();
        let lifespan = <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000;
        let now = 1234;
        let current_block = frame_system::Pallet::<Runtime>::block_number();
        // the lifespan of N ms corresponds to at least
        // ceil(N / 6000) blocks of the order being available
        let end_of_lifespan_block = current_block
            + <u32>::try_from(lifespan.div_ceil(<Runtime as Config>::MILLISECS_PER_BLOCK)).unwrap();

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(now);

        // fix state before
        let bids_before = OrderBookPallet::bids(&order_book_id, &price).unwrap_or_default();
        let agg_bids_before = OrderBookPallet::aggregated_bids(&order_book_id);
        let price_volume_before = agg_bids_before.get(&price).cloned().unwrap_or_default();
        let user_orders_before =
            OrderBookPallet::user_limit_orders(&caller, &order_book_id).unwrap_or_default();
        let balance_before = free_balance::<Runtime>(&order_book_id.quote, &caller);

        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(caller.clone()).into(),
            order_book_id,
            *price.balance(),
            *amount.balance(),
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
            current_block,
        );

        let deal_amount = *expected_order
            .deal_amount(MarketRole::Taker, None)
            .unwrap()
            .value();

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

        let balance = free_balance::<Runtime>(&order_book_id.quote, &caller);
        let balance_with_order = balance_before - deal_amount.balance();
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
            free_balance::<Runtime>(&order_book_id.quote, &caller),
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
            free_balance::<Runtime>(&order_book_id.quote, &caller),
            balance_before
        );
    })
}

#[test]
#[ignore] // it works, but takes a lot of time (~2-120 secs depending on settings)
fn should_enforce_expiration_and_weight_limits() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };
        let order_book = create_empty_order_book::<Runtime>(order_book_id);

        let price = balance!(10);
        let amount = balance!(100);
        let lifespan = <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000;
        let now = 1234;
        let current_block = frame_system::Pallet::<Runtime>::block_number();
        // the lifespan of N ms corresponds to at least
        // ceil(N / 6000) blocks of the order being available
        let end_of_lifespan_block = current_block
            + <u32>::try_from(lifespan.div_ceil(<Runtime as Config>::MILLISECS_PER_BLOCK)).unwrap();

        pallet_timestamp::Pallet::<Runtime>::set_timestamp(now);

        let max_orders_expire_at_block = <Runtime as Config>::MaxExpiringOrdersPerBlock::get();
        let mut placed_orders = vec![];

        for i in 0..max_orders_expire_at_block {
            // in order to avoid cap on orders from single account
            let caller = generate_account::<Runtime>(i);
            // in order to avoid cap on orders for a single price
            let price = price + order_book.tick_size.balance() * i as u128;
            fill_balance::<Runtime>(caller.clone(), order_book_id);
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
        let caller = generate_account::<Runtime>(max_orders_expire_at_block);
        fill_balance::<Runtime>(caller.clone(), order_book_id);
        assert_err!(
            OrderBookPallet::place_limit_order(
                RawOrigin::Signed(caller.clone()).into(),
                order_book_id,
                price,
                amount,
                PriceVariant::Buy,
                Some(lifespan)
            ),
            E::BlockScheduleFull
        );

        // All orders are indeed placed
        for order_id in &placed_orders {
            assert!(OrderBookPallet::limit_orders(order_book_id, order_id).is_some());
        }

        let weight_left_for_single_expiration = <Runtime as Config>::MaxExpirationWeightPerBlock::get() - <Runtime as Config>::WeightInfo::service_base() - <Runtime as Config>::WeightInfo::service_block_base();
        let max_expired_per_block: u64 = min(
            weight_left_for_single_expiration.ref_time() / <Runtime as Config>::WeightInfo::service_single_expiration().ref_time(),
            weight_left_for_single_expiration.proof_size() / <Runtime as Config>::WeightInfo::service_single_expiration().proof_size()
        );
        // Check a bit after the expected expiration because it's ok to remove
        // it a few blocks later (due to rounding up)
        let blocks_needed: u32 = (placed_orders.len() as u64 / max_expired_per_block + 2).unique_saturated_into();
        for i in 0..=blocks_needed {
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
#[cfg_attr(
    debug_assertions,
    should_panic(
        expected = "apparently removal of order book or order did not cleanup expiration schedule"
    )
)]
fn should_emit_event_on_expiration_failure() {
    ext().execute_with(|| {
        // To be able to assert events
        frame_system::Pallet::<Runtime>::set_block_number(1);

        let non_existent_order_book_id = OrderBookId {
            dex_id: DEX.into(),
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
                error: E::UnknownLimitOrder.into(),
            }
            .into(),
        );
    })
}

fn should_assemble_order_book_id() {
    ext().execute_with(|| {
        let polkaswap_order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let polkaswap_xstusd_order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: common::DEXId::PolkaswapXSTUSD.into(),
            base: VAL.into(),
            quote: XSTUSD.into(),
        };

        assert_eq!(
            OrderBookPallet::assemble_order_book_id(common::DEXId::Polkaswap.into(), &XOR, &VAL)
                .unwrap(),
            polkaswap_order_book_id
        );
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(common::DEXId::Polkaswap.into(), &VAL, &XOR)
                .unwrap(),
            polkaswap_order_book_id
        );
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(
                common::DEXId::PolkaswapXSTUSD.into(),
                &XSTUSD,
                &VAL
            )
            .unwrap(),
            polkaswap_xstusd_order_book_id
        );
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(
                common::DEXId::PolkaswapXSTUSD.into(),
                &VAL,
                &XSTUSD
            )
            .unwrap(),
            polkaswap_xstusd_order_book_id
        );
    });
}

#[test]
fn should_not_assemble_order_book_id_without_dex_base() {
    ext().execute_with(|| {
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(common::DEXId::Polkaswap.into(), &XSTUSD, &VAL),
            None
        );
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(common::DEXId::Polkaswap.into(), &VAL, &XSTUSD),
            None
        );
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(
                common::DEXId::Polkaswap.into(),
                &XSTUSD,
                &XSTUSD
            ),
            None
        );
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(common::DEXId::Polkaswap.into(), &XOR, &XOR),
            None
        );
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(
                common::DEXId::PolkaswapXSTUSD.into(),
                &XOR,
                &VAL
            ),
            None
        );
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(
                common::DEXId::PolkaswapXSTUSD.into(),
                &VAL,
                &XOR
            ),
            None
        );
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(
                common::DEXId::PolkaswapXSTUSD.into(),
                &XOR,
                &XOR
            ),
            None
        );
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(
                common::DEXId::PolkaswapXSTUSD.into(),
                &XSTUSD,
                &XSTUSD
            ),
            None
        );
    });
}

#[test]
fn can_exchange() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_empty_order_book::<Runtime>(order_book_id);

        assert!(OrderBookPallet::can_exchange(&DEX.into(), &XOR, &VAL));
        assert!(OrderBookPallet::can_exchange(&DEX.into(), &VAL, &XOR));
    });
}

#[test]
fn cannot_exchange_with_non_existed_order_book() {
    ext().execute_with(|| {
        assert!(!OrderBookPallet::can_exchange(&DEX.into(), &XOR, &VAL));
        assert!(!OrderBookPallet::can_exchange(&DEX.into(), &VAL, &XOR));
    });
}

#[test]
fn cannot_exchange_with_not_trade_status() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let mut order_book = OrderBook::<Runtime>::default(order_book_id);

        order_book.status = OrderBookStatus::PlaceAndCancel;
        order_book::OrderBooks::<Runtime>::insert(order_book_id, order_book.clone());
        assert!(!OrderBookPallet::can_exchange(&DEX.into(), &XOR, &VAL));
        assert!(!OrderBookPallet::can_exchange(&DEX.into(), &VAL, &XOR));

        order_book.status = OrderBookStatus::OnlyCancel;
        order_book::OrderBooks::<Runtime>::insert(order_book_id, order_book.clone());
        assert!(!OrderBookPallet::can_exchange(&DEX.into(), &XOR, &VAL));
        assert!(!OrderBookPallet::can_exchange(&DEX.into(), &VAL, &XOR));

        order_book.status = OrderBookStatus::Stop;
        order_book::OrderBooks::<Runtime>::insert(order_book_id, order_book.clone());
        assert!(!OrderBookPallet::can_exchange(&DEX.into(), &XOR, &VAL));
        assert!(!OrderBookPallet::can_exchange(&DEX.into(), &VAL, &XOR));

        // success for Trade status
        order_book.status = OrderBookStatus::Trade;
        order_book::OrderBooks::<Runtime>::insert(order_book_id, order_book.clone());
        assert!(OrderBookPallet::can_exchange(&DEX.into(), &XOR, &VAL));
        assert!(OrderBookPallet::can_exchange(&DEX.into(), &VAL, &XOR));
    });
}

#[test]
fn should_quote() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_and_fill_order_book::<Runtime>(order_book_id);

        // without fee
        assert_eq!(
            OrderBookPallet::quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_input(balance!(3000).into()),
                false
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(271.00535).into(), 0)
        );

        assert_eq!(
            OrderBookPallet::quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(200).into()),
                false
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(2204.74).into(), 0)
        );

        assert_eq!(
            OrderBookPallet::quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_input(balance!(200).into()),
                false
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(1993.7).into(), 0)
        );

        assert_eq!(
            OrderBookPallet::quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(2500).into()),
                false
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(251.66326).into(), 0)
        );

        // with fee
        assert_eq!(
            OrderBookPallet::quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_input(balance!(3000).into()),
                true
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(271.00535).into(), 0)
        );

        assert_eq!(
            OrderBookPallet::quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(200).into()),
                true
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(2204.74).into(), 0)
        );

        assert_eq!(
            OrderBookPallet::quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_input(balance!(200).into()),
                true
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(1993.7).into(), 0)
        );

        assert_eq!(
            OrderBookPallet::quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(2500).into()),
                true
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(251.66326).into(), 0)
        );
    });
}

#[test]
fn should_not_quote_with_non_existed_order_book() {
    ext().execute_with(|| {
        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(200).into()),
                true
            ),
            E::UnknownOrderBook
        );

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(2500).into()),
                true
            ),
            E::UnknownOrderBook
        );
    });
}

#[test]
fn should_not_quote_with_empty_side() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_empty_order_book::<Runtime>(order_book_id);

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(200).into()),
                true
            ),
            E::NotEnoughLiquidityInOrderBook
        );

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(2500).into()),
                true
            ),
            E::NotEnoughLiquidityInOrderBook
        );
    });
}

#[test]
fn should_not_quote_with_small_amount() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_and_fill_order_book::<Runtime>(order_book_id);

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(0.000001).into()),
                true
            ),
            E::InvalidOrderAmount
        );

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(0).into()),
                true
            ),
            E::InvalidOrderAmount
        );

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(0.000001).into()),
                true
            ),
            E::InvalidOrderAmount
        );

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(0).into()),
                true
            ),
            E::InvalidOrderAmount
        );
    });
}

#[test]
fn should_not_quote_if_amount_is_greater_than_liquidity() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_and_fill_order_book::<Runtime>(order_book_id);

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(1000).into()),
                true
            ),
            E::NotEnoughLiquidityInOrderBook
        );

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(10000).into()),
                true
            ),
            E::NotEnoughLiquidityInOrderBook
        );
    });
}

#[test]
fn should_quote_without_impact() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_and_fill_order_book::<Runtime>(order_book_id);

        // without fee
        assert_eq!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_input(balance!(3000).into()),
                false
            )
            .unwrap(),
            SwapOutcome::new(balance!(272.72727).into(), 0)
        );

        assert_eq!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(200).into()),
                false
            )
            .unwrap(),
            SwapOutcome::new(balance!(2200).into(), 0)
        );

        assert_eq!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_input(balance!(200).into()),
                false
            )
            .unwrap(),
            SwapOutcome::new(balance!(2000).into(), 0)
        );

        assert_eq!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(2500).into()),
                false
            )
            .unwrap(),
            SwapOutcome::new(balance!(250).into(), 0)
        );

        // with fee
        assert_eq!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_input(balance!(3000).into()),
                true
            )
            .unwrap(),
            SwapOutcome::new(balance!(272.72727).into(), 0)
        );

        assert_eq!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(200).into()),
                true
            )
            .unwrap(),
            SwapOutcome::new(balance!(2200).into(), 0)
        );

        assert_eq!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_input(balance!(200).into()),
                true
            )
            .unwrap(),
            SwapOutcome::new(balance!(2000).into(), 0)
        );

        assert_eq!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(2500).into()),
                true
            )
            .unwrap(),
            SwapOutcome::new(balance!(250).into(), 0)
        );
    });
}

#[test]
fn should_not_quote_without_impact_with_non_existed_order_book() {
    ext().execute_with(|| {
        assert_err!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(200).into()),
                true
            ),
            E::UnknownOrderBook
        );

        assert_err!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(2500).into()),
                true
            ),
            E::UnknownOrderBook
        );
    });
}

#[test]
fn should_not_quote_without_impact_with_empty_side() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_empty_order_book::<Runtime>(order_book_id);

        assert_err!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(200).into()),
                true
            ),
            E::NotEnoughLiquidityInOrderBook
        );

        assert_err!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(2500).into()),
                true
            ),
            E::NotEnoughLiquidityInOrderBook
        );
    });
}

#[test]
fn should_not_quote_without_impact_with_small_amount() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_and_fill_order_book::<Runtime>(order_book_id);

        assert_err!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(0.000001).into()),
                true
            ),
            E::InvalidOrderAmount
        );

        assert_err!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(0).into()),
                true
            ),
            E::InvalidOrderAmount
        );

        assert_err!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(0.000001).into()),
                true
            ),
            E::InvalidOrderAmount
        );

        assert_err!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(0).into()),
                true
            ),
            E::InvalidOrderAmount
        );
    });
}

#[test]
fn should_exchange_and_transfer_to_owner() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_and_fill_order_book::<Runtime>(order_book_id);
        fill_balance::<Runtime>(alice::<Runtime>(), order_book_id);

        let mut alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &alice::<Runtime>());
        let mut alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &alice::<Runtime>());

        // buy with desired output
        assert_eq!(
            OrderBookPallet::exchange(
                &alice::<Runtime>(),
                &alice::<Runtime>(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_output(balance!(200).into(), balance!(2500).into()),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(2204.74).into(), 0)
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &alice::<Runtime>()),
            alice_base_balance + balance!(200)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &alice::<Runtime>()),
            alice_quote_balance - balance!(2204.74)
        );

        alice_base_balance = free_balance::<Runtime>(&order_book_id.base, &alice::<Runtime>());
        alice_quote_balance = free_balance::<Runtime>(&order_book_id.quote, &alice::<Runtime>());

        // buy with desired input
        assert_eq!(
            OrderBookPallet::exchange(
                &alice::<Runtime>(),
                &alice::<Runtime>(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_input(balance!(2000).into(), balance!(150).into()),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(177.95391).into(), 0)
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &alice::<Runtime>()),
            alice_base_balance + balance!(177.95391)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &alice::<Runtime>()),
            alice_quote_balance - balance!(1999.999965)
        );

        alice_base_balance = free_balance::<Runtime>(&order_book_id.base, &alice::<Runtime>());
        alice_quote_balance = free_balance::<Runtime>(&order_book_id.quote, &alice::<Runtime>());

        // sell with desired output
        assert_eq!(
            OrderBookPallet::exchange(
                &alice::<Runtime>(),
                &alice::<Runtime>(),
                &DEX.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_output(balance!(2000).into(), balance!(210).into()),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(200.64285).into(), 0)
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &alice::<Runtime>()),
            alice_base_balance - balance!(200.64285)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &alice::<Runtime>()),
            alice_quote_balance + balance!(1999.99993)
        );

        alice_base_balance = free_balance::<Runtime>(&order_book_id.base, &alice::<Runtime>());
        alice_quote_balance = free_balance::<Runtime>(&order_book_id.quote, &alice::<Runtime>());

        // sell with desired input
        assert_eq!(
            OrderBookPallet::exchange(
                &alice::<Runtime>(),
                &alice::<Runtime>(),
                &DEX.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_input(balance!(200).into(), balance!(210).into()),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(1932.327145).into(), 0)
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &alice::<Runtime>()),
            alice_base_balance - balance!(200)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &alice::<Runtime>()),
            alice_quote_balance + balance!(1932.327145)
        );
    });
}

#[test]
fn should_exchange_and_transfer_to_another_account() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_and_fill_order_book::<Runtime>(order_book_id);
        fill_balance::<Runtime>(alice::<Runtime>(), order_book_id);

        let mut alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &alice::<Runtime>());
        let mut alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &alice::<Runtime>());

        let mut dave_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &dave::<Runtime>());
        let mut dave_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &dave::<Runtime>());

        // buy with desired output
        assert_eq!(
            OrderBookPallet::exchange(
                &alice::<Runtime>(),
                &dave::<Runtime>(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_output(balance!(200).into(), balance!(2500).into()),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(2204.74).into(), 0)
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &alice::<Runtime>()),
            alice_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &alice::<Runtime>()),
            alice_quote_balance - balance!(2204.74)
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &dave::<Runtime>()),
            dave_base_balance + balance!(200)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &dave::<Runtime>()),
            dave_quote_balance
        );

        alice_base_balance = free_balance::<Runtime>(&order_book_id.base, &alice::<Runtime>());
        alice_quote_balance = free_balance::<Runtime>(&order_book_id.quote, &alice::<Runtime>());

        dave_base_balance = free_balance::<Runtime>(&order_book_id.base, &dave::<Runtime>());
        dave_quote_balance = free_balance::<Runtime>(&order_book_id.quote, &dave::<Runtime>());

        // buy with desired input
        assert_eq!(
            OrderBookPallet::exchange(
                &alice::<Runtime>(),
                &dave::<Runtime>(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_input(balance!(2000).into(), balance!(150).into()),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(177.95391).into(), 0)
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &alice::<Runtime>()),
            alice_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &alice::<Runtime>()),
            alice_quote_balance - balance!(1999.999965)
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &dave::<Runtime>()),
            dave_base_balance + balance!(177.95391)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &dave::<Runtime>()),
            dave_quote_balance
        );

        alice_base_balance = free_balance::<Runtime>(&order_book_id.base, &alice::<Runtime>());
        alice_quote_balance = free_balance::<Runtime>(&order_book_id.quote, &alice::<Runtime>());

        dave_base_balance = free_balance::<Runtime>(&order_book_id.base, &dave::<Runtime>());
        dave_quote_balance = free_balance::<Runtime>(&order_book_id.quote, &dave::<Runtime>());

        // sell with desired output
        assert_eq!(
            OrderBookPallet::exchange(
                &alice::<Runtime>(),
                &dave::<Runtime>(),
                &DEX.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_output(balance!(2000).into(), balance!(210).into()),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(200.64285).into(), 0)
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &alice::<Runtime>()),
            alice_base_balance - balance!(200.64285)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &alice::<Runtime>()),
            alice_quote_balance
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &dave::<Runtime>()),
            dave_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &dave::<Runtime>()),
            dave_quote_balance + balance!(1999.99993)
        );

        alice_base_balance = free_balance::<Runtime>(&order_book_id.base, &alice::<Runtime>());
        alice_quote_balance = free_balance::<Runtime>(&order_book_id.quote, &alice::<Runtime>());

        dave_base_balance = free_balance::<Runtime>(&order_book_id.base, &dave::<Runtime>());
        dave_quote_balance = free_balance::<Runtime>(&order_book_id.quote, &dave::<Runtime>());

        // sell with desired input
        assert_eq!(
            OrderBookPallet::exchange(
                &alice::<Runtime>(),
                &dave::<Runtime>(),
                &DEX.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_input(balance!(200).into(), balance!(210).into()),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(1932.327145).into(), 0)
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &alice::<Runtime>()),
            alice_base_balance - balance!(200)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &alice::<Runtime>()),
            alice_quote_balance
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &dave::<Runtime>()),
            dave_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &dave::<Runtime>()),
            dave_quote_balance + balance!(1932.327145)
        );
    });
}

#[test]
fn should_not_exchange_with_non_existed_order_book() {
    ext().execute_with(|| {
        assert_err!(
            OrderBookPallet::exchange(
                &alice::<Runtime>(),
                &alice::<Runtime>(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_output(balance!(200).into(), balance!(1800).into()),
            ),
            E::UnknownOrderBook
        );

        assert_err!(
            OrderBookPallet::exchange(
                &alice::<Runtime>(),
                &alice::<Runtime>(),
                &DEX.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_output(balance!(2500).into(), balance!(200).into()),
            ),
            E::UnknownOrderBook
        );
    });
}

#[test]
fn should_not_exchange_with_invalid_slippage() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_and_fill_order_book::<Runtime>(order_book_id);
        fill_balance::<Runtime>(alice::<Runtime>(), order_book_id);

        assert_err!(
            OrderBookPallet::exchange(
                &alice::<Runtime>(),
                &alice::<Runtime>(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_output(balance!(200).into(), balance!(1800).into()),
            ),
            E::SlippageLimitExceeded
        );

        assert_err!(
            OrderBookPallet::exchange(
                &alice::<Runtime>(),
                &alice::<Runtime>(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_input(balance!(2000).into(), balance!(210).into()),
            ),
            E::SlippageLimitExceeded
        );

        assert_err!(
            OrderBookPallet::exchange(
                &alice::<Runtime>(),
                &alice::<Runtime>(),
                &DEX.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_output(balance!(2000).into(), balance!(180).into()),
            ),
            E::SlippageLimitExceeded
        );

        assert_err!(
            OrderBookPallet::exchange(
                &alice::<Runtime>(),
                &alice::<Runtime>(),
                &DEX.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_input(balance!(200).into(), balance!(2100).into()),
            ),
            E::SlippageLimitExceeded
        );
    });
}
