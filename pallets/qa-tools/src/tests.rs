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

use common::{balance, AssetInfoProvider, DEXId, VAL, XOR};
use frame_support::assert_ok;
use framenode_chain_spec::ext;
use framenode_runtime::qa_tools::{self, Config, OrderBookFillSettings, WeightInfo};
use framenode_runtime::{Runtime, RuntimeOrigin, System};
use order_book::OrderBookId;

pub type QAToolsPallet = framenode_runtime::qa_tools::Pallet<Runtime>;

pub fn alice() -> <Runtime as frame_system::Config>::AccountId {
    <Runtime as frame_system::Config>::AccountId::new([1u8; 32])
}

#[test]
fn should_create_and_fill_orderbook() {
    ext().execute_with(|| {
        let start_balance_xor = assets::Pallet::<Runtime>::total_balance(&XOR, &alice()).unwrap();
        let start_balance_val = assets::Pallet::<Runtime>::total_balance(&VAL, &alice()).unwrap();
        let order_book_id = OrderBookId {
            base: VAL,
            quote: XOR,
        };
        assert_ok!(QAToolsPallet::order_book_create_and_fill_many(
            RuntimeOrigin::signed(alice()),
            DEXId::Polkaswap.into(),
            alice(),
            alice(),
            vec![(
                order_book_id,
                OrderBookFillSettings {
                    best_bid_price: balance!(10),
                    best_ask_price: balance!(11)
                }
            )]
        ));
        assert_eq!(
            assets::Pallet::<Runtime>::total_balance(&XOR, &alice()).unwrap(),
            start_balance_xor
        );
        assert_eq!(
            assets::Pallet::<Runtime>::total_balance(&VAL, &alice()).unwrap(),
            start_balance_val
        );

        assert_eq!(
            order_book::Pallet::<Runtime>::aggregated_bids(order_book_id).len(),
            3
        );
        assert_eq!(
            order_book::Pallet::<Runtime>::aggregated_asks(order_book_id).len(),
            3
        );
    });
}
