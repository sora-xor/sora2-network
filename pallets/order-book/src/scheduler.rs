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

use crate::traits::{AlignmentScheduler, ExpirationScheduler};
use crate::weights::WeightInfo;
use crate::{
    AlignmentCursor, CacheDataLayer, Config, DataLayer, Error, Event, ExpirationsAgenda,
    IncompleteExpirationsSince, LimitOrder, LimitOrders, OrderBookId, OrderBookTechStatus,
    OrderBooks, Pallet,
};
use assets::AssetIdOf;
use common::weights::check_accrue_n;
use frame_support::weights::WeightMeter;
use frame_system::pallet_prelude::BlockNumberFor;
use sp_runtime::traits::{One, Zero};
use sp_runtime::{DispatchError, Saturating};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec::Vec;

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
                    order_book_id: *order_book_id,
                    order_id,
                    error,
                });
                return;
            }
        };
        let Some(order_book) = <OrderBooks<T>>::get(order_book_id) else {
            debug_assert!(
                false,
                "apparently removal of order book did not cleanup expiration schedule; \
                order {:?} is set to expire but corresponding order book {:?} is not found",
                order_id, order_book_id
            );
            Self::deposit_event(Event::<T>::ExpirationFailure {
                order_book_id: *order_book_id,
                order_id,
                error: Error::<T>::UnknownOrderBook.into(),
            });
            return;
        };

        // It's fine to fail on unschedule again inside this method
        // since the queue is taken from the storage before this method.
        // (thus `ignore_unschedule_error` is `true`)
        match order_book.expire_limit_order(order, data_layer) {
            Ok(_) => {}
            Err(error) => {
                debug_assert!(
                    false,
                    "expiration of order {:?} resulted in error: {:?}",
                    order_id, error
                );
                Self::deposit_event(Event::<T>::ExpirationFailure {
                    order_book_id: *order_book_id,
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
    pub fn service_expiration_block(
        data_layer: &mut impl DataLayer<T>,
        block: BlockNumberFor<T>,
        weight: &mut WeightMeter,
    ) -> bool {
        if !weight.check_accrue(<T as Config>::WeightInfo::service_expiration_block_base()) {
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
            true,
        );
        let mut serviced = 0;
        while let Some((order_book_id, order_id)) = expirations.pop() {
            Self::service_single_expiration(data_layer, &order_book_id, order_id);
            serviced += 1;
            if serviced >= to_service {
                break;
            }
        }
        if !expirations.is_empty() {
            // Will later continue from this block
            <ExpirationsAgenda<T>>::insert(block, expirations);

            false
        } else {
            true
        }
    }

    pub fn get_limit_orders(
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DEXId>,
        maybe_cursor: Option<T::OrderId>,
        count: usize,
    ) -> Vec<LimitOrder<T>> {
        if let Some(cursor) = maybe_cursor {
            if !cursor.is_zero() {
                let key = <LimitOrders<T>>::hashed_key_for(order_book_id, cursor);
                return <LimitOrders<T>>::iter_prefix_from(order_book_id, key)
                    .take(count)
                    .map(|(_, value)| value)
                    .collect();
            }
        }

        <LimitOrders<T>>::iter_prefix_values(order_book_id)
            .take(count)
            .collect()
    }
}

impl<T: Config>
    ExpirationScheduler<
        BlockNumberFor<T>,
        OrderBookId<AssetIdOf<T>, T::DEXId>,
        T::DEXId,
        T::OrderId,
        DispatchError,
    > for Pallet<T>
{
    fn service_expiration(current_block: BlockNumberFor<T>, weight: &mut WeightMeter) {
        if !weight.check_accrue(<T as Config>::WeightInfo::service_expiration_base()) {
            return;
        }

        let mut incomplete_since = current_block + One::one();
        let mut when = IncompleteExpirationsSince::<T>::take().unwrap_or(current_block);

        let service_block_base_weight = <T as Config>::WeightInfo::service_expiration_block_base();
        let mut data_layer = CacheDataLayer::<T>::new();
        while when <= current_block && weight.can_accrue(service_block_base_weight) {
            if !Self::service_expiration_block(&mut data_layer, when, weight) {
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

    fn schedule_expiration(
        when: BlockNumberFor<T>,
        order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        order_id: T::OrderId,
    ) -> Result<(), DispatchError> {
        <ExpirationsAgenda<T>>::try_mutate(when, |block_expirations| {
            block_expirations
                .try_push((order_book_id, order_id))
                .map_err(|_| Error::<T>::BlockScheduleFull.into())
        })
    }

    fn unschedule_expiration(
        when: BlockNumberFor<T>,
        order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        order_id: T::OrderId,
    ) -> Result<(), DispatchError> {
        <ExpirationsAgenda<T>>::try_mutate(when, |block_expirations| {
            let Some(remove_index) = block_expirations
                .iter()
                .position(|next| next == &(order_book_id, order_id))
            else {
                return Err(Error::<T>::ExpirationNotFound.into());
            };
            block_expirations.remove(remove_index);
            Ok(())
        })
    }
}

impl<T: Config> AlignmentScheduler for Pallet<T> {
    fn service_alignment(weight: &mut WeightMeter) {
        // return if it cannot align even 1 limit order
        if !weight.can_accrue(<T as Config>::WeightInfo::align_single_order()) {
            return;
        }

        let mut data = CacheDataLayer::<T>::new();

        let mut new_cursors = BTreeMap::new();
        let mut finished = Vec::new();

        for (order_book_id, cursor) in <AlignmentCursor<T>>::iter() {
            // break if it cannot align even 1 limit order for the order-book
            if !weight.can_accrue(<T as Config>::WeightInfo::align_single_order()) {
                break;
            }

            let Some(order_book) = <OrderBooks<T>>::get(order_book_id) else {
                debug_assert!(
                    false,
                    "order-book {order_book_id:?} was not found during alignment"
                );
                Self::deposit_event(Event::<T>::AlignmentFailure {
                    order_book_id,
                    error: Error::<T>::UnknownOrderBook.into(),
                });
                return;
            };

            let count = check_accrue_n(
                weight,
                <T as Config>::WeightInfo::align_single_order(),
                T::SOFT_MIN_MAX_RATIO as u64,
                false,
            );

            let limit_orders = Self::get_limit_orders(&order_book_id, Some(cursor), count as usize);

            if let Some(last) = limit_orders.last() {
                new_cursors.insert(order_book_id, last.id);
            } else {
                // it means `limit_orders` is empty
                finished.push(order_book);
                continue;
            };

            weight.defensive_saturating_accrue(
                <T as Config>::WeightInfo::align_single_order()
                    .saturating_mul(limit_orders.len() as u64),
            );

            match order_book.align_limit_orders(limit_orders, &mut data) {
                Ok(_) => (),
                Err(error) => {
                    debug_assert!(
                        false,
                        "Error {error:?} occurs during the alignment of order-book {order_book_id:?}"
                    );
                    Self::deposit_event(Event::<T>::AlignmentFailure {
                        order_book_id,
                        error,
                    });
                    return;
                }
            }
        }

        data.commit();

        for (order_book_id, new_cursor) in new_cursors {
            <AlignmentCursor<T>>::insert(order_book_id, new_cursor);
        }

        for mut order_book in finished {
            <AlignmentCursor<T>>::remove(order_book.order_book_id);

            order_book.tech_status = OrderBookTechStatus::Ready;
            <OrderBooks<T>>::set(order_book.order_book_id, Some(order_book));
        }
    }
}
