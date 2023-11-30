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

use common::{
    balance, AssetId32, AssetName, AssetSymbol, Balance, DEXId, PredefinedAssetId, VAL, XOR,
};
use frame_support::{assert_err, assert_ok};
use framenode_chain_spec::ext;
use framenode_runtime::qa_tools::{self, OrderBookAttributes, OrderBookFillSettings};
use framenode_runtime::{Runtime, RuntimeOrigin};
use order_book::OrderBookId;
use sp_runtime::traits::BadOrigin;

type FrameSystem = framenode_runtime::frame_system::Pallet<Runtime>;
pub type QAToolsPallet = qa_tools::Pallet<Runtime>;

pub fn alice() -> <Runtime as frame_system::Config>::AccountId {
    <Runtime as frame_system::Config>::AccountId::new([1u8; 32])
}

pub fn bob() -> <Runtime as frame_system::Config>::AccountId {
    <Runtime as frame_system::Config>::AccountId::new([2u8; 32])
}

#[test]
fn should_create_and_fill_orderbook() {
    ext().execute_with(|| {
        fn test_create_and_fill_batch(
            base: AssetId32<PredefinedAssetId>,
            quote: AssetId32<PredefinedAssetId>,
            best_bid_price: Balance,
            best_ask_price: Balance,
            attributes: OrderBookAttributes,
        ) {
            let order_book_id = OrderBookId {
                dex_id: DEXId::Polkaswap.into(),
                base,
                quote,
            };

            assert_err!(
                QAToolsPallet::order_book_create_and_fill_batch(
                    RuntimeOrigin::signed(alice()),
                    alice(),
                    alice(),
                    vec![]
                ),
                BadOrigin
            );

            assert_ok!(QAToolsPallet::order_book_create_and_fill_batch(
                RuntimeOrigin::root(),
                alice(),
                alice(),
                vec![(
                    order_book_id,
                    attributes,
                    OrderBookFillSettings {
                        best_bid_price: best_bid_price.into(),
                        best_ask_price: best_ask_price.into(),
                        lifespan: None,
                    }
                )]
            ));

            assert!(order_book::Pallet::<Runtime>::order_books(order_book_id).is_some());

            assert_eq!(
                order_book::Pallet::<Runtime>::aggregated_bids(order_book_id).len(),
                3
            );
            assert_eq!(
                order_book::Pallet::<Runtime>::aggregated_asks(order_book_id).len(),
                3
            );
        }

        test_create_and_fill_batch(
            VAL,
            XOR,
            balance!(10),
            balance!(11),
            OrderBookAttributes::default(),
        );

        FrameSystem::inc_providers(&bob());
        let nft = assets::Pallet::<Runtime>::register_from(
            &bob(),
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            1000,
            false,
            None,
            None,
        )
        .unwrap();
        test_create_and_fill_batch(
            nft,
            XOR,
            balance!(10),
            balance!(11),
            OrderBookAttributes {
                tick_size: balance!(0.00001),
                step_lot_size: 1,
                min_lot_size: 1,
                max_lot_size: 1000,
            },
        );
    });
}

#[test]
fn should_create_empty_orderbook() {
    ext().execute_with(|| {
        fn test_create_empty_batch(
            base: AssetId32<PredefinedAssetId>,
            quote: AssetId32<PredefinedAssetId>,
        ) {
            let order_book_id = OrderBookId {
                dex_id: DEXId::Polkaswap.into(),
                base,
                quote,
            };

            assert_err!(
                QAToolsPallet::order_book_create_empty_batch(
                    RuntimeOrigin::signed(alice()),
                    vec![]
                ),
                BadOrigin
            );

            assert_ok!(QAToolsPallet::order_book_create_empty_batch(
                RuntimeOrigin::root(),
                vec![order_book_id]
            ));

            assert!(order_book::Pallet::<Runtime>::order_books(order_book_id).is_some());
            assert!(order_book::Pallet::<Runtime>::aggregated_bids(order_book_id).is_empty(),);
            assert!(order_book::Pallet::<Runtime>::aggregated_asks(order_book_id).is_empty(),);
        }

        test_create_empty_batch(VAL, XOR);

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
        test_create_empty_batch(nft, XOR);
    });
}
