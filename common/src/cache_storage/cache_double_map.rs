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

use crate::cache_storage::item::{Item, State};
use codec::{FullCodec, FullEncode};
use frame_support::StorageDoubleMap;
use sp_std::cmp::Ord;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::marker::PhantomData;

pub struct CacheStorageDoubleMap<Key1, Key2, Value, Storage>
where
    Key1: Ord + FullEncode + Copy + Clone,
    Key2: Ord + FullEncode + Copy + Clone,
    Value: FullCodec + Clone,
    Storage: StorageDoubleMap<Key1, Key2, Value>,
{
    cache: BTreeMap<Key1, BTreeMap<Key2, Option<Item<Value>>>>,
    _phantom: PhantomData<Storage>,
}

impl<Key1, Key2, Value, Storage> CacheStorageDoubleMap<Key1, Key2, Value, Storage>
where
    Key1: Ord + FullEncode + Copy + Clone,
    Key2: Ord + FullEncode + Copy + Clone,
    Value: FullCodec + Clone,
    Storage: StorageDoubleMap<Key1, Key2, Value>,
{
    pub fn new() -> Self {
        Self {
            cache: BTreeMap::new(),
            _phantom: PhantomData,
        }
    }

    pub fn get(&mut self, key1: Key1, key2: Key2) -> Option<&Value> {
        if let Some(second_map) = self.cache.get_mut(&key1) {
            if !second_map.contains_key(&key2) {
                if let Ok(value) = Storage::try_get(key1, key2) {
                    second_map.insert(key2, Some(Item::cache(value)));
                } else {
                    second_map.insert(key2, None);
                }
            }
        } else {
            let mut second_map: BTreeMap<Key2, Option<Item<Value>>> = BTreeMap::new();
            second_map.insert(key2, None);
            self.cache.insert(key1, second_map);
        }

        if let Some(Some(item)) = self.cache.get(&key1).unwrap().get(&key2) {
            Some(&item.value)
        } else {
            None
        }
    }

    pub fn set(&mut self, key1: Key1, key2: Key2, value: Value) {
        if let Some(second_map) = self.cache.get_mut(&key1) {
            second_map.insert(key2, Some(Item::new(value)));
        } else {
            let mut second_map: BTreeMap<Key2, Option<Item<Value>>> = BTreeMap::new();
            second_map.insert(key2, Some(Item::new(value)));
            self.cache.insert(key1, second_map);
        }
    }

    pub fn remove(&mut self, key1: Key1, key2: Key2) {
        if let Some(second_map) = self.cache.get_mut(&key1) {
            if let Some(Some(item)) = second_map.get_mut(&key2) {
                item.remove();
            }
        }
    }

    pub fn commit(&mut self) {
        for (key1, second_map) in self.cache.iter_mut() {
            for (key2, maybe_item) in second_map.iter_mut() {
                if let Some(item) = maybe_item {
                    match item.state {
                        State::Updated => {
                            Storage::insert(key1, key2, item.value.clone());
                            item.state = State::Original;
                        }
                        State::Removed => {
                            Storage::remove(key1, key2);
                        }
                        State::Original => {}
                    }
                }
            }

            second_map.retain(|_, v| {
                if let Some(value) = v {
                    value.state == State::Original
                } else {
                    false
                }
            });
        }
    }

    pub fn reset(&mut self) {
        self.cache.clear();
    }
}
