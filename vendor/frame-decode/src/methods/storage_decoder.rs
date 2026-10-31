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

use super::storage_encoder::encode_storage_key_prefix;
use super::storage_type_info::{StorageHasher, StorageInfo, StorageTypeInfo};
use crate::methods::storage_type_info::StorageInfoError;
use crate::utils::{
    DecodableValues, DecodeErrorTrace, IntoDecodableValues, decode_with_error_tracing,
};
use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;
use scale_type_resolver::TypeResolver;

/// An error returned trying to decode storage bytes.
#[non_exhaustive]
#[allow(missing_docs)]
#[derive(Clone, Debug, thiserror::Error)]
pub enum StorageKeyDecodeError<TypeId> {
    #[error("Cannot get storage info: {0}")]
    CannotGetInfo(StorageInfoError<'static>),
    #[error(
        "The hashed storage prefix given does not match the pallet and storage name asked to decode."
    )]
    PrefixMismatch,
    #[error("Not enough bytes left: we need at least {needed} bytes but have {have} bytes")]
    NotEnoughBytes { needed: usize, have: usize },
    #[error(
        "Cannot decode storage key '{ty:?}':\n\n{reason}\n\nDecoded so far:\n\n{decoded_so_far}"
    )]
    CannotDecodeKey {
        ty: TypeId,
        reason: DecodeErrorTrace,
        decoded_so_far: StorageKey<TypeId>,
    },
}

impl<TypeId> StorageKeyDecodeError<TypeId> {
    /// Map the storage key error type IDs to something else.
    pub fn map_type_id<NewTypeId, F>(self, mut f: F) -> StorageKeyDecodeError<NewTypeId>
    where
        F: FnMut(TypeId) -> NewTypeId,
    {
        match self {
            StorageKeyDecodeError::CannotGetInfo(e) => StorageKeyDecodeError::CannotGetInfo(e),
            StorageKeyDecodeError::PrefixMismatch => StorageKeyDecodeError::PrefixMismatch,
            StorageKeyDecodeError::NotEnoughBytes { needed, have } => {
                StorageKeyDecodeError::NotEnoughBytes { needed, have }
            }
            StorageKeyDecodeError::CannotDecodeKey {
                ty,
                reason,
                decoded_so_far,
            } => StorageKeyDecodeError::CannotDecodeKey {
                ty: f(ty),
                reason,
                decoded_so_far: decoded_so_far.map_type_id(f),
            },
        }
    }
}

/// An error returned trying to decode the values in storage keys
#[non_exhaustive]
#[allow(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum StorageKeyValueDecodeError {
    #[error("Cannot decode storage value at index {index}: {error}")]
    DecodeError {
        index: usize,
        error: scale_decode::Error,
    },
    #[error("Cannot decode storage key values; need {need} values but have {have}")]
    WrongNumberOfValues { have: usize, need: usize },
    #[error(
        "There were leftover bytes after decoding a value, indicating that decoding was not successful"
    )]
    LeftoverBytes { bytes: Vec<u8> },
    #[error(
        "An invalid byte range was asked for from the key bytes, implying that the key bytes are not those that the information is about"
    )]
    InvalidRange,
}

/// An error returned trying to decode storage bytes.
#[non_exhaustive]
#[allow(missing_docs)]
#[derive(Clone, Debug)]
pub enum StorageValueDecodeError<TypeId> {
    CannotGetInfo(StorageInfoError<'static>),
    CannotDecodeValue {
        ty: TypeId,
        reason: DecodeErrorTrace,
    },
}

impl<TypeId: core::fmt::Debug> core::error::Error for StorageValueDecodeError<TypeId> {}

impl<TypeId: core::fmt::Debug> core::fmt::Display for StorageValueDecodeError<TypeId> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            StorageValueDecodeError::CannotGetInfo(storage_info_error) => {
                write!(f, "Cannot get storage info:\n\n{storage_info_error}")
            }
            StorageValueDecodeError::CannotDecodeValue { ty, reason } => {
                write!(f, "Cannot decode value with type ID {ty:?}:\n\n{reason}")
            }
        }
    }
}

impl<TypeId> StorageValueDecodeError<TypeId> {
    /// Map the storage value error type IDs to something else.
    pub fn map_type_id<NewTypeId, F>(self, mut f: F) -> StorageValueDecodeError<NewTypeId>
    where
        F: FnMut(TypeId) -> NewTypeId,
    {
        match self {
            StorageValueDecodeError::CannotGetInfo(e) => StorageValueDecodeError::CannotGetInfo(e),
            StorageValueDecodeError::CannotDecodeValue { ty, reason } => {
                StorageValueDecodeError::CannotDecodeValue { ty: f(ty), reason }
            }
        }
    }
}

/// Details about a storage key.
#[derive(Clone, Debug)]
pub struct StorageKey<TypeId> {
    parts: Vec<StorageKeyPart<TypeId>>,
}

impl<TypeId: core::fmt::Debug> core::fmt::Display for StorageKey<TypeId> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Plain entries have no keys:
        if self.parts.is_empty() {
            write!(f, "No storage parts")?;
            return Ok(());
        }

        // hash type: blake2,
        // hash range: 0..13,
        // value range: 13..23,
        // value type: AccountId
        //
        // ...
        for key in self.parts.iter() {
            writeln!(f, "Hash type: {:?}", key.hasher)?;
            writeln!(
                f,
                "Hash range: {}..{}",
                key.hash_range.start, key.hash_range.end
            )?;
            if let Some(v) = &key.value {
                writeln!(f, "Value type: {:?}", v.ty)?;
                writeln!(f, "Value range: {}..{}", v.range.start, v.range.end)?;
            }

            writeln!(f)?;
        }

        Ok(())
    }
}

impl<TypeId> core::ops::Index<usize> for StorageKey<TypeId> {
    type Output = StorageKeyPart<TypeId>;
    fn index(&self, index: usize) -> &Self::Output {
        &self.parts[index]
    }
}

impl<TypeId> StorageKey<TypeId> {
    /// Iterate over the parts of this storage key.
    pub fn parts(&self) -> impl ExactSizeIterator<Item = &StorageKeyPart<TypeId>> {
        self.parts.iter()
    }

    /// Map the storage key type IDs to something else.
    pub fn map_type_id<NewTypeId, F>(self, mut f: F) -> StorageKey<NewTypeId>
    where
        F: FnMut(TypeId) -> NewTypeId,
    {
        StorageKey {
            parts: self
                .parts
                .into_iter()
                .map(|p| p.map_type_id(&mut f))
                .collect(),
        }
    }
}

/// The decoded representation of a storage key.
#[derive(Clone, Debug)]
pub struct StorageKeyPart<TypeId> {
    hash_range: Range<u32>,
    value: Option<StorageKeyPartValue<TypeId>>,
    hasher: StorageHasher,
}

impl<TypeId> StorageKeyPart<TypeId> {
    /// The byte range of the hash for this storage key part.
    pub fn hash_range(&self) -> Range<usize> {
        Range {
            start: self.hash_range.start as usize,
            end: self.hash_range.end as usize,
        }
    }

    /// The hasher used for this storage key part.
    pub fn hasher(&self) -> StorageHasher {
        self.hasher
    }

    /// If applicable (ie this part uses a concat or ident hasher), return information
    /// about the value encoded into this hash.
    pub fn value(&self) -> Option<&StorageKeyPartValue<TypeId>> {
        self.value.as_ref()
    }

    /// Map the storage part type ID to something else.
    pub fn map_type_id<NewTypeId, F>(self, f: F) -> StorageKeyPart<NewTypeId>
    where
        F: FnMut(TypeId) -> NewTypeId,
    {
        StorageKeyPart {
            hash_range: self.hash_range,
            value: self.value.map(|v| v.map_type_id(f)),
            hasher: self.hasher,
        }
    }
}

/// Information about the value contained within a storage key part hash.
#[derive(Clone, Debug)]
pub struct StorageKeyPartValue<TypeId> {
    range: Range<u32>,
    ty: TypeId,
}

impl<TypeId> StorageKeyPartValue<TypeId> {
    /// The byte range for this value in the storage key.
    pub fn range(&self) -> Range<usize> {
        Range {
            start: self.range.start as usize,
            end: self.range.end as usize,
        }
    }

    /// The type ID for this value.
    pub fn ty(&self) -> &TypeId {
        &self.ty
    }

    /// Map the storage part type ID to something else.
    pub fn map_type_id<NewTypeId, F>(self, mut f: F) -> StorageKeyPartValue<NewTypeId>
    where
        F: FnMut(TypeId) -> NewTypeId,
    {
        StorageKeyPartValue {
            range: self.range,
            ty: f(self.ty),
        }
    }
}

/// Decode a storage key, returning information about it.
///
/// This information can be used to identify and, where possible, decode the parts of the storage key.
///
/// # Example
///
/// Here, we decode some storage keys from a block.
///
/// ```rust
/// use frame_decode::storage::decode_storage_key;
/// use frame_decode::helpers::decode_with_visitor;
/// use frame_metadata::RuntimeMetadata;
/// use parity_scale_codec::Decode;
/// use scale_value::scale::ValueVisitor;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata_10000000_9180.scale").unwrap();
/// let RuntimeMetadata::V14(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// let storage_keyval_bytes = std::fs::read("artifacts/storage_10000000_9180_system_account.json").unwrap();
/// let storage_keyval_hex: Vec<(String, String)> = serde_json::from_slice(&storage_keyval_bytes).unwrap();
///
/// for (key, _val) in storage_keyval_hex {
///     let key_bytes = hex::decode(key.trim_start_matches("0x")).unwrap();
///
///     // Decode the storage key, returning information about it:
///     let storage_info = decode_storage_key(
///         "System",
///         "Account",
///         &mut &*key_bytes,
///         &metadata,
///         &metadata.types
///     ).unwrap();
///
///     for part in storage_info.parts() {
///         // Access information about the hasher for this part of the key:
///         let hash_bytes = &key_bytes[part.hash_range()];
///         let hasher = part.hasher();
///
///         // If the value is encoded as part of the hasher, we can find and
///         // decode the value too:
///         if let Some(value_info) = part.value() {
///             let value_bytes = &key_bytes[value_info.range()];
///             let value = decode_with_visitor(
///                 &mut &*value_bytes,
///                 *value_info.ty(),
///                 &metadata.types,
///                 ValueVisitor::new()
///             ).unwrap();
///         }
///     }
/// }
/// ```
pub fn decode_storage_key<Info, Resolver>(
    pallet_name: &str,
    storage_entry: &str,
    cursor: &mut &[u8],
    info: &Info,
    type_resolver: &Resolver,
) -> Result<StorageKey<Info::TypeId>, StorageKeyDecodeError<Info::TypeId>>
where
    Info: StorageTypeInfo,
    Info::TypeId: Clone + core::fmt::Debug,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
{
    let storage_info = info
        .storage_info(pallet_name, storage_entry)
        .map_err(|e| StorageKeyDecodeError::CannotGetInfo(e.into_owned()))?;

    // Sanity check that the storage key prefix is what we expect:
    let expected_prefix = encode_storage_key_prefix(pallet_name, storage_entry);
    if cursor[..32] != expected_prefix {
        return Err(StorageKeyDecodeError::PrefixMismatch);
    }

    decode_storage_key_with_info(cursor, &storage_info, type_resolver)
}

/// Decode a storage key, returning information about it.
///
/// Unlike [`decode_storage_key`], which obtains the storage info internally given the pallet and storage entry names,
/// this function takes the storage info as an argument. This is useful if you already have the storage info available,
/// for example if you are decoding multiple keys for the same storage entry.
///
/// # Warning
///
/// Unlike [`decode_storage_key`], this does not check that the bytes start with the expected prefix; ensuring that the
/// storage information lines up with the bytes is the caller's responsibility.
pub fn decode_storage_key_with_info<Resolver>(
    cursor: &mut &[u8],
    storage_info: &StorageInfo<<Resolver as TypeResolver>::TypeId>,
    type_resolver: &Resolver,
) -> Result<
    StorageKey<<Resolver as TypeResolver>::TypeId>,
    StorageKeyDecodeError<<Resolver as TypeResolver>::TypeId>,
>
where
    Resolver: TypeResolver,
    <Resolver as TypeResolver>::TypeId: Clone + core::fmt::Debug,
{
    let bytes = *cursor;
    let curr_idx = |cursor: &mut &[u8]| (bytes.len() - cursor.len()) as u32;

    let _prefix = strip_bytes(cursor, 32)?;

    let mut parts = vec![];
    for key in &*storage_info.keys {
        let hasher = if storage_info.use_old_v9_storage_hashers {
            map_to_old_v9_storage_hashers(key.hasher)
        } else {
            key.hasher
        };

        let start_idx = curr_idx(cursor);
        let part = match &hasher {
            StorageHasher::Blake2_128 | StorageHasher::Twox128 => {
                strip_bytes(cursor, 16)?;
                StorageKeyPart {
                    hash_range: Range {
                        start: start_idx,
                        end: curr_idx(cursor),
                    },
                    value: None,
                    hasher,
                }
            }
            StorageHasher::Blake2_256 | StorageHasher::Twox256 => {
                strip_bytes(cursor, 32)?;
                StorageKeyPart {
                    hash_range: Range {
                        start: start_idx,
                        end: curr_idx(cursor),
                    },
                    value: None,
                    hasher,
                }
            }
            StorageHasher::Blake2_128Concat => {
                strip_bytes(cursor, 16)?;
                let hash_end_idx = curr_idx(cursor);
                decode_with_error_tracing(
                    cursor,
                    key.key_id.clone(),
                    type_resolver,
                    scale_decode::visitor::IgnoreVisitor::new(),
                )
                .map_err(|e| StorageKeyDecodeError::CannotDecodeKey {
                    ty: key.key_id.clone(),
                    reason: e,
                    decoded_so_far: StorageKey {
                        parts: parts.clone(),
                    },
                })?;
                StorageKeyPart {
                    hash_range: Range {
                        start: start_idx,
                        end: hash_end_idx,
                    },
                    value: Some(StorageKeyPartValue {
                        range: Range {
                            start: hash_end_idx,
                            end: curr_idx(cursor),
                        },
                        ty: key.key_id.clone(),
                    }),
                    hasher,
                }
            }
            StorageHasher::Twox64Concat => {
                strip_bytes(cursor, 8)?;
                let hash_end_idx = curr_idx(cursor);
                decode_with_error_tracing(
                    cursor,
                    key.key_id.clone(),
                    type_resolver,
                    scale_decode::visitor::IgnoreVisitor::new(),
                )
                .map_err(|e| StorageKeyDecodeError::CannotDecodeKey {
                    ty: key.key_id.clone(),
                    reason: e,
                    decoded_so_far: StorageKey {
                        parts: parts.clone(),
                    },
                })?;
                StorageKeyPart {
                    hash_range: Range {
                        start: start_idx,
                        end: hash_end_idx,
                    },
                    value: Some(StorageKeyPartValue {
                        range: Range {
                            start: hash_end_idx,
                            end: curr_idx(cursor),
                        },
                        ty: key.key_id.clone(),
                    }),
                    hasher,
                }
            }
            StorageHasher::Identity => {
                decode_with_error_tracing(
                    cursor,
                    key.key_id.clone(),
                    type_resolver,
                    scale_decode::visitor::IgnoreVisitor::new(),
                )
                .map_err(|e| StorageKeyDecodeError::CannotDecodeKey {
                    ty: key.key_id.clone(),
                    reason: e,
                    decoded_so_far: StorageKey {
                        parts: parts.clone(),
                    },
                })?;
                StorageKeyPart {
                    hash_range: Range {
                        start: start_idx,
                        end: start_idx,
                    },
                    value: Some(StorageKeyPartValue {
                        range: Range {
                            start: start_idx,
                            end: curr_idx(cursor),
                        },
                        ty: key.key_id.clone(),
                    }),
                    hasher,
                }
            }
        };
        parts.push(part)
    }

    Ok(StorageKey { parts })
}

/// Attempt to decode the values attached to parts of a storage key into the provided output type.
/// [`decode_storage_key`] and related functions take a storage key and return information (in the
/// form of [`StorageKey`]) which describes each of the parts of the key.
///
/// This function takes that information and returns a user-defined generic type which implements
/// [`IntoDecodableValues`], and attempts to decode some or all of the values from it (in order).
///
/// # Example
///
/// Here, we decode some storage keys from a block.
///
/// ```rust
/// use frame_decode::storage::{ decode_storage_key, decode_storage_key_values };
/// use frame_decode::helpers::decode_with_visitor;
/// use frame_metadata::RuntimeMetadata;
/// use parity_scale_codec::Decode;
/// use scale_value::scale::ValueVisitor;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata_10000000_9180.scale").unwrap();
/// let RuntimeMetadata::V14(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// let storage_keyval_bytes = std::fs::read("artifacts/storage_10000000_9180_system_account.json").unwrap();
/// let storage_keyval_hex: Vec<(String, String)> = serde_json::from_slice(&storage_keyval_bytes).unwrap();
///
/// for (key, _val) in storage_keyval_hex {
///     let key_bytes = hex::decode(key.trim_start_matches("0x")).unwrap();
///
///     // First decode the storage key, returning information about it:
///     let storage_info = decode_storage_key(
///         "System",
///         "Account",
///         &mut &*key_bytes,
///         &metadata,
///         &metadata.types
///     ).unwrap();
///
///     // Use this information to decode any values within the key. Here
///     // we ask for the first (and here only) value present, which we know to
///     // be decodable into a [u8; 32] because it's an AccountId32.
///     let values: ([u8;32],) = decode_storage_key_values(
///         &key_bytes,
///         &storage_info,
///         &metadata.types
///     ).unwrap();
///
///     println!("Account ID Hex: {}", hex::encode(&values.0));
///
///     // If we don't know what we are decoding, we can target a Vec<scale_value::Value>
///     // which allows arbitrary items to be decoded into it.
///     let values: Vec<scale_value::Value> = decode_storage_key_values(
///         &key_bytes,
///         &storage_info,
///         &metadata.types
///     ).unwrap();
///
///     println!("All values extracted from key:");
///     for value in values {
///         println!("  {value}");
///     }
/// }
/// ```
pub fn decode_storage_key_values<Values, Resolver>(
    key_bytes: &[u8],
    decoded_key: &StorageKey<Resolver::TypeId>,
    types: &Resolver,
) -> Result<Values, StorageKeyValueDecodeError>
where
    Values: IntoDecodableValues,
    Resolver: TypeResolver,
{
    let num_values = decoded_key
        .parts()
        .filter(|part| part.value.is_some())
        .count();

    let needed_values = Values::num_decodable_values();

    // If a specific number of values are needed, ensure that we have enough available.
    if let Some(needed_values) = needed_values
        && num_values < needed_values
    {
        return Err(StorageKeyValueDecodeError::WrongNumberOfValues {
            have: num_values,
            need: needed_values,
        });
    }

    let mut decode_target = Values::into_decodable_values();

    // Iterate over **at most** needed_values, or all of the available values if no limit.
    let value_info_iter =
        (0..needed_values.unwrap_or(usize::MAX)).zip(decoded_key.parts().filter_map(|p| p.value()));

    for (idx, value_info) in value_info_iter {
        // This will panic if the key bytes provided don't line up with the information.
        let value_bytes = &mut key_bytes
            .get(value_info.range())
            .ok_or(StorageKeyValueDecodeError::InvalidRange)?;
        let value_ty = value_info.ty().clone();

        decode_target
            .decode_next_value(value_bytes, value_ty, types)
            .map_err(|e| StorageKeyValueDecodeError::DecodeError {
                index: idx,
                error: e,
            })?;

        if !value_bytes.is_empty() {
            return Err(StorageKeyValueDecodeError::LeftoverBytes {
                bytes: value_bytes.to_vec(),
            });
        }
    }

    Ok(decode_target.decoded_target())
}

/// Decode a storage value.
///
/// # Example
///
/// Here, we decode some storage values from a block.
///
/// ```rust
/// use frame_decode::storage::decode_storage_value;
/// use frame_decode::helpers::decode_with_visitor;
/// use frame_metadata::RuntimeMetadata;
/// use parity_scale_codec::Decode;
/// use scale_value::scale::ValueVisitor;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata_10000000_9180.scale").unwrap();
/// let RuntimeMetadata::V14(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// let storage_keyval_bytes = std::fs::read("artifacts/storage_10000000_9180_system_account.json").unwrap();
/// let storage_keyval_hex: Vec<(String, String)> = serde_json::from_slice(&storage_keyval_bytes).unwrap();
///
/// for (_key, val) in storage_keyval_hex {
///     let value_bytes = hex::decode(val.trim_start_matches("0x")).unwrap();
///
///     // Decode the storage value, here into a scale_value::Value:
///     let account_value = decode_storage_value(
///         "System",
///         "Account",
///         &mut &*value_bytes,
///         &metadata,
///         &metadata.types,
///         ValueVisitor::new()
///     ).unwrap();
/// }
/// ```
pub fn decode_storage_value<'scale, 'resolver, Info, Resolver, V>(
    pallet_name: &str,
    storage_entry: &str,
    cursor: &mut &'scale [u8],
    info: &Info,
    type_resolver: &'resolver Resolver,
    visitor: V,
) -> Result<V::Value<'scale, 'resolver>, StorageValueDecodeError<Info::TypeId>>
where
    Info: StorageTypeInfo,
    Info::TypeId: Clone + core::fmt::Debug,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
    V: scale_decode::Visitor<TypeResolver = Resolver>,
    V::Error: core::fmt::Debug,
{
    let storage_info = info
        .storage_info(pallet_name, storage_entry)
        .map_err(|e| StorageValueDecodeError::CannotGetInfo(e.into_owned()))?;

    decode_storage_value_with_info(cursor, &storage_info, type_resolver, visitor)
}

/// Decode a storage value.
///
/// Unlike [`decode_storage_value`], which obtains the storage info internally given the pallet and storage entry names,
/// this function takes the storage info as an argument. This is useful if you already have the storage info available,
/// for example if you are decoding multiple keys for the same storage entry.
pub fn decode_storage_value_with_info<'scale, 'resolver, V>(
    cursor: &mut &'scale [u8],
    storage_info: &StorageInfo<<V::TypeResolver as TypeResolver>::TypeId>,
    type_resolver: &'resolver V::TypeResolver,
    visitor: V,
) -> Result<
    V::Value<'scale, 'resolver>,
    StorageValueDecodeError<<V::TypeResolver as TypeResolver>::TypeId>,
>
where
    V: scale_decode::Visitor,
    V::Error: core::fmt::Debug,
{
    let value_id = storage_info.value_id.clone();

    decode_with_error_tracing(cursor, value_id.clone(), type_resolver, visitor).map_err(|e| {
        StorageValueDecodeError::CannotDecodeValue {
            ty: value_id,
            reason: e,
        }
    })
}

/// Decode the default storage value given some [`StorageInfo`].
///
/// The resulting value may be tied to the lifetime of the [`StorageInfo`] being provided if the implementation decides to borrow
/// from it. This is also why no `decode_default_storage_value` function exists; the [`StorageInfo`] must outlive this call.
pub fn decode_default_storage_value_with_info<'info, 'resolver, V>(
    storage_info: &'info StorageInfo<<V::TypeResolver as TypeResolver>::TypeId>,
    type_resolver: &'resolver V::TypeResolver,
    visitor: V,
) -> Result<
    Option<V::Value<'info, 'resolver>>,
    StorageValueDecodeError<<V::TypeResolver as TypeResolver>::TypeId>,
>
where
    V: scale_decode::Visitor,
    V::Error: core::fmt::Debug,
{
    let value_id = storage_info.value_id.clone();

    let Some(default_bytes) = &storage_info.default_value else {
        return Ok(None);
    };
    let value = decode_with_error_tracing(
        &mut &**default_bytes,
        value_id.clone(),
        type_resolver,
        visitor,
    )
    .map_err(|e| StorageValueDecodeError::CannotDecodeValue {
        ty: value_id,
        reason: e,
    })?;

    Ok(Some(value))
}

fn strip_bytes<'a, T>(
    cursor: &mut &'a [u8],
    num: usize,
) -> Result<&'a [u8], StorageKeyDecodeError<T>> {
    let bytes = cursor
        .get(..num)
        .ok_or_else(|| StorageKeyDecodeError::NotEnoughBytes {
            needed: num,
            have: cursor.len(),
        })?;

    *cursor = &cursor[num..];
    Ok(bytes)
}

fn map_to_old_v9_storage_hashers(hasher: StorageHasher) -> StorageHasher {
    match hasher {
        // See https://github.com/paritytech/substrate/commit/bbb363f4320b4a72e059c0fca96af42296d5a6bf#diff-aa7bc120d701816def0f2a5eb469212d2b7021d2fc9d3b284f843f3f8089e91a.
        // We SCALE decode into the "new" variants seen there, but need to translate back to the old ones, which
        // is what was actually encoded from the runtime.
        StorageHasher::Blake2_128 => StorageHasher::Blake2_128,
        StorageHasher::Blake2_256 => StorageHasher::Blake2_256,
        StorageHasher::Blake2_128Concat => StorageHasher::Twox128,
        StorageHasher::Twox128 => StorageHasher::Twox256,
        StorageHasher::Twox256 => StorageHasher::Twox64Concat,

        // These types didn't exist prior to the change, so just leave them alone:
        StorageHasher::Twox64Concat => StorageHasher::Twox64Concat,
        StorageHasher::Identity => StorageHasher::Identity,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_strip_bytes() {
        let v = vec![0, 1, 2, 3, 4, 5, 6, 7, 8];
        let cursor = &mut &*v;
        let stripped = strip_bytes::<()>(cursor, 4).unwrap();
        assert_eq!(stripped, &[0, 1, 2, 3]);
        assert_eq!(cursor, &[4, 5, 6, 7, 8]);
    }
}
