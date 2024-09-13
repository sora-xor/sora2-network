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

use codec::{EncodeLike, FullCodec, FullEncode};
use frame_support::storage::generator;
use frame_support::storage::StorageDecodeLength;
use frame_support::{StorageDoubleMap, StorageMap, StorageValue};
use sp_core::bounded::{BoundedBTreeMap, BoundedBTreeSet, BoundedVec};
use sp_core::Get;

pub trait StorageDecodeIsFull: StorageDecodeLength {
    /// Bound on the length of the collection
    fn bound() -> usize;
    /// Decode if the storage value at `key` is full.
    ///
    /// This function assumes that the length is at the beginning of the encoded object
    /// and is a `Compact<u32>`.
    ///
    /// Returns `None` if the storage value does not exist or the decoding failed.
    fn decode_is_full(key: &[u8]) -> Option<bool> {
        Self::decode_len(key).map(|l| l >= Self::bound())
    }
}

impl<K, V, S: Get<u32>> StorageDecodeIsFull for BoundedBTreeMap<K, V, S> {
    fn bound() -> usize {
        S::get() as usize
    }
}

// TODO: commited for build
// impl<T, S: Get<u32>> StorageDecodeIsFull for BoundedBTreeSet<T, S> {
//     fn bound() -> usize {
//         S::get() as usize
//     }
// }

impl<T, S: Get<u32>> StorageDecodeIsFull for BoundedVec<T, S> {
    fn bound() -> usize {
        S::get() as usize
    }
}

pub trait DecodeIsFullValue<T: StorageDecodeIsFull> {
    fn decode_is_full() -> Option<bool>;
}

impl<T, StorageValueT> DecodeIsFullValue<T> for StorageValueT
where
    T: StorageDecodeIsFull + FullCodec,
    StorageValueT: generator::StorageValue<T>,
{
    /// Check if the storage value is full without decoding the entire value.
    ///
    /// `T` is required to implement [`StorageDecodeIsFull`].
    ///
    /// If the value does not exists or it fails to decode the length, `None` is returned.
    /// Otherwise `Some(is_full)` is returned.
    ///
    /// # Warning
    ///
    /// `None` does not mean that `get()` does not return a value. The default value is completely
    /// ignored by this function.
    fn decode_is_full() -> Option<bool> {
        T::decode_is_full(&StorageValueT::hashed_key())
    }
}

pub trait DecodeIsFullMap<K, T>
where
    K: FullEncode,
    T: StorageDecodeIsFull,
{
    fn decode_is_full(key: impl EncodeLike<K>) -> Option<bool>;
}

impl<K, T, StorageMapT> DecodeIsFullMap<K, T> for StorageMapT
where
    K: FullEncode,
    T: StorageDecodeIsFull + FullCodec,
    StorageMapT: generator::StorageMap<K, T>,
{
    /// Check if the storage value is full without decoding the entire value under the given `key`.
    ///
    /// `T` is required to implement [`StorageDecodeIsFull`].
    ///
    /// If the value does not exists or it fails to decode the length, `None` is returned.
    /// Otherwise `Some(is_full)` is returned.
    ///
    /// # Warning
    ///
    /// `None` does not mean that `get()` does not return a value. The default value is completely
    /// ignored by this function.
    fn decode_is_full(key: impl EncodeLike<K>) -> Option<bool> {
        T::decode_is_full(&StorageMapT::hashed_key_for(key))
    }
}

pub trait DecodeIsFullDoubleMap<K1, K2, T>
where
    K1: FullEncode,
    K2: FullEncode,
    T: StorageDecodeIsFull,
{
    fn decode_is_full(key1: impl EncodeLike<K1>, key2: impl EncodeLike<K2>) -> Option<bool>;
}

impl<K1, K2, T, StorageDoubleMapT> DecodeIsFullDoubleMap<K1, K2, T> for StorageDoubleMapT
where
    K1: FullEncode,
    K2: FullEncode,
    T: StorageDecodeIsFull + FullCodec,
    StorageDoubleMapT: generator::StorageDoubleMap<K1, K2, T>,
{
    /// Check if the storage value is full without decoding the entire value under the given `key`.
    ///
    /// `T` is required to implement [`StorageDecodeIsFull`].
    ///
    /// If the value does not exists or it fails to decode the length, `None` is returned.
    /// Otherwise `Some(is_full)` is returned.
    ///
    /// # Warning
    ///
    /// `None` does not mean that `get()` does not return a value. The default value is completely
    /// ignored by this function.
    fn decode_is_full(key1: impl EncodeLike<K1>, key2: impl EncodeLike<K2>) -> Option<bool> {
        T::decode_is_full(&StorageDoubleMapT::hashed_key_for(key1, key2))
    }
}

#[cfg(test)]
mod test {
    use crate::storage::{DecodeIsFullDoubleMap, DecodeIsFullMap, DecodeIsFullValue};
    use frame_support::{assert_ok, storage_alias, Twox128};
    use sp_core::bounded::{BoundedBTreeMap, BoundedBTreeSet, BoundedVec};
    use sp_core::ConstU32;
    use sp_io::TestExternalities;

    #[storage_alias]
    type Foo = StorageValue<Prefix, BoundedVec<u32, ConstU32<3>>>;
    #[storage_alias]
    type MapFoo = StorageMap<Prefix, Twox128, u32, BoundedVec<u32, ConstU32<3>>>;
    #[storage_alias]
    type DoubleMapFoo =
        StorageDoubleMap<Prefix, Twox128, u32, Twox128, u32, BoundedVec<u32, ConstU32<3>>>;

    #[test]
    fn decode_is_full_works_for_different_storages() {
        // StorageValue, BoundedVec
        TestExternalities::default().execute_with(|| {
            assert_eq!(Foo::decode_is_full(), None);
            let bounded: BoundedVec<u32, ConstU32<3>> = vec![1, 2].try_into().unwrap();
            Foo::put(bounded);
            assert_eq!(Foo::decode_is_full(), Some(false));
            assert_ok!(Foo::try_append(3));
            assert_eq!(Foo::decode_is_full(), Some(true));
            assert!(Foo::try_append(4).is_err());
            assert_eq!(Foo::decode_is_full(), Some(true));
        });

        // StorageMap, BoundedVec
        TestExternalities::default().execute_with(|| {
            assert_eq!(MapFoo::decode_is_full(0), None);
            let bounded: BoundedVec<u32, ConstU32<3>> = vec![1, 2].try_into().unwrap();
            MapFoo::insert(0, bounded);
            assert_eq!(MapFoo::decode_is_full(0), Some(false));
            assert_ok!(MapFoo::try_append(0, 3));
            assert_eq!(MapFoo::decode_is_full(0), Some(true));
            assert!(MapFoo::try_append(0, 4).is_err());
            assert_eq!(MapFoo::decode_is_full(0), Some(true));
        });

        // StorageDoubleMap, BoundedVec
        TestExternalities::default().execute_with(|| {
            assert_eq!(DoubleMapFoo::decode_is_full(0, 0), None);
            let bounded: BoundedVec<u32, ConstU32<3>> = vec![1, 2].try_into().unwrap();
            DoubleMapFoo::insert(0, 0, bounded);
            assert_eq!(DoubleMapFoo::decode_is_full(0, 0), Some(false));
            assert_ok!(DoubleMapFoo::try_append(0, 0, 3));
            assert_eq!(DoubleMapFoo::decode_is_full(0, 0), Some(true));
            assert!(DoubleMapFoo::try_append(0, 0, 4).is_err());
            assert_eq!(DoubleMapFoo::decode_is_full(0, 0), Some(true));
        });
    }

    #[storage_alias]
    type FooSet = StorageValue<Prefix, BoundedBTreeSet<u32, ConstU32<3>>>;
    #[storage_alias]
    type FooMap = StorageValue<Prefix, BoundedBTreeMap<u32, u32, ConstU32<3>>>;

    #[test]
    fn decode_is_full_works_for_different_collections() {
        // StorageValue, BoundedVec
        TestExternalities::default().execute_with(|| {
            assert_eq!(Foo::decode_is_full(), None);
            let bounded: BoundedVec<u32, ConstU32<3>> = vec![1, 2].try_into().unwrap();
            Foo::put(bounded);
            assert_eq!(Foo::decode_is_full(), Some(false));
            assert_ok!(Foo::try_append(3));
            assert_eq!(Foo::decode_is_full(), Some(true));
            assert!(Foo::try_append(4).is_err());
            assert_eq!(Foo::decode_is_full(), Some(true));
        });

        // StorageValue, BoundedBTreeSet
        TestExternalities::default().execute_with(|| {
            assert_eq!(FooSet::decode_is_full(), None);
            let bounded: BoundedBTreeSet<u32, ConstU32<3>> =
                std::collections::BTreeSet::from([1, 2]).try_into().unwrap();
            FooSet::put(bounded);
            assert_eq!(FooSet::decode_is_full(), Some(false));
            assert_ok!(FooSet::mutate(|set| set.as_mut().unwrap().try_insert(3)));
            assert_eq!(FooSet::decode_is_full(), Some(true));
            assert!(FooSet::mutate(|set| set.as_mut().unwrap().try_insert(4)).is_err());
            assert_eq!(FooSet::decode_is_full(), Some(true));
        });

        // StorageValue, BoundedBTreeMap
        TestExternalities::default().execute_with(|| {
            assert_eq!(FooMap::decode_is_full(), None);
            let bounded: BoundedBTreeMap<u32, u32, ConstU32<3>> =
                std::collections::BTreeMap::from([(1, 10), (2, 20)])
                    .try_into()
                    .unwrap();
            FooMap::put(bounded);
            assert_eq!(FooMap::decode_is_full(), Some(false));
            assert_ok!(FooMap::mutate(|map| map
                .as_mut()
                .unwrap()
                .try_insert(3, 30)));
            assert_eq!(FooMap::decode_is_full(), Some(true));
            assert!(FooMap::mutate(|map| map.as_mut().unwrap().try_insert(4, 40)).is_err());
            assert_eq!(FooMap::decode_is_full(), Some(true));
        });
    }
}
