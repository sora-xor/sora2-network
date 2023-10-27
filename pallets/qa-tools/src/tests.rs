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
    DexIdOf, PredefinedAssetId, VAL, XOR,
};
use frame_support::pallet_prelude::DispatchResult;
use frame_support::{assert_err, assert_ok};
use framenode_chain_spec::ext;
use framenode_runtime::qa_tools::{self, OrderBookFillSettings, WhitelistedCallers};
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
        ) -> OrderBookId<AssetIdOf<Runtime>, DexIdOf<Runtime>> {
            let mut start_balance_base =
                assets::Pallet::<Runtime>::total_balance(&base, &alice()).unwrap();
            let start_balance_quote =
                assets::Pallet::<Runtime>::total_balance(&quote, &alice()).unwrap();
            let order_book_id = OrderBookId {
                dex_id: DEXId::Polkaswap.into(),
                base,
                quote,
            };
            let _ = QAToolsPallet::add_to_whitelist(RuntimeOrigin::root(), alice());
            assert_ok!(QAToolsPallet::order_book_create_and_fill_batch(
                RuntimeOrigin::signed(alice()),
                alice(),
                alice(),
                vec![(
                    order_book_id,
                    OrderBookFillSettings {
                        best_bid_price: best_bid_price.into(),
                        best_ask_price: best_ask_price.into(),
                        lifespan: None,
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
                3
            );
            assert_eq!(
                order_book::Pallet::<Runtime>::aggregated_asks(order_book_id).len(),
                3
            );
            order_book_id
        }

        test_create_and_fill_batch(VAL, XOR, balance!(10), balance!(11));

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
        test_create_and_fill_batch(nft, XOR, balance!(10), balance!(11));
    });
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
