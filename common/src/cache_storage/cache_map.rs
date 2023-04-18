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
use frame_support::StorageMap;
use sp_std::cmp::Ord;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::marker::PhantomData;

pub struct CacheStorageMap<Key, Value, Storage>
where
    Key: Ord + FullEncode + Copy + Clone,
    Value: FullCodec + Clone,
    Storage: StorageMap<Key, Value>,
{
    cache: BTreeMap<Key, Option<Item<Value>>>,
    _phantom: PhantomData<Storage>,
}

impl<Key: Ord, Value, Storage> CacheStorageMap<Key, Value, Storage>
where
    Key: Ord + FullEncode + Copy + Clone,
    Value: FullCodec + Clone,
    Storage: StorageMap<Key, Value>,
{
    pub fn new() -> Self {
        Self {
            cache: BTreeMap::new(),
            _phantom: PhantomData,
        }
    }

    pub fn get(&mut self, key: Key) -> Option<&Value> {
        if !self.cache.contains_key(&key) {
            if let Ok(value) = Storage::try_get(key) {
                self.cache.insert(key, Some(Item::cache(value)));
            } else {
                self.cache.insert(key, None);
            }
        }

        if let Some(Some(item)) = self.cache.get(&key) {
            Some(&item.value)
        } else {
            None
        }
    }

    pub fn set(&mut self, key: Key, value: Value) {
        self.cache.insert(key, Some(Item::new(value)));
    }

    pub fn remove(&mut self, key: Key) {
        if let Some(Some(item)) = self.cache.get_mut(&key) {
            item.remove();
        }
    }

    pub fn commit(&mut self) {
        for (key, maybe_item) in self.cache.iter_mut() {
            if let Some(item) = maybe_item {
                match item.state {
                    State::Updated => {
                        Storage::insert(key, item.value.clone());
                        item.state = State::Original;
                    }
                    State::Removed => {
                        Storage::remove(key);
                    }
                    State::Original => {}
                }
            }
        }
        self.cache.retain(|_, v| {
            if let Some(value) = v {
                value.state == State::Original
            } else {
                false
            }
        });
    }

    pub fn reset(&mut self) {
        self.cache.clear();
    }
}
