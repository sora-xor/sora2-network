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

use crate::{Error, MarketRole, MomentOf, OrderAmount, OrderPrice, OrderVolume};
use codec::{Decode, Encode, MaxEncodedLen};
use common::PriceVariant;
use core::fmt::Debug;
use frame_support::ensure;
use frame_support::sp_runtime::DispatchError;
use frame_system::pallet_prelude::BlockNumberFor;
use sp_runtime::traits::{CheckedMul, Zero};
use sp_runtime::{SaturatedConversion, Saturating};

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

    pub time: MomentOf<T>,
    pub lifespan: MomentOf<T>,
    pub expires_at: BlockNumberFor<T>,
}

impl<T: crate::Config + Sized> LimitOrder<T> {
    pub fn new(
        id: T::OrderId,
        owner: T::AccountId,
        side: PriceVariant,
        price: OrderPrice,
        amount: OrderVolume,
        time: MomentOf<T>,
        lifespan: MomentOf<T>,
        current_block: BlockNumberFor<T>,
    ) -> Self {
        let expires_at = Self::resolve_lifespan(current_block, lifespan);
        Self {
            id,
            owner,
            side,
            price,
            original_amount: amount,
            amount,
            time,
            lifespan,
            expires_at,
        }
    }

    /// Returns block number at which to expire the order.
    /// Aims to expire no earlier than provided lifespan (in ms)
    pub fn resolve_lifespan(
        current_block: BlockNumberFor<T>,
        lifespan: MomentOf<T>,
    ) -> BlockNumberFor<T> {
        let lifespan = lifespan.saturated_into::<u64>();
        let millis_per_block: u64 = T::MILLISECS_PER_BLOCK.saturated_into::<u64>();
        let mut lifespan_blocks = lifespan.div_ceil(millis_per_block);
        // Expire after the lifespan ends.
        //
        // For example, if we want an order to live 9000 ms (or 9s, or 9/6=1.5 blocks),
        // then the order should be available for at least ceil(1.5)=2 blocks.
        //
        // Expirations happen before extrinsic dispatches, so to allow executing
        // the order at the second block, we need to expire it at the initialization of block 3.
        lifespan_blocks = lifespan_blocks.saturating_add(1);
        let lifespan = lifespan_blocks.saturated_into::<BlockNumberFor<T>>();
        current_block.saturating_add(lifespan)
    }

    pub fn ensure_valid(&self) -> Result<(), DispatchError> {
        ensure!(
            T::MIN_ORDER_LIFESPAN <= self.lifespan && self.lifespan <= T::MAX_ORDER_LIFESPAN,
            Error::<T>::InvalidLifespan
        );
        ensure!(
            !self.original_amount.is_zero(),
            Error::<T>::InvalidOrderAmount
        );
        ensure!(!self.price.is_zero(), Error::<T>::InvalidLimitOrderPrice);
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.amount.is_zero()
    }

    /// Returns appropriate deal amount of asset.
    /// Used to get total amount of associated asset if order is executed.
    ///
    /// If `base_amount_to_take` defined, it is used as `base` asset amount involved in the deal, otherwise the limit order `amount` is fully involved in the deal.
    /// `base_amount_to_take` cannot be greater then limit order `amount`.
    ///
    /// If limit order is Buy - it means maker wants to buy and taker wants to sell `amount` of `base` asset for `quote` asset at the `price`
    /// In this case if order is executed, maker receives appropriate amount of `base` asset and taker receives appropriate amount of `quote` asset.
    ///
    /// If limit order is Sell - it means maker wants to sell and taker wants to buy `amount` of `base` asset that they have for `quote` asset at the `price`
    /// In this case if order is executed, maker receives appropriate amount of `quote` asset and taker receives appropriate amount of `base` asset.
    pub fn deal_amount(
        &self,
        role: MarketRole,
        base_amount_to_take: Option<OrderVolume>,
    ) -> Result<OrderAmount, DispatchError> {
        let base_amount = if let Some(base_amount) = base_amount_to_take {
            ensure!(base_amount <= self.amount, Error::<T>::InvalidOrderAmount);
            base_amount
        } else {
            self.amount
        };

        let deal_amount =
            match (role, self.side) {
                (MarketRole::Maker, PriceVariant::Buy)
                | (MarketRole::Taker, PriceVariant::Sell) => OrderAmount::Base(base_amount),
                (MarketRole::Maker, PriceVariant::Sell)
                | (MarketRole::Taker, PriceVariant::Buy) => OrderAmount::Quote(
                    (self.price.checked_mul(&base_amount))
                        .ok_or(Error::<T>::AmountCalculationFailed)?,
                ),
            };

        Ok(deal_amount)
    }
}
