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

use crate::{OrderBookId, OrderPrice, OrderVolume};
use codec::{Decode, Encode, MaxEncodedLen};
use common::balance;
use core::fmt::Debug;
use sp_runtime::traits::{One, Zero};
use sp_std::ops::Add;

#[derive(
    Encode, Decode, PartialEq, Eq, Copy, Clone, Debug, scale_info::TypeInfo, MaxEncodedLen,
)]
pub enum OrderBookStatus {
    Trade,
    PlaceAndCancel,
    OnlyCancel,
    Stop,
}

#[derive(Encode, Decode, PartialEq, Eq, Clone, Debug, scale_info::TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct OrderBook<T>
where
    T: crate::Config,
{
    pub order_book_id: OrderBookId<T>,
    pub dex_id: T::DEXId,
    pub status: OrderBookStatus,
    pub last_order_id: T::OrderId,
    pub tick_size: OrderPrice,      // price precision
    pub step_lot_size: OrderVolume, // amount precision
    pub min_lot_size: OrderVolume,
    pub max_lot_size: OrderVolume,
}

impl<T: crate::Config + Sized> OrderBook<T> {
    pub fn new(
        order_book_id: OrderBookId<T>,
        dex_id: T::DEXId,
        tick_size: OrderPrice,
        step_lot_size: OrderVolume,
        min_lot_size: OrderVolume,
        max_lot_size: OrderVolume,
    ) -> Self {
        Self {
            order_book_id: order_book_id,
            dex_id: dex_id,
            status: OrderBookStatus::Trade,
            last_order_id: T::OrderId::zero(),
            tick_size: tick_size,
            step_lot_size: step_lot_size,
            min_lot_size: min_lot_size,
            max_lot_size: max_lot_size,
        }
    }

    pub fn default(order_book_id: OrderBookId<T>, dex_id: T::DEXId) -> Self {
        Self::new(
            order_book_id,
            dex_id,
            balance!(0.00001), // TODO: order-book clarify
            balance!(0.00001), // TODO: order-book clarify
            balance!(1),       // TODO: order-book clarify
            balance!(100000),  // TODO: order-book clarify
        )
    }

    pub fn default_nft(order_book_id: OrderBookId<T>, dex_id: T::DEXId) -> Self {
        Self::new(
            order_book_id,
            dex_id,
            balance!(0.00001), // TODO: order-book clarify
            balance!(1),       // TODO: order-book clarify
            balance!(1),       // TODO: order-book clarify
            balance!(100000),  // TODO: order-book clarify
        )
    }

    pub fn next_order_id(&mut self) -> T::OrderId {
        self.last_order_id = self.last_order_id.add(T::OrderId::one());
        self.last_order_id
    }
}
