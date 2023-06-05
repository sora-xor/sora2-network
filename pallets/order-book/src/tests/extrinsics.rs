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
    balance, AssetId32, AssetName, AssetSymbol, PriceVariant, DEFAULT_BALANCE_PRECISION, VAL, XOR,
};
use frame_support::error::BadOrigin;
use frame_support::{assert_err, assert_ok};
use frame_system::RawOrigin;
use framenode_chain_spec::ext;
use framenode_runtime::order_book::{
    Config, LimitOrder, MarketRole, OrderBook, OrderBookId, OrderBookStatus,
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
fn should_not_create_order_book_with_same_assets() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: XOR.into(),
            quote: XOR.into(),
        };

        assert_err!(
            OrderBookPallet::create_orderbook(
                RawOrigin::Signed(alice()).into(),
                DEX.into(),
                order_book_id
            ),
            E::ForbiddenToCreateOrderBookWithSameAssets
        );
    });
}

#[test]
fn should_not_create_order_book_with_wrong_quote_asset() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: XOR.into(),
            quote: VAL.into(),
        };

        assert_err!(
            OrderBookPallet::create_orderbook(
                RawOrigin::Signed(alice()).into(),
                DEX.into(),
                order_book_id
            ),
            E::NotAllowedBaseAsset
        );
    });
}

#[test]
fn should_not_create_order_book_with_non_existed_asset() {
    ext().execute_with(|| {
        let wrong_asset = AssetId32::from_bytes(hex!(
            "0123456789012345678901234567890123456789012345678901234567890123"
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: wrong_asset.into(),
            quote: XOR.into(),
        };

        assert_err!(
            OrderBookPallet::create_orderbook(
                RawOrigin::Signed(alice()).into(),
                DEX.into(),
                order_book_id
            ),
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

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: new_asset.into(),
            quote: XOR.into(),
        };

        assert_err!(
            OrderBookPallet::create_orderbook(
                RawOrigin::Signed(caller).into(),
                DEX.into(),
                order_book_id
            ),
            trading_pair::Error::<Runtime>::TradingPairDoesntExist
        );
    });
}

#[test]
fn should_create_order_book_for_regular_assets() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        assert_ok!(OrderBookPallet::create_orderbook(
            RawOrigin::Signed(alice()).into(),
            DEX.into(),
            order_book_id
        ));

        assert_eq!(
            OrderBookPallet::order_books(order_book_id).unwrap(),
            OrderBook::default(order_book_id, DEX.into())
        );
    });
}

#[test]
fn should_not_create_order_book_that_already_exists() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        assert_ok!(OrderBookPallet::create_orderbook(
            RawOrigin::Signed(alice()).into(),
            DEX.into(),
            order_book_id
        ));

        assert_err!(
            OrderBookPallet::create_orderbook(
                RawOrigin::Signed(alice()).into(),
                DEX.into(),
                order_book_id
            ),
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
            balance!(1),
            false,
            None,
            None,
        )
        .unwrap();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
            OrderBookPallet::create_orderbook(
                RawOrigin::Signed(caller).into(),
                DEX.into(),
                order_book_id
            ),
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
            balance!(1),
            false,
            None,
            None,
        )
        .unwrap();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
        Assets::transfer(
            RawOrigin::Signed(caller.clone()).into(),
            nft,
            user,
            balance!(1),
        )
        .unwrap();

        assert_err!(
            OrderBookPallet::create_orderbook(
                RawOrigin::Signed(caller).into(),
                DEX.into(),
                order_book_id
            ),
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
            balance!(1),
            false,
            None,
            None,
        )
        .unwrap();

        Assets::transfer(
            RawOrigin::Signed(creator).into(),
            nft,
            caller.clone(),
            balance!(1),
        )
        .unwrap();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
            DEX.into(),
            order_book_id
        ));

        assert_eq!(
            OrderBookPallet::order_books(order_book_id).unwrap(),
            OrderBook::default_nft(order_book_id, DEX.into())
        );
    });
}

#[test]
fn should_check_permissions_for_delete_order_book() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id);
        let owner = bob();

        let tech_account = technical::Pallet::<Runtime>::tech_account_id_to_account_id(
            &OrderBookPallet::tech_account_for_order_book(DEX.into(), order_book_id.clone()),
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
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        assert_ok!(OrderBookPallet::create_orderbook(
            RawOrigin::Signed(alice()).into(),
            DEX.into(),
            order_book_id
        ));

        let order_book = OrderBookPallet::order_books(order_book_id).unwrap();

        let mut buy_price = balance!(1000);
        let mut sell_price = balance!(1001);

        let max_prices_for_side: u32 = <Runtime as Config>::MaxSidePriceCount::get();

        for i in 0..max_prices_for_side {
            // get new owner for each order to not get UserHasMaxCountOfOpenedOrders error
            let account = generate_account(i);

            fill_balance(account.clone(), order_book_id);

            buy_price -= order_book.tick_size;
            sell_price += order_book.tick_size;

            assert_ok!(OrderBookPallet::place_limit_order(
                RawOrigin::Signed(account.clone()).into(),
                order_book_id,
                buy_price,
                balance!(10),
                PriceVariant::Buy,
                Some(10000)
            ));

            assert_ok!(OrderBookPallet::place_limit_order(
                RawOrigin::Signed(account).into(),
                order_book_id,
                sell_price,
                balance!(10),
                PriceVariant::Sell,
                Some(10000)
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
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
            balance!(100),
            false,
            None,
            None,
        )
        .unwrap();
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
            balance!(2),
            balance!(4),
            balance!(100),
        ),);
        assert_err!(
            OrderBookPallet::update_orderbook(
                RawOrigin::Signed(asset_owner_quote).into(),
                order_book_id,
                balance!(0.01),
                balance!(2),
                balance!(4),
                balance!(100),
            ),
            BadOrigin
        );
        assert_err!(
            OrderBookPallet::update_orderbook(
                RawOrigin::Signed(alice()).into(),
                order_book_id,
                balance!(0.01),
                balance!(2),
                balance!(4),
                balance!(100),
            ),
            BadOrigin
        );
    });
}

#[test]
fn should_not_update_unknown_order_book() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
                balance!(0),
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
                balance!(0),
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
                balance!(0),
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
                balance!(0)
            ),
            E::InvalidMaxLotSize
        );
    });
}

#[test]
fn should_not_update_order_book_with_simple_mistakes() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
            E::TickSizeAndStepLotSizeAreTooSmall
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
            E::TickSizeAndStepLotSizeAreTooSmall
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
            E::TickSizeAndStepLotSizeAreTooSmall
        );
    });
}

#[test]
fn should_not_update_order_book_when_atributes_exceed_total_supply() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
fn should_not_update_order_book_with_nft_bounds() {
    ext().execute_with(|| {
        FrameSystem::inc_providers(&alice());

        let nft = Assets::register_from(
            &alice(),
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            balance!(100),
            false,
            None,
            None,
        )
        .unwrap();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
            DEX.into(),
            order_book_id
        ));

        assert_err!(
            OrderBookPallet::update_orderbook(
                RuntimeOrigin::root(),
                order_book_id,
                balance!(0.01),
                balance!(0.5),
                balance!(1),
                balance!(10)
            ),
            E::InvalidStepLotSize
        );

        assert_err!(
            OrderBookPallet::update_orderbook(
                RuntimeOrigin::root(),
                order_book_id,
                balance!(0.01),
                balance!(1.1),
                balance!(1),
                balance!(10)
            ),
            E::InvalidStepLotSize
        );
    });
}

#[test]
fn should_update_order_book_with_regular_asset() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_empty_order_book(order_book_id);

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

        assert_eq!(order_book.tick_size, tick_size);
        assert_eq!(order_book.step_lot_size, step_lot_size);
        assert_eq!(order_book.min_lot_size, min_lot_size);
        assert_eq!(order_book.max_lot_size, max_lot_size);
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
            balance!(100),
            false,
            None,
            None,
        )
        .unwrap();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
            DEX.into(),
            order_book_id
        ));

        let tick_size = balance!(0.01);
        let step_lot_size = balance!(2);
        let min_lot_size = balance!(4);
        let max_lot_size = balance!(100);

        assert_ok!(OrderBookPallet::update_orderbook(
            RuntimeOrigin::root(),
            order_book_id,
            tick_size,
            step_lot_size,
            min_lot_size,
            max_lot_size
        ));

        let order_book = OrderBookPallet::order_books(order_book_id).unwrap();

        assert_eq!(order_book.tick_size, tick_size);
        assert_eq!(order_book.step_lot_size, step_lot_size);
        assert_eq!(order_book.min_lot_size, min_lot_size);
        assert_eq!(order_book.max_lot_size, max_lot_size);
    });
}

#[test]
fn should_check_permissions_for_change_order_book_status() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id);
        fill_balance(caller.clone(), order_book_id);

        let price = balance!(10);
        let amount = balance!(100);
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
            price,
            amount,
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
        let expected_balance = balance_before - deal_amount;
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
            balance!(1),
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

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
            DEX.into(),
            order_book_id
        ));

        let price = balance!(10);
        let amount = balance!(1);
        let lifespan = 10000;
        let now = 1234;
        let current_block = frame_system::Pallet::<Runtime>::block_number();

        Timestamp::set_timestamp(now);

        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(caller.clone()).into(),
            order_book_id,
            price,
            amount,
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
#[ignore] // it works, but takes a lot of time
fn should_place_a_lot_of_orders() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        assert_ok!(OrderBookPallet::create_orderbook(
            RawOrigin::Signed(alice()).into(),
            DEX.into(),
            order_book_id
        ));

        let order_book = OrderBookPallet::order_books(order_book_id).unwrap();

        let mut buy_price = balance!(1000);
        let mut sell_price = balance!(1001);

        let max_prices_for_side: u32 = <Runtime as Config>::MaxSidePriceCount::get();

        for i in 0..max_prices_for_side {
            // get new owner for each order to not get UserHasMaxCountOfOpenedOrders error
            let account = generate_account(i);

            fill_balance(account.clone(), order_book_id);

            buy_price -= order_book.tick_size;
            sell_price += order_book.tick_size;

            assert_ok!(OrderBookPallet::place_limit_order(
                RawOrigin::Signed(account.clone()).into(),
                order_book_id,
                buy_price,
                balance!(10),
                PriceVariant::Buy,
                Some(10000)
            ));

            assert_ok!(OrderBookPallet::place_limit_order(
                RawOrigin::Signed(account).into(),
                order_book_id,
                sell_price,
                balance!(10),
                PriceVariant::Sell,
                Some(10000)
            ));
        }
    });
}

#[test]
fn should_not_cancel_unknown_limit_order() {
    ext().execute_with(|| {
        let caller = alice();
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
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
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book(order_book_id);

        let order_id = 5;

        let order = OrderBookPallet::limit_orders(&order_book_id, order_id).unwrap();

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
        let expected_balance = balance_before + deal_amount;
        assert_eq!(balance, expected_balance);
    });
}
