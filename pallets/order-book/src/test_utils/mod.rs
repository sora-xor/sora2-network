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

// TODO: rename to `order_book` after upgrading to nightly-2023-07-01+
#[cfg(not(test))]
use crate as order_book_imported;
#[cfg(test)]
use framenode_runtime::order_book as order_book_imported;

use order_book_imported::{
    Config, OrderBook, OrderBookId, OrderBookStatus, OrderBookTechStatus, OrderPrice, OrderVolume,
    Pallet, PriceOrders,
};

use common::AssetIdOf;
use common::{balance, AssetInfoProvider, AssetManager, Balance, DexIdOf, PriceVariant};
use frame_support::assert_ok;
use frame_support::dispatch::DispatchResult;
use frame_system::RawOrigin;
use sp_std::{collections::btree_map::BTreeMap, vec::Vec};

pub const DEX: common::DexId = common::DexId::Polkaswap;
pub const INIT_BALANCE: Balance = balance!(1000000);

pub mod accounts;
pub mod fill_tools;
#[cfg(feature = "std")]
pub mod print_tools;

pub fn free_balance<T: technical::Config>(
    asset: &AssetIdOf<T>,
    account: &<T as frame_system::Config>::AccountId,
) -> Balance {
    T::AssetInfoProvider::free_balance(asset, account).expect("Asset must exist")
}

pub fn fill_balance<T: common::Config>(
    account: <T as frame_system::Config>::AccountId,
    order_book_id: OrderBookId<AssetIdOf<T>, DexIdOf<T>>,
) {
    assert_ok!(T::AssetManager::update_balance(
        RawOrigin::Root.into(),
        account.clone(),
        order_book_id.base,
        INIT_BALANCE.try_into().unwrap()
    ));

    assert_ok!(T::AssetManager::update_balance(
        RawOrigin::Root.into(),
        account,
        order_book_id.quote,
        INIT_BALANCE.try_into().unwrap()
    ));
}

pub fn get_last_order_id<T: Config>(
    order_book_id: OrderBookId<AssetIdOf<T>, DexIdOf<T>>,
) -> Option<<T as Config>::OrderId> {
    Pallet::<T>::order_books(order_book_id).map(|order_book| order_book.last_order_id)
}

pub fn update_orderbook_unchecked<T: Config>(
    order_book_id: OrderBookId<AssetIdOf<T>, DexIdOf<T>>,
    tick_size: Balance,
    step_lot_size: Balance,
    min_lot_size: Balance,
    max_lot_size: Balance,
) -> OrderBook<T> {
    let mut order_book = Pallet::order_books(order_book_id).unwrap();
    order_book.tick_size.set(tick_size);
    order_book.step_lot_size.set(step_lot_size);
    order_book.min_lot_size.set(min_lot_size);
    order_book.max_lot_size.set(max_lot_size);
    order_book_imported::OrderBooks::<T>::set(order_book_id, Some(order_book));

    Pallet::order_books(order_book_id).unwrap()
}

/// Update orderbook with temporarily setting its status to `Stop`.
///
/// If some parameter is `None`, then leave it as is.
pub fn update_order_book_with_set_status<T: Config>(
    order_book: &mut OrderBook<T>,
    tick_size: Option<OrderPrice>,
    step_lot_size: Option<OrderVolume>,
    min_lot_size: Option<OrderVolume>,
    max_lot_size: Option<OrderVolume>,
) -> DispatchResult {
    let original_status = order_book.status;
    Pallet::<T>::change_orderbook_status(
        RawOrigin::Root.into(),
        order_book.order_book_id,
        OrderBookStatus::Stop,
    )?;
    let tick_size = tick_size.unwrap_or(order_book.tick_size);
    let step_lot_size = step_lot_size.unwrap_or(order_book.step_lot_size);
    let min_lot_size = min_lot_size.unwrap_or(order_book.min_lot_size);
    let max_lot_size = max_lot_size.unwrap_or(order_book.max_lot_size);
    Pallet::<T>::update_orderbook(
        RawOrigin::Root.into(),
        order_book.order_book_id,
        *tick_size.balance(),
        *step_lot_size.balance(),
        *min_lot_size.balance(),
        *max_lot_size.balance(),
    )?;
    Pallet::<T>::change_orderbook_status(
        RawOrigin::Root.into(),
        order_book.order_book_id,
        original_status,
    )?;

    order_book.tick_size = tick_size;
    order_book.step_lot_size = step_lot_size;
    order_book.min_lot_size = min_lot_size;
    order_book.max_lot_size = max_lot_size;
    Ok(())
}

pub fn lock_order_book<T: Config>(order_book_id: OrderBookId<AssetIdOf<T>, DexIdOf<T>>) {
    let mut order_book = Pallet::order_books(order_book_id).unwrap();
    order_book.tech_status = OrderBookTechStatus::Updating;
    order_book_imported::OrderBooks::<T>::set(order_book_id, Some(order_book));
}

pub fn create_empty_order_book<T: Config>(
    order_book_id: OrderBookId<AssetIdOf<T>, DexIdOf<T>>,
) -> OrderBook<T> {
    fill_balance::<T>(accounts::alice::<T>(), order_book_id);

    assert_ok!(Pallet::<T>::create_orderbook(
        RawOrigin::Root.into(),
        order_book_id,
        balance!(0.00001),
        balance!(0.00001),
        balance!(1),
        balance!(1000)
    ));

    Pallet::<T>::order_books(order_book_id).unwrap()
}

// Creates and fills the order book
// price | volume | orders
//          Asks
//  11.5 |  255.8 | sell4, sell5, sell6
//  11.2 |  178.6 | sell2, sell3
//  11.0 |  176.3 | sell1
//  spread
//  10.0 |  168.5 | buy1
//   9.8 |  139.9 | buy2, buy3
//   9.5 |  261.3 | buy4, buy5, buy6
//          Bids
pub fn create_and_fill_order_book<T: Config>(
    order_book_id: OrderBookId<AssetIdOf<T>, DexIdOf<T>>,
) -> OrderBook<T> {
    fill_balance::<T>(accounts::bob::<T>(), order_book_id);
    fill_balance::<T>(accounts::charlie::<T>(), order_book_id);

    assert_ok!(Pallet::<T>::create_orderbook(
        RawOrigin::Root.into(),
        order_book_id,
        balance!(0.00001),
        balance!(0.00001),
        balance!(1),
        balance!(1000)
    ));

    let lifespan = Some(100000u32.into());

    // prices
    let bp1 = balance!(10);
    let bp2 = balance!(9.8);
    let bp3 = balance!(9.5);
    let sp1 = balance!(11);
    let sp2 = balance!(11.2);
    let sp3 = balance!(11.5);

    // buy amounts
    let amount1 = balance!(168.5);
    let amount2 = balance!(95.2);
    let amount3 = balance!(44.7);
    let amount4 = balance!(56.4);
    let amount5 = balance!(89.9);
    let amount6 = balance!(115);

    // sell amounts
    let amount7 = balance!(176.3);
    let amount8 = balance!(85.4);
    let amount9 = balance!(93.2);
    let amount10 = balance!(36.6);
    let amount11 = balance!(205.5);
    let amount12 = balance!(13.7);

    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(accounts::bob::<T>()).into(),
        order_book_id,
        bp1,
        amount1,
        PriceVariant::Buy,
        lifespan
    ));
    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(accounts::charlie::<T>()).into(),
        order_book_id,
        bp2,
        amount2,
        PriceVariant::Buy,
        lifespan
    ));
    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(accounts::bob::<T>()).into(),
        order_book_id,
        bp2,
        amount3,
        PriceVariant::Buy,
        lifespan
    ));
    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(accounts::charlie::<T>()).into(),
        order_book_id,
        bp3,
        amount4,
        PriceVariant::Buy,
        lifespan
    ));
    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(accounts::bob::<T>()).into(),
        order_book_id,
        bp3,
        amount5,
        PriceVariant::Buy,
        lifespan
    ));
    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(accounts::charlie::<T>()).into(),
        order_book_id,
        bp3,
        amount6,
        PriceVariant::Buy,
        lifespan
    ));

    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(accounts::bob::<T>()).into(),
        order_book_id,
        sp1,
        amount7,
        PriceVariant::Sell,
        lifespan
    ));
    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(accounts::charlie::<T>()).into(),
        order_book_id,
        sp2,
        amount8,
        PriceVariant::Sell,
        lifespan
    ));
    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(accounts::bob::<T>()).into(),
        order_book_id,
        sp2,
        amount9,
        PriceVariant::Sell,
        lifespan
    ));
    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(accounts::charlie::<T>()).into(),
        order_book_id,
        sp3,
        amount10,
        PriceVariant::Sell,
        lifespan
    ));
    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(accounts::bob::<T>()).into(),
        order_book_id,
        sp3,
        amount11,
        PriceVariant::Sell,
        lifespan
    ));
    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(accounts::charlie::<T>()).into(),
        order_book_id,
        sp3,
        amount12,
        PriceVariant::Sell,
        lifespan
    ));

    fn slice_to_price_orders<T: Config>(
        v: &[u32],
    ) -> PriceOrders<T::OrderId, T::MaxLimitOrdersForPrice> {
        v.iter()
            .map(|id| T::OrderId::from(*id))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap()
    }

    // check
    assert_eq!(
        Pallet::<T>::bids(order_book_id, OrderPrice::divisible(bp1)).unwrap(),
        slice_to_price_orders::<T>(&[1])
    );
    assert_eq!(
        Pallet::<T>::bids(order_book_id, OrderPrice::divisible(bp2)).unwrap(),
        slice_to_price_orders::<T>(&[2, 3])
    );
    assert_eq!(
        Pallet::<T>::bids(order_book_id, OrderPrice::divisible(bp3)).unwrap(),
        slice_to_price_orders::<T>(&[4, 5, 6])
    );

    assert_eq!(
        Pallet::<T>::asks(order_book_id, OrderPrice::divisible(sp1)).unwrap(),
        slice_to_price_orders::<T>(&[7])
    );
    assert_eq!(
        Pallet::<T>::asks(order_book_id, OrderPrice::divisible(sp2)).unwrap(),
        slice_to_price_orders::<T>(&[8, 9])
    );
    assert_eq!(
        Pallet::<T>::asks(order_book_id, OrderPrice::divisible(sp3)).unwrap(),
        slice_to_price_orders::<T>(&[10, 11, 12])
    );

    assert_eq!(
        Pallet::<T>::aggregated_bids(order_book_id),
        BTreeMap::from([
            (bp1.into(), amount1.into()),
            (bp2.into(), (amount2 + amount3).into()),
            (bp3.into(), (amount4 + amount5 + amount6).into())
        ])
    );
    assert_eq!(
        Pallet::<T>::aggregated_asks(order_book_id),
        BTreeMap::from([
            (sp1.into(), amount7.into()),
            (sp2.into(), (amount8 + amount9).into()),
            (sp3.into(), (amount10 + amount11 + amount12).into())
        ])
    );

    Pallet::<T>::order_books(order_book_id).unwrap()
}

#[cfg(test)]
pub use test_only::*;

#[cfg(test)]
mod test_only {
    use super::*;
    use frame_support::traits::Hooks;
    use frame_support::weights::Weight;
    use framenode_runtime::order_book::{self, Pallet};
    use framenode_runtime::Runtime;

    pub type E = order_book::Error<Runtime>;
    pub type OrderBookPallet = Pallet<Runtime>;
    pub type DexId = DexIdOf<Runtime>;

    /// Returns weight spent on initializations
    pub fn run_to_block(n: u32) -> Weight {
        type System = frame_system::Pallet<Runtime>;
        let mut total_init_weight = 0.into();
        while System::block_number() < n {
            OrderBookPallet::on_finalize(System::block_number());
            System::set_block_number(System::block_number() + 1);
            total_init_weight += OrderBookPallet::on_initialize(System::block_number());
        }
        total_init_weight
    }
}
