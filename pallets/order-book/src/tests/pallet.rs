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
use common::prelude::{QuoteAmount, SwapAmount, SwapOutcome};
use common::{balance, AssetName, AssetSymbol, Balance, DEXId, LiquiditySource, VAL, XOR, XSTUSD};
use frame_support::{assert_err, assert_ok};
use framenode_chain_spec::ext;
use framenode_runtime::order_book::{
    self, CurrencyLocker, CurrencyUnlocker, OrderBook, OrderBookId, OrderBookStatus,
};
use framenode_runtime::{Runtime, RuntimeOrigin};
use sp_std::collections::btree_map::BTreeMap;

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
    let balance_before = free_balance(asset_id, account);

    assert_ok!(OrderBookPallet::lock_liquidity(
        dex_id.into(),
        account,
        order_book_id,
        asset_id,
        amount_to_lock
    ));

    let balance_after_lock = free_balance(asset_id, account);
    assert_eq!(balance_after_lock, balance_before - amount_to_lock);

    assert_ok!(OrderBookPallet::unlock_liquidity(
        dex_id.into(),
        account,
        order_book_id,
        asset_id,
        amount_to_lock
    ));

    let balance_after_unlock = free_balance(asset_id, account);
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
    let lock_account_balance_before = free_balance(asset_id, lock_account);
    let unlock_account_balance_before = free_balance(asset_id, unlock_account);

    assert_ok!(OrderBookPallet::lock_liquidity(
        dex_id.into(),
        lock_account,
        order_book_id,
        asset_id,
        amount_to_lock
    ));

    let lock_account_balance_after_lock = free_balance(asset_id, lock_account);
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

    let unlock_account_balance_after_unlock = free_balance(asset_id, unlock_account);
    assert_eq!(
        unlock_account_balance_after_unlock,
        unlock_account_balance_before + amount_to_lock
    );
}

fn test_lock_unlock_other_accounts(
    dex_id: common::DEXId,
    order_book_id: OrderBookId<AssetIdOf<Runtime>>,
    asset_id: &AssetIdOf<Runtime>,
    amount_to_lock: Balance,
    lock_account: &<Runtime as frame_system::Config>::AccountId,
    unlock_account1: &<Runtime as frame_system::Config>::AccountId,
    unlock_account2: &<Runtime as frame_system::Config>::AccountId,
) {
    let lock_account_balance_before = free_balance(asset_id, lock_account);
    let unlock_account_balance_before1 = free_balance(asset_id, unlock_account1);
    let unlock_account_balance_before2 = free_balance(asset_id, unlock_account2);

    assert_ok!(OrderBookPallet::lock_liquidity(
        dex_id.into(),
        lock_account,
        order_book_id,
        asset_id,
        amount_to_lock
    ));

    let lock_account_balance_after_lock = free_balance(asset_id, lock_account);
    assert_eq!(
        lock_account_balance_after_lock,
        lock_account_balance_before - amount_to_lock
    );

    let unlock_amount1: Balance = (amount_to_lock / 4) * 3;
    let unlock_amount2: Balance = amount_to_lock / 4;

    let unlocks = BTreeMap::from([
        (unlock_account1.clone(), unlock_amount1),
        (unlock_account2.clone(), unlock_amount2),
    ]);

    assert_ok!(OrderBookPallet::unlock_liquidity_batch(
        dex_id.into(),
        order_book_id,
        asset_id,
        &unlocks
    ));

    let unlock_account_balance_after_unlock1 = free_balance(asset_id, unlock_account1);
    assert_eq!(
        unlock_account_balance_after_unlock1,
        unlock_account_balance_before1 + unlock_amount1
    );

    let unlock_account_balance_after_unlock2 = free_balance(asset_id, unlock_account2);
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

        // Alice -> Bob & Charlie
        test_lock_unlock_other_accounts(
            DEX,
            order_book_id,
            &XOR,
            amount_to_lock,
            &alice(),
            &bob(),
            &charlie(),
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
fn should_lock_unlock_multiple_indivisible_nfts() {
    ext().execute_with(|| {
        framenode_runtime::frame_system::Pallet::<Runtime>::inc_providers(&alice());

        let nft = assets::Pallet::<Runtime>::register_from(
            &alice(),
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            balance!(4),
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

        // Alice -> Bob & Charlie
        test_lock_unlock_other_accounts(
            DEX,
            order_book_id,
            &nft,
            balance!(4),
            &alice(),
            &bob(),
            &charlie(),
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
fn should_assemble_order_book_id() {
    ext().execute_with(|| {
        let polkaswap_order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let polkaswap_xstusd_order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XSTUSD.into(),
        };

        assert_eq!(
            OrderBookPallet::assemble_order_book_id(&DEXId::Polkaswap.into(), &XOR, &VAL).unwrap(),
            polkaswap_order_book_id
        );
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(&DEXId::Polkaswap.into(), &VAL, &XOR).unwrap(),
            polkaswap_order_book_id
        );
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(&DEXId::PolkaswapXSTUSD.into(), &XSTUSD, &VAL)
                .unwrap(),
            polkaswap_xstusd_order_book_id
        );
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(&DEXId::PolkaswapXSTUSD.into(), &VAL, &XSTUSD)
                .unwrap(),
            polkaswap_xstusd_order_book_id
        );
    });
}

#[test]
fn should_not_assemble_order_book_id_without_dex_base() {
    ext().execute_with(|| {
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(&DEXId::Polkaswap.into(), &XSTUSD, &VAL),
            None
        );
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(&DEXId::Polkaswap.into(), &VAL, &XSTUSD),
            None
        );
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(&DEXId::Polkaswap.into(), &XSTUSD, &XSTUSD),
            None
        );
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(&DEXId::Polkaswap.into(), &XOR, &XOR),
            None
        );
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(&DEXId::PolkaswapXSTUSD.into(), &XOR, &VAL),
            None
        );
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(&DEXId::PolkaswapXSTUSD.into(), &VAL, &XOR),
            None
        );
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(&DEXId::PolkaswapXSTUSD.into(), &XOR, &XOR),
            None
        );
        assert_eq!(
            OrderBookPallet::assemble_order_book_id(
                &DEXId::PolkaswapXSTUSD.into(),
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
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_empty_order_book(order_book_id);

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
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let mut order_book = OrderBook::<Runtime>::default(order_book_id, DEX.into());

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
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_and_fill_order_book(order_book_id);

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
            SwapOutcome::new(balance!(271.00535), 0)
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
            SwapOutcome::new(balance!(2204.74), 0)
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
            SwapOutcome::new(balance!(1993.7), 0)
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
            SwapOutcome::new(balance!(251.66326), 0)
        );

        // todo (m.tagirov) remake when fee introduced
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
            SwapOutcome::new(balance!(271.00535), 0)
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
            SwapOutcome::new(balance!(2204.74), 0)
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
            SwapOutcome::new(balance!(1993.7), 0)
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
            SwapOutcome::new(balance!(251.66326), 0)
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
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_empty_order_book(order_book_id);

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(200)),
                true
            ),
            E::NotEnoughLiquidity
        );

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(2500)),
                true
            ),
            E::NotEnoughLiquidity
        );
    });
}

#[test]
fn should_not_quote_with_small_amount() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_and_fill_order_book(order_book_id);

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
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_and_fill_order_book(order_book_id);

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(1000)),
                true
            ),
            E::NotEnoughLiquidity
        );

        assert_err!(
            OrderBookPallet::quote(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(10000)),
                true
            ),
            E::NotEnoughLiquidity
        );
    });
}

#[test]
fn should_quote_without_impact() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_and_fill_order_book(order_book_id);

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
            SwapOutcome::new(balance!(272.72727), 0)
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
            SwapOutcome::new(balance!(2200), 0)
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
            SwapOutcome::new(balance!(2000), 0)
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
            SwapOutcome::new(balance!(250), 0)
        );

        // todo (m.tagirov) remake when fee introduced
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
            SwapOutcome::new(balance!(272.72727), 0)
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
            SwapOutcome::new(balance!(2200), 0)
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
            SwapOutcome::new(balance!(2000), 0)
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
            SwapOutcome::new(balance!(250), 0)
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
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_empty_order_book(order_book_id);

        assert_err!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(200)),
                true
            ),
            E::NotEnoughLiquidity
        );

        assert_err!(
            OrderBookPallet::quote_without_impact(
                &DEX.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(2500)),
                true
            ),
            E::NotEnoughLiquidity
        );
    });
}

#[test]
fn should_not_quote_without_impact_with_small_amount() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_and_fill_order_book(order_book_id);

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
fn should_exchange_and_transfer_to_owner() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_and_fill_order_book(order_book_id);
        fill_balance(alice(), order_book_id);

        let mut alice_base_balance = free_balance(&order_book_id.base, &alice());
        let mut alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        // buy with desired output
        assert_eq!(
            OrderBookPallet::exchange(
                &alice(),
                &alice(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_output(balance!(200), balance!(2500)),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(2204.74), 0)
        );

        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance + balance!(200)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance - balance!(2204.74)
        );

        alice_base_balance = free_balance(&order_book_id.base, &alice());
        alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        // buy with desired input
        assert_eq!(
            OrderBookPallet::exchange(
                &alice(),
                &alice(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_input(balance!(2000), balance!(150)),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(177.95391), 0)
        );

        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance + balance!(177.95391)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance - balance!(1999.999965)
        );

        alice_base_balance = free_balance(&order_book_id.base, &alice());
        alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        // sell with desired output
        assert_eq!(
            OrderBookPallet::exchange(
                &alice(),
                &alice(),
                &DEX.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_output(balance!(2000), balance!(210)),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(200.64285), 0)
        );

        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance - balance!(200.64285)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance + balance!(1999.99993)
        );

        alice_base_balance = free_balance(&order_book_id.base, &alice());
        alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        // sell with desired input
        assert_eq!(
            OrderBookPallet::exchange(
                &alice(),
                &alice(),
                &DEX.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_input(balance!(200), balance!(210)),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(1932.327145), 0)
        );

        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance - balance!(200)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance + balance!(1932.327145)
        );
    });
}

#[test]
fn should_exchange_and_transfer_to_another_account() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_and_fill_order_book(order_book_id);
        fill_balance(alice(), order_book_id);

        let mut alice_base_balance = free_balance(&order_book_id.base, &alice());
        let mut alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        let mut dave_base_balance = free_balance(&order_book_id.base, &dave());
        let mut dave_quote_balance = free_balance(&order_book_id.quote, &dave());

        // buy with desired output
        assert_eq!(
            OrderBookPallet::exchange(
                &alice(),
                &dave(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_output(balance!(200), balance!(2500)),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(2204.74), 0)
        );

        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance - balance!(2204.74)
        );

        assert_eq!(
            free_balance(&order_book_id.base, &dave()),
            dave_base_balance + balance!(200)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &dave()),
            dave_quote_balance
        );

        alice_base_balance = free_balance(&order_book_id.base, &alice());
        alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        dave_base_balance = free_balance(&order_book_id.base, &dave());
        dave_quote_balance = free_balance(&order_book_id.quote, &dave());

        // buy with desired input
        assert_eq!(
            OrderBookPallet::exchange(
                &alice(),
                &dave(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_input(balance!(2000), balance!(150)),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(177.95391), 0)
        );

        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance - balance!(1999.999965)
        );

        assert_eq!(
            free_balance(&order_book_id.base, &dave()),
            dave_base_balance + balance!(177.95391)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &dave()),
            dave_quote_balance
        );

        alice_base_balance = free_balance(&order_book_id.base, &alice());
        alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        dave_base_balance = free_balance(&order_book_id.base, &dave());
        dave_quote_balance = free_balance(&order_book_id.quote, &dave());

        // sell with desired output
        assert_eq!(
            OrderBookPallet::exchange(
                &alice(),
                &dave(),
                &DEX.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_output(balance!(2000), balance!(210)),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(200.64285), 0)
        );

        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance - balance!(200.64285)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance
        );

        assert_eq!(
            free_balance(&order_book_id.base, &dave()),
            dave_base_balance
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &dave()),
            dave_quote_balance + balance!(1999.99993)
        );

        alice_base_balance = free_balance(&order_book_id.base, &alice());
        alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        dave_base_balance = free_balance(&order_book_id.base, &dave());
        dave_quote_balance = free_balance(&order_book_id.quote, &dave());

        // sell with desired input
        assert_eq!(
            OrderBookPallet::exchange(
                &alice(),
                &dave(),
                &DEX.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_input(balance!(200), balance!(210)),
            )
            .unwrap()
            .0,
            SwapOutcome::new(balance!(1932.327145), 0)
        );

        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance - balance!(200)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance
        );

        assert_eq!(
            free_balance(&order_book_id.base, &dave()),
            dave_base_balance
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &dave()),
            dave_quote_balance + balance!(1932.327145)
        );
    });
}

#[test]
fn should_not_exchange_with_non_existed_order_book() {
    ext().execute_with(|| {
        assert_err!(
            OrderBookPallet::exchange(
                &alice(),
                &alice(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_output(balance!(200), balance!(1800)),
            ),
            E::UnknownOrderBook
        );

        assert_err!(
            OrderBookPallet::exchange(
                &alice(),
                &alice(),
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
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let _ = create_and_fill_order_book(order_book_id);
        fill_balance(alice(), order_book_id);

        assert_err!(
            OrderBookPallet::exchange(
                &alice(),
                &alice(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_output(balance!(200), balance!(1800)),
            ),
            E::SlippageLimitExceeded
        );

        assert_err!(
            OrderBookPallet::exchange(
                &alice(),
                &alice(),
                &DEX.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_input(balance!(2000), balance!(210)),
            ),
            E::SlippageLimitExceeded
        );

        assert_err!(
            OrderBookPallet::exchange(
                &alice(),
                &alice(),
                &DEX.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_output(balance!(2000), balance!(180)),
            ),
            E::SlippageLimitExceeded
        );

        assert_err!(
            OrderBookPallet::exchange(
                &alice(),
                &alice(),
                &DEX.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_input(balance!(200), balance!(2100)),
            ),
            E::SlippageLimitExceeded
        );
    });
}
