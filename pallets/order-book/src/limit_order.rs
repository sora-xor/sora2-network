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

use crate::{Error, OrderBookId, OrderPrice, OrderVolume};
use assets::AssetIdOf;
use codec::{Decode, Encode, MaxEncodedLen};
use common::prelude::FixedWrapper;
use common::PriceVariant;
use core::fmt::Debug;
use frame_support::ensure;
use frame_support::sp_runtime::DispatchError;
use sp_runtime::traits::Zero;

/// GTC Limit Order
#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq, scale_info::TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct LimitOrder<T>
where
    T: crate::Config,
{
    pub id: T::OrderId,
    pub owner: T::AccountId,
    pub side: PriceVariant,

    /// Price is specified in OrderBookId `quote` asset.
    /// It should be a base asset of DEX.
    pub price: OrderPrice,

    pub original_amount: OrderVolume,

    /// Amount of OrderBookId `base` asset
    pub amount: OrderVolume,

    pub time: T::Moment,
    pub lifespan: T::Moment,
}

impl<T: crate::Config + Sized> LimitOrder<T> {
    pub fn new(
        id: T::OrderId,
        owner: T::AccountId,
        side: PriceVariant,
        price: OrderPrice,
        amount: OrderVolume,
        time: T::Moment,
        lifespan: T::Moment,
    ) -> Self {
        Self {
            id: id,
            owner: owner,
            side: side,
            price: price,
            original_amount: amount,
            amount: amount,
            time: time,
            lifespan: lifespan,
        }
    }

    pub fn ensure_valid(&self) -> Result<(), DispatchError> {
        ensure!(
            T::MIN_ORDER_LIFETIME <= self.lifespan && self.lifespan <= T::MAX_ORDER_LIFETIME,
            Error::<T>::InvalidLifespan
        );
        ensure!(
            !self.original_amount.is_zero(),
            Error::<T>::InvalidOrderAmount
        );
        ensure!(!self.price.is_zero(), Error::<T>::InvalidLimitOrderPrice);
        Ok(())
    }

    pub fn is_expired(&self) -> bool {
        pallet_timestamp::Pallet::<T>::now() > self.time + self.lifespan
    }

    pub fn is_empty(&self) -> bool {
        self.amount.is_zero()
    }

    /// Returns appropriate amount of asset.
    /// Used to get total amount of associated asset to lock.
    ///
    /// If order is Buy - it means user wants to buy `amount` of `base` asset for `quote` asset at the `price`
    /// In this case we need to lock `quote` asset and the appropriate amount of `quote` asset is returned.
    ///
    /// If order is Sell - it means user wants to sell `amount` of `base` asset that they have for `quote` asset at the `price`
    /// In this case we need to lock `base` asset and the appropriate amount of `base` asset is returned.
    pub fn appropriate_amount(&self) -> Result<OrderVolume, DispatchError> {
        let appropriate_amount = match self.side {
            PriceVariant::Buy => (FixedWrapper::from(self.price) * FixedWrapper::from(self.amount))
                .try_into_balance()
                .map_err(|_| Error::<T>::AmountCalculationFailed)?,
            PriceVariant::Sell => self.amount,
        };
        Ok(appropriate_amount)
    }

    /// Returns appropriate asset and it's amount.
    /// Used to get proper asset and the total amount to lock.
    ///
    /// If order is Buy - it means user wants to buy `amount` of `base` asset for `quote` asset at the `price`
    /// In this case we need to lock `quote` asset. The `quote` asset and it's amount are returned.
    ///
    /// If order is Sell - it means user wants to sell `amount` of `base` asset that they have for `quote` asset at the `price`
    /// In this case we need to lock `base` asset. The `base` asset and it's amount are returned.
    pub fn appropriate_asset_and_amount<'a>(
        &'a self,
        order_book_id: &'a OrderBookId<AssetIdOf<T>>,
    ) -> Result<(&AssetIdOf<T>, OrderVolume), DispatchError> {
        let appropriate_amount = self.appropriate_amount()?;
        let appropriate_asset = match self.side {
            PriceVariant::Buy => &order_book_id.quote,
            PriceVariant::Sell => &order_book_id.base,
        };

        Ok((appropriate_asset, appropriate_amount))
    }
}
