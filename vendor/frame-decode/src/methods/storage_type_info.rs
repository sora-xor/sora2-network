// Copyright (C) 2022-2025 Parity Technologies (UK) Ltd. (admin@parity.io)
// This file is a part of the frame-decode crate.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//         http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::Entry;
use crate::utils::Either;
use alloc::borrow::Cow;
use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;

/// This is implemented for all metadatas exposed from `frame_metadata` and is responsible for extracting the
/// type IDs and related info needed to decode storage entries.
pub trait StorageTypeInfo {
    /// The type of type IDs that we are using to obtain type information.
    type TypeId: Clone;

    /// Get the information needed to decode a specific storage entry key/value.
    fn storage_info(
        &self,
        pallet_name: &str,
        storage_entry: &str,
    ) -> Result<StorageInfo<'_, Self::TypeId>, StorageInfoError<'_>>;
}

/// This can be implemented for anything capable of providing information about the available Storage Entries
pub trait StorageEntryInfo {
    /// Iterate over all of the available Storage Entries, returning [`Entry`] as we go.
    fn storage_entries(&self) -> impl Iterator<Item = StorageEntry<'_>>;
    /// Iterate over all of the available Storage Entries, returning a pair of `(pallet_name, constant_name)` as we go.
    fn storage_tuples(&self) -> impl Iterator<Item = (Cow<'_, str>, Cow<'_, str>)> {
        Entry::tuples_of(self.storage_entries())
    }
    /// Iterate over all of the available Storage Entries in a given pallet.
    fn storage_in_pallet(&self, pallet: &str) -> impl Iterator<Item = Cow<'_, str>> {
        Entry::entries_in(self.storage_entries(), pallet)
    }
}

/// An entry denoting a pallet or a constant name.
pub type StorageEntry<'a> = Entry<Cow<'a, str>, Cow<'a, str>>;

/// An error returned trying to access storage type information.
#[non_exhaustive]
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StorageInfoError<'info> {
    #[error("Pallet not found: {pallet_name}")]
    PalletNotFound { pallet_name: String },
    #[error("Storage item not found: {name} in pallet {pallet_name}")]
    StorageNotFound {
        name: String,
        pallet_name: Cow<'info, str>,
    },
    #[error("Cannot parse type name {name}:\n\n{reason}.")]
    #[cfg(feature = "legacy")]
    CannotParseTypeName {
        name: Cow<'info, str>,
        reason: scale_info_legacy::lookup_name::ParseError,
    },
    #[error(
        "Number of hashers and keys does not line up for {pallet_name}.{entry_name}; we have {num_hashers} hashers and {num_keys} keys."
    )]
    HasherKeyMismatch {
        entry_name: Cow<'info, str>,
        pallet_name: Cow<'info, str>,
        num_hashers: usize,
        num_keys: usize,
    },
    #[error("Cannot find type ID {id} for {pallet_name}.{entry_name}.")]
    StorageTypeNotFound {
        entry_name: Cow<'info, str>,
        pallet_name: Cow<'info, str>,
        id: u32,
    },
}

impl StorageInfoError<'_> {
    /// Take ownership of this error, turning any lifetimes to `'static`.
    pub fn into_owned(self) -> StorageInfoError<'static> {
        match self {
            StorageInfoError::PalletNotFound { pallet_name } => {
                StorageInfoError::PalletNotFound { pallet_name }
            }
            StorageInfoError::StorageNotFound { name, pallet_name } => {
                StorageInfoError::StorageNotFound {
                    name,
                    pallet_name: Cow::Owned(pallet_name.into_owned()),
                }
            }
            #[cfg(feature = "legacy")]
            StorageInfoError::CannotParseTypeName { name, reason } => {
                StorageInfoError::CannotParseTypeName {
                    name: Cow::Owned(name.into_owned()),
                    reason,
                }
            }
            StorageInfoError::HasherKeyMismatch {
                entry_name,
                pallet_name,
                num_hashers,
                num_keys,
            } => StorageInfoError::HasherKeyMismatch {
                entry_name: Cow::Owned(entry_name.into_owned()),
                pallet_name: Cow::Owned(pallet_name.into_owned()),
                num_hashers,
                num_keys,
            },
            StorageInfoError::StorageTypeNotFound {
                entry_name,
                pallet_name,
                id,
            } => StorageInfoError::StorageTypeNotFound {
                entry_name: Cow::Owned(entry_name.into_owned()),
                pallet_name: Cow::Owned(pallet_name.into_owned()),
                id,
            },
        }
    }
}

/// Information about a storage entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageInfo<'info, TypeId: Clone> {
    /// No entries if a plain storage entry, or N entries for N maps.
    pub keys: Cow<'info, [StorageKeyInfo<TypeId>]>,
    /// The type of the values.
    pub value_id: TypeId,
    /// Bytes representing the default value for this entry, if one exists.
    pub default_value: Option<Cow<'info, [u8]>>,
    /// Are we using V9 metadata prior to a change which added a new storage hasher?
    ///
    /// See https://github.com/paritytech/substrate/commit/bbb363f4320b4a72e059c0fca96af42296d5a6bf#diff-aa7bc120d701816def0f2a5eb469212d2b7021d2fc9d3b284f843f3f8089e91a
    /// Here a new hasher is added in the middle of the hashers enum. Thus, Metadata produced
    /// by V9 runtimes prior to this change will not correctly decode into `frame-metadata`'s V9
    /// which includes the change.
    ///
    /// On Kusama for instance, this should be set to true when using metadata from any spec
    /// version below 1032 in order to enable decoding correctly from it.
    pub use_old_v9_storage_hashers: bool,
}

impl<'info, TypeId: Clone + 'static> StorageInfo<'info, TypeId> {
    /// For older V9 metadatas, this needs toggling. See the docs on [`StorageInfo::use_old_v9_storage_hashers`].
    pub fn use_use_old_v9_storage_hashers(self, b: bool) -> Self {
        StorageInfo {
            use_old_v9_storage_hashers: b,
            ..self
        }
    }

    /// Take ownership of this [`StorageInfo`], turning any lifetimes to `'static`.
    pub fn into_owned(self) -> StorageInfo<'static, TypeId> {
        StorageInfo {
            keys: Cow::Owned(self.keys.into_owned()),
            value_id: self.value_id,
            default_value: self.default_value.map(|v| Cow::Owned(v.into_owned())),
            use_old_v9_storage_hashers: self.use_old_v9_storage_hashers,
        }
    }

    /// Map the type IDs in this [`StorageInfo`], returning a new one or bailing early with an error if something goes wrong.
    /// This also takes ownership of the [`StorageInfo`], turning the lifetime to static.
    pub fn map_ids<NewTypeId: Clone, E, F: FnMut(TypeId) -> Result<NewTypeId, E>>(
        self,
        mut f: F,
    ) -> Result<StorageInfo<'static, NewTypeId>, E> {
        let new_value_id = f(self.value_id)?;
        let mut new_keys = Vec::with_capacity(self.keys.len());

        for k in self.keys.iter() {
            new_keys.push(StorageKeyInfo {
                hasher: k.hasher,
                key_id: f(k.key_id.clone())?,
            });
        }

        Ok(StorageInfo {
            keys: Cow::Owned(new_keys),
            value_id: new_value_id,
            default_value: self.default_value.map(|d| Cow::Owned(d.into_owned())),
            use_old_v9_storage_hashers: false,
        })
    }
}

/// Information about a single key within a storage entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageKeyInfo<TypeId> {
    /// How is this key hashed?
    pub hasher: StorageHasher,
    /// The type of the key.
    pub key_id: TypeId,
}

/// Hasher used by storage maps
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum StorageHasher {
    /// 128-bit Blake2 hash.
    Blake2_128,
    /// 256-bit Blake2 hash.
    Blake2_256,
    /// Multiple 128-bit Blake2 hashes concatenated.
    Blake2_128Concat,
    /// 128-bit XX hash.
    Twox128,
    /// 256-bit XX hash.
    Twox256,
    /// 64-bit XX hashes concatentation.
    Twox64Concat,
    /// Identity hashing (no hashing).
    Identity,
}

impl StorageHasher {
    /// The hash produced by a [`StorageHasher`] can have these two components, in order:
    ///
    /// 1. A fixed size hash. (not present for [`StorageHasher::Identity`]).
    /// 2. The SCALE encoded key that was used as an input to the hasher (only present for
    ///    [`StorageHasher::Twox64Concat`], [`StorageHasher::Blake2_128Concat`] or [`StorageHasher::Identity`]).
    ///
    /// This function returns the number of bytes used to represent the first of these.
    pub fn len_excluding_key(&self) -> usize {
        match self {
            StorageHasher::Blake2_128Concat => 16,
            StorageHasher::Twox64Concat => 8,
            StorageHasher::Blake2_128 => 16,
            StorageHasher::Blake2_256 => 32,
            StorageHasher::Twox128 => 16,
            StorageHasher::Twox256 => 32,
            StorageHasher::Identity => 0,
        }
    }

    /// Returns true if the key used to produce the hash is appended to the hash itself.
    pub fn ends_with_key(&self) -> bool {
        matches!(
            self,
            StorageHasher::Blake2_128Concat | StorageHasher::Twox64Concat | StorageHasher::Identity
        )
    }
}

macro_rules! impl_storage_type_info_for_v14_to_v16 {
    ($path:path, $name:ident, $to_storage_hasher:ident) => {
        const _: () = {
            use scale_info::{PortableRegistry, form::PortableForm};
            use $path as path;

            fn storage_entry_type_to_storage_info<'info>(
                pallet_name: &'info str,
                entry: &'info path::StorageEntryMetadata<PortableForm>,
                types: &'info PortableRegistry,
            ) -> Result<StorageInfo<'info, u32>, StorageInfoError<'info>> {
                let default_value = match entry.modifier {
                    path::StorageEntryModifier::Optional => None,
                    path::StorageEntryModifier::Default => Some(Cow::Borrowed(&*entry.default)),
                };

                match &entry.ty {
                    path::StorageEntryType::Plain(value) => Ok(StorageInfo {
                        keys: Cow::Owned(Vec::new()),
                        value_id: value.id,
                        default_value,
                        use_old_v9_storage_hashers: false,
                    }),
                    path::StorageEntryType::Map {
                        hashers,
                        key,
                        value,
                    } => {
                        let value_id = value.id;
                        let key_id = key.id;
                        let key_ty = types.resolve(key_id).ok_or_else(|| {
                            StorageInfoError::StorageTypeNotFound {
                                pallet_name: Cow::Borrowed(pallet_name),
                                entry_name: Cow::Borrowed(&entry.name),
                                id: key_id,
                            }
                        })?;

                        if hashers.len() == 1 {
                            // One hasher, so hash the single key we have with it.
                            Ok(StorageInfo {
                                keys: Cow::Owned(Vec::from_iter([StorageKeyInfo {
                                    hasher: $to_storage_hasher(&hashers[0]),
                                    key_id,
                                }])),
                                value_id,
                                default_value,
                                use_old_v9_storage_hashers: false,
                            })
                        } else if let scale_info::TypeDef::Tuple(tuple) = &key_ty.type_def {
                            // Else, if the key is a tuple, we expect a matching number of hashers
                            // and will hash each field of the tuple with a different hasher.
                            if hashers.len() == tuple.fields.len() {
                                // One hasher per key
                                let keys = tuple
                                    .fields
                                    .iter()
                                    .zip(hashers)
                                    .map(|(field, hasher)| StorageKeyInfo {
                                        hasher: $to_storage_hasher(hasher),
                                        key_id: field.id,
                                    })
                                    .collect();
                                Ok(StorageInfo {
                                    keys,
                                    value_id,
                                    default_value,
                                    use_old_v9_storage_hashers: false,
                                })
                            } else {
                                // Hasher and key mismatch
                                Err(StorageInfoError::HasherKeyMismatch {
                                    pallet_name: Cow::Borrowed(pallet_name),
                                    entry_name: Cow::Borrowed(&entry.name),
                                    num_hashers: hashers.len(),
                                    num_keys: tuple.fields.len(),
                                })
                            }
                        } else {
                            // Multiple hashers but only one key; error.
                            Err(StorageInfoError::HasherKeyMismatch {
                                pallet_name: Cow::Borrowed(pallet_name),
                                entry_name: Cow::Borrowed(&entry.name),
                                num_hashers: hashers.len(),
                                num_keys: 1,
                            })
                        }
                    }
                }
            }

            impl StorageTypeInfo for path::$name {
                type TypeId = u32;
                fn storage_info(
                    &'_ self,
                    pallet_name: &str,
                    storage_entry: &str,
                ) -> Result<StorageInfo<'_, Self::TypeId>, StorageInfoError<'_>> {
                    let pallet = self
                        .pallets
                        .iter()
                        .find(|p| p.name.as_ref() as &str == pallet_name)
                        .ok_or_else(|| StorageInfoError::PalletNotFound {
                            pallet_name: pallet_name.to_owned(),
                        })?;

                    let storages = pallet.storage.as_ref().ok_or_else(|| {
                        StorageInfoError::StorageNotFound {
                            name: storage_entry.to_owned(),
                            pallet_name: Cow::Borrowed(&pallet.name),
                        }
                    })?;

                    let storage = storages
                        .entries
                        .iter()
                        .find(|e| e.name.as_ref() as &str == storage_entry)
                        .ok_or_else(|| StorageInfoError::StorageNotFound {
                            name: storage_entry.to_owned(),
                            pallet_name: Cow::Borrowed(&pallet.name),
                        })?;

                    storage_entry_type_to_storage_info(&pallet.name, &storage, &self.types)
                }
            }
            impl StorageEntryInfo for path::$name {
                fn storage_entries(&self) -> impl Iterator<Item = StorageEntry<'_>> {
                    self.pallets.iter().flat_map(|p| {
                        // Not strictly necessary, but we may as well filter out
                        // returning palelt names that have no entries in them.
                        let Some(storage) = &p.storage else {
                            return Either::Left(core::iter::empty());
                        };

                        Either::Right(
                            core::iter::once(Entry::In(Cow::Borrowed(&*p.name))).chain(
                                storage
                                    .entries
                                    .iter()
                                    .map(|e| Entry::Name(Cow::Borrowed(&*e.name))),
                            ),
                        )
                    })
                }
                fn storage_in_pallet(
                    &self,
                    pallet_name: &str,
                ) -> impl Iterator<Item = Cow<'_, str>> {
                    let pallet = self
                        .pallets
                        .iter()
                        .find(|p| p.name.as_ref() as &str == pallet_name);

                    let Some(pallet) = pallet else {
                        return Either::Left(core::iter::empty());
                    };
                    let Some(storage) = &pallet.storage else {
                        return Either::Left(core::iter::empty());
                    };

                    let pallet_storage = storage.entries.iter().map(|s| Cow::Borrowed(&*s.name));

                    Either::Right(pallet_storage)
                }
            }
        };
    };
}

impl_storage_type_info_for_v14_to_v16!(
    frame_metadata::v14,
    RuntimeMetadataV14,
    to_storage_hasher_v14
);
impl_storage_type_info_for_v14_to_v16!(
    frame_metadata::v15,
    RuntimeMetadataV15,
    to_storage_hasher_v15
);
impl_storage_type_info_for_v14_to_v16!(
    frame_metadata::v16,
    RuntimeMetadataV16,
    to_storage_hasher_v16
);

macro_rules! to_latest_storage_hasher {
    ($ident:ident, $path:path) => {
        fn $ident(hasher: &$path) -> StorageHasher {
            match hasher {
                <$path>::Blake2_128 => StorageHasher::Blake2_128,
                <$path>::Blake2_128Concat => StorageHasher::Blake2_128Concat,
                <$path>::Blake2_256 => StorageHasher::Blake2_256,
                <$path>::Twox128 => StorageHasher::Twox128,
                <$path>::Twox256 => StorageHasher::Twox256,
                <$path>::Twox64Concat => StorageHasher::Twox64Concat,
                <$path>::Identity => StorageHasher::Identity,
            }
        }
    };
}

to_latest_storage_hasher!(to_storage_hasher_v14, frame_metadata::v14::StorageHasher);
to_latest_storage_hasher!(to_storage_hasher_v15, frame_metadata::v15::StorageHasher);
to_latest_storage_hasher!(to_storage_hasher_v16, frame_metadata::v16::StorageHasher);

#[cfg(feature = "legacy")]
mod legacy {
    use super::*;
    use crate::utils::as_decoded;
    use alloc::format;
    use frame_metadata::decode_different::DecodeDifferent;
    use scale_info_legacy::LookupName;

    macro_rules! impl_storage_type_info_for_v8_to_v12 {
        ($path:path, $name:ident, $to_storage_hasher:ident, $is_linked_field:ident) => {
            const _: () = {
                use $path as path;
                impl StorageTypeInfo for path::$name {
                    type TypeId = LookupName;

                    fn storage_info(
                        &self,
                        pallet_name: &str,
                        storage_entry: &str,
                    ) -> Result<StorageInfo<'_, Self::TypeId>, StorageInfoError<'_>> {
                        let modules = as_decoded(&self.modules);

                        let m = modules
                            .iter()
                            .find(|m| as_decoded(&m.name).as_ref() as &str == pallet_name)
                            .ok_or_else(|| StorageInfoError::PalletNotFound {
                                pallet_name: pallet_name.to_owned(),
                            })?;

                        let pallet_name = as_decoded(&m.name);

                        let storages =
                            m.storage.as_ref().map(|s| as_decoded(s)).ok_or_else(|| {
                                StorageInfoError::StorageNotFound {
                                    name: storage_entry.to_owned(),
                                    pallet_name: Cow::Borrowed(pallet_name),
                                }
                            })?;

                        let storage = as_decoded(&storages.entries)
                            .iter()
                            .find(|s| as_decoded(&s.name).as_ref() as &str == storage_entry)
                            .ok_or_else(|| StorageInfoError::StorageNotFound {
                                name: storage_entry.to_owned(),
                                pallet_name: Cow::Borrowed(pallet_name),
                            })?;

                        let default_value = match storage.modifier {
                            path::StorageEntryModifier::Optional => None,
                            path::StorageEntryModifier::Default => {
                                Some(Cow::Borrowed(&**as_decoded(&storage.default)))
                            }
                        };

                        match &storage.ty {
                            path::StorageEntryType::Plain(ty) => {
                                let value_id = decode_lookup_name_or_err(ty, pallet_name)?;
                                Ok(StorageInfo {
                                    keys: Cow::Owned(Vec::new()),
                                    value_id,
                                    default_value,
                                    use_old_v9_storage_hashers: false,
                                })
                            }
                            path::StorageEntryType::Map {
                                hasher,
                                key,
                                value,
                                $is_linked_field: is_linked,
                                ..
                            } => {
                                // is_linked is some weird field that only appears on single-maps (not DoubleMap etc)
                                // and, if true, indicates that the value comes back with some trailing bytes pointing
                                // at the previous and next linked entry. Thus, we need to modify our output type ID
                                // to accomodate this.
                                let value_id = if *is_linked {
                                    decode_is_linked_lookup_name_or_err(value, pallet_name)?
                                } else {
                                    decode_lookup_name_or_err(value, pallet_name)?
                                };

                                let key_id = decode_lookup_name_or_err(key, pallet_name)?;
                                let hasher = $to_storage_hasher(hasher);
                                Ok(StorageInfo {
                                    keys: Cow::Owned(Vec::from_iter([StorageKeyInfo {
                                        hasher,
                                        key_id,
                                    }])),
                                    value_id,
                                    default_value,
                                    use_old_v9_storage_hashers: false,
                                })
                            }
                            path::StorageEntryType::DoubleMap {
                                hasher,
                                key1,
                                key2,
                                value,
                                key2_hasher,
                            } => {
                                let key1_id = decode_lookup_name_or_err(key1, pallet_name)?;
                                let key1_hasher = $to_storage_hasher(hasher);
                                let key2_id = decode_lookup_name_or_err(key2, pallet_name)?;
                                let key2_hasher = $to_storage_hasher(key2_hasher);
                                let value_id = decode_lookup_name_or_err(value, pallet_name)?;
                                Ok(StorageInfo {
                                    keys: Cow::Owned(Vec::from_iter([
                                        StorageKeyInfo {
                                            hasher: key1_hasher,
                                            key_id: key1_id,
                                        },
                                        StorageKeyInfo {
                                            hasher: key2_hasher,
                                            key_id: key2_id,
                                        },
                                    ])),
                                    value_id,
                                    default_value,
                                    use_old_v9_storage_hashers: false,
                                })
                            }
                        }
                    }
                }
                impl StorageEntryInfo for path::$name {
                    fn storage_entries(&self) -> impl Iterator<Item = StorageEntry<'_>> {
                        use crate::utils::as_decoded;
                        as_decoded(&self.modules).iter().flat_map(|module| {
                            let Some(storage) = &module.storage else {
                                return Either::Left(core::iter::empty());
                            };
                            let pallet = as_decoded(&module.name);
                            let storage = as_decoded(storage);
                            let entries = as_decoded(&storage.entries);

                            Either::Right(
                                core::iter::once(Entry::In(Cow::Borrowed(pallet.as_ref()))).chain(
                                    entries.iter().map(|e| {
                                        let entry = as_decoded(&e.name);
                                        Entry::Name(Cow::Borrowed(entry.as_ref()))
                                    }),
                                ),
                            )
                        })
                    }
                    fn storage_in_pallet(
                        &self,
                        pallet_name: &str,
                    ) -> impl Iterator<Item = Cow<'_, str>> {
                        let module = as_decoded(&self.modules)
                            .iter()
                            .find(|p| as_decoded(&p.name) == &pallet_name);

                        let Some(module) = module else {
                            return Either::Left(core::iter::empty());
                        };
                        let Some(storage) = &module.storage else {
                            return Either::Left(core::iter::empty());
                        };

                        let storage = as_decoded(storage);
                        let entries = as_decoded(&storage.entries);

                        let module_constants = entries.iter().map(|s| {
                            let entry_name = as_decoded(&s.name);
                            Cow::Borrowed(&**entry_name)
                        });

                        Either::Right(module_constants)
                    }
                }
            };
        };
    }

    impl_storage_type_info_for_v8_to_v12!(
        frame_metadata::v8,
        RuntimeMetadataV8,
        to_storage_hasher_v8,
        is_linked
    );
    impl_storage_type_info_for_v8_to_v12!(
        frame_metadata::v9,
        RuntimeMetadataV9,
        to_storage_hasher_v9,
        is_linked
    );
    impl_storage_type_info_for_v8_to_v12!(
        frame_metadata::v10,
        RuntimeMetadataV10,
        to_storage_hasher_v10,
        is_linked
    );
    impl_storage_type_info_for_v8_to_v12!(
        frame_metadata::v11,
        RuntimeMetadataV11,
        to_storage_hasher_v11,
        unused
    );
    impl_storage_type_info_for_v8_to_v12!(
        frame_metadata::v12,
        RuntimeMetadataV12,
        to_storage_hasher_v12,
        unused
    );

    impl StorageTypeInfo for frame_metadata::v13::RuntimeMetadataV13 {
        type TypeId = LookupName;

        fn storage_info(
            &self,
            pallet_name: &str,
            storage_entry: &str,
        ) -> Result<StorageInfo<'_, Self::TypeId>, StorageInfoError<'_>> {
            let modules = as_decoded(&self.modules);

            let m = modules
                .iter()
                .find(|m| as_decoded(&m.name).as_ref() as &str == pallet_name)
                .ok_or_else(|| StorageInfoError::PalletNotFound {
                    pallet_name: pallet_name.to_owned(),
                })?;

            let pallet_name = as_decoded(&m.name);

            let storages = m.storage.as_ref().map(as_decoded).ok_or_else(|| {
                StorageInfoError::StorageNotFound {
                    name: storage_entry.to_owned(),
                    pallet_name: Cow::Borrowed(pallet_name),
                }
            })?;

            let storage = as_decoded(&storages.entries)
                .iter()
                .find(|s| as_decoded(&s.name).as_ref() as &str == storage_entry)
                .ok_or_else(|| StorageInfoError::StorageNotFound {
                    name: storage_entry.to_owned(),
                    pallet_name: Cow::Borrowed(pallet_name),
                })?;

            let default_value = match storage.modifier {
                frame_metadata::v13::StorageEntryModifier::Optional => None,
                frame_metadata::v13::StorageEntryModifier::Default => {
                    Some(Cow::Borrowed(&**as_decoded(&storage.default)))
                }
            };

            let storage_name = as_decoded(&storage.name);

            match &storage.ty {
                frame_metadata::v13::StorageEntryType::Plain(ty) => {
                    let value_id = decode_lookup_name_or_err(ty, pallet_name)?;
                    Ok(StorageInfo {
                        keys: Cow::Owned(Vec::new()),
                        value_id,
                        default_value,
                        use_old_v9_storage_hashers: false,
                    })
                }
                frame_metadata::v13::StorageEntryType::Map {
                    hasher, key, value, ..
                } => {
                    let key_id = decode_lookup_name_or_err(key, pallet_name)?;
                    let hasher = to_storage_hasher_v13(hasher);
                    let value_id = decode_lookup_name_or_err(value, pallet_name)?;
                    Ok(StorageInfo {
                        keys: Cow::Owned(Vec::from_iter([StorageKeyInfo { hasher, key_id }])),
                        value_id,
                        default_value,
                        use_old_v9_storage_hashers: false,
                    })
                }
                frame_metadata::v13::StorageEntryType::DoubleMap {
                    hasher,
                    key1,
                    key2,
                    value,
                    key2_hasher,
                } => {
                    let key1_id = decode_lookup_name_or_err(key1, pallet_name)?;
                    let key1_hasher = to_storage_hasher_v13(hasher);
                    let key2_id = decode_lookup_name_or_err(key2, pallet_name)?;
                    let key2_hasher = to_storage_hasher_v13(key2_hasher);
                    let value_id = decode_lookup_name_or_err(value, pallet_name)?;
                    Ok(StorageInfo {
                        keys: Cow::Owned(Vec::from_iter([
                            StorageKeyInfo {
                                hasher: key1_hasher,
                                key_id: key1_id,
                            },
                            StorageKeyInfo {
                                hasher: key2_hasher,
                                key_id: key2_id,
                            },
                        ])),
                        value_id,
                        default_value,
                        use_old_v9_storage_hashers: false,
                    })
                }
                frame_metadata::v13::StorageEntryType::NMap {
                    keys,
                    hashers,
                    value,
                } => {
                    let keys = as_decoded(keys);
                    let hashers = as_decoded(hashers);
                    let value_id = decode_lookup_name_or_err(value, pallet_name)?;

                    let keys: Result<Vec<_>, StorageInfoError<'_>> = if hashers.len() == keys.len()
                    {
                        // Same number of hashers as keys? Hash each key with the hasher.
                        keys.iter()
                            .zip(hashers)
                            .map(|(key, hasher)| {
                                let hasher = to_storage_hasher_v13(hasher);
                                let key_id = lookup_name_or_err(key, pallet_name)?;
                                Ok(StorageKeyInfo { hasher, key_id })
                            })
                            .collect()
                    } else if hashers.len() == 1 {
                        // One hasher but many keys? Construct tuple of keys to be hashed together.
                        let hasher = to_storage_hasher_v13(&hashers[0]);
                        let key_string = alloc::format!("({})", keys.join(","));
                        let key_id = lookup_name_or_err(&key_string, pallet_name)?;
                        Ok(Vec::from_iter([StorageKeyInfo { hasher, key_id }]))
                    } else {
                        // Else we have a mismatch and error.
                        Err(StorageInfoError::HasherKeyMismatch {
                            pallet_name: Cow::Borrowed(pallet_name),
                            entry_name: Cow::Borrowed(storage_name),
                            num_hashers: hashers.len(),
                            num_keys: keys.len(),
                        })
                    };

                    Ok(StorageInfo {
                        keys: Cow::Owned(keys?),
                        value_id,
                        default_value,
                        use_old_v9_storage_hashers: false,
                    })
                }
            }
        }
    }
    impl StorageEntryInfo for frame_metadata::v13::RuntimeMetadataV13 {
        fn storage_entries(&self) -> impl Iterator<Item = StorageEntry<'_>> {
            use crate::utils::as_decoded;
            as_decoded(&self.modules).iter().flat_map(|module| {
                let Some(storage) = &module.storage else {
                    return Either::Left(core::iter::empty());
                };
                let pallet = as_decoded(&module.name);
                let storage = as_decoded(storage);
                let entries = as_decoded(&storage.entries);

                Either::Right(
                    core::iter::once(Entry::In(Cow::Borrowed(pallet.as_ref()))).chain(
                        entries.iter().map(|e| {
                            let entry = as_decoded(&e.name);
                            Entry::Name(Cow::Borrowed(entry.as_ref()))
                        }),
                    ),
                )
            })
        }

        fn storage_in_pallet(&self, pallet_name: &str) -> impl Iterator<Item = Cow<'_, str>> {
            let module = as_decoded(&self.modules)
                .iter()
                .find(|p| as_decoded(&p.name).as_ref() as &str == pallet_name);

            let Some(module) = module else {
                return Either::Left(core::iter::empty());
            };
            let Some(storage) = &module.storage else {
                return Either::Left(core::iter::empty());
            };

            let storage = as_decoded(storage);
            let entries = as_decoded(&storage.entries);

            let module_constants = entries.iter().map(|s| {
                let entry_name = as_decoded(&s.name);
                Cow::Borrowed(&**entry_name)
            });

            Either::Right(module_constants)
        }
    }

    fn to_storage_hasher_v8(hasher: &frame_metadata::v8::StorageHasher) -> StorageHasher {
        match hasher {
            frame_metadata::v8::StorageHasher::Blake2_128 => StorageHasher::Blake2_128,
            frame_metadata::v8::StorageHasher::Blake2_256 => StorageHasher::Blake2_256,
            frame_metadata::v8::StorageHasher::Twox128 => StorageHasher::Twox128,
            frame_metadata::v8::StorageHasher::Twox256 => StorageHasher::Twox256,
            frame_metadata::v8::StorageHasher::Twox64Concat => StorageHasher::Twox64Concat,
        }
    }
    fn to_storage_hasher_v9(hasher: &frame_metadata::v9::StorageHasher) -> StorageHasher {
        match hasher {
            frame_metadata::v9::StorageHasher::Blake2_128 => StorageHasher::Blake2_128,
            frame_metadata::v9::StorageHasher::Blake2_128Concat => StorageHasher::Blake2_128Concat,
            frame_metadata::v9::StorageHasher::Blake2_256 => StorageHasher::Blake2_256,
            frame_metadata::v9::StorageHasher::Twox128 => StorageHasher::Twox128,
            frame_metadata::v9::StorageHasher::Twox256 => StorageHasher::Twox256,
            frame_metadata::v9::StorageHasher::Twox64Concat => StorageHasher::Twox64Concat,
        }
    }
    fn to_storage_hasher_v10(hasher: &frame_metadata::v10::StorageHasher) -> StorageHasher {
        match hasher {
            frame_metadata::v10::StorageHasher::Blake2_128 => StorageHasher::Blake2_128,
            frame_metadata::v10::StorageHasher::Blake2_128Concat => StorageHasher::Blake2_128Concat,
            frame_metadata::v10::StorageHasher::Blake2_256 => StorageHasher::Blake2_256,
            frame_metadata::v10::StorageHasher::Twox128 => StorageHasher::Twox128,
            frame_metadata::v10::StorageHasher::Twox256 => StorageHasher::Twox256,
            frame_metadata::v10::StorageHasher::Twox64Concat => StorageHasher::Twox64Concat,
        }
    }

    to_latest_storage_hasher!(to_storage_hasher_v11, frame_metadata::v11::StorageHasher);
    to_latest_storage_hasher!(to_storage_hasher_v12, frame_metadata::v12::StorageHasher);
    to_latest_storage_hasher!(to_storage_hasher_v13, frame_metadata::v13::StorageHasher);

    fn decode_lookup_name_or_err<S: AsRef<str>>(
        s: &DecodeDifferent<&str, S>,
        pallet_name: &str,
    ) -> Result<LookupName, StorageInfoError<'static>> {
        let ty = sanitize_type_name(as_decoded(s).as_ref());
        lookup_name_or_err(&ty, pallet_name)
    }

    fn decode_is_linked_lookup_name_or_err<S: AsRef<str>>(
        s: &DecodeDifferent<&str, S>,
        pallet_name: &str,
    ) -> Result<LookupName, StorageInfoError<'static>> {
        let ty = sanitize_type_name(as_decoded(s).as_ref());
        // Append a hardcoded::Linked type to the end, which we expect in the type definitions
        // to be something like { previous: Option<AccountId>, next: Option<AccountId> }:
        let ty = format!("({ty}, hardcoded::Linked)");
        lookup_name_or_err(&ty, pallet_name)
    }

    fn lookup_name_or_err(
        ty: &str,
        pallet_name: &str,
    ) -> Result<LookupName, StorageInfoError<'static>> {
        let id = LookupName::parse(ty)
            .map_err(|e| StorageInfoError::CannotParseTypeName {
                name: Cow::Owned(ty.to_owned()),
                reason: e,
            })?
            .in_pallet(pallet_name);
        Ok(id)
    }

    fn sanitize_type_name(name: &str) -> Cow<'_, str> {
        if name.contains('\n') {
            Cow::Owned(name.replace('\n', ""))
        } else {
            Cow::Borrowed(name)
        }
    }
}
