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

// This implementation is based on substrate `Scheduler` pallet
// https://github.com/paritytech/substrate/blob/3c8666b1906680ad9461a6c46fe17439629ab082/frame/scheduler/src/lib.rs
//
// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

use crate::weights::WeightInfo;
use crate::ExpirationsAgenda;
use crate::{
    traits::ExpirationScheduler, CacheDataLayer, CancelReason, Config, DataLayer, Error, Event,
    IncompleteExpirationsSince, OrderBookId, OrderBooks, Pallet,
};
use assets::AssetIdOf;
use common::weights::check_accrue_n;
use frame_support::weights::WeightMeter;
use sp_runtime::traits::One;
use sp_runtime::{DispatchError, Saturating};

impl<T: Config> Pallet<T> {
    pub fn service_single_expiration(
        data_layer: &mut impl DataLayer<T>,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DEXId>,
        order_id: T::OrderId,
    ) {
        let order = match data_layer.get_limit_order(order_book_id, order_id) {
            Ok(o) => o,
            Err(error) => {
                // in `debug` environment will panic
                debug_assert!(
                    false,
                    "apparently removal of order book or order did not cleanup expiration schedule; \
                    order {:?} is set to expire but we cannot retrieve it: {:?}", order_id, error
                );
                // in `release` will emit event
                Self::deposit_event(Event::<T>::ExpirationFailure {
                    order_book_id: order_book_id.clone(),
                    order_id,
                    error,
                });
                return;
            }
        };
        let order_owner = order.owner.clone();
        let Some(order_book) = <OrderBooks<T>>::get(order_book_id) else {
            debug_assert!(false, "apparently removal of order book did not cleanup expiration schedule; \
                order {:?} is set to expire but corresponding order book {:?} is not found", order_id, order_book_id);
            Self::deposit_event(Event::<T>::ExpirationFailure {
                order_book_id: order_book_id.clone(),
                order_id,
                error: Error::<T>::UnknownOrderBook.into(),
            });
            return;
        };

        // It's fine to fail on unschedule again inside this method
        // since the queue is taken from the storage before this method.
        // (thus `ignore_unschedule_error` is `true`)
        match order_book.cancel_limit_order_unchecked(order, data_layer, true) {
            Ok(_) => {
                Self::deposit_event(Event::<T>::LimitOrderCanceled {
                    order_book_id: *order_book_id,
                    order_id,
                    owner_id: order_owner,
                    reason: CancelReason::Expired,
                });
            }
            Err(error) => {
                debug_assert!(
                    false,
                    "expiration of order {:?} resulted in error: {:?}",
                    order_id, error
                );
                Self::deposit_event(Event::<T>::ExpirationFailure {
                    order_book_id: order_book_id.clone(),
                    order_id,
                    error,
                });
            }
        }
    }

    /// Expire orders that are scheduled to expire at `block`.
    /// `weight` is used to track weight spent on the expirations, so that
    /// it doesn't accidentally spend weight of the entire block (or even more).
    ///
    /// Returns `true` if all expirations were processed and `false` if some expirations
    /// need to be retried when more weight is available.
    pub fn service_block(
        data_layer: &mut impl DataLayer<T>,
        block: T::BlockNumber,
        weight: &mut WeightMeter,
    ) -> bool {
        if !weight.check_accrue(<T as Config>::WeightInfo::service_block_base()) {
            return false;
        }

        let mut expirations = <ExpirationsAgenda<T>>::take(block);
        if expirations.is_empty() {
            return true;
        }
        // how many we can service with remaining weight;
        // the weight is consumed right away
        let to_service = check_accrue_n(
            weight,
            <T as Config>::WeightInfo::service_single_expiration(),
            expirations.len() as u64,
        );
        let postponed = expirations.len() as u64 - to_service;
        let mut serviced = 0;
        while let Some((order_book_id, order_id)) = expirations.last() {
            if serviced >= to_service {
                break;
            }
            Self::service_single_expiration(data_layer, order_book_id, *order_id);
            serviced += 1;
            expirations.pop();
        }
        if postponed != 0 {
            // Will later continue from this block
            <ExpirationsAgenda<T>>::insert(block, expirations);
        }
        postponed == 0
    }
}

impl<T: Config>
    ExpirationScheduler<
        T::BlockNumber,
        OrderBookId<AssetIdOf<T>, T::DEXId>,
        T::DEXId,
        T::OrderId,
        DispatchError,
    > for Pallet<T>
{
    fn service(current_block: T::BlockNumber, weight: &mut WeightMeter) {
        if !weight.check_accrue(<T as Config>::WeightInfo::service_base()) {
            return;
        }

        let mut incomplete_since = current_block + One::one();
        let mut when = IncompleteExpirationsSince::<T>::take().unwrap_or(current_block);

        let service_block_base_weight = <T as Config>::WeightInfo::service_block_base();
        let mut data_layer = CacheDataLayer::<T>::new();
        while when <= current_block && weight.can_accrue(service_block_base_weight) {
            if !Self::service_block(&mut data_layer, when, weight) {
                incomplete_since = incomplete_since.min(when);
            }
            when.saturating_inc();
        }
        incomplete_since = incomplete_since.min(when);
        if incomplete_since <= current_block {
            IncompleteExpirationsSince::<T>::put(incomplete_since);
        }
        data_layer.commit();
    }

    fn schedule(
        when: T::BlockNumber,
        order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        order_id: T::OrderId,
    ) -> Result<(), DispatchError> {
        <ExpirationsAgenda<T>>::try_mutate(when, |block_expirations| {
            block_expirations
                .try_push((order_book_id, order_id))
                .map_err(|_| Error::<T>::BlockScheduleFull.into())
        })
    }

    fn unschedule(
        when: T::BlockNumber,
        order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        order_id: T::OrderId,
    ) -> Result<(), DispatchError> {
        <ExpirationsAgenda<T>>::try_mutate(when, |block_expirations| {
            let Some(remove_index) = block_expirations.iter().position(|next| next == &(order_book_id, order_id)) else {
                return Err(Error::<T>::ExpirationNotFound.into());
            };
            block_expirations.remove(remove_index);
            Ok(())
        })
    }
}
