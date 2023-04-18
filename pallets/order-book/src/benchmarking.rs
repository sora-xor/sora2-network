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

//! Benchmarking setup for order-book

#![cfg(feature = "runtime-benchmarks")]
// order-book
#![cfg(feature = "wip")]
// now it works only as benchmarks, not as unit tests
// TODO fix when new approach be developed
#![cfg(not(test))]

#[cfg(not(test))]
use crate::{Config, Event, OrderBook, OrderBookId, Pallet};
#[cfg(test)]
use framenode_runtime::order_book::{Config, Event, OrderBook, OrderBookId, Pallet};

use codec::Decode;
use common::{balance, AssetName, AssetSymbol, DEXId, XOR};
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;

use assets::Pallet as Assets;
use frame_system::Pallet as FrameSystem;
use trading_pair::Pallet as TradingPair;
use Pallet as OrderBookPallet;

pub const DEX: DEXId = DEXId::Polkaswap;

fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    where_clause {
        where T: trading_pair::Config + core::fmt::Debug
    }

    create_orderbook {
        let caller = alice::<T>();
        FrameSystem::<T>::inc_providers(&caller);

        let nft = Assets::<T>::register_from(
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

        let order_book_id = OrderBookId::<T> {
            base_asset_id: XOR.into(),
            target_asset_id: nft.into(),
        };

        TradingPair::<T>::register(
            RawOrigin::Signed(caller.clone()).into(),
            DEX.into(),
            order_book_id.base_asset_id,
            order_book_id.target_asset_id
        ).unwrap();
    }: {
        OrderBookPallet::<T>::create_orderbook(
            RawOrigin::Signed(caller.clone()).into(),
            DEX.into(),
            order_book_id
        ).unwrap();
    }
    verify {
        assert_last_event::<T>(Event::<T>::OrderBookCreated{order_book_id: order_book_id, dex_id: DEX.into(), creator: caller}.into());

        assert_eq!(
            OrderBookPallet::<T>::order_books(order_book_id).unwrap(),
            OrderBook::<T>::default_nft(order_book_id, DEX.into())
        );
    }

    delete_orderbook {
    }: {
        // todo (m.tagirov)
    }
    verify {
    }

    update_orderbook {
    }: {
        // todo (m.tagirov)
    }
    verify {
    }

    change_orderbook_status {
    }: {
        // todo (m.tagirov)
    }
    verify {
    }

    place_limit_order {
    }: {
        // todo (m.tagirov)
    }
    verify {
    }

    cancel_limit_order {
    }: {
        // todo (m.tagirov)
    }
    verify {
    }

    quote {
    }: {
        // todo (m.tagirov)
    }
    verify {
    }

    exchange {
    }: {
        // todo (m.tagirov)
    }
    verify {
    }

    impl_benchmark_test_suite!(Pallet, framenode_chain_spec::ext(), framenode_runtime::Runtime);
}
