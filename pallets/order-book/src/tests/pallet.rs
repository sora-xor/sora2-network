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
use common::{balance, AssetInfoProvider, AssetName, AssetSymbol, Balance, VAL, XOR};
use frame_support::{assert_err, assert_ok};
use framenode_chain_spec::ext;
use framenode_runtime::order_book::{CurrencyLocker, CurrencyUnlocker, OrderBookId};
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
