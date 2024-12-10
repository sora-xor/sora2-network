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
use common::prelude::BalanceUnit;
use common::{OrderBookId, PriceVariant};
use frame_support::sp_runtime::DispatchError;
use frame_support::{BoundedBTreeMap, BoundedVec};
use sp_runtime::traits::{CheckedAdd, CheckedDiv, CheckedSub, Saturating, Zero};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::ops::{Add, Sub};

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

pub type OrderPrice = BalanceUnit;
pub type OrderVolume = BalanceUnit;
pub type PriceOrders<OrderId, MaxLimitOrdersForPrice> = BoundedVec<OrderId, MaxLimitOrdersForPrice>;
pub type MarketSide<MaxSidePriceCount> =
    BoundedBTreeMap<OrderPrice, OrderVolume, MaxSidePriceCount>;
pub type UserOrders<OrderId, MaxOpenedLimitOrdersPerUser> =
    BoundedVec<OrderId, MaxOpenedLimitOrdersPerUser>;

/// The public status of the order book that defines the list of allowed operations with the order book.
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

/// The internal tech status of the order book which indicates an opportunity to change the attributes or public status.
#[derive(
    Encode, Decode, PartialEq, Eq, Copy, Clone, Debug, scale_info::TypeInfo, MaxEncodedLen,
)]
pub enum OrderBookTechStatus {
    /// Order Book is enabled
    Ready,

    /// Order Book is locked during the updating
    Updating,
}

#[derive(
    Encode, Decode, PartialEq, Eq, Copy, Clone, Debug, scale_info::TypeInfo, MaxEncodedLen,
)]
pub enum CancelReason {
    /// User cancels the limit order by themself
    Manual,

    /// A lifetime of the order has expired and it is cancelled by the system
    Expired,

    /// The limit order is cancelled during alignment, because it has too small amount
    Aligned,
}

#[derive(
    Encode, Decode, Eq, PartialEq, Clone, Copy, Debug, scale_info::TypeInfo, MaxEncodedLen,
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
        matches!(
            (self, other),
            (Self::Base(..), Self::Base(..)) | (Self::Quote(..), Self::Quote(..))
        )
    }

    pub fn copy_type(&self, amount: OrderVolume) -> Self {
        match self {
            Self::Base(..) => Self::Base(amount),
            Self::Quote(..) => Self::Quote(amount),
        }
    }

    pub fn associated_asset<'a, AssetId, DEXId>(
        &'a self,
        order_book_id: &'a OrderBookId<AssetId, DEXId>,
    ) -> &AssetId {
        match self {
            Self::Base(..) => &order_book_id.base,
            Self::Quote(..) => &order_book_id.quote,
        }
    }

    pub fn average_price(input: OrderAmount, output: OrderAmount) -> Option<OrderPrice> {
        if input.is_quote() {
            input.value().checked_div(output.value())
        } else {
            output.value().checked_div(input.value())
        }
    }
}

impl Add for OrderAmount {
    type Output = Option<Self>;

    fn add(self, other: Self) -> Self::Output {
        if !self.is_same(&other) {
            return None;
        }

        let result = self.value().checked_add(other.value())?;
        Some(self.copy_type(result))
    }
}

impl Sub for OrderAmount {
    type Output = Option<Self>;

    fn sub(self, other: Self) -> Self::Output {
        if !self.is_same(&other) {
            return None;
        }

        let result = self.value().checked_sub(other.value())?;
        Some(self.copy_type(result))
    }
}

#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub enum MarketRole {
    Maker,
    Taker,
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct DealInfo<AssetId> {
    pub input_asset_id: AssetId,
    pub input_amount: OrderAmount,
    pub output_asset_id: AssetId,
    pub output_amount: OrderAmount,
    pub average_price: OrderPrice,
    pub direction: PriceVariant,
}

impl<AssetId: PartialEq> DealInfo<AssetId> {
    #[allow(clippy::nonminimal_bool)]
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

/// Instructions about payments.
/// It contains lists of which liquidity should be locked in the tech account
/// and which liquidity should be unlocked from the tech account to users.
#[derive(Eq, PartialEq, Clone, Debug)]
pub struct Payment<AssetId, AccountId, DEXId> {
    pub order_book_id: OrderBookId<AssetId, DEXId>,
    pub to_lock: BTreeMap<AssetId, BTreeMap<AccountId, OrderVolume>>,
    pub to_unlock: BTreeMap<AssetId, BTreeMap<AccountId, OrderVolume>>,
}

impl<AssetId, AccountId, DEXId> Payment<AssetId, AccountId, DEXId>
where
    AssetId: Copy + PartialEq + Ord,
    AccountId: Ord + Clone,
    DEXId: Copy + PartialEq,
{
    pub fn new(order_book_id: OrderBookId<AssetId, DEXId>) -> Self {
        Self {
            order_book_id,
            to_lock: BTreeMap::new(),
            to_unlock: BTreeMap::new(),
        }
    }

    pub fn merge(&mut self, other: &Self) -> Option<()> {
        if self.order_book_id != other.order_book_id {
            return None;
        }

        for (map, to_merge) in [
            (&mut self.to_lock, &other.to_lock),
            (&mut self.to_unlock, &other.to_unlock),
        ] {
            Self::merge_asset_map(map, to_merge);
        }

        Some(())
    }

    fn merge_account_map(
        account_map: &mut BTreeMap<AccountId, OrderVolume>,
        to_merge: &BTreeMap<AccountId, OrderVolume>,
    ) {
        for (account, volume) in to_merge {
            account_map
                .entry(account.clone())
                .and_modify(|current_volune| {
                    *current_volune = current_volune.saturating_add(*volume)
                })
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
                Locker::lock_liquidity(account, self.order_book_id, asset_id, *amount)?;
            }
        }

        Ok(())
    }

    pub fn unlock<Unlocker>(&self) -> Result<(), DispatchError>
    where
        Unlocker: CurrencyUnlocker<AccountId, AssetId, DEXId, DispatchError>,
    {
        for (asset_id, to_whom) in self.to_unlock.iter() {
            Unlocker::unlock_liquidity_batch(self.order_book_id, asset_id, to_whom)?;
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

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct MarketChange<AccountId, AssetId, DEXId, OrderId, LimitOrder> {
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
    /// Limit orders that should be placed in the order book
    pub to_place: BTreeMap<OrderId, LimitOrder>,

    /// Limit orders that should be partially executed and executed amount
    pub to_part_execute: BTreeMap<OrderId, (LimitOrder, OrderAmount)>,

    /// Limit orders that should be fully executed
    pub to_full_execute: BTreeMap<OrderId, LimitOrder>,

    /// Limit orders that should be cancelled
    pub to_cancel: BTreeMap<OrderId, (LimitOrder, CancelReason)>,

    /// Limit orders that should be forcibly updated
    pub to_force_update: BTreeMap<OrderId, LimitOrder>,

    pub payment: Payment<AssetId, AccountId, DEXId>,
    pub ignore_unschedule_error: bool,
}

impl<AccountId, AssetId, DEXId, OrderId, LimitOrder>
    MarketChange<AccountId, AssetId, DEXId, OrderId, LimitOrder>
where
    AssetId: Copy + PartialEq + Ord,
    AccountId: Ord + Clone,
    DEXId: Copy + PartialEq,
    OrderId: Copy + Ord,
{
    pub fn new(order_book_id: OrderBookId<AssetId, DEXId>) -> Self {
        Self {
            deal_input: None,
            deal_output: None,
            market_input: None,
            market_output: None,
            to_place: BTreeMap::new(),
            to_part_execute: BTreeMap::new(),
            to_full_execute: BTreeMap::new(),
            to_cancel: BTreeMap::new(),
            to_force_update: BTreeMap::new(),
            payment: Payment::new(order_book_id),
            ignore_unschedule_error: false,
        }
    }

    pub fn merge(&mut self, mut other: Self) -> Option<()> {
        let join =
            |lhs: Option<OrderAmount>, rhs: Option<OrderAmount>| -> Option<Option<OrderAmount>> {
                let result = match (lhs, rhs) {
                    (Some(left_amount), Some(right_amount)) => Some((left_amount + right_amount)?),
                    (Some(left_amount), None) => Some(left_amount),
                    (None, Some(right_amount)) => Some(right_amount),
                    (None, None) => None,
                };
                Some(result)
            };

        self.deal_input = join(self.deal_input, other.deal_input)?;
        self.deal_output = join(self.deal_output, other.deal_output)?;
        self.market_input = join(self.market_input, other.market_input)?;
        self.market_output = join(self.market_output, other.market_output)?;

        self.to_place.append(&mut other.to_place);
        self.to_part_execute.append(&mut other.to_part_execute);
        self.to_full_execute.append(&mut other.to_full_execute);
        self.to_cancel.append(&mut other.to_cancel);
        self.to_force_update.append(&mut other.to_force_update);

        self.payment.merge(&other.payment)?;

        self.ignore_unschedule_error =
            self.ignore_unschedule_error || other.ignore_unschedule_error;

        Some(())
    }

    pub fn average_deal_price(&self) -> Option<OrderPrice> {
        let (Some(input), Some(output)) = (self.deal_input, self.deal_output) else {
            return None;
        };

        OrderAmount::average_price(input, output)
    }

    pub fn deal_base_amount(&self) -> Option<OrderVolume> {
        let (Some(input), Some(output)) = (self.deal_input, self.deal_output) else {
            return None;
        };

        if input.is_base() {
            Some(*input.value())
        } else {
            Some(*output.value())
        }
    }

    pub fn count_of_executed_orders(&self) -> usize {
        self.to_full_execute
            .len()
            .saturating_add(self.to_part_execute.len())
    }
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub enum OrderBookEvent<AccountId, OrderId, Moment> {
    LimitOrderPlaced {
        order_id: OrderId,
        owner_id: AccountId,
        side: PriceVariant,
        price: OrderPrice,
        amount: OrderVolume,
        lifetime: Moment,
    },

    LimitOrderConvertedToMarketOrder {
        owner_id: AccountId,
        direction: PriceVariant,
        amount: OrderAmount,
        average_price: OrderPrice,
    },

    LimitOrderIsSplitIntoMarketOrderAndLimitOrder {
        owner_id: AccountId,
        market_order_direction: PriceVariant,
        market_order_amount: OrderAmount,
        market_order_average_price: OrderPrice,
        limit_order_id: OrderId,
    },

    LimitOrderCanceled {
        order_id: OrderId,
        owner_id: AccountId,
        reason: CancelReason,
    },

    LimitOrderExecuted {
        order_id: OrderId,
        owner_id: AccountId,
        side: PriceVariant,
        price: OrderPrice,
        amount: OrderAmount,
    },

    LimitOrderFilled {
        order_id: OrderId,
        owner_id: AccountId,
    },

    LimitOrderUpdated {
        order_id: OrderId,
        owner_id: AccountId,
        new_amount: OrderVolume,
    },

    MarketOrderExecuted {
        owner_id: AccountId,
        direction: PriceVariant,
        amount: OrderAmount,
        average_price: OrderPrice,
        to: Option<AccountId>,
    },
}
