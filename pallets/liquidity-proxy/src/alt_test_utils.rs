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

#![cfg(feature = "wip")] // ALT

use codec::Decode;
use common::prelude::FixedWrapper;
use common::{balance, AccountIdOf, AssetIdOf, AssetInfoProvider, Balance, DexIdOf, XOR};
use frame_support::assert_ok;
use frame_system::RawOrigin;
use framenode_runtime::liquidity_proxy;
use framenode_runtime::{Runtime, RuntimeOrigin};
use qa_tools::pallet_tools::liquidity_proxy::liquidity_sources;
use qa_tools::pallet_tools::mcbc::{
    CollateralCommonParameters, OtherCollateralInput, TbcdCollateralInput,
};
use qa_tools::pallet_tools::pool_xyk::AssetPairInput;
use qa_tools::pallet_tools::price_tools::AssetPrices;
use sp_std::vec;

pub type OrderBookId = order_book::OrderBookId<AssetIdOf<Runtime>, DexIdOf<Runtime>>;
pub const DEX: common::DEXId = common::DEXId::Polkaswap;

pub fn alice() -> AccountIdOf<Runtime> {
    AccountIdOf::<Runtime>::decode(&mut &[1u8; 32][..]).unwrap()
}

pub fn bob() -> AccountIdOf<Runtime> {
    AccountIdOf::<Runtime>::decode(&mut &[2u8; 32][..]).unwrap()
}

pub fn add_balance(account: AccountIdOf<Runtime>, asset: AssetIdOf<Runtime>, balance: Balance) {
    assert_ok!(<Runtime as common::Config>::AssetManager::update_balance(
        RuntimeOrigin::root(),
        account,
        asset,
        balance.try_into().unwrap(),
    ));
}

pub fn free_balance(asset: &AssetIdOf<Runtime>, account: &AccountIdOf<Runtime>) -> Balance {
    <Runtime as liquidity_proxy::Config>::AssetInfoProvider::free_balance(asset, account).unwrap()
}

pub fn create_empty_order_book(order_book_id: OrderBookId) {
    assert_ok!(order_book::Pallet::<Runtime>::create_orderbook(
        RuntimeOrigin::root(),
        order_book_id,
        balance!(0.00001),
        balance!(0.00001),
        balance!(1),
        balance!(1000)
    ));
}

pub fn init_xyk_pool(
    asset_a: AssetIdOf<Runtime>,
    asset_b: AssetIdOf<Runtime>,
    price: Balance,
    reserve: Option<Balance>,
    caller: AccountIdOf<Runtime>,
) {
    let pair = AssetPairInput::new(DEX.into(), asset_a, asset_b, price, reserve);
    assert_ok!(liquidity_sources::initialize_xyk::<Runtime>(
        caller,
        vec![pair]
    ));
}

pub fn init_order_book(
    base_asset: AssetIdOf<Runtime>,
    bid_price: Balance,
    ask_price: Balance,
    amount: Balance,
    depth: u128,
    price_step: Balance,
    caller: AccountIdOf<Runtime>,
) {
    let order_book_id = OrderBookId {
        dex_id: DEX.into(),
        base: base_asset,
        quote: XOR,
    };

    create_empty_order_book(order_book_id);

    let base_balance = amount * depth;
    let quote_balance = (FixedWrapper::from(bid_price) * FixedWrapper::from(amount))
        .try_into_balance()
        .unwrap()
        * depth;

    add_balance(caller.clone(), base_asset, base_balance);
    add_balance(caller.clone(), XOR, quote_balance);

    for i in 0..depth {
        assert_ok!(order_book::Pallet::<Runtime>::place_limit_order(
            RawOrigin::Signed(caller.clone()).into(),
            order_book_id,
            ask_price + i * price_step,
            amount,
            common::PriceVariant::Sell,
            None
        ));

        assert_ok!(order_book::Pallet::<Runtime>::place_limit_order(
            RawOrigin::Signed(caller.clone()).into(),
            order_book_id,
            bid_price - i * price_step,
            amount,
            common::PriceVariant::Buy,
            None
        ));
    }
}

pub fn init_mcbc_pool(asset: AssetIdOf<Runtime>, price: Balance, reserve: Balance) {
    assert_ok!(liquidity_sources::initialize_mcbc::<Runtime>(
        None,
        vec![OtherCollateralInput {
            asset,
            parameters: CollateralCommonParameters {
                ref_prices: Some(AssetPrices {
                    buy: price,
                    sell: price,
                }),
                reserves: Some(reserve),
            },
        }],
        Some(TbcdCollateralInput {
            parameters: CollateralCommonParameters {
                ref_prices: Some(AssetPrices {
                    buy: balance!(1),
                    sell: balance!(1)
                }),
                reserves: Some(balance!(10000))
            },
            ref_xor_prices: Some(AssetPrices {
                buy: balance!(2),
                sell: balance!(2)
            })
        }),
    ));
}
