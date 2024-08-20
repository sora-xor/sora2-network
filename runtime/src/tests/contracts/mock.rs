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

use crate::{AssetId, Assets, OrderBook, Runtime, RuntimeOrigin, TradingPair, Weight};
use assets::AssetIdOf;
use common::mock::{alice, bob, charlie};
use common::{
    balance, AssetName, AssetSymbol, DEXId, DEXInfo, PriceVariant, DEFAULT_BALANCE_PRECISION,
    PSWAP, VAL, XOR, XST,
};
use frame_system::RawOrigin;
use order_book::OrderBookId;
use sp_core::crypto::AccountId32;
use sp_runtime::BuildStorage;

pub const GAS_LIMIT: Weight = Weight::from_parts(100_000_000_000_000, 1024 * 1024);

pub struct ExtBuilder {
    initial_dex_list: Vec<(u32, DEXInfo<AssetId>)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            initial_dex_list: vec![(
                DEXId::Polkaswap.into(),
                DEXInfo {
                    base_asset_id: XOR,
                    synthetic_base_asset_id: XST,
                    is_public: true,
                },
            )],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        use env_logger::{Builder, Env};

        let env = Env::new().default_filter_or("runtime=debug");
        let _ = Builder::from_env(env).is_test(true).try_init();

        let mut t = frame_system::GenesisConfig::<Runtime>::default()
            .build_storage()
            .unwrap();

        pallet_balances::GenesisConfig::<Runtime> {
            balances: vec![
                (alice(), balance!(9900000000)),
                (bob(), balance!(9900000000)),
                (charlie(), balance!(9900000000)),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        assets::GenesisConfig::<Runtime> {
            endowed_assets: vec![
                (
                    XOR.into(),
                    alice(),
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    balance!(10000000000000000000),
                    true,
                    None,
                    None,
                ),
                (
                    PSWAP.into(),
                    alice(),
                    AssetSymbol(b"PSWAP".to_vec()),
                    AssetName(b"PSWAP".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    balance!(10000000000000000000),
                    true,
                    None,
                    None,
                ),
                (
                    VAL.into(),
                    alice(),
                    AssetSymbol(b"VAL".to_vec()),
                    AssetName(b"VAL".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    balance!(10000000000000000000),
                    true,
                    None,
                    None,
                ),
                (
                    XST.into(),
                    alice(),
                    AssetSymbol(b"XST".to_vec()),
                    AssetName(b"XST".to_vec()),
                    0,
                    balance!(10000000000000000000),
                    true,
                    None,
                    None,
                ),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        dex_manager::GenesisConfig::<Runtime> {
            dex_list: self.initial_dex_list,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        let ext = sp_io::TestExternalities::new(t);
        ext
    }
}

pub fn create_order_book(caller: AccountId32) -> OrderBookId<AssetIdOf<Runtime>, u32> {
    Assets::transfer(
        RuntimeOrigin::signed(alice()),
        XST,
        caller.clone(),
        balance!(1000000000000000000),
    )
    .expect("Error while transfer XST to contract");

    Assets::transfer(
        RuntimeOrigin::signed(alice()),
        XOR,
        caller.clone(),
        balance!(1000000000000000000),
    )
    .expect("Error while transfer XST to contract");

    let order_book_id1 = OrderBookId::<AssetIdOf<Runtime>, u32> {
        dex_id: DEXId::Polkaswap.into(),
        base: XST,
        quote: XOR,
    };
    TradingPair::register_pair(DEXId::Polkaswap.into(), XOR, XST)
        .expect("Error while register pair");
    OrderBook::create_orderbook(
        RawOrigin::Root.into(),
        order_book_id1,
        balance!(0.00001),
        balance!(0.00001),
        balance!(1),
        balance!(1000),
    )
    .expect("Error while create order book");

    OrderBook::place_limit_order(
        RawOrigin::Signed(caller.clone()).into(),
        order_book_id1,
        balance!(1),
        balance!(16),
        PriceVariant::Sell,
        Some(<Runtime as order_book::Config>::MIN_ORDER_LIFESPAN + 1000000),
    )
    .expect("Error while place new limit order");

    OrderBook::place_limit_order(
        RawOrigin::Signed(caller).into(),
        order_book_id1,
        balance!(1),
        balance!(10),
        PriceVariant::Sell,
        Some(<Runtime as order_book::Config>::MIN_ORDER_LIFESPAN + 1000000),
    )
    .expect("Error while place new limit order");

    order_book_id1
}

pub fn place_limit_orders(caller: AccountId32) {}

pub fn instantiate_contract(code: Vec<u8>) -> AccountId32 {
    crate::Contracts::bare_instantiate(
        alice(),
        balance!(10),
        GAS_LIMIT,
        None,
        pallet_contracts_primitives::Code::Upload(code),
        vec![],
        vec![0],
        pallet_contracts::DebugInfo::Skip,
        pallet_contracts::CollectEvents::Skip,
    )
    .result
    .expect("Error while instantiate contract")
    .account_id
}
