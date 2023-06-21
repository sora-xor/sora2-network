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

use crate::traits::{CurrencyLocker, CurrencyUnlocker};
use codec::{Decode, Encode, MaxEncodedLen};
use common::{Balance, PriceVariant, TradingPair};
use frame_support::ensure;
use frame_support::sp_runtime::DispatchError;
use frame_support::{BoundedBTreeMap, BoundedVec, RuntimeDebug};
use sp_runtime::traits::Zero;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::ops::{Add, Sub};

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

pub type OrderPrice = Balance;
pub type OrderVolume = Balance;
pub type PriceOrders<OrderId, MaxLimitOrdersForPrice> = BoundedVec<OrderId, MaxLimitOrdersForPrice>;
pub type MarketSide<MaxSidePriceCount> =
    BoundedBTreeMap<OrderPrice, OrderVolume, MaxSidePriceCount>;
pub type UserOrders<OrderId, MaxOpenedLimitOrdersPerUser> =
    BoundedVec<OrderId, MaxOpenedLimitOrdersPerUser>;

#[derive(
    Encode, Decode, PartialEq, Eq, Copy, Clone, Debug, scale_info::TypeInfo, MaxEncodedLen,
)]
pub enum OrderBookStatus {
    /// All operations are allowed.
    Trade,

    /// Users can place and cancel limit order, but trading is forbidden.
    PlaceAndCancel,

    /// Users can only cancel their limit orders. Placement and trading are forbidden.
    OnlyCancel,

    /// All operations with order book are forbidden. Current limit orders are frozen and users cannot cancel them.
    Stop,
}

#[derive(
    Encode, Decode, Eq, PartialEq, Clone, Copy, RuntimeDebug, scale_info::TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum OrderAmount {
    Base(OrderVolume),
    Quote(OrderVolume),
}

impl OrderAmount {
    pub fn value(&self) -> &OrderVolume {
        match self {
            Self::Base(value) => value,
            Self::Quote(value) => value,
        }
    }

    pub fn is_base(&self) -> bool {
        match self {
            Self::Base(..) => true,
            Self::Quote(..) => false,
        }
    }

    pub fn is_quote(&self) -> bool {
        match self {
            Self::Base(..) => false,
            Self::Quote(..) => true,
        }
    }

    pub fn is_same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Base(..), Self::Base(..)) | (Self::Quote(..), Self::Quote(..)) => true,
            _ => false,
        }
    }

    pub fn copy_type(&self, amount: OrderVolume) -> Self {
        match self {
            Self::Base(..) => Self::Base(amount),
            Self::Quote(..) => Self::Quote(amount),
        }
    }

    pub fn associated_asset<'a, AssetId>(
        &'a self,
        order_book_id: &'a OrderBookId<AssetId>,
    ) -> &AssetId {
        match self {
            Self::Base(..) => &order_book_id.base,
            Self::Quote(..) => &order_book_id.quote,
        }
    }
}

impl Add for OrderAmount {
    type Output = Result<Self, ()>;

    fn add(self, other: Self) -> Self::Output {
        ensure!(self.is_same(&other), ());
        Ok(self.copy_type(self.value().checked_add(*other.value()).ok_or(())?))
    }
}

impl Sub for OrderAmount {
    type Output = Result<Self, ()>;

    fn sub(self, other: Self) -> Self::Output {
        ensure!(self.is_same(&other), ());
        Ok(self.copy_type(self.value().checked_sub(*other.value()).ok_or(())?))
    }
}

#[derive(Eq, PartialEq, Clone, Copy, RuntimeDebug)]
pub enum MarketRole {
    Maker,
    Taker,
}

#[derive(
    Encode,
    Decode,
    Eq,
    PartialEq,
    Copy,
    Clone,
    PartialOrd,
    Ord,
    RuntimeDebug,
    Hash,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OrderBookId<AssetId> {
    /// Base asset.
    pub base: AssetId,
    /// Quote asset. It should be a base asset of DEX.
    pub quote: AssetId,
}

impl<AssetId> From<TradingPair<AssetId>> for OrderBookId<AssetId> {
    fn from(trading_pair: TradingPair<AssetId>) -> Self {
        Self {
            base: trading_pair.target_asset_id,
            quote: trading_pair.base_asset_id,
        }
    }
}

impl<AssetId> From<OrderBookId<AssetId>> for TradingPair<AssetId> {
    fn from(order_book_id: OrderBookId<AssetId>) -> Self {
        Self {
            base_asset_id: order_book_id.quote,
            target_asset_id: order_book_id.base,
        }
    }
}

#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct DealInfo<AssetId> {
    pub input_asset_id: AssetId,
    pub input_amount: OrderAmount,
    pub output_asset_id: AssetId,
    pub output_amount: OrderAmount,
    pub average_price: OrderPrice,
    pub direction: PriceVariant,
}

impl<AssetId: PartialEq> DealInfo<AssetId> {
    pub fn is_valid(&self) -> bool {
        self.input_asset_id != self.output_asset_id
            && !(self.input_amount.is_base() && self.output_amount.is_base())
            && !(self.input_amount.is_quote() && self.output_amount.is_quote())
            && !self.input_amount.value().is_zero()
            && !self.output_amount.value().is_zero()
            && !self.average_price.is_zero()
    }

    pub fn base_amount(&self) -> OrderVolume {
        if self.input_amount.is_base() {
            *self.input_amount.value()
        } else {
            *self.output_amount.value()
        }
    }

    pub fn quote_amount(&self) -> OrderVolume {
        if self.input_amount.is_quote() {
            *self.input_amount.value()
        } else {
            *self.output_amount.value()
        }
    }
}

#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct Payment<AssetId, AccountId, DEXId> {
    pub dex_id: DEXId,
    pub order_book_id: OrderBookId<AssetId>,
    pub to_lock: BTreeMap<AssetId, BTreeMap<AccountId, OrderVolume>>,
    pub to_unlock: BTreeMap<AssetId, BTreeMap<AccountId, OrderVolume>>,
}

impl<AssetId, AccountId, DEXId> Payment<AssetId, AccountId, DEXId>
where
    AssetId: Copy + PartialEq + Ord,
    AccountId: Ord + Clone,
    DEXId: Copy + PartialEq,
{
    pub fn new(dex_id: DEXId, order_book_id: OrderBookId<AssetId>) -> Self {
        Self {
            dex_id,
            order_book_id,
            to_lock: BTreeMap::new(),
            to_unlock: BTreeMap::new(),
        }
    }

    pub fn merge(&mut self, other: &Self) -> Result<(), ()> {
        ensure!(self.dex_id == other.dex_id, ());
        ensure!(self.order_book_id == other.order_book_id, ());

        for (map, to_merge) in [
            (&mut self.to_lock, &other.to_lock),
            (&mut self.to_unlock, &other.to_unlock),
        ] {
            Self::merge_asset_map(map, to_merge);
        }

        Ok(())
    }

    fn merge_account_map(
        account_map: &mut BTreeMap<AccountId, OrderVolume>,
        to_merge: &BTreeMap<AccountId, OrderVolume>,
    ) {
        for (account, volume) in to_merge {
            account_map
                .entry(account.clone())
                .and_modify(|current_volune| *current_volune += volume)
                .or_insert(*volume);
        }
    }

    fn merge_asset_map(
        map: &mut BTreeMap<AssetId, BTreeMap<AccountId, OrderVolume>>,
        to_merge: &BTreeMap<AssetId, BTreeMap<AccountId, OrderVolume>>,
    ) {
        for (asset, whom) in to_merge {
            map.entry(*asset)
                .and_modify(|account_map| {
                    Self::merge_account_map(account_map, whom);
                })
                .or_insert(whom.clone());
        }
    }

    pub fn lock<Locker>(&self) -> Result<(), DispatchError>
    where
        Locker: CurrencyLocker<AccountId, AssetId, DEXId, DispatchError>,
    {
        for (asset_id, from_whom) in self.to_lock.iter() {
            for (account, amount) in from_whom.iter() {
                Locker::lock_liquidity(
                    self.dex_id,
                    account,
                    self.order_book_id,
                    asset_id,
                    *amount,
                )?;
            }
        }

        Ok(())
    }

    pub fn unlock<Unlocker>(&self) -> Result<(), DispatchError>
    where
        Unlocker: CurrencyUnlocker<AccountId, AssetId, DEXId, DispatchError>,
    {
        for (asset_id, to_whom) in self.to_unlock.iter() {
            Unlocker::unlock_liquidity_batch(self.dex_id, self.order_book_id, asset_id, to_whom)?;
        }

        Ok(())
    }

    pub fn execute_all<Locker, Unlocker>(&self) -> Result<(), DispatchError>
    where
        Locker: CurrencyLocker<AccountId, AssetId, DEXId, DispatchError>,
        Unlocker: CurrencyUnlocker<AccountId, AssetId, DEXId, DispatchError>,
    {
        self.lock::<Locker>()?;
        self.unlock::<Unlocker>()?;
        Ok(())
    }
}

#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct MarketChange<AccountId, AssetId, DEXId, OrderId, LimitOrder, BlockNumber> {
    // Info fields
    /// The amount of the input asset for the exchange deal
    pub deal_input: Option<OrderAmount>,

    /// The amount of the output asset for the exchange deal
    pub deal_output: Option<OrderAmount>,

    /// The amount of the input asset that is placed into the market
    pub market_input: Option<OrderAmount>,

    /// The amount of the output asset that is placed into the market
    pub market_output: Option<OrderAmount>,

    // Fields to apply
    pub to_add: BTreeMap<OrderId, LimitOrder>,
    pub to_update: BTreeMap<OrderId, OrderVolume>,
    /// order id and number of block it is scheduled to expire at
    pub to_delete: BTreeMap<OrderId, BlockNumber>,
    pub payment: Payment<AssetId, AccountId, DEXId>,
    pub ignore_unschedule_error: bool,
}

impl<AccountId, AssetId, DEXId, OrderId, LimitOrder, BlockNumber>
    MarketChange<AccountId, AssetId, DEXId, OrderId, LimitOrder, BlockNumber>
where
    AssetId: Copy + PartialEq + Ord,
    AccountId: Ord + Clone,
    DEXId: Copy + PartialEq,
    OrderId: Copy + Ord,
{
    pub fn new(dex_id: DEXId, order_book_id: OrderBookId<AssetId>) -> Self {
        Self {
            deal_input: None,
            deal_output: None,
            market_input: None,
            market_output: None,
            to_add: BTreeMap::new(),
            to_update: BTreeMap::new(),
            to_delete: BTreeMap::new(),
            payment: Payment::new(dex_id, order_book_id),
            ignore_unschedule_error: false,
        }
    }

    pub fn merge(&mut self, mut other: Self) -> Result<(), ()> {
        let join = |lhs: Option<OrderAmount>,
                    rhs: Option<OrderAmount>|
         -> Result<Option<OrderAmount>, ()> {
            let result = match (lhs, rhs) {
                (Some(left_amount), Some(right_amount)) => Some((left_amount + right_amount)?),
                (Some(left_amount), None) => Some(left_amount),
                (None, Some(right_amount)) => Some(right_amount),
                (None, None) => None,
            };
            Ok(result)
        };

        self.deal_input = join(self.deal_input, other.deal_input)?;
        self.deal_output = join(self.deal_output, other.deal_output)?;
        self.market_input = join(self.market_input, other.market_input)?;
        self.market_output = join(self.market_output, other.market_output)?;

        self.to_add.append(&mut other.to_add);
        self.to_update.append(&mut other.to_update);
        self.to_delete.append(&mut other.to_delete);

        self.payment.merge(&other.payment)?;

        self.ignore_unschedule_error =
            self.ignore_unschedule_error || other.ignore_unschedule_error;

        Ok(())
    }
}
