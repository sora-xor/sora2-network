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

use core::cmp::min;

use crate::test_utils::*;
use assets::AssetIdOf;
use common::prelude::{QuoteAmount, SwapAmount, SwapOutcome};
use common::{
    balance, AssetName, AssetSymbol, Balance, LiquiditySource, PriceVariant, SwapChunk, VAL, XOR,
    XSTUSD,
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
use sp_std::collections::vec_deque::VecDeque;

#[test]
fn should_register_technical_account() {
    ext().execute_with(|| {
        framenode_runtime::frame_system::Pallet::<Runtime>::inc_providers(&accounts::alice::<
            Runtime,
        >());
        let nft = assets::Pallet::<Runtime>::register_from(
            &accounts::alice::<Runtime>(),
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            1,
            false,
            None,
            None,
        )
        .unwrap();

        let order_books = [
            OrderBookId::<AssetIdOf<Runtime>, DEXId> {
                dex_id: DEX.into(),
                base: VAL,
                quote: XOR,
            },
            OrderBookId::<AssetIdOf<Runtime>, DEXId> {
                dex_id: DEX.into(),
                base: nft,
                quote: XOR,
            },
        ];

        // register (on order book creation)
        for order_book_id in order_books {
            assert_ok!(OrderBookPallet::register_tech_account(order_book_id));
        }

        // deregister (on order book removal)
        for order_book_id in order_books {
            assert_ok!(OrderBookPallet::deregister_tech_account(order_book_id));
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
            accounts::alice::<Runtime>(),
            XOR,
            amount_to_mint.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        // Alice -> Alice (expected on order cancellation)
        test_lock_unlock_same_account(
            order_book_id,
            &XOR,
            amount_to_lock,
            &accounts::alice::<Runtime>(),
        );

        // Alice -> Bob (expected exchange mechanism)
        test_lock_unlock_other_account(
            order_book_id,
            &XOR,
            amount_to_lock,
            &accounts::alice::<Runtime>(),
            &accounts::bob::<Runtime>(),
        );

        // Alice -> Bob & Charlie
        test_lock_unlock_other_accounts(
            order_book_id,
            &XOR,
            amount_to_lock,
            &accounts::alice::<Runtime>(),
            &accounts::bob::<Runtime>(),
            &accounts::charlie::<Runtime>(),
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
            accounts::alice::<Runtime>(),
            VAL,
            amount_to_mint.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        // Alice -> Alice (expected on order cancellation)
        test_lock_unlock_same_account(
            order_book_id,
            &VAL,
            amount_to_lock,
            &accounts::alice::<Runtime>(),
        );

        // Alice -> Bob (expected exchange mechanism)
        test_lock_unlock_other_account(
            order_book_id,
            &VAL,
            amount_to_lock,
            &accounts::alice::<Runtime>(),
            &accounts::bob::<Runtime>(),
        );
    });
}

#[test]
fn should_lock_unlock_indivisible_nft() {
    ext().execute_with(|| {
        framenode_runtime::frame_system::Pallet::<Runtime>::inc_providers(&accounts::alice::<
            Runtime,
        >());

        let nft = assets::Pallet::<Runtime>::register_from(
            &accounts::alice::<Runtime>(),
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
            base: nft,
            quote: XOR,
        };
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        // Alice -> Alice (expected on order cancellation)
        test_lock_unlock_same_account(order_book_id, &nft, 1, &accounts::alice::<Runtime>());

        // Alice -> Bob (expected exchange mechanism)
        test_lock_unlock_other_account(
            order_book_id,
            &nft,
            1,
            &accounts::alice::<Runtime>(),
            &accounts::bob::<Runtime>(),
        );
    });
}

#[test]
fn should_lock_unlock_multiple_indivisible_nfts() {
    ext().execute_with(|| {
        framenode_runtime::frame_system::Pallet::<Runtime>::inc_providers(&accounts::alice::<
            Runtime,
        >());

        let nft = assets::Pallet::<Runtime>::register_from(
            &accounts::alice::<Runtime>(),
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
            base: nft,
            quote: XOR,
        };
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        // Alice -> Bob & Charlie
        test_lock_unlock_other_accounts(
            order_book_id,
            &nft,
            4,
            &accounts::alice::<Runtime>(),
            &accounts::bob::<Runtime>(),
            &accounts::charlie::<Runtime>(),
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
            accounts::alice::<Runtime>(),
            XOR,
            amount_to_mint.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        assert_err!(
            OrderBookPallet::lock_liquidity(
                &accounts::alice::<Runtime>(),
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
            accounts::alice::<Runtime>(),
            VAL,
            amount_to_mint.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        assert_err!(
            OrderBookPallet::lock_liquidity(
                &accounts::alice::<Runtime>(),
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
        let caller = accounts::alice::<Runtime>();
        let creator = accounts::bob::<Runtime>();
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
            base: nft,
            quote: XOR,
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
            accounts::alice::<Runtime>(),
            XOR,
            amount_to_mint.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        assert_ok!(OrderBookPallet::lock_liquidity(
            &accounts::alice::<Runtime>(),
            order_book_id,
            &XOR,
            amount_to_lock.into()
        ));

        assert_err!(
            OrderBookPallet::unlock_liquidity(
                &accounts::alice::<Runtime>(),
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
            accounts::alice::<Runtime>(),
            VAL,
            amount_to_mint.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        assert_ok!(OrderBookPallet::lock_liquidity(
            &accounts::alice::<Runtime>(),
            order_book_id,
            &VAL,
            amount_to_lock.into()
        ));

        assert_err!(
            OrderBookPallet::unlock_liquidity(
                &accounts::alice::<Runtime>(),
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
        framenode_runtime::frame_system::Pallet::<Runtime>::inc_providers(&accounts::alice::<
            Runtime,
        >());

        let nft = assets::Pallet::<Runtime>::register_from(
            &accounts::alice::<Runtime>(),
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
            base: nft,
            quote: XOR,
        };
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        assert_err!(
            OrderBookPallet::unlock_liquidity(
                &accounts::alice::<Runtime>(),
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
        let caller = accounts::alice::<Runtime>();
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
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

        let order_id = get_last_order_id::<Runtime>(order_book_id).unwrap();

        // check
        let expected_order = LimitOrder::<Runtime>::new(
            order_id,
            caller,
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
        let caller = accounts::alice::<Runtime>();
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
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
        let bids_before = OrderBookPallet::bids(order_book_id, price).unwrap_or_default();
        let agg_bids_before = OrderBookPallet::aggregated_bids(order_book_id);
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

        let order_id = get_last_order_id::<Runtime>(order_book_id).unwrap();

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
            OrderBookPallet::bids(order_book_id, price).unwrap(),
            bids_with_order
        );

        let price_volume_with_order = price_volume_before + amount;
        let mut agg_bids_with_order = agg_bids_before.clone();
        assert_ok!(agg_bids_with_order.try_insert(price, price_volume_with_order));
        assert_eq!(
            OrderBookPallet::aggregated_bids(order_book_id),
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

        let order_id = get_last_order_id::<Runtime>(order_book_id).unwrap();

        // The order is still there
        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_id).unwrap(),
            expected_order
        );
        assert_eq!(
            OrderBookPallet::bids(order_book_id, price).unwrap(),
            bids_with_order
        );
        assert_eq!(
            OrderBookPallet::aggregated_bids(order_book_id),
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
            OrderBookPallet::bids(order_book_id, price).unwrap_or_default(),
            bids_before
        );
        assert_eq!(
            OrderBookPallet::aggregated_bids(order_book_id),
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
fn should_enforce_expiration_and_weight_limits() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
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
            let caller = accounts::generate_account::<Runtime>(i);
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
            placed_orders.push(get_last_order_id::<Runtime>(order_book_id).unwrap());
        }
        let caller = accounts::generate_account::<Runtime>(max_orders_expire_at_block);
        fill_balance::<Runtime>(caller.clone(), order_book_id);
        assert_err!(
            OrderBookPallet::place_limit_order(
                RawOrigin::Signed(caller).into(),
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

        let weight_left_for_single_expiration = <Runtime as Config>::MaxExpirationWeightPerBlock::get() - <Runtime as Config>::WeightInfo::service_expiration_base() - <Runtime as Config>::WeightInfo::service_expiration_block_base();
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
            assert!(<Runtime as Config>::MaxExpirationWeightPerBlock::get().saturating_add(<Runtime as Config>::MaxAlignmentWeightPerBlock::get())
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
        let expiration_block = 2u32;
        assert_ok!(OrderBookPallet::schedule_expiration(
            expiration_block,
            non_existent_order_book_id,
            non_existent_order_id
        ));
        run_to_block(expiration_block);
        frame_system::Pallet::<Runtime>::assert_has_event(
            order_book::Event::ExpirationFailure {
                order_book_id: non_existent_order_book_id,
                order_id: non_existent_order_id,
                error: E::UnknownLimitOrder.into(),
            }
            .into(),
        );
    })
}

#[test]
fn should_assemble_order_book_id() {
    ext().execute_with(|| {
        let polkaswap_order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let polkaswap_xstusd_order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: common::DEXId::PolkaswapXSTUSD.into(),
            base: VAL,
            quote: XSTUSD,
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
            base: VAL,
            quote: XOR,
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
            base: VAL,
            quote: XOR,
        };

        let mut order_book = OrderBook::<Runtime>::new(
            order_book_id,
            OrderPrice::divisible(balance!(0.00001)),
            OrderVolume::divisible(balance!(0.00001)),
            OrderVolume::divisible(balance!(1)),
            OrderVolume::divisible(balance!(1000)),
        );

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
        order_book::OrderBooks::<Runtime>::insert(order_book_id, order_book);
        assert!(OrderBookPallet::can_exchange(&DEX.into(), &XOR, &VAL));
        assert!(OrderBookPallet::can_exchange(&DEX.into(), &VAL, &XOR));
    });
}

#[test]
fn should_quote() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let _ = create_and_fill_order_book::<Runtime>(order_book_id);

        // without fee
        assert_eq!(
            OrderBookPallet::quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_input(balance!(3000)),
                false
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(271.00535), Default::default())
        );

        assert_eq!(
            OrderBookPallet::quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(200)),
                false
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(2204.74), Default::default())
        );

        assert_eq!(
            OrderBookPallet::quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_input(balance!(200)),
                false
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(1993.7), Default::default())
        );

        assert_eq!(
            OrderBookPallet::quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(2500)),
                false
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(251.66326), Default::default())
        );

        // with fee
        assert_eq!(
            OrderBookPallet::quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_input(balance!(3000)),
                true
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(271.00535), Default::default())
        );

        assert_eq!(
            OrderBookPallet::quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(200)),
                true
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(2204.74), Default::default())
        );

        assert_eq!(
            OrderBookPallet::quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_input(balance!(200)),
                true
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(1993.7), Default::default())
        );

        assert_eq!(
            OrderBookPallet::quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(2500)),
                true
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(251.66326), Default::default())
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
                QuoteAmount::with_desired_output(balance!(200)),
                true
            ),
            E::UnknownOrderBook
        );

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(2500)),
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
            base: VAL,
            quote: XOR,
        };

        let _ = create_empty_order_book::<Runtime>(order_book_id);

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(200)),
                true
            ),
            E::NotEnoughLiquidityInOrderBook
        );

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(2500)),
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
            base: VAL,
            quote: XOR,
        };

        let _ = create_and_fill_order_book::<Runtime>(order_book_id);

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(0.000001)),
                true
            ),
            E::InvalidOrderAmount
        );

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(0)),
                true
            ),
            E::InvalidOrderAmount
        );

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(0.000001)),
                true
            ),
            E::InvalidOrderAmount
        );

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(0)),
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
            base: VAL,
            quote: XOR,
        };

        let _ = create_and_fill_order_book::<Runtime>(order_book_id);

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(1000)),
                true
            ),
            E::NotEnoughLiquidityInOrderBook
        );

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(10000)),
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
            base: VAL,
            quote: XOR,
        };

        let _ = create_and_fill_order_book::<Runtime>(order_book_id);

        // without fee
        assert_eq!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_input(balance!(3000)),
                false
            )
            .unwrap(),
            SwapOutcome::new(balance!(272.72727), Default::default())
        );

        assert_eq!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(200)),
                false
            )
            .unwrap(),
            SwapOutcome::new(balance!(2200), Default::default())
        );

        assert_eq!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_input(balance!(200)),
                false
            )
            .unwrap(),
            SwapOutcome::new(balance!(2000), Default::default())
        );

        assert_eq!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(2500)),
                false
            )
            .unwrap(),
            SwapOutcome::new(balance!(250), Default::default())
        );

        // with fee
        assert_eq!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_input(balance!(3000)),
                true
            )
            .unwrap(),
            SwapOutcome::new(balance!(272.72727), Default::default())
        );

        assert_eq!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(200)),
                true
            )
            .unwrap(),
            SwapOutcome::new(balance!(2200), Default::default())
        );

        assert_eq!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_input(balance!(200)),
                true
            )
            .unwrap(),
            SwapOutcome::new(balance!(2000), Default::default())
        );

        assert_eq!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(2500)),
                true
            )
            .unwrap(),
            SwapOutcome::new(balance!(250), Default::default())
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
                QuoteAmount::with_desired_output(balance!(200)),
                true
            ),
            E::UnknownOrderBook
        );

        assert_err!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(2500)),
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
            base: VAL,
            quote: XOR,
        };

        let _ = create_empty_order_book::<Runtime>(order_book_id);

        assert_err!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(200)),
                true
            ),
            E::NotEnoughLiquidityInOrderBook
        );

        assert_err!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(2500)),
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
            base: VAL,
            quote: XOR,
        };

        let _ = create_and_fill_order_book::<Runtime>(order_book_id);

        assert_err!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(0.000001)),
                true
            ),
            E::InvalidOrderAmount
        );

        assert_err!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(0)),
                true
            ),
            E::InvalidOrderAmount
        );

        assert_err!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(0.000001)),
                true
            ),
            E::InvalidOrderAmount
        );

        assert_err!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(0)),
                true
            ),
            E::InvalidOrderAmount
        );
    });
}

#[test]
fn should_step_quote() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let _ = create_and_fill_order_book::<Runtime>(order_book_id);

        // XOR -> VAL with desired input

        assert_eq!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_input(balance!(0)),
                10,
                true
            )
            .unwrap()
            .0,
            VecDeque::new()
        );

        assert_eq!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_input(balance!(1000)),
                10,
                false
            )
            .unwrap()
            .0,
            VecDeque::from([SwapChunk::new(balance!(1939.3), balance!(176.3), 0)])
        );

        assert_eq!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_input(balance!(2000)),
                10,
                true
            )
            .unwrap()
            .0,
            VecDeque::from([
                SwapChunk::new(balance!(1939.3), balance!(176.3), 0),
                SwapChunk::new(balance!(2000.32), balance!(178.6), 0)
            ])
        );

        assert_eq!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_input(balance!(5000)),
                10,
                false
            )
            .unwrap()
            .0,
            VecDeque::from([
                SwapChunk::new(balance!(1939.3), balance!(176.3), 0),
                SwapChunk::new(balance!(2000.32), balance!(178.6), 0),
                SwapChunk::new(balance!(2941.7), balance!(255.8), 0),
            ])
        );

        assert_eq!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_input(balance!(10000)),
                10,
                true
            )
            .unwrap()
            .0,
            VecDeque::from([
                SwapChunk::new(balance!(1939.3), balance!(176.3), 0),
                SwapChunk::new(balance!(2000.32), balance!(178.6), 0),
                SwapChunk::new(balance!(2941.7), balance!(255.8), 0),
            ])
        );

        // XOR -> VAL with desired output

        assert_eq!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(0)),
                10,
                false
            )
            .unwrap()
            .0,
            VecDeque::new()
        );

        assert_eq!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(100)),
                10,
                true
            )
            .unwrap()
            .0,
            VecDeque::from([SwapChunk::new(balance!(1939.3), balance!(176.3), 0)])
        );

        assert_eq!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(200)),
                10,
                false
            )
            .unwrap()
            .0,
            VecDeque::from([
                SwapChunk::new(balance!(1939.3), balance!(176.3), 0),
                SwapChunk::new(balance!(2000.32), balance!(178.6), 0)
            ])
        );

        assert_eq!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(500)),
                10,
                true
            )
            .unwrap()
            .0,
            VecDeque::from([
                SwapChunk::new(balance!(1939.3), balance!(176.3), 0),
                SwapChunk::new(balance!(2000.32), balance!(178.6), 0),
                SwapChunk::new(balance!(2941.7), balance!(255.8), 0),
            ])
        );

        assert_eq!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(1000)),
                10,
                false
            )
            .unwrap()
            .0,
            VecDeque::from([
                SwapChunk::new(balance!(1939.3), balance!(176.3), 0),
                SwapChunk::new(balance!(2000.32), balance!(178.6), 0),
                SwapChunk::new(balance!(2941.7), balance!(255.8), 0),
            ])
        );

        // VAL -> XOR with desired input

        assert_eq!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_input(balance!(0)),
                10,
                true
            )
            .unwrap()
            .0,
            VecDeque::new()
        );

        assert_eq!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_input(balance!(100)),
                10,
                false
            )
            .unwrap()
            .0,
            VecDeque::from([SwapChunk::new(balance!(168.5), balance!(1685), 0)])
        );

        assert_eq!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_input(balance!(200)),
                10,
                true
            )
            .unwrap()
            .0,
            VecDeque::from([
                SwapChunk::new(balance!(168.5), balance!(1685), 0),
                SwapChunk::new(balance!(139.9), balance!(1371.02), 0)
            ])
        );

        assert_eq!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_input(balance!(500)),
                10,
                false
            )
            .unwrap()
            .0,
            VecDeque::from([
                SwapChunk::new(balance!(168.5), balance!(1685), 0),
                SwapChunk::new(balance!(139.9), balance!(1371.02), 0),
                SwapChunk::new(balance!(261.3), balance!(2482.35), 0),
            ])
        );

        assert_eq!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_input(balance!(1000)),
                10,
                true
            )
            .unwrap()
            .0,
            VecDeque::from([
                SwapChunk::new(balance!(168.5), balance!(1685), 0),
                SwapChunk::new(balance!(139.9), balance!(1371.02), 0),
                SwapChunk::new(balance!(261.3), balance!(2482.35), 0),
            ])
        );

        // VAL -> XOR with desired output

        assert_eq!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(0)),
                10,
                false
            )
            .unwrap()
            .0,
            VecDeque::new()
        );

        assert_eq!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(1000)),
                10,
                true
            )
            .unwrap()
            .0,
            VecDeque::from([SwapChunk::new(balance!(168.5), balance!(1685), 0)])
        );

        assert_eq!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(2000)),
                10,
                false
            )
            .unwrap()
            .0,
            VecDeque::from([
                SwapChunk::new(balance!(168.5), balance!(1685), 0),
                SwapChunk::new(balance!(139.9), balance!(1371.02), 0)
            ])
        );

        assert_eq!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(5000)),
                10,
                true
            )
            .unwrap()
            .0,
            VecDeque::from([
                SwapChunk::new(balance!(168.5), balance!(1685), 0),
                SwapChunk::new(balance!(139.9), balance!(1371.02), 0),
                SwapChunk::new(balance!(261.3), balance!(2482.35), 0),
            ])
        );

        assert_eq!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(10000)),
                10,
                false
            )
            .unwrap()
            .0,
            VecDeque::from([
                SwapChunk::new(balance!(168.5), balance!(1685), 0),
                SwapChunk::new(balance!(139.9), balance!(1371.02), 0),
                SwapChunk::new(balance!(261.3), balance!(2482.35), 0),
            ])
        );
    });
}

#[test]
fn should_not_step_quote_with_non_existed_order_book() {
    ext().execute_with(|| {
        assert_err!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(200)),
                10,
                true
            ),
            E::UnknownOrderBook
        );

        assert_err!(
            OrderBookPallet::step_quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(2500)),
                10,
                false
            ),
            E::UnknownOrderBook
        );
    });
}

#[test]
fn should_exchange_and_transfer_to_owner() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let _ = create_and_fill_order_book::<Runtime>(order_book_id);
        fill_balance::<Runtime>(accounts::alice::<Runtime>(), order_book_id);

        let mut alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        let mut alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        // buy with desired output
        assert_eq!(
            OrderBookPallet::exchange(
                &accounts::alice::<Runtime>(),
                &accounts::alice::<Runtime>(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_output(balance!(200), balance!(2500)),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(2204.74), Default::default())
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance + balance!(200)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance - balance!(2204.74)
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        // buy with desired input
        assert_eq!(
            OrderBookPallet::exchange(
                &accounts::alice::<Runtime>(),
                &accounts::alice::<Runtime>(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_input(balance!(2000), balance!(150)),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(177.95391), Default::default())
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance + balance!(177.95391)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance - balance!(1999.999965)
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        // sell with desired output
        assert_eq!(
            OrderBookPallet::exchange(
                &accounts::alice::<Runtime>(),
                &accounts::alice::<Runtime>(),
                &DEX.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_output(balance!(2000), balance!(210)),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(200.64285), Default::default())
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance - balance!(200.64285)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance + balance!(1999.99993)
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        // sell with desired input
        assert_eq!(
            OrderBookPallet::exchange(
                &accounts::alice::<Runtime>(),
                &accounts::alice::<Runtime>(),
                &DEX.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_input(balance!(200), balance!(210)),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(1932.327145), Default::default())
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance - balance!(200)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance + balance!(1932.327145)
        );
    });
}

#[test]
fn should_exchange_and_transfer_to_another_account() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let _ = create_and_fill_order_book::<Runtime>(order_book_id);
        fill_balance::<Runtime>(accounts::alice::<Runtime>(), order_book_id);

        let mut alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        let mut alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        let mut dave_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>());
        let mut dave_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>());

        // buy with desired output
        assert_eq!(
            OrderBookPallet::exchange(
                &accounts::alice::<Runtime>(),
                &accounts::dave::<Runtime>(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_output(balance!(200), balance!(2500)),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(2204.74), Default::default())
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance - balance!(2204.74)
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>()),
            dave_base_balance + balance!(200)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>()),
            dave_quote_balance
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        dave_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>());
        dave_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>());

        // buy with desired input
        assert_eq!(
            OrderBookPallet::exchange(
                &accounts::alice::<Runtime>(),
                &accounts::dave::<Runtime>(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_input(balance!(2000), balance!(150)),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(177.95391), Default::default())
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance - balance!(1999.999965)
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>()),
            dave_base_balance + balance!(177.95391)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>()),
            dave_quote_balance
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        dave_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>());
        dave_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>());

        // sell with desired output
        assert_eq!(
            OrderBookPallet::exchange(
                &accounts::alice::<Runtime>(),
                &accounts::dave::<Runtime>(),
                &DEX.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_output(balance!(2000), balance!(210)),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(200.64285), Default::default())
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance - balance!(200.64285)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>()),
            dave_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>()),
            dave_quote_balance + balance!(1999.99993)
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        dave_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>());
        dave_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>());

        // sell with desired input
        assert_eq!(
            OrderBookPallet::exchange(
                &accounts::alice::<Runtime>(),
                &accounts::dave::<Runtime>(),
                &DEX.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_input(balance!(200), balance!(210)),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(1932.327145), Default::default())
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance - balance!(200)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance
        );

        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>()),
            dave_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>()),
            dave_quote_balance + balance!(1932.327145)
        );
    });
}

#[test]
fn should_not_exchange_with_non_existed_order_book() {
    ext().execute_with(|| {
        assert_err!(
            OrderBookPallet::exchange(
                &accounts::alice::<Runtime>(),
                &accounts::alice::<Runtime>(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_output(balance!(200), balance!(1800)),
            ),
            E::UnknownOrderBook
        );

        assert_err!(
            OrderBookPallet::exchange(
                &accounts::alice::<Runtime>(),
                &accounts::alice::<Runtime>(),
                &DEX.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_output(balance!(2500), balance!(200)),
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
            base: VAL,
            quote: XOR,
        };

        let _ = create_and_fill_order_book::<Runtime>(order_book_id);
        fill_balance::<Runtime>(accounts::alice::<Runtime>(), order_book_id);

        assert_err!(
            OrderBookPallet::exchange(
                &accounts::alice::<Runtime>(),
                &accounts::alice::<Runtime>(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_output(balance!(200), balance!(1800)),
            ),
            E::SlippageLimitExceeded
        );

        assert_err!(
            OrderBookPallet::exchange(
                &accounts::alice::<Runtime>(),
                &accounts::alice::<Runtime>(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_input(balance!(2000), balance!(210)),
            ),
            E::SlippageLimitExceeded
        );

        assert_err!(
            OrderBookPallet::exchange(
                &accounts::alice::<Runtime>(),
                &accounts::alice::<Runtime>(),
                &DEX.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_output(balance!(2000), balance!(180)),
            ),
            E::SlippageLimitExceeded
        );

        assert_err!(
            OrderBookPallet::exchange(
                &accounts::alice::<Runtime>(),
                &accounts::alice::<Runtime>(),
                &DEX.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_input(balance!(200), balance!(2100)),
            ),
            E::SlippageLimitExceeded
        );
    });
}
