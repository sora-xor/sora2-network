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
use common::{
    balance, AssetId32, AssetName, AssetSymbol, PriceVariant, DEFAULT_BALANCE_PRECISION, ETH,
    PSWAP, VAL, XOR, XST, XSTUSD,
};
use frame_support::error::BadOrigin;
use frame_support::{assert_err, assert_ok};
use frame_system::RawOrigin;
use framenode_chain_spec::ext;
use framenode_runtime::order_book::{
    Config, LimitOrder, MarketRole, OrderBook, OrderBookId, OrderBookStatus, OrderPrice,
    OrderVolume,
};
use framenode_runtime::{Runtime, RuntimeOrigin};
use hex_literal::hex;
use sp_core::Get;
use sp_std::collections::btree_map::BTreeMap;

type Assets = framenode_runtime::assets::Pallet<Runtime>;
type TradingPair = framenode_runtime::trading_pair::Pallet<Runtime>;
type FrameSystem = framenode_runtime::frame_system::Pallet<Runtime>;
type Timestamp = pallet_timestamp::Pallet<Runtime>;
type TechnicalRawOrigin = pallet_collective::RawOrigin<
    <Runtime as frame_system::Config>::AccountId,
    framenode_runtime::TechnicalCollective,
>;

#[test]
fn should_not_create_order_book_with_disallowed_dex_id() {
    ext().execute_with(|| {
        let mut order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: common::DEXId::PolkaswapXSTUSD.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        assert_err!(
            OrderBookPallet::create_orderbook(RawOrigin::Signed(alice()).into(), order_book_id),
            E::NotAllowedDEXId,
        );

        // any number except 0 (polkaswap dex id) should not be allowed
        order_book_id.dex_id = 12345678;
        assert_err!(
            OrderBookPallet::create_orderbook(RawOrigin::Signed(alice()).into(), order_book_id),
            E::NotAllowedDEXId,
        );
    });
}

#[test]
fn should_create_order_book_with_correct_dex_id() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        assert_ok!(OrderBookPallet::create_orderbook(
            RawOrigin::Signed(alice()).into(),
            order_book_id
        ));

        assert_eq!(
            OrderBookPallet::order_books(order_book_id).unwrap(),
            OrderBook::default(order_book_id)
        );
    });
}

#[test]
fn should_not_create_order_book_with_same_assets() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: XOR.into(),
            quote: XOR.into(),
        };

        assert_err!(
            OrderBookPallet::create_orderbook(RawOrigin::Signed(alice()).into(), order_book_id),
            E::ForbiddenToCreateOrderBookWithSameAssets
        );
    });
}

#[test]
fn should_not_create_order_book_with_wrong_quote_asset() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: XOR.into(),
            quote: VAL.into(),
        };

        assert_err!(
            OrderBookPallet::create_orderbook(RawOrigin::Signed(alice()).into(), order_book_id),
            E::NotAllowedQuoteAsset
        );
    });
}

#[test]
fn should_create_order_book_with_synthetic_base_asset() {
    ext().execute_with(|| {
        let xstusd_order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: XSTUSD.into(),
            quote: XOR.into(),
        };

        if !TradingPair::is_trading_pair_enabled(
            &xstusd_order_book_id.dex_id,
            &xstusd_order_book_id.quote,
            &xstusd_order_book_id.base,
        )
        .unwrap()
        {
            assert_ok!(TradingPair::register(
                RawOrigin::Signed(alice()).into(),
                xstusd_order_book_id.dex_id,
                xstusd_order_book_id.quote,
                xstusd_order_book_id.base
            ));
        }

        assert_ok!(OrderBookPallet::create_orderbook(
            RawOrigin::Signed(alice()).into(),
            xstusd_order_book_id
        ));

        // it should work with synthetic base asset - XST as well
        let xst_order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: XST.into(),
            quote: XOR.into(),
        };

        if !TradingPair::is_trading_pair_enabled(
            &xst_order_book_id.dex_id,
            &xst_order_book_id.quote,
            &xst_order_book_id.base,
        )
        .unwrap()
        {
            assert_ok!(TradingPair::register(
                RawOrigin::Signed(alice()).into(),
                xst_order_book_id.dex_id,
                xst_order_book_id.quote,
                xst_order_book_id.base
            ));
        }

        assert_ok!(OrderBookPallet::create_orderbook(
            RawOrigin::Signed(alice()).into(),
            xst_order_book_id
        ));
    });
}

#[test]
fn should_not_create_order_book_with_non_existed_asset() {
    ext().execute_with(|| {
        let wrong_asset = AssetId32::from_bytes(hex!(
            "0123456789012345678901234567890123456789012345678901234567890123"
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: wrong_asset.into(),
            quote: XOR.into(),
        };

        assert_err!(
            OrderBookPallet::create_orderbook(RawOrigin::Signed(alice()).into(), order_book_id),
            assets::Error::<Runtime>::AssetIdNotExists
        );
    });
}

#[test]
fn should_not_create_order_book_with_non_existed_trading_pair() {
    ext().execute_with(|| {
        let caller = alice();
        FrameSystem::inc_providers(&caller);

        let new_asset = Assets::register_from(
            &caller,
            AssetSymbol(b"TEST".to_vec()),
            AssetName(b"Test".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            balance!(100),
            false,
            None,
            None,
        )
        .unwrap();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: new_asset.into(),
            quote: XOR.into(),
        };

        assert_err!(
            OrderBookPallet::create_orderbook(RawOrigin::Signed(caller).into(), order_book_id),
            trading_pair::Error::<Runtime>::TradingPairDoesntExist
        );
    });
}

#[test]
fn should_create_order_book_for_regular_assets() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        assert_ok!(OrderBookPallet::create_orderbook(
            RawOrigin::Signed(alice()).into(),
            order_book_id
        ));

        assert_eq!(
            OrderBookPallet::order_books(order_book_id).unwrap(),
            OrderBook::default(order_book_id)
        );
    });
}

#[test]
fn should_not_create_order_book_that_already_exists() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        assert_ok!(OrderBookPallet::create_orderbook(
            RawOrigin::Signed(alice()).into(),
            order_book_id
        ));

        assert_err!(
            OrderBookPallet::create_orderbook(RawOrigin::Signed(alice()).into(), order_book_id),
            E::OrderBookAlreadyExists
        );
    });
}

#[test]
fn should_not_create_order_book_for_user_without_nft() {
    ext().execute_with(|| {
        let caller = alice();
        let creator = bob();
        FrameSystem::inc_providers(&creator);

        let nft = Assets::register_from(
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
            quote: XOR.into(),
        };

        assert_ok!(TradingPair::register(
            RawOrigin::Signed(creator.clone()).into(),
            DEX.into(),
            order_book_id.quote,
            order_book_id.base
        ));

        assert_err!(
            OrderBookPallet::create_orderbook(RawOrigin::Signed(caller).into(), order_book_id),
            E::UserHasNoNft
        );
    });
}

#[test]
fn should_not_create_order_book_for_nft_owner_without_nft() {
    ext().execute_with(|| {
        let caller = alice();
        let user = bob();
        FrameSystem::inc_providers(&caller);

        let nft = Assets::register_from(
            &caller,
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
            quote: XOR.into(),
        };

        assert_ok!(TradingPair::register(
            RawOrigin::Signed(caller.clone()).into(),
            DEX.into(),
            order_book_id.quote,
            order_book_id.base
        ));

        // caller creates NFT and then send it to another user.
        // That means they cannot create order book with this NFT even they are NFT asset owner
        Assets::transfer(RawOrigin::Signed(caller.clone()).into(), nft, user, 1).unwrap();

        assert_err!(
            OrderBookPallet::create_orderbook(RawOrigin::Signed(caller).into(), order_book_id),
            E::UserHasNoNft
        );
    });
}

#[test]
fn should_create_order_book_for_nft() {
    ext().execute_with(|| {
        let caller = alice();
        let creator = bob();
        FrameSystem::inc_providers(&creator);

        let nft = Assets::register_from(
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

        Assets::transfer(RawOrigin::Signed(creator).into(), nft, caller.clone(), 1).unwrap();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: nft,
            quote: XOR.into(),
        };

        assert_ok!(TradingPair::register(
            RawOrigin::Signed(caller.clone()).into(),
            DEX.into(),
            order_book_id.quote,
            order_book_id.base
        ));

        assert_ok!(OrderBookPallet::create_orderbook(
            RawOrigin::Signed(caller).into(),
            order_book_id
        ));

        assert_eq!(
            OrderBookPallet::order_books(order_book_id).unwrap(),
            OrderBook::default_indivisible(order_book_id)
        );
    });
}

#[test]
fn should_check_permissions_for_delete_order_book() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_empty_order_book(order_book_id);
        assert_err!(
            OrderBookPallet::delete_orderbook(RawOrigin::Signed(alice()).into(), order_book_id),
            BadOrigin
        );

        // Root should be allowed

        assert_ok!(OrderBookPallet::delete_orderbook(
            RawOrigin::Root.into(),
            order_book_id
        ),);

        // Only more than half approvals from technical commitee are accepted

        create_empty_order_book(order_book_id);
        if pallet_collective::Pallet::<Runtime, framenode_runtime::TechnicalCollective>::members()
            .len()
            > 1
        {
            assert_err!(
                OrderBookPallet::delete_orderbook(
                    TechnicalRawOrigin::Member(alice()).into(),
                    order_book_id
                ),
                BadOrigin,
            );
        } else {
            assert_ok!(OrderBookPallet::delete_orderbook(
                TechnicalRawOrigin::Member(alice()).into(),
                order_book_id
            ),);
            create_empty_order_book(order_book_id);
        }
        assert_err!(
            OrderBookPallet::delete_orderbook(
                TechnicalRawOrigin::Members(3, 6).into(),
                order_book_id
            ),
            BadOrigin
        );
        assert_ok!(OrderBookPallet::delete_orderbook(
            TechnicalRawOrigin::Members(4, 6).into(),
            order_book_id
        ));
    });
}

#[test]
fn should_not_delete_unknown_order_book() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        assert_err!(
            OrderBookPallet::delete_orderbook(RuntimeOrigin::root(), order_book_id),
            E::UnknownOrderBook
        );
    });
}

#[test]
fn should_delete_order_book() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id);
        let owner = bob();

        let tech_account = technical::Pallet::<Runtime>::tech_account_id_to_account_id(
            &OrderBookPallet::tech_account_for_order_book(order_book_id.clone()),
        )
        .unwrap();

        // not empty at the beginning
        assert!(!OrderBookPallet::aggregated_bids(&order_book_id).is_empty());
        assert!(!OrderBookPallet::aggregated_asks(&order_book_id).is_empty());
        assert!(!OrderBookPallet::user_limit_orders(&owner, &order_book_id)
            .unwrap()
            .is_empty());

        // some balance is locked in limit orders
        assert_ne!(free_balance(&order_book_id.base, &owner), INIT_BALANCE);
        assert_ne!(free_balance(&order_book_id.quote, &owner), INIT_BALANCE);

        // tech account keeps the locked assets
        assert!(free_balance(&order_book_id.base, &tech_account) > balance!(0));
        assert!(free_balance(&order_book_id.quote, &tech_account) > balance!(0));

        // delete the order book
        assert_ok!(OrderBookPallet::delete_orderbook(
            RuntimeOrigin::root(),
            order_book_id
        ));

        // empty after canceling of all limit orders
        assert!(OrderBookPallet::aggregated_bids(&order_book_id).is_empty());
        assert!(OrderBookPallet::aggregated_asks(&order_book_id).is_empty());
        assert_eq!(
            OrderBookPallet::user_limit_orders(&owner, &order_book_id),
            None
        );

        // locked balance is unlocked
        assert_eq!(free_balance(&order_book_id.base, &owner), INIT_BALANCE);
        assert_eq!(free_balance(&order_book_id.quote, &owner), INIT_BALANCE);

        // tech account balance is empty after canceling of all limit orders
        assert_eq!(
            free_balance(&order_book_id.base, &tech_account),
            balance!(0)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &tech_account),
            balance!(0)
        );
    });
}

#[test]
#[ignore] // it works, but takes a lot of time
fn should_delete_order_book_with_a_lot_of_orders() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        assert_ok!(OrderBookPallet::create_orderbook(
            RawOrigin::Signed(alice()).into(),
            order_book_id
        ));

        let order_book = OrderBookPallet::order_books(order_book_id).unwrap();

        let mut buy_price: OrderPrice = balance!(1000).into();
        let mut buy_lifespan = 10000; // ms
        let mut sell_price: OrderPrice = balance!(1001).into();
        let mut sell_lifespan = 10000; // ms

        let max_prices_for_side: u32 = <Runtime as Config>::MaxSidePriceCount::get();

        for i in 0..max_prices_for_side {
            // get new owner for each order to not get UserHasMaxCountOfOpenedOrders error
            let account = generate_account(i);

            fill_balance(account.clone(), order_book_id);

            buy_price -= order_book.tick_size;
            sell_price += order_book.tick_size;
            buy_lifespan += 5000;
            sell_lifespan += 5000;

            assert_ok!(OrderBookPallet::place_limit_order(
                RawOrigin::Signed(account.clone()).into(),
                order_book_id,
                *buy_price.balance(),
                balance!(10),
                PriceVariant::Buy,
                Some(buy_lifespan)
            ));

            assert_ok!(OrderBookPallet::place_limit_order(
                RawOrigin::Signed(account).into(),
                order_book_id,
                *sell_price.balance(),
                balance!(10),
                PriceVariant::Sell,
                Some(sell_lifespan)
            ));
        }

        // delete the order book
        assert_ok!(OrderBookPallet::delete_orderbook(
            RuntimeOrigin::root(),
            order_book_id
        ));
    });
}

#[test]
fn should_check_permissions_for_update_order_book() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_empty_order_book(order_book_id);

        assert_err!(
            OrderBookPallet::update_orderbook(
                RawOrigin::Signed(alice()).into(),
                order_book_id,
                balance!(0.01),
                balance!(0.001),
                balance!(1),
                balance!(10000)
            ),
            BadOrigin
        );

        // Root should be allowed

        assert_ok!(OrderBookPallet::update_orderbook(
            RawOrigin::Root.into(),
            order_book_id,
            balance!(0.01),
            balance!(0.001),
            balance!(1),
            balance!(10000)
        ),);

        // Only more than half approvals from technical commitee are accepted

        if pallet_collective::Pallet::<Runtime, framenode_runtime::TechnicalCollective>::members()
            .len()
            > 1
        {
            assert_err!(
                OrderBookPallet::update_orderbook(
                    TechnicalRawOrigin::Member(alice()).into(),
                    order_book_id,
                    balance!(0.01),
                    balance!(0.001),
                    balance!(1),
                    balance!(10000)
                ),
                BadOrigin,
            );
        } else {
            assert_ok!(OrderBookPallet::update_orderbook(
                TechnicalRawOrigin::Member(alice()).into(),
                order_book_id,
                balance!(0.01),
                balance!(0.001),
                balance!(1),
                balance!(10000)
            ),);
            create_empty_order_book(order_book_id);
        }
        assert_err!(
            OrderBookPallet::update_orderbook(
                TechnicalRawOrigin::Members(3, 6).into(),
                order_book_id,
                balance!(0.01),
                balance!(0.001),
                balance!(1),
                balance!(10000)
            ),
            BadOrigin
        );
        assert_ok!(OrderBookPallet::update_orderbook(
            TechnicalRawOrigin::Members(4, 6).into(),
            order_book_id,
            balance!(0.01),
            balance!(0.001),
            balance!(1),
            balance!(10000)
        ),);

        // nft to have custom owner of quote asset
        FrameSystem::inc_providers(&bob());

        let nft = Assets::register_from(
            &bob(),
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            100,
            false,
            None,
            None,
        )
        .unwrap();
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: nft,
            quote: XOR.into(),
        };
        assert_ok!(TradingPair::register(
            RawOrigin::Signed(bob()).into(),
            DEX.into(),
            order_book_id.quote,
            order_book_id.base
        ));
        create_empty_order_book(order_book_id);

        let asset_owner_base = Assets::asset_owner(&order_book_id.base).unwrap();
        let asset_owner_quote = Assets::asset_owner(&order_book_id.quote).unwrap();
        assert_ok!(OrderBookPallet::update_orderbook(
            RawOrigin::Signed(asset_owner_base).into(),
            order_book_id,
            balance!(0.01),
            2,
            4,
            100,
        ),);
        assert_err!(
            OrderBookPallet::update_orderbook(
                RawOrigin::Signed(asset_owner_quote).into(),
                order_book_id,
                balance!(0.01),
                2,
                4,
                100,
            ),
            BadOrigin
        );
        assert_err!(
            OrderBookPallet::update_orderbook(
                RawOrigin::Signed(alice()).into(),
                order_book_id,
                balance!(0.01),
                2,
                4,
                100,
            ),
            BadOrigin
        );
    });
}

#[test]
fn should_not_update_unknown_order_book() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        assert_err!(
            OrderBookPallet::update_orderbook(
                RuntimeOrigin::root(),
                order_book_id,
                balance!(0.01),
                balance!(0.001),
                balance!(1),
                balance!(10000)
            ),
            E::UnknownOrderBook
        );
    });
}

#[test]
fn should_not_update_order_book_with_zero_attributes() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_empty_order_book(order_book_id);

        let tick_size = balance!(0.01);
        let step_lot_size = balance!(0.001);
        let min_lot_size = balance!(1);
        let max_lot_size = balance!(10000);

        assert_err!(
            OrderBookPallet::update_orderbook(
                RuntimeOrigin::root(),
                order_book_id,
                0,
                step_lot_size,
                min_lot_size,
                max_lot_size
            ),
            E::InvalidTickSize
        );

        assert_err!(
            OrderBookPallet::update_orderbook(
                RuntimeOrigin::root(),
                order_book_id,
                tick_size,
                0,
                min_lot_size,
                max_lot_size
            ),
            E::InvalidStepLotSize
        );

        assert_err!(
            OrderBookPallet::update_orderbook(
                RuntimeOrigin::root(),
                order_book_id,
                tick_size,
                step_lot_size,
                0,
                max_lot_size
            ),
            E::InvalidMinLotSize
        );

        assert_err!(
            OrderBookPallet::update_orderbook(
                RuntimeOrigin::root(),
                order_book_id,
                tick_size,
                step_lot_size,
                min_lot_size,
                0
            ),
            E::InvalidMaxLotSize
        );
    });
}

#[test]
fn should_not_update_order_book_with_simple_mistakes() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_empty_order_book(order_book_id);

        // min > max
        assert_err!(
            OrderBookPallet::update_orderbook(
                RuntimeOrigin::root(),
                order_book_id,
                balance!(0.01),
                balance!(0.001),
                balance!(100),
                balance!(10)
            ),
            E::InvalidMaxLotSize
        );

        // min & max couldn't be less then `step_lot_size`
        assert_err!(
            OrderBookPallet::update_orderbook(
                RuntimeOrigin::root(),
                order_book_id,
                balance!(0.01),
                balance!(0.001),
                balance!(0.0001),
                balance!(10000)
            ),
            E::InvalidMinLotSize
        );
        assert_err!(
            OrderBookPallet::update_orderbook(
                RuntimeOrigin::root(),
                order_book_id,
                balance!(0.01),
                balance!(0.001),
                balance!(1),
                balance!(0.0001)
            ),
            E::InvalidMaxLotSize
        );

        // min & max must be a multiple of `step_lot_size`
        assert_err!(
            OrderBookPallet::update_orderbook(
                RuntimeOrigin::root(),
                order_book_id,
                balance!(0.01),
                balance!(0.001),
                balance!(1.0001),
                balance!(10000)
            ),
            E::InvalidMinLotSize
        );
        assert_err!(
            OrderBookPallet::update_orderbook(
                RuntimeOrigin::root(),
                order_book_id,
                balance!(0.01),
                balance!(0.001),
                balance!(1),
                balance!(10000.00001)
            ),
            E::InvalidMaxLotSize
        );
    });
}

#[test]
fn should_not_update_order_book_with_wrong_min_deal_amount() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_empty_order_book(order_book_id);

        assert_err!(
            OrderBookPallet::update_orderbook(
                RuntimeOrigin::root(),
                order_book_id,
                balance!(1000000000000),
                balance!(1000000000000),
                balance!(10000000000000),
                balance!(100000000000000)
            ),
            E::TickSizeAndStepLotSizeAreTooBig
        );

        assert_err!(
            OrderBookPallet::update_orderbook(
                RuntimeOrigin::root(),
                order_book_id,
                balance!(0.0000000001),
                balance!(0.0000000001),
                balance!(1),
                balance!(10000)
            ),
            E::TickSizeAndStepLotSizeLosePrecision
        );

        assert_err!(
            OrderBookPallet::update_orderbook(
                RuntimeOrigin::root(),
                order_book_id,
                balance!(0.000000000000000001),
                balance!(0.1),
                balance!(1),
                balance!(10000)
            ),
            E::TickSizeAndStepLotSizeLosePrecision
        );

        assert_err!(
            OrderBookPallet::update_orderbook(
                RuntimeOrigin::root(),
                order_book_id,
                balance!(0.1),
                balance!(0.000000000000000001),
                balance!(1),
                balance!(10000)
            ),
            E::TickSizeAndStepLotSizeLosePrecision
        );

        assert_err!(
            OrderBookPallet::update_orderbook(
                RuntimeOrigin::root(),
                order_book_id,
                balance!(0.1),
                balance!(5.000000000000000001),
                balance!(10.000000000000000002), // should be a multiple of step_lot_size
                balance!(10000.000000000000002)  // should be a multiple of step_lot_size
            ),
            E::TickSizeAndStepLotSizeLosePrecision
        );

        assert_err!(
            OrderBookPallet::update_orderbook(
                RuntimeOrigin::root(),
                order_book_id,
                balance!(5.000000000000000001),
                balance!(0.1),
                balance!(1),
                balance!(10000)
            ),
            E::TickSizeAndStepLotSizeLosePrecision
        );

        assert_err!(
            OrderBookPallet::update_orderbook(
                RuntimeOrigin::root(),
                order_book_id,
                balance!(5.000000001),
                balance!(5.0000000001),
                balance!(10.0000000002), // should be a multiple of step_lot_size
                balance!(10000.0000002)  // should be a multiple of step_lot_size
            ),
            E::TickSizeAndStepLotSizeLosePrecision
        );
    });
}

#[test]
fn should_not_update_order_book_when_attributes_exceed_total_supply() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_empty_order_book(order_book_id);

        assert_err!(
            OrderBookPallet::update_orderbook(
                RuntimeOrigin::root(),
                order_book_id,
                balance!(0.01),
                balance!(0.001),
                balance!(1),
                balance!(10000000000)
            ),
            E::MaxLotSizeIsMoreThanTotalSupply
        );
    });
}

#[test]
fn should_update_order_book_with_regular_asset() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id);

        let limit_order1 = OrderBookPallet::limit_orders(order_book_id, 1).unwrap();
        let limit_order2 = OrderBookPallet::limit_orders(order_book_id, 2).unwrap();
        let limit_order3 = OrderBookPallet::limit_orders(order_book_id, 3).unwrap();
        let limit_order4 = OrderBookPallet::limit_orders(order_book_id, 4).unwrap();
        let limit_order5 = OrderBookPallet::limit_orders(order_book_id, 5).unwrap();
        let limit_order6 = OrderBookPallet::limit_orders(order_book_id, 6).unwrap();
        let limit_order7 = OrderBookPallet::limit_orders(order_book_id, 7).unwrap();
        let limit_order8 = OrderBookPallet::limit_orders(order_book_id, 8).unwrap();
        let limit_order9 = OrderBookPallet::limit_orders(order_book_id, 9).unwrap();
        let limit_order10 = OrderBookPallet::limit_orders(order_book_id, 10).unwrap();
        let limit_order11 = OrderBookPallet::limit_orders(order_book_id, 11).unwrap();
        let limit_order12 = OrderBookPallet::limit_orders(order_book_id, 12).unwrap();

        // check amounts before update
        assert_eq!(limit_order1.amount, balance!(168.5).into());
        assert_eq!(limit_order2.amount, balance!(95.2).into());
        assert_eq!(limit_order3.amount, balance!(44.7).into());
        assert_eq!(limit_order4.amount, balance!(56.4).into());
        assert_eq!(limit_order5.amount, balance!(89.9).into());
        assert_eq!(limit_order6.amount, balance!(115).into());
        assert_eq!(limit_order7.amount, balance!(176.3).into());
        assert_eq!(limit_order8.amount, balance!(85.4).into());
        assert_eq!(limit_order9.amount, balance!(93.2).into());
        assert_eq!(limit_order10.amount, balance!(36.6).into());
        assert_eq!(limit_order11.amount, balance!(205.5).into());
        assert_eq!(limit_order12.amount, balance!(13.7).into());

        let tick_size = balance!(0.01);
        let step_lot_size = balance!(0.001);
        let min_lot_size = balance!(1);
        let max_lot_size = balance!(10000);

        assert_ok!(OrderBookPallet::update_orderbook(
            RuntimeOrigin::root(),
            order_book_id,
            tick_size,
            step_lot_size,
            min_lot_size,
            max_lot_size
        ));

        let order_book = OrderBookPallet::order_books(order_book_id).unwrap();

        // check new attributes
        assert_eq!(order_book.tick_size, tick_size.into());
        assert_eq!(order_book.step_lot_size, step_lot_size.into());
        assert_eq!(order_book.min_lot_size, min_lot_size.into());
        assert_eq!(order_book.max_lot_size, max_lot_size.into());

        let limit_order1 = OrderBookPallet::limit_orders(order_book_id, 1).unwrap();
        let limit_order2 = OrderBookPallet::limit_orders(order_book_id, 2).unwrap();
        let limit_order3 = OrderBookPallet::limit_orders(order_book_id, 3).unwrap();
        let limit_order4 = OrderBookPallet::limit_orders(order_book_id, 4).unwrap();
        let limit_order5 = OrderBookPallet::limit_orders(order_book_id, 5).unwrap();
        let limit_order6 = OrderBookPallet::limit_orders(order_book_id, 6).unwrap();
        let limit_order7 = OrderBookPallet::limit_orders(order_book_id, 7).unwrap();
        let limit_order8 = OrderBookPallet::limit_orders(order_book_id, 8).unwrap();
        let limit_order9 = OrderBookPallet::limit_orders(order_book_id, 9).unwrap();
        let limit_order10 = OrderBookPallet::limit_orders(order_book_id, 10).unwrap();
        let limit_order11 = OrderBookPallet::limit_orders(order_book_id, 11).unwrap();
        let limit_order12 = OrderBookPallet::limit_orders(order_book_id, 12).unwrap();

        // check that amounts are not changed after update
        // because they are suitable for new step_lot_size
        assert_eq!(limit_order1.amount, balance!(168.5).into());
        assert_eq!(limit_order2.amount, balance!(95.2).into());
        assert_eq!(limit_order3.amount, balance!(44.7).into());
        assert_eq!(limit_order4.amount, balance!(56.4).into());
        assert_eq!(limit_order5.amount, balance!(89.9).into());
        assert_eq!(limit_order6.amount, balance!(115).into());
        assert_eq!(limit_order7.amount, balance!(176.3).into());
        assert_eq!(limit_order8.amount, balance!(85.4).into());
        assert_eq!(limit_order9.amount, balance!(93.2).into());
        assert_eq!(limit_order10.amount, balance!(36.6).into());
        assert_eq!(limit_order11.amount, balance!(205.5).into());
        assert_eq!(limit_order12.amount, balance!(13.7).into());
    });
}

#[test]
fn should_update_order_book_with_nft() {
    ext().execute_with(|| {
        FrameSystem::inc_providers(&alice());

        let nft = Assets::register_from(
            &alice(),
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            100,
            false,
            None,
            None,
        )
        .unwrap();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: nft,
            quote: XOR.into(),
        };

        assert_ok!(TradingPair::register(
            RawOrigin::Signed(alice()).into(),
            DEX.into(),
            order_book_id.quote,
            order_book_id.base
        ));

        assert_ok!(OrderBookPallet::create_orderbook(
            RawOrigin::Signed(alice()).into(),
            order_book_id
        ));

        let tick_size = OrderPrice::divisible(balance!(0.01));
        let step_lot_size = OrderVolume::indivisible(2);
        let min_lot_size = OrderVolume::indivisible(4);
        let max_lot_size = OrderVolume::indivisible(100);

        assert_ok!(OrderBookPallet::update_orderbook(
            RuntimeOrigin::root(),
            order_book_id,
            *tick_size.balance(),
            *step_lot_size.balance(),
            *min_lot_size.balance(),
            *max_lot_size.balance()
        ));

        let order_book = OrderBookPallet::order_books(order_book_id).unwrap();

        assert_eq!(order_book.tick_size, tick_size);
        assert_eq!(order_book.step_lot_size, step_lot_size);
        assert_eq!(order_book.min_lot_size, min_lot_size);
        assert_eq!(order_book.max_lot_size, max_lot_size);
    });
}

#[test]
fn should_align_limit_orders_when_update_order_book() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id);

        let limit_order1 = OrderBookPallet::limit_orders(order_book_id, 1).unwrap();
        let limit_order2 = OrderBookPallet::limit_orders(order_book_id, 2).unwrap();
        let limit_order3 = OrderBookPallet::limit_orders(order_book_id, 3).unwrap();
        let limit_order4 = OrderBookPallet::limit_orders(order_book_id, 4).unwrap();
        let limit_order5 = OrderBookPallet::limit_orders(order_book_id, 5).unwrap();
        let limit_order6 = OrderBookPallet::limit_orders(order_book_id, 6).unwrap();
        let limit_order7 = OrderBookPallet::limit_orders(order_book_id, 7).unwrap();
        let limit_order8 = OrderBookPallet::limit_orders(order_book_id, 8).unwrap();
        let limit_order9 = OrderBookPallet::limit_orders(order_book_id, 9).unwrap();
        let limit_order10 = OrderBookPallet::limit_orders(order_book_id, 10).unwrap();
        let limit_order11 = OrderBookPallet::limit_orders(order_book_id, 11).unwrap();
        let limit_order12 = OrderBookPallet::limit_orders(order_book_id, 12).unwrap();

        // check that amounts are original before align
        assert_eq!(limit_order1.amount, balance!(168.5).into());
        assert_eq!(limit_order2.amount, balance!(95.2).into());
        assert_eq!(limit_order3.amount, balance!(44.7).into());
        assert_eq!(limit_order4.amount, balance!(56.4).into());
        assert_eq!(limit_order5.amount, balance!(89.9).into());
        assert_eq!(limit_order6.amount, balance!(115).into());
        assert_eq!(limit_order7.amount, balance!(176.3).into());
        assert_eq!(limit_order8.amount, balance!(85.4).into());
        assert_eq!(limit_order9.amount, balance!(93.2).into());
        assert_eq!(limit_order10.amount, balance!(36.6).into());
        assert_eq!(limit_order11.amount, balance!(205.5).into());
        assert_eq!(limit_order12.amount, balance!(13.7).into());

        // get balances before align
        let bob_base_balance = free_balance(&order_book_id.base, &bob());
        let bob_quote_balance = free_balance(&order_book_id.quote, &bob());
        let charlie_base_balance = free_balance(&order_book_id.base, &charlie());
        let charlie_quote_balance = free_balance(&order_book_id.quote, &charlie());

        let tick_size = balance!(0.01);
        let step_lot_size = balance!(1); // change lot size precision
        let min_lot_size = balance!(1);
        let max_lot_size = balance!(10000);

        assert_ok!(OrderBookPallet::update_orderbook(
            RuntimeOrigin::root(),
            order_book_id,
            tick_size,
            step_lot_size,
            min_lot_size,
            max_lot_size
        ));

        let limit_order1 = OrderBookPallet::limit_orders(order_book_id, 1).unwrap();
        let limit_order2 = OrderBookPallet::limit_orders(order_book_id, 2).unwrap();
        let limit_order3 = OrderBookPallet::limit_orders(order_book_id, 3).unwrap();
        let limit_order4 = OrderBookPallet::limit_orders(order_book_id, 4).unwrap();
        let limit_order5 = OrderBookPallet::limit_orders(order_book_id, 5).unwrap();
        let limit_order6 = OrderBookPallet::limit_orders(order_book_id, 6).unwrap();
        let limit_order7 = OrderBookPallet::limit_orders(order_book_id, 7).unwrap();
        let limit_order8 = OrderBookPallet::limit_orders(order_book_id, 8).unwrap();
        let limit_order9 = OrderBookPallet::limit_orders(order_book_id, 9).unwrap();
        let limit_order10 = OrderBookPallet::limit_orders(order_book_id, 10).unwrap();
        let limit_order11 = OrderBookPallet::limit_orders(order_book_id, 11).unwrap();
        let limit_order12 = OrderBookPallet::limit_orders(order_book_id, 12).unwrap();

        // check that amouts are aligned
        assert_eq!(limit_order1.amount, balance!(168).into());
        assert_eq!(limit_order2.amount, balance!(95).into());
        assert_eq!(limit_order3.amount, balance!(44).into());
        assert_eq!(limit_order4.amount, balance!(56).into());
        assert_eq!(limit_order5.amount, balance!(89).into());
        assert_eq!(limit_order6.amount, balance!(115).into());
        assert_eq!(limit_order7.amount, balance!(176).into());
        assert_eq!(limit_order8.amount, balance!(85).into());
        assert_eq!(limit_order9.amount, balance!(93).into());
        assert_eq!(limit_order10.amount, balance!(36).into());
        assert_eq!(limit_order11.amount, balance!(205).into());
        assert_eq!(limit_order12.amount, balance!(13).into());

        // check dust refund
        assert_eq!(
            free_balance(&order_book_id.base, &bob()),
            bob_base_balance + balance!(1)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &bob()),
            bob_quote_balance + balance!(20.41)
        );
        assert_eq!(
            free_balance(&order_book_id.base, &charlie()),
            charlie_base_balance + balance!(1.7)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &charlie()),
            charlie_quote_balance + balance!(5.76)
        );
    });
}

#[test]
fn should_check_permissions_for_change_order_book_status() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_empty_order_book(order_book_id);
        assert_err!(
            OrderBookPallet::change_orderbook_status(
                RawOrigin::Signed(alice()).into(),
                order_book_id,
                OrderBookStatus::Trade
            ),
            BadOrigin
        );

        // Root should be allowed

        assert_ok!(OrderBookPallet::change_orderbook_status(
            RawOrigin::Root.into(),
            order_book_id,
            OrderBookStatus::Trade
        ),);

        // Only more than half approvals from technical commitee are accepted

        if pallet_collective::Pallet::<Runtime, framenode_runtime::TechnicalCollective>::members()
            .len()
            > 1
        {
            assert_err!(
                OrderBookPallet::change_orderbook_status(
                    TechnicalRawOrigin::Member(alice()).into(),
                    order_book_id,
                    OrderBookStatus::Trade
                ),
                BadOrigin,
            );
        } else {
            assert_ok!(OrderBookPallet::change_orderbook_status(
                TechnicalRawOrigin::Member(alice()).into(),
                order_book_id,
                OrderBookStatus::Trade
            ),);
        }
        assert_err!(
            OrderBookPallet::change_orderbook_status(
                TechnicalRawOrigin::Members(3, 6).into(),
                order_book_id,
                OrderBookStatus::Trade
            ),
            BadOrigin
        );
        assert_ok!(OrderBookPallet::change_orderbook_status(
            TechnicalRawOrigin::Members(4, 6).into(),
            order_book_id,
            OrderBookStatus::Trade
        ),);
    });
}

#[test]
fn should_not_change_status_of_unknown_order_book() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        assert_err!(
            OrderBookPallet::change_orderbook_status(
                RuntimeOrigin::root(),
                order_book_id,
                OrderBookStatus::Trade
            ),
            E::UnknownOrderBook
        );
    });
}

#[test]
fn should_change_order_book_status() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_empty_order_book(order_book_id);

        assert_eq!(
            OrderBookPallet::order_books(order_book_id).unwrap().status,
            OrderBookStatus::Trade
        );

        assert_ok!(OrderBookPallet::change_orderbook_status(
            RuntimeOrigin::root(),
            order_book_id,
            OrderBookStatus::Stop
        ));

        assert_eq!(
            OrderBookPallet::order_books(order_book_id).unwrap().status,
            OrderBookStatus::Stop
        );

        assert_ok!(OrderBookPallet::change_orderbook_status(
            RuntimeOrigin::root(),
            order_book_id,
            OrderBookStatus::OnlyCancel
        ));

        assert_eq!(
            OrderBookPallet::order_books(order_book_id).unwrap().status,
            OrderBookStatus::OnlyCancel
        );

        assert_ok!(OrderBookPallet::change_orderbook_status(
            RuntimeOrigin::root(),
            order_book_id,
            OrderBookStatus::PlaceAndCancel
        ));

        assert_eq!(
            OrderBookPallet::order_books(order_book_id).unwrap().status,
            OrderBookStatus::PlaceAndCancel
        );

        assert_ok!(OrderBookPallet::change_orderbook_status(
            RuntimeOrigin::root(),
            order_book_id,
            OrderBookStatus::Trade
        ));

        assert_eq!(
            OrderBookPallet::order_books(order_book_id).unwrap().status,
            OrderBookStatus::Trade
        );
    });
}

#[test]
fn should_not_place_limit_order_in_unknown_order_book() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        assert_err!(
            OrderBookPallet::place_limit_order(
                RawOrigin::Signed(alice()).into(),
                order_book_id,
                balance!(10),
                balance!(100),
                PriceVariant::Buy,
                Some(1000)
            ),
            E::UnknownOrderBook
        );
    });
}

#[test]
fn should_place_limit_order() {
    ext().execute_with(|| {
        let caller = alice();
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id);
        fill_balance(caller.clone(), order_book_id);

        let price: OrderPrice = balance!(10).into();
        let amount: OrderVolume = balance!(100).into();
        let lifespan = 10000;
        let now = 1234;
        let current_block = frame_system::Pallet::<Runtime>::block_number();

        Timestamp::set_timestamp(now);

        // fix state before
        let bids_before = OrderBookPallet::bids(&order_book_id, &price).unwrap_or_default();
        let agg_bids_before = OrderBookPallet::aggregated_bids(&order_book_id);
        let price_volume_before = agg_bids_before.get(&price).cloned().unwrap_or_default();
        let user_orders_before =
            OrderBookPallet::user_limit_orders(&caller, &order_book_id).unwrap_or_default();
        let balance_before = free_balance(&order_book_id.quote, &caller);

        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(caller.clone()).into(),
            order_book_id,
            *price.balance(),
            *amount.balance(),
            PriceVariant::Buy,
            Some(lifespan)
        ));

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

        let mut expected_bids = bids_before.clone();
        assert_ok!(expected_bids.try_push(order_id));
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &price).unwrap(),
            expected_bids
        );

        let expected_price_volume = price_volume_before + amount;
        let mut expected_agg_bids = agg_bids_before.clone();
        assert_ok!(expected_agg_bids.try_insert(price, expected_price_volume));
        assert_eq!(
            OrderBookPallet::aggregated_bids(&order_book_id),
            expected_agg_bids
        );

        let mut expected_user_orders = user_orders_before.clone();
        assert_ok!(expected_user_orders.try_push(order_id));
        assert_eq!(
            OrderBookPallet::user_limit_orders(&caller, &order_book_id).unwrap(),
            expected_user_orders
        );

        let balance = free_balance(&order_book_id.quote, &caller);
        let expected_balance = balance_before - deal_amount.balance();
        assert_eq!(balance, expected_balance);
    });
}

#[test]
fn should_place_limit_order_with_nft() {
    ext().execute_with(|| {
        let caller = alice();
        frame_system::Pallet::<Runtime>::inc_providers(&caller);

        let nft = Assets::register_from(
            &caller,
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            1,
            false,
            None,
            None,
        )
        .unwrap();

        assert_ok!(assets::Pallet::<Runtime>::update_balance(
            RuntimeOrigin::root(),
            caller.clone(),
            XOR,
            INIT_BALANCE.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: nft,
            quote: XOR.into(),
        };

        assert_ok!(TradingPair::register(
            RawOrigin::Signed(caller.clone()).into(),
            DEX.into(),
            order_book_id.quote,
            order_book_id.base
        ));

        assert_ok!(OrderBookPallet::create_orderbook(
            RawOrigin::Signed(caller.clone()).into(),
            order_book_id
        ));

        let price: OrderPrice = balance!(10).into();
        let amount = OrderVolume::indivisible(1);
        let lifespan = 10000;
        let now = 1234;
        let current_block = frame_system::Pallet::<Runtime>::block_number();

        Timestamp::set_timestamp(now);

        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(caller.clone()).into(),
            order_book_id,
            *price.balance(),
            *amount.balance(),
            PriceVariant::Sell,
            Some(lifespan)
        ));

        let order_id = get_last_order_id(order_book_id).unwrap();

        // check
        let expected_order = LimitOrder::<Runtime>::new(
            order_id,
            caller.clone(),
            PriceVariant::Sell,
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

        assert_eq!(
            OrderBookPallet::asks(&order_book_id, &price).unwrap(),
            vec![order_id]
        );
        assert_eq!(
            OrderBookPallet::aggregated_asks(&order_book_id),
            BTreeMap::from([(price, amount)])
        );
        assert_eq!(
            OrderBookPallet::user_limit_orders(&caller, &order_book_id).unwrap(),
            vec![order_id]
        );

        let balance = free_balance(&order_book_id.base, &caller);
        assert_eq!(balance, balance!(0));
    });
}

#[test]
fn should_place_limit_order_out_of_spread() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id);
        fill_balance(alice(), order_book_id);

        let now = 1234;
        Timestamp::set_timestamp(now);

        let lifespan = 100000;

        let bid_price1: OrderPrice = balance!(10).into();
        let bid_price2: OrderPrice = balance!(9.8).into();
        let bid_price3: OrderPrice = balance!(9.5).into();
        let new_bid_price: OrderPrice = balance!(11.1).into();

        let ask_price1: OrderPrice = balance!(11).into();
        let ask_price2: OrderPrice = balance!(11.2).into();
        let ask_price3: OrderPrice = balance!(11.5).into();
        let new_ask_price: OrderPrice = balance!(9.9).into();

        // check state before

        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price1).unwrap(),
            vec![1]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price2).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(
            OrderBookPallet::asks(&order_book_id, &ask_price1).unwrap(),
            vec![7]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id, &ask_price2).unwrap(),
            vec![8, 9]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id, &ask_price3).unwrap(),
            vec![10, 11, 12]
        );

        assert_eq!(
            OrderBookPallet::aggregated_bids(&order_book_id),
            BTreeMap::from([
                (bid_price1, balance!(168.5).into()),
                (bid_price2, balance!(139.9).into()),
                (bid_price3, balance!(261.3).into())
            ])
        );
        assert_eq!(
            OrderBookPallet::aggregated_asks(&order_book_id),
            BTreeMap::from([
                (ask_price1, balance!(176.3).into()),
                (ask_price2, balance!(178.6).into()),
                (ask_price3, balance!(255.8).into())
            ])
        );

        // buy order 1
        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(alice()).into(),
            order_book_id,
            *new_bid_price.balance(),
            balance!(26.3),
            PriceVariant::Buy,
            Some(lifespan)
        ));

        // check state

        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price1).unwrap(),
            vec![1]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price2).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(
            OrderBookPallet::asks(&order_book_id, &ask_price1).unwrap(),
            vec![7]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id, &ask_price2).unwrap(),
            vec![8, 9]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id, &ask_price3).unwrap(),
            vec![10, 11, 12]
        );

        assert_eq!(
            OrderBookPallet::aggregated_bids(&order_book_id),
            BTreeMap::from([
                (bid_price1, balance!(168.5).into()),
                (bid_price2, balance!(139.9).into()),
                (bid_price3, balance!(261.3).into())
            ])
        );
        assert_eq!(
            OrderBookPallet::aggregated_asks(&order_book_id),
            BTreeMap::from([
                (ask_price1, balance!(150).into()),
                (ask_price2, balance!(178.6).into()),
                (ask_price3, balance!(255.8).into())
            ])
        );

        // buy order 2
        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(alice()).into(),
            order_book_id,
            *new_bid_price.balance(),
            balance!(300),
            PriceVariant::Buy,
            Some(lifespan)
        ));

        // check state

        let buy_order_id2 = get_last_order_id(order_book_id).unwrap();

        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &new_bid_price).unwrap(),
            vec![buy_order_id2]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price1).unwrap(),
            vec![1]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price2).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(OrderBookPallet::asks(&order_book_id, &ask_price1), None);
        assert_eq!(
            OrderBookPallet::asks(&order_book_id, &ask_price2).unwrap(),
            vec![8, 9]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id, &ask_price3).unwrap(),
            vec![10, 11, 12]
        );

        assert_eq!(
            OrderBookPallet::aggregated_bids(&order_book_id),
            BTreeMap::from([
                (new_bid_price, balance!(150).into()),
                (bid_price1, balance!(168.5).into()),
                (bid_price2, balance!(139.9).into()),
                (bid_price3, balance!(261.3).into())
            ])
        );
        assert_eq!(
            OrderBookPallet::aggregated_asks(&order_book_id),
            BTreeMap::from([
                (ask_price2, balance!(178.6).into()),
                (ask_price3, balance!(255.8).into())
            ])
        );

        // cancel limit order
        assert_ok!(OrderBookPallet::cancel_limit_order(
            RawOrigin::Signed(alice()).into(),
            order_book_id,
            buy_order_id2
        ));

        // sell order 1
        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(alice()).into(),
            order_book_id,
            *new_ask_price.balance(),
            balance!(18.5),
            PriceVariant::Sell,
            Some(lifespan)
        ));

        // check state

        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price1).unwrap(),
            vec![1]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price2).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(OrderBookPallet::asks(&order_book_id, &ask_price1), None);
        assert_eq!(
            OrderBookPallet::asks(&order_book_id, &ask_price2).unwrap(),
            vec![8, 9]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id, &ask_price3).unwrap(),
            vec![10, 11, 12]
        );

        assert_eq!(
            OrderBookPallet::aggregated_bids(&order_book_id),
            BTreeMap::from([
                (bid_price1, balance!(150).into()),
                (bid_price2, balance!(139.9).into()),
                (bid_price3, balance!(261.3).into())
            ])
        );
        assert_eq!(
            OrderBookPallet::aggregated_asks(&order_book_id),
            BTreeMap::from([
                (ask_price2, balance!(178.6).into()),
                (ask_price3, balance!(255.8).into())
            ])
        );

        // sell order 2
        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(alice()).into(),
            order_book_id,
            *new_ask_price.balance(),
            balance!(300),
            PriceVariant::Sell,
            Some(lifespan)
        ));

        // check state

        let sell_order_id2 = get_last_order_id(order_book_id).unwrap();

        assert_eq!(OrderBookPallet::bids(&order_book_id, &bid_price1), None);
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price2).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(
            OrderBookPallet::asks(&order_book_id, &new_ask_price).unwrap(),
            vec![sell_order_id2]
        );
        assert_eq!(OrderBookPallet::asks(&order_book_id, &ask_price1), None);
        assert_eq!(
            OrderBookPallet::asks(&order_book_id, &ask_price2).unwrap(),
            vec![8, 9]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id, &ask_price3).unwrap(),
            vec![10, 11, 12]
        );

        assert_eq!(
            OrderBookPallet::aggregated_bids(&order_book_id),
            BTreeMap::from([
                (bid_price2, balance!(139.9).into()),
                (bid_price3, balance!(261.3).into())
            ])
        );
        assert_eq!(
            OrderBookPallet::aggregated_asks(&order_book_id),
            BTreeMap::from([
                (new_ask_price, balance!(150).into()),
                (ask_price2, balance!(178.6).into()),
                (ask_price3, balance!(255.8).into())
            ])
        );
    });
}

#[test]
fn should_place_limit_order_out_of_spread_with_small_remaining_amount() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id);
        fill_balance(alice(), order_book_id);

        let now = 1234;
        Timestamp::set_timestamp(now);

        let lifespan = 100000;

        let bid_price1 = balance!(10).into();
        let bid_price2 = balance!(9.8).into();
        let bid_price3 = balance!(9.5).into();

        let ask_price1 = balance!(11).into();
        let ask_price2 = balance!(11.2).into();
        let ask_price3 = balance!(11.5).into();

        // check state before

        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price1).unwrap(),
            vec![1]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price2).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(
            OrderBookPallet::asks(&order_book_id, &ask_price1).unwrap(),
            vec![7]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id, &ask_price2).unwrap(),
            vec![8, 9]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id, &ask_price3).unwrap(),
            vec![10, 11, 12]
        );

        assert_eq!(
            OrderBookPallet::aggregated_bids(&order_book_id),
            BTreeMap::from([
                (bid_price1, balance!(168.5).into()),
                (bid_price2, balance!(139.9).into()),
                (bid_price3, balance!(261.3).into())
            ])
        );
        assert_eq!(
            OrderBookPallet::aggregated_asks(&order_book_id),
            BTreeMap::from([
                (ask_price1, balance!(176.3).into()),
                (ask_price2, balance!(178.6).into()),
                (ask_price3, balance!(255.8).into())
            ])
        );

        // buy order 1
        // small remaining amount executes in market
        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(alice()).into(),
            order_book_id,
            balance!(11.1),
            balance!(177),
            PriceVariant::Buy,
            Some(lifespan)
        ));

        // check state

        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price1).unwrap(),
            vec![1]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price2).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(OrderBookPallet::asks(&order_book_id, &ask_price1), None);
        assert_eq!(
            OrderBookPallet::asks(&order_book_id, &ask_price2).unwrap(),
            vec![8, 9]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id, &ask_price3).unwrap(),
            vec![10, 11, 12]
        );

        assert_eq!(
            OrderBookPallet::aggregated_bids(&order_book_id),
            BTreeMap::from([
                (bid_price1, balance!(168.5).into()),
                (bid_price2, balance!(139.9).into()),
                (bid_price3, balance!(261.3).into())
            ])
        );
        assert_eq!(
            OrderBookPallet::aggregated_asks(&order_book_id),
            BTreeMap::from([
                (ask_price2, balance!(177.9).into()),
                (ask_price3, balance!(255.8).into())
            ])
        );

        // buy order 2
        // small remaining amount cancelled
        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(alice()).into(),
            order_book_id,
            balance!(11.6),
            balance!(434),
            PriceVariant::Buy,
            Some(lifespan)
        ));

        // check state
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price1).unwrap(),
            vec![1]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price2).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(OrderBookPallet::asks(&order_book_id, &ask_price1), None);
        assert_eq!(OrderBookPallet::asks(&order_book_id, &ask_price2), None);
        assert_eq!(OrderBookPallet::asks(&order_book_id, &ask_price3), None);

        assert_eq!(
            OrderBookPallet::aggregated_bids(&order_book_id),
            BTreeMap::from([
                (bid_price1, balance!(168.5).into()),
                (bid_price2, balance!(139.9).into()),
                (bid_price3, balance!(261.3).into())
            ])
        );
        assert_eq!(
            OrderBookPallet::aggregated_asks(&order_book_id),
            BTreeMap::from([])
        );

        // sell order 1
        // small remaining amount executes in market
        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(alice()).into(),
            order_book_id,
            balance!(9.9),
            balance!(169),
            PriceVariant::Sell,
            Some(lifespan)
        ));

        // check state
        assert_eq!(OrderBookPallet::bids(&order_book_id, &bid_price1), None);
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price2).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(OrderBookPallet::asks(&order_book_id, &ask_price1), None);
        assert_eq!(OrderBookPallet::asks(&order_book_id, &ask_price2), None);
        assert_eq!(OrderBookPallet::asks(&order_book_id, &ask_price3), None);

        assert_eq!(
            OrderBookPallet::aggregated_bids(&order_book_id),
            BTreeMap::from([
                (bid_price2, balance!(139.4).into()),
                (bid_price3, balance!(261.3).into())
            ])
        );
        assert_eq!(
            OrderBookPallet::aggregated_asks(&order_book_id),
            BTreeMap::from([])
        );

        // sell order 2
        // small remaining amount cancelled
        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(alice()).into(),
            order_book_id,
            balance!(9.4),
            balance!(401),
            PriceVariant::Sell,
            Some(lifespan)
        ));

        // check state
        assert_eq!(OrderBookPallet::bids(&order_book_id, &bid_price1), None);
        assert_eq!(OrderBookPallet::bids(&order_book_id, &bid_price2), None);
        assert_eq!(OrderBookPallet::bids(&order_book_id, &bid_price3), None);

        assert_eq!(OrderBookPallet::asks(&order_book_id, &ask_price1), None);
        assert_eq!(OrderBookPallet::asks(&order_book_id, &ask_price2), None);
        assert_eq!(OrderBookPallet::asks(&order_book_id, &ask_price3), None);

        assert_eq!(
            OrderBookPallet::aggregated_bids(&order_book_id),
            BTreeMap::from([])
        );
        assert_eq!(
            OrderBookPallet::aggregated_asks(&order_book_id),
            BTreeMap::from([])
        );
    });
}

#[test]
#[ignore] // it works, but takes a lot of time
fn should_place_a_lot_of_orders() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        assert_ok!(OrderBookPallet::create_orderbook(
            RawOrigin::Signed(alice()).into(),
            order_book_id
        ));

        let order_book = OrderBookPallet::order_books(order_book_id).unwrap();

        let mut buy_price: OrderPrice = balance!(1000).into();
        let mut buy_lifespan = 10000; // ms
        let mut sell_price: OrderPrice = balance!(1001).into();
        let mut sell_lifespan = 10000; // ms

        let max_prices_for_side: u32 = <Runtime as Config>::MaxSidePriceCount::get();

        for i in 0..max_prices_for_side {
            // get new owner for each order to not get UserHasMaxCountOfOpenedOrders error
            let account = generate_account(i);

            fill_balance(account.clone(), order_book_id);

            buy_price -= order_book.tick_size;
            sell_price += order_book.tick_size;
            buy_lifespan += 5000;
            sell_lifespan += 5000;

            assert_ok!(OrderBookPallet::place_limit_order(
                RawOrigin::Signed(account.clone()).into(),
                order_book_id,
                *buy_price.balance(),
                balance!(10),
                PriceVariant::Buy,
                Some(buy_lifespan)
            ));

            assert_ok!(OrderBookPallet::place_limit_order(
                RawOrigin::Signed(account).into(),
                order_book_id,
                *sell_price.balance(),
                balance!(10),
                PriceVariant::Sell,
                Some(sell_lifespan)
            ));
        }
    });
}

#[test]
fn should_not_cancel_unknown_limit_order() {
    ext().execute_with(|| {
        let caller = alice();
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id);

        let unknown_order_id = 1234;

        assert_err!(
            OrderBookPallet::cancel_limit_order(
                RawOrigin::Signed(caller).into(),
                order_book_id,
                unknown_order_id
            ),
            E::UnknownLimitOrder
        );
    });
}

#[test]
fn should_not_cancel_not_own_limit_order() {
    ext().execute_with(|| {
        let caller = alice(); // but owner is Bob
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id);

        let order_id = 1;

        assert_err!(
            OrderBookPallet::cancel_limit_order(
                RawOrigin::Signed(caller).into(),
                order_book_id,
                order_id
            ),
            E::Unauthorized
        );
    });
}

#[test]
fn should_cancel_limit_order() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id);

        let order_id = 5;

        let order = OrderBookPallet::limit_orders(order_book_id, order_id).unwrap();

        // fix state before
        let bids_before = OrderBookPallet::bids(&order_book_id, &order.price).unwrap_or_default();
        let agg_bids_before = OrderBookPallet::aggregated_bids(&order_book_id);
        let price_volume_before = agg_bids_before
            .get(&order.price)
            .cloned()
            .unwrap_or_default();
        let user_orders_before =
            OrderBookPallet::user_limit_orders(&order.owner, &order_book_id).unwrap_or_default();
        let balance_before = free_balance(&order_book_id.quote, &order.owner);

        // cancel the limit order
        assert_ok!(OrderBookPallet::cancel_limit_order(
            RawOrigin::Signed(order.owner.clone()).into(),
            order_book_id,
            order_id
        ));

        let deal_amount = *order.deal_amount(MarketRole::Taker, None).unwrap().value();

        // check
        let mut expected_bids = bids_before.clone();
        expected_bids.retain(|&id| id != order.id);
        assert_eq!(
            OrderBookPallet::bids(&order_book_id, &order.price).unwrap(),
            expected_bids
        );

        let expected_price_volume = price_volume_before - order.amount;
        let mut expected_agg_bids = agg_bids_before.clone();
        assert_ok!(expected_agg_bids.try_insert(order.price, expected_price_volume));
        assert_eq!(
            OrderBookPallet::aggregated_bids(&order_book_id),
            expected_agg_bids
        );

        let mut expected_user_orders = user_orders_before.clone();
        expected_user_orders.retain(|&id| id != order.id);
        assert_eq!(
            OrderBookPallet::user_limit_orders(&order.owner, &order_book_id).unwrap(),
            expected_user_orders
        );

        let balance = free_balance(&order_book_id.quote, &order.owner);
        let expected_balance = balance_before + deal_amount.balance();
        assert_eq!(balance, expected_balance);
    });
}

#[test]
fn should_not_cancel_not_own_limit_orders_batch() {
    ext().execute_with(|| {
        let order_book_id1 = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id1);

        let order_book_id2 = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: PSWAP.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id2);

        // Bob owns orders (1, 3, 5, 7, 9, 11) in both order books
        let to_cancel = vec![
            (order_book_id1, vec![1, 3, 5, 7, 9, 11]),
            (order_book_id2, vec![1, 2, 5, 7, 9, 11]), // add not owned order 2
        ];

        assert_err!(
            OrderBookPallet::cancel_limit_orders_batch(RawOrigin::Signed(bob()).into(), to_cancel),
            E::Unauthorized
        );
    });
}

#[test]
fn should_not_cancel_unknown_limit_orders_batch() {
    ext().execute_with(|| {
        let order_book_id1 = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id1);

        let order_book_id2 = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: PSWAP.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id2);

        // Bob owns orders (1, 3, 5, 7, 9, 11) in both order books
        let to_cancel = vec![
            (order_book_id1, vec![1, 3, 5, 7, 9, 11]),
            (order_book_id2, vec![1, 3, 5, 7, 9, 11, 100]), // add unknown order 100
        ];

        assert_err!(
            OrderBookPallet::cancel_limit_orders_batch(RawOrigin::Signed(bob()).into(), to_cancel),
            E::UnknownLimitOrder
        );
    });
}

#[test]
fn should_not_cancel_limit_orders_batch_in_stopped_order_book() {
    ext().execute_with(|| {
        let order_book_id1 = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id1);

        let order_book_id2 = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: PSWAP.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id2);

        let order_book_id3 = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: ETH.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id3);

        assert_ok!(OrderBookPallet::change_orderbook_status(
            RawOrigin::Root.into(),
            order_book_id1,
            OrderBookStatus::PlaceAndCancel
        ));

        assert_ok!(OrderBookPallet::change_orderbook_status(
            RawOrigin::Root.into(),
            order_book_id2,
            OrderBookStatus::OnlyCancel
        ));

        assert_ok!(OrderBookPallet::change_orderbook_status(
            RawOrigin::Root.into(),
            order_book_id3,
            OrderBookStatus::Stop
        ));

        // Bob owns orders (1, 3, 5, 7, 9, 11) in all order books
        let to_cancel = vec![
            (order_book_id1, vec![1, 3, 5, 7, 9, 11]),
            (order_book_id2, vec![1, 3, 5, 7, 9, 11]),
            (order_book_id3, vec![1, 3, 5, 7, 9, 11]),
        ];

        assert_err!(
            OrderBookPallet::cancel_limit_orders_batch(RawOrigin::Signed(bob()).into(), to_cancel),
            E::CancellationOfLimitOrdersIsForbidden
        );
    });
}

#[test]
fn should_cancel_all_user_limit_orders_batch() {
    ext().execute_with(|| {
        let order_book_id1 = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id1);

        let order_book_id2 = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: PSWAP.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id2);

        let bid_price1: OrderPrice = balance!(10).into();
        let bid_price2: OrderPrice = balance!(9.8).into();
        let bid_price3: OrderPrice = balance!(9.5).into();

        let ask_price1: OrderPrice = balance!(11).into();
        let ask_price2: OrderPrice = balance!(11.2).into();
        let ask_price3: OrderPrice = balance!(11.5).into();

        // check state before

        // order book 1
        assert_eq!(
            OrderBookPallet::bids(&order_book_id1, &bid_price1).unwrap(),
            vec![1]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id1, &bid_price2).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id1, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(
            OrderBookPallet::asks(&order_book_id1, &ask_price1).unwrap(),
            vec![7]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id1, &ask_price2).unwrap(),
            vec![8, 9]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id1, &ask_price3).unwrap(),
            vec![10, 11, 12]
        );

        // order book 2
        assert_eq!(
            OrderBookPallet::bids(&order_book_id2, &bid_price1).unwrap(),
            vec![1]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id2, &bid_price2).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id2, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(
            OrderBookPallet::asks(&order_book_id2, &ask_price1).unwrap(),
            vec![7]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id2, &ask_price2).unwrap(),
            vec![8, 9]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id2, &ask_price3).unwrap(),
            vec![10, 11, 12]
        );

        // cancel all Bob's limit orders
        // Bob owns orders (1, 3, 5, 7, 9, 11) in both order books
        let to_cancel = vec![
            (order_book_id1, vec![1, 3, 5, 7, 9, 11]),
            (order_book_id2, vec![1, 3, 5, 7, 9, 11]),
        ];

        assert_ok!(OrderBookPallet::cancel_limit_orders_batch(
            RawOrigin::Signed(bob()).into(),
            to_cancel
        ));

        // check state after

        // order book 1
        assert_eq!(OrderBookPallet::bids(&order_book_id1, &bid_price1), None);
        assert_eq!(
            OrderBookPallet::bids(&order_book_id1, &bid_price2).unwrap(),
            vec![2]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id1, &bid_price3).unwrap(),
            vec![4, 6]
        );

        assert_eq!(OrderBookPallet::asks(&order_book_id1, &ask_price1), None);
        assert_eq!(
            OrderBookPallet::asks(&order_book_id1, &ask_price2).unwrap(),
            vec![8]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id1, &ask_price3).unwrap(),
            vec![10, 12]
        );

        // order book 2
        assert_eq!(OrderBookPallet::bids(&order_book_id2, &bid_price1), None);
        assert_eq!(
            OrderBookPallet::bids(&order_book_id2, &bid_price2).unwrap(),
            vec![2]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id2, &bid_price3).unwrap(),
            vec![4, 6]
        );

        assert_eq!(OrderBookPallet::asks(&order_book_id2, &ask_price1), None);
        assert_eq!(
            OrderBookPallet::asks(&order_book_id2, &ask_price2).unwrap(),
            vec![8]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id2, &ask_price3).unwrap(),
            vec![10, 12]
        );
    });
}

#[test]
fn should_cancel_part_of_all_user_limit_orders_batch() {
    ext().execute_with(|| {
        let order_book_id1 = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id1);

        let order_book_id2 = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: PSWAP.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id2);

        let bid_price1: OrderPrice = balance!(10).into();
        let bid_price2: OrderPrice = balance!(9.8).into();
        let bid_price3: OrderPrice = balance!(9.5).into();

        let ask_price1: OrderPrice = balance!(11).into();
        let ask_price2: OrderPrice = balance!(11.2).into();
        let ask_price3: OrderPrice = balance!(11.5).into();

        // check state before

        // order book 1
        assert_eq!(
            OrderBookPallet::bids(&order_book_id1, &bid_price1).unwrap(),
            vec![1]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id1, &bid_price2).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id1, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(
            OrderBookPallet::asks(&order_book_id1, &ask_price1).unwrap(),
            vec![7]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id1, &ask_price2).unwrap(),
            vec![8, 9]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id1, &ask_price3).unwrap(),
            vec![10, 11, 12]
        );

        // order book 2
        assert_eq!(
            OrderBookPallet::bids(&order_book_id2, &bid_price1).unwrap(),
            vec![1]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id2, &bid_price2).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id2, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(
            OrderBookPallet::asks(&order_book_id2, &ask_price1).unwrap(),
            vec![7]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id2, &ask_price2).unwrap(),
            vec![8, 9]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id2, &ask_price3).unwrap(),
            vec![10, 11, 12]
        );

        // cancel all Bob's limit orders
        // Bob owns orders (1, 3, 5, 7, 9, 11) in both order books
        let to_cancel = vec![
            (order_book_id1, vec![1, 5, 9]),
            (order_book_id2, vec![3, 7, 11]),
        ];

        assert_ok!(OrderBookPallet::cancel_limit_orders_batch(
            RawOrigin::Signed(bob()).into(),
            to_cancel
        ));

        // check state after

        // order book 1
        assert_eq!(OrderBookPallet::bids(&order_book_id1, &bid_price1), None);
        assert_eq!(
            OrderBookPallet::bids(&order_book_id1, &bid_price2).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id1, &bid_price3).unwrap(),
            vec![4, 6]
        );

        assert_eq!(
            OrderBookPallet::asks(&order_book_id1, &ask_price1).unwrap(),
            vec![7]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id1, &ask_price2).unwrap(),
            vec![8]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id1, &ask_price3).unwrap(),
            vec![10, 11, 12]
        );

        // order book 2
        assert_eq!(
            OrderBookPallet::bids(&order_book_id2, &bid_price1).unwrap(),
            vec![1]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id2, &bid_price2).unwrap(),
            vec![2]
        );
        assert_eq!(
            OrderBookPallet::bids(&order_book_id2, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(OrderBookPallet::asks(&order_book_id2, &ask_price1), None);
        assert_eq!(
            OrderBookPallet::asks(&order_book_id2, &ask_price2).unwrap(),
            vec![8, 9]
        );
        assert_eq!(
            OrderBookPallet::asks(&order_book_id2, &ask_price3).unwrap(),
            vec![10, 12]
        );
    });
}

#[test]
fn should_not_execute_market_order_with_divisible_asset() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id);
        fill_balance(alice(), order_book_id);

        assert_err!(
            OrderBookPallet::execute_market_order(
                RawOrigin::Signed(alice()).into(),
                order_book_id,
                PriceVariant::Buy,
                balance!(10)
            ),
            E::MarketOrdersAllowedOnlyForIndivisibleAssets
        );

        assert_err!(
            OrderBookPallet::execute_market_order(
                RawOrigin::Signed(alice()).into(),
                order_book_id,
                PriceVariant::Sell,
                balance!(10)
            ),
            E::MarketOrdersAllowedOnlyForIndivisibleAssets
        );
    });
}

#[test]
fn should_execute_market_order_with_indivisible_asset() {
    ext().execute_with(|| {
        FrameSystem::inc_providers(&bob());

        let nft = Assets::register_from(
            &bob(),
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            100000,
            false,
            None,
            None,
        )
        .unwrap();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: nft,
            quote: XOR.into(),
        };

        fill_balance(alice(), order_book_id);
        fill_balance(bob(), order_book_id);

        assert_ok!(TradingPair::register(
            RawOrigin::Signed(bob()).into(),
            DEX.into(),
            order_book_id.quote,
            order_book_id.base
        ));

        assert_ok!(OrderBookPallet::create_orderbook(
            RawOrigin::Signed(bob()).into(),
            order_book_id
        ));

        let buy_price = balance!(10);
        let sell_price = balance!(11);

        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(bob()).into(),
            order_book_id,
            buy_price,
            100,
            PriceVariant::Buy,
            None
        ));

        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(bob()).into(),
            order_book_id,
            sell_price,
            100,
            PriceVariant::Sell,
            None
        ));

        let mut alice_base_balance = free_balance(&order_book_id.base, &alice());
        let mut alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        let mut bob_base_balance = free_balance(&order_book_id.base, &bob());
        let mut bob_quote_balance = free_balance(&order_book_id.quote, &bob());

        let amount = 20;

        // buy market order
        assert_ok!(OrderBookPallet::execute_market_order(
            RawOrigin::Signed(alice()).into(),
            order_book_id,
            PriceVariant::Buy,
            amount
        ));

        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance + amount
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance - amount * sell_price
        );
        assert_eq!(free_balance(&order_book_id.base, &bob()), bob_base_balance);
        assert_eq!(
            free_balance(&order_book_id.quote, &bob()),
            bob_quote_balance + amount * sell_price
        );

        alice_base_balance = free_balance(&order_book_id.base, &alice());
        alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        bob_base_balance = free_balance(&order_book_id.base, &bob());
        bob_quote_balance = free_balance(&order_book_id.quote, &bob());

        // sell market order
        assert_ok!(OrderBookPallet::execute_market_order(
            RawOrigin::Signed(alice()).into(),
            order_book_id,
            PriceVariant::Sell,
            amount
        ));

        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance - amount
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance + amount * buy_price
        );
        assert_eq!(
            free_balance(&order_book_id.base, &bob()),
            bob_base_balance + amount
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &bob()),
            bob_quote_balance
        );
    });
}
