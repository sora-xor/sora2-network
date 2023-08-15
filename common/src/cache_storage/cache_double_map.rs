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
use codec::FullCodec;
use frame_support::storage::IterableStorageDoubleMap;
use sp_std::cmp::Ord;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::marker::PhantomData;

/// CacheStorageDoubleMap is a wrapper of StorageDoubleMap that follows the idea one read, one write the same data.
pub struct CacheStorageDoubleMap<Key1, Key2, Value, Storage>
where
    Key1: Ord + FullCodec + Clone,
    Key2: Ord + FullCodec + Clone,
    Value: FullCodec + Clone + PartialEq,
    Storage: IterableStorageDoubleMap<Key1, Key2, Value>,
{
    cache: BTreeMap<Key1, BTreeMap<Key2, Option<Item<Value>>>>,
    _phantom: PhantomData<Storage>,
}

impl<Key1, Key2, Value, Storage> CacheStorageDoubleMap<Key1, Key2, Value, Storage>
where
    Key1: Ord + FullCodec + Clone,
    Key2: Ord + FullCodec + Clone,
    Value: FullCodec + Clone + PartialEq,
    Storage: IterableStorageDoubleMap<Key1, Key2, Value>,
{
    pub fn new() -> Self {
        Self {
            cache: BTreeMap::new(),
            _phantom: PhantomData,
        }
    }

    pub fn contains_key(&self, key1: &Key1, key2: &Key2) -> bool {
        if let Some(second_map) = self.cache.get(key1) {
            if let Some(maybe_item) = second_map.get(key2) {
                if let Some(item) = maybe_item {
                    return *item != Item::Removed;
                } else {
                    return false;
                }
            }
        }
        Storage::contains_key(key1, key2)
    }

    /// Returns the cached value if it is,
    /// otherwise tries to get the value from `Storage`.
    /// If `Storage` has the value, CacheStorageDoubleMap caches it and returns.
    /// If `Storage` has no the value, the None is kept and returned.
    ///
    /// When client calls `get` with the same `keys` again,
    /// the cached value or None is returned without trying to get it from `Storage`.
    pub fn get(&mut self, key1: &Key1, key2: &Key2) -> Option<&Value> {
        if let Some(item) = self
            .cache
            .entry(key1.clone())
            .or_default()
            .entry(key2.clone())
            .or_insert_with(|| {
                Storage::try_get(key1, key2)
                    .ok()
                    .map(|value| Item::Original(value))
            })
        {
            item.value()
        } else {
            None
        }
    }

    /// Loads and returns all values by `key1`.
    /// Values of `Removed` items are omitted.
    pub fn get_by_prefix(&mut self, key1: &Key1) -> BTreeMap<Key2, Value> {
        self.load(key1);
        BTreeMap::from_iter(
            self.cache
                .entry(key1.clone())
                .or_default()
                .iter()
                .filter_map(|(key2, maybe_item)| {
                    if let Some(Some(value)) = maybe_item.as_ref().map(|item| item.value().cloned())
                    {
                        Some((key2.clone(), value))
                    } else {
                        None
                    }
                }),
        )
    }

    /// Sets the value and mark it as `Updated`
    pub fn set(&mut self, key1: &Key1, key2: &Key2, value: Value) {
        self.cache
            .entry(key1.clone())
            .or_default()
            .insert(key2.clone(), Some(Item::Updated(value)));
    }

    /// Marks the cached value as `Removed`. Now the None will be returned for `get` with the same keys
    /// If there is no this cached value, then None is kept or `Removed` if `Storage` contains it.
    pub fn remove(&mut self, key1: &Key1, key2: &Key2) {
        self.cache
            .entry(key1.clone())
            .or_default()
            .entry(key2.clone())
            .and_modify(|maybe_item| {
                if let Some(item) = maybe_item {
                    *item = Item::Removed
                }
            })
            .or_insert_with(|| {
                if Storage::contains_key(key1, key2) {
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
        for (key1, second_map) in self.cache.iter_mut() {
            for (key2, maybe_item) in second_map.iter_mut() {
                if let Some(item) = maybe_item {
                    match item {
                        Item::Updated(value) => {
                            Storage::insert(key1, key2, value.clone());
                            *item = Item::Original(value.clone());
                        }
                        Item::Removed => {
                            Storage::remove(key1, key2);
                        }
                        Item::Original(_) => {}
                    }
                }
            }

            second_map.retain(|_, v| {
                if let Some(Item::Original(_)) = v {
                    true
                } else {
                    false
                }
            });
        }
    }

    pub fn reset(&mut self) {
        self.cache.clear();
    }

    /// Loads and caches all values from `Storage` by `key1`.
    /// The existing cache value has higher priority and is not overwritten by the `Storage` value.
    fn load(&mut self, key1: &Key1) {
        let second_map = self.cache.entry(key1.clone()).or_default();

        for (key2, value) in Storage::iter_prefix(key1) {
            second_map
                .entry(key2)
                .or_insert(Some(Item::Original(value)));
        }
    }

    /// Returns mutable reference to the cached value if it is,
    /// otherwise tries to get the value from `Storage`.
    /// If `Storage` has the value, CacheStorageDoubleMap caches it and returns
    /// mutable ref to it.
    /// If `Storage` has no the value, then `None` is kept and returned.
    ///
    /// When client calls `get` with the same `keys` again,
    /// ref to the cached value or None is returned without
    /// trying to get it from `Storage`.
    pub fn get_mut(&mut self, key1: &Key1, key2: &Key2) -> Option<&mut Value> {
        if let Some(item) = self
            .cache
            .entry(key1.clone())
            .or_default()
            .entry(key2.clone())
            .or_insert_with(|| {
                Storage::try_get(key1, key2)
                    .ok()
                    .map(|value| Item::Original(value))
            })
        {
            item.value_mut()
        } else {
            None
        }
    }
}
