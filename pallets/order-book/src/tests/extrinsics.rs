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

use common::{
    balance, AssetId32, AssetName, AssetSymbol, DEXId, DEFAULT_BALANCE_PRECISION, VAL, XOR,
};
use frame_support::{assert_err, assert_ok};
use frame_system::RawOrigin;
use framenode_chain_spec::ext;
use framenode_runtime::order_book::{OrderBook, OrderBookId, Pallet};
use framenode_runtime::{order_book, Runtime};
use hex_literal::hex;

type Assets = framenode_runtime::assets::Pallet<Runtime>;
type OrderBookPallet = Pallet<Runtime>;
type TradingPair = framenode_runtime::trading_pair::Pallet<Runtime>;
type FrameSystem = framenode_runtime::frame_system::Pallet<Runtime>;

type E = order_book::Error<Runtime>;
pub const DEX: DEXId = DEXId::Polkaswap;

fn alice() -> <Runtime as frame_system::Config>::AccountId {
    <Runtime as frame_system::Config>::AccountId::new([1u8; 32])
}

fn bob() -> <Runtime as frame_system::Config>::AccountId {
    <Runtime as frame_system::Config>::AccountId::new([2u8; 32])
}

#[test]
fn should_not_create_order_book_with_same_assets() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<Runtime> {
            base_asset_id: XOR.into(),
            target_asset_id: XOR.into(),
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
fn should_not_create_order_book_with_wrong_base_asset() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<Runtime> {
            base_asset_id: VAL.into(),
            target_asset_id: XOR.into(),
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

        let order_book_id = OrderBookId::<Runtime> {
            base_asset_id: XOR.into(),
            target_asset_id: wrong_asset.into(),
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

        let order_book_id = OrderBookId::<Runtime> {
            base_asset_id: XOR.into(),
            target_asset_id: new_asset.into(),
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
        let order_book_id = OrderBookId::<Runtime> {
            base_asset_id: XOR.into(),
            target_asset_id: VAL.into(),
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
        let order_book_id = OrderBookId::<Runtime> {
            base_asset_id: XOR.into(),
            target_asset_id: VAL.into(),
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

        let order_book_id = OrderBookId::<Runtime> {
            base_asset_id: XOR.into(),
            target_asset_id: nft.into(),
        };

        assert_ok!(TradingPair::register(
            RawOrigin::Signed(creator.clone()).into(),
            DEX.into(),
            order_book_id.base_asset_id,
            order_book_id.target_asset_id
        ));

        assert_err!(
            OrderBookPallet::create_orderbook(
                RawOrigin::Signed(caller).into(),
                DEX.into(),
                order_book_id
            ),
            E::UserDoesntHaveNft
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

        let order_book_id = OrderBookId::<Runtime> {
            base_asset_id: XOR.into(),
            target_asset_id: nft.into(),
        };

        assert_ok!(TradingPair::register(
            RawOrigin::Signed(caller.clone()).into(),
            DEX.into(),
            order_book_id.base_asset_id,
            order_book_id.target_asset_id
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
            E::UserDoesntHaveNft
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

        let order_book_id = OrderBookId::<Runtime> {
            base_asset_id: XOR.into(),
            target_asset_id: nft.into(),
        };

        assert_ok!(TradingPair::register(
            RawOrigin::Signed(caller.clone()).into(),
            DEX.into(),
            order_book_id.base_asset_id,
            order_book_id.target_asset_id
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
