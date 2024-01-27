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

use crate::cache_storage::item::Item;
use codec::{FullCodec, FullEncode};
use frame_support::StorageMap;
use sp_std::cmp::Ord;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::marker::PhantomData;

/// CacheStorageMap is a wrapper of StorageMap that follows the idea one read, one write the same data.
pub struct CacheStorageMap<Key, Value, Storage>
where
    Key: Ord + FullEncode + Clone,
    Value: FullCodec + Clone + PartialEq,
    Storage: StorageMap<Key, Value>,
{
    cache: BTreeMap<Key, Option<Item<Value>>>,
    _phantom: PhantomData<Storage>,
}

impl<Key, Value, Storage> Default for CacheStorageMap<Key, Value, Storage>
where
    Key: Ord + FullEncode + Clone,
    Value: FullCodec + Clone + PartialEq,
    Storage: StorageMap<Key, Value>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<Key, Value, Storage> CacheStorageMap<Key, Value, Storage>
where
    Key: Ord + FullEncode + Clone,
    Value: FullCodec + Clone + PartialEq,
    Storage: StorageMap<Key, Value>,
{
    pub fn new() -> Self {
        Self {
            cache: BTreeMap::new(),
            _phantom: PhantomData,
        }
    }

    pub fn contains_key(&self, key: &Key) -> bool {
        if let Some(maybe_item) = self.cache.get(key) {
            if let Some(item) = maybe_item {
                *item != Item::Removed
            } else {
                false
            }
        } else {
            Storage::contains_key(key)
        }
    }

    /// Returns the cached value if it is,
    /// otherwise tries to get the value from `Storage`.
    /// If `Storage` has the value, CacheStorageMap caches it and returns.
    /// If `Storage` has no the value, the None is kept and returned.
    ///
    /// When client calls `get` with the same `key` again,
    /// the cached value or None is returned without trying to get it from `Storage`.
    pub fn get(&mut self, key: &Key) -> Option<&Value> {
        if let Some(item) = self.cache.entry(key.clone()).or_insert_with(|| {
            Storage::try_get(key)
                .ok()
                .map(|value| Item::Original(value))
        }) {
            item.value()
        } else {
            None
        }
    }

    /// Sets the value and mark it as `Updated`
    pub fn set(&mut self, key: Key, value: Value) {
        self.cache.insert(key, Some(Item::Updated(value)));
    }

    /// Marks the cached value as `Removed`. Now the None will be returned for `get` with the same `key`
    /// If there is no this cached value, then None is kept or `Removed` if `Storage` contains it.
    pub fn remove(&mut self, key: &Key) {
        self.cache
            .entry(key.clone())
            .and_modify(|maybe_item| {
                if let Some(item) = maybe_item {
                    *item = Item::Removed
                }
            })
            .or_insert_with(|| {
                if Storage::contains_key(key) {
                    Some(Item::Removed)
                } else {
                    None
                }
            });
    }

    /// Syncs all the data with `Storage`.
    /// Inserts in `Storage` all values are marked as `Updated` and marks them as `Original`.
    /// Removes from `Storage` all values are marked as `Removed`.
    /// Does nothing with `Original` values.
    /// And then removes all non-`Original` values.
    pub fn commit(&mut self) {
        for (key, maybe_item) in self.cache.iter_mut() {
            if let Some(item) = maybe_item {
                match item {
                    Item::Updated(value) => {
                        Storage::insert(key, value.clone());
                        *item = Item::Original(value.clone());
                    }
                    Item::Removed => {
                        Storage::remove(key);
                    }
                    Item::Original(_) => {}
                }
            }
        }
        self.cache
            .retain(|_, v| matches!(v, Some(Item::Original(_))));
    }

    /// Resets the cache
    pub fn reset(&mut self) {
        self.cache.clear();
    }

    /// Returns mutable reference to the cached value if it is,
    /// otherwise tries to get the value from `Storage`.
    /// If `Storage` has the value, CacheStorageDoubleMap caches it and returns
    /// mutable ref to it.
    /// If `Storage` has no the value, then `None` is kept and returned.
    ///
    /// When client calls `get` with the same `key` again,
    /// ref to the cached value or None is returned without
    /// trying to get it from `Storage`.
    pub fn get_mut(&mut self, key: &Key) -> Option<&mut Value> {
        if let Some(item) = self.cache.entry(key.clone()).or_insert_with(|| {
            Storage::try_get(key)
                .ok()
                .map(|value| Item::Original(value))
        }) {
            item.value_mut()
        } else {
            None
        }
    }
}
