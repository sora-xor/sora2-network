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

use super::storage_type_info::{StorageHasher, StorageInfo, StorageTypeInfo};
use crate::methods::storage_type_info::StorageInfoError;
use crate::utils::{EncodableValues, IntoEncodableValues};
use alloc::vec::Vec;
use scale_type_resolver::TypeResolver;

/// An error returned trying to encode storage keys.
#[non_exhaustive]
#[allow(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum StorageKeyEncodeError {
    #[error("Cannot get storage info: {0}")]
    CannotGetInfo(StorageInfoError<'static>),
    #[error("Failed to encode storage key: {0}")]
    EncodeError(#[from] scale_encode::Error),
    #[error("Too many keys provided: expected at most {max_keys_expected}")]
    TooManyKeysProvided {
        /// The maximum number of keys that were expected.
        max_keys_expected: usize,
    },
}

/// Encode a storage key prefix from a pallet name and storage entry name. This prefix
/// is the first 32 bytes of any storage key which comes from a pallet, and is essentially
/// `twox_128(pallet_name) + twox_128(storage_entry_name)`.
pub fn encode_storage_key_prefix(pallet_name: &str, storage_entry: &str) -> [u8; 32] {
    let mut prefix = [0u8; 32];

    let pallet_bytes = sp_crypto_hashing::twox_128(pallet_name.as_bytes());
    let entry_bytes = sp_crypto_hashing::twox_128(storage_entry.as_bytes());

    prefix[..16].copy_from_slice(&pallet_bytes);
    prefix[16..].copy_from_slice(&entry_bytes);

    prefix
}

/// Encode a complete storage key for a given pallet and storage entry and a set of keys that
/// are each able to be encoded via [`scale_encode::EncodeAsType`].
///
/// This is the same as [`encode_storage_key_to`], but returns the encoded key as a `Vec<u8>`, rather
/// than accepting a mutable output buffer.
///
/// # Example
///
/// ```rust
/// use frame_decode::storage::encode_storage_key;
/// use frame_metadata::RuntimeMetadata;
/// use parity_scale_codec::Decode;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata_10000000_9180.scale").unwrap();
/// let RuntimeMetadata::V14(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// let account_id = [0u8; 32];
///
/// // System.Account needs only one key to point at a specific value; the account ID.
/// // We just fake an account ID for this example  by providing 32 0 bytes, but anything
/// // which would `scale_encode::EncodeAsType` into 32 bytes would work.
/// let encoded_key = encode_storage_key(
///     "System",
///     "Account",
///     &[account_id],
///     &metadata,
///     &metadata.types,
/// ).unwrap();
/// ```
pub fn encode_storage_key<Info, Resolver, Keys>(
    pallet_name: &str,
    storage_entry: &str,
    keys: Keys,
    info: &Info,
    type_resolver: &Resolver,
) -> Result<Vec<u8>, StorageKeyEncodeError>
where
    Keys: IntoEncodableValues,
    Info: StorageTypeInfo,
    Info::TypeId: Clone + core::fmt::Debug,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
{
    // pre-allocate at least as many bytes as we need for the root/prefix.
    let mut out = Vec::with_capacity(32);
    encode_storage_key_to(
        pallet_name,
        storage_entry,
        keys,
        info,
        type_resolver,
        &mut out,
    )?;
    Ok(out)
}

/// Encode a complete storage key for a given pallet and storage entry and a set of keys that
/// are each able to be encoded via [`scale_encode::EncodeAsType`].
///
/// # Example
///
/// ```rust
/// use frame_decode::storage::encode_storage_key_to;
/// use frame_metadata::RuntimeMetadata;
/// use parity_scale_codec::Decode;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata_10000000_9180.scale").unwrap();
/// let RuntimeMetadata::V14(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// let account_id = [0u8; 32];
///
/// // We'll encode the key to this.
/// let mut encoded_key = Vec::new();
///
/// // System.Account needs only one key to point at a specific value; the account ID.
/// // We just fake an account ID for this example  by providing 32 0 bytes, but anything
/// // which would `scale_encode::EncodeAsType` into 32 bytes would work.
/// encode_storage_key_to(
///     "System",
///     "Account",
///     &[account_id],
///     &metadata,
///     &metadata.types,
///     &mut encoded_key,
/// ).unwrap();
/// ```
pub fn encode_storage_key_to<Info, Resolver, Keys>(
    pallet_name: &str,
    storage_entry: &str,
    keys: Keys,
    info: &Info,
    type_resolver: &Resolver,
    out: &mut Vec<u8>,
) -> Result<(), StorageKeyEncodeError>
where
    Keys: IntoEncodableValues,
    Info: StorageTypeInfo,
    Info::TypeId: Clone + core::fmt::Debug,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
{
    let storage_info = info
        .storage_info(pallet_name, storage_entry)
        .map_err(|e| StorageKeyEncodeError::CannotGetInfo(e.into_owned()))?;

    encode_storage_key_with_info_to(
        pallet_name,
        storage_entry,
        keys,
        &storage_info,
        type_resolver,
        out,
    )
}

/// Encode a complete storage key for a given pallet and storage entry and a set of keys that
/// are each able to be encoded via [`scale_encode::EncodeAsType`].
///
/// Unlike [`encode_storage_key`], which obtains the storage info internally given the pallet and storage entry names,
/// this function takes the storage info as an argument. This is useful if you already have the storage info available,
/// for example if you are encoding multiple keys for the same storage entry.
pub fn encode_storage_key_with_info<Resolver, Keys>(
    pallet_name: &str,
    storage_entry: &str,
    keys: Keys,
    storage_info: &StorageInfo<<Resolver as TypeResolver>::TypeId>,
    type_resolver: &Resolver,
) -> Result<Vec<u8>, StorageKeyEncodeError>
where
    Keys: IntoEncodableValues,
    Resolver: TypeResolver,
    <Resolver as TypeResolver>::TypeId: Clone + core::fmt::Debug,
{
    let mut out = Vec::with_capacity(32);
    encode_storage_key_with_info_to(
        pallet_name,
        storage_entry,
        keys,
        storage_info,
        type_resolver,
        &mut out,
    )?;
    Ok(out)
}

/// Encode a complete storage key for a given pallet and storage entry and a set of keys that
/// are each able to be encoded via [`scale_encode::EncodeAsType`]. Write the response to the provided `Vec`.
///
/// Unlike [`encode_storage_key`], which obtains the storage info internally given the pallet and storage entry names,
/// this function takes the storage info as an argument. This is useful if you already have the storage info available,
/// for example if you are encoding multiple keys for the same storage entry.
pub fn encode_storage_key_with_info_to<Resolver, Keys>(
    pallet_name: &str,
    storage_entry: &str,
    keys: Keys,
    storage_info: &StorageInfo<<Resolver as TypeResolver>::TypeId>,
    type_resolver: &Resolver,
    out: &mut Vec<u8>,
) -> Result<(), StorageKeyEncodeError>
where
    Keys: IntoEncodableValues,
    Resolver: TypeResolver,
    <Resolver as TypeResolver>::TypeId: Clone + core::fmt::Debug,
{
    let num_encodable_values = keys.num_encodable_values();

    // If we provide more encodable values than there are keys, bail.
    // If we provide less, that's ok and we just don't encode every part of the key
    // (useful if eg iterating a bunch of entries under some prefix of a key).
    if num_encodable_values > storage_info.keys.len() {
        return Err(StorageKeyEncodeError::TooManyKeysProvided {
            max_keys_expected: storage_info.keys.len(),
        });
    }

    // Encode the prefix:
    let prefix = encode_storage_key_prefix(pallet_name, storage_entry);
    out.extend_from_slice(&prefix);

    // Encode the keys:
    let mut keys = keys.into_encodable_values();
    let mut temp = Vec::with_capacity(32);
    let iter = (0..num_encodable_values)
        .zip(&*storage_info.keys)
        .map(|(_, k)| k);

    for key_info in iter {
        keys.encode_next_value_to(key_info.key_id.clone(), type_resolver, &mut temp)
            .map_err(StorageKeyEncodeError::EncodeError)?;

        match key_info.hasher {
            StorageHasher::Blake2_128 => {
                let hash = sp_crypto_hashing::blake2_128(&temp);
                out.extend_from_slice(&hash);
            }
            StorageHasher::Blake2_256 => {
                let hash = sp_crypto_hashing::blake2_256(&temp);
                out.extend_from_slice(&hash);
            }
            StorageHasher::Blake2_128Concat => {
                let hash = sp_crypto_hashing::blake2_128(&temp);
                out.extend_from_slice(&hash);
                out.extend_from_slice(&temp);
            }
            StorageHasher::Twox128 => {
                let hash = sp_crypto_hashing::twox_128(&temp);
                out.extend_from_slice(&hash);
            }
            StorageHasher::Twox256 => {
                let hash = sp_crypto_hashing::twox_256(&temp);
                out.extend_from_slice(&hash);
            }
            StorageHasher::Twox64Concat => {
                let hash = sp_crypto_hashing::twox_64(&temp);
                out.extend_from_slice(&hash);
                out.extend_from_slice(&temp);
            }
            StorageHasher::Identity => {
                out.extend_from_slice(&temp);
            }
        }

        // Clear our temp space ready for the next key.
        temp.clear();
    }

    Ok(())
}

/// Encode the end part of a storage key (ie everything except for the prefix that can be encoded
/// via [`encode_storage_key_prefix`]) for a given pallet and storage entry and a set of keys that are each
/// able to be encoded via [`scale_encode::EncodeAsType`].
///
/// This is the same as [`encode_storage_key_suffix_to`], but returns the encoded key as a `Vec<u8>`, rather
/// than accepting a mutable output buffer.
///
/// Prefer [`encode_storage_key`] if you need to encode the entire storage key at once and not just the suffix.
///
/// # Example
///
/// ```rust
/// use frame_decode::storage::{ encode_storage_key_prefix, encode_storage_key_suffix };
/// use frame_metadata::RuntimeMetadata;
/// use parity_scale_codec::Decode;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata_10000000_9180.scale").unwrap();
/// let RuntimeMetadata::V14(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// let account_id = [0u8; 32];
///
/// // System.Account needs only one key to point at a specific value; the account ID.
/// // We just fake an account ID for this example  by providing 32 0 bytes, but anything
/// // which would `scale_encode::EncodeAsType` into 32 bytes would work.
/// let encoded_key_suffix = encode_storage_key_suffix(
///     "System",
///     "Account",
///     &[account_id],
///     &metadata,
///     &metadata.types,
/// ).unwrap();
/// ```
pub fn encode_storage_key_suffix<Info, Resolver, Keys>(
    pallet_name: &str,
    storage_entry: &str,
    keys: Keys,
    info: &Info,
    type_resolver: &Resolver,
) -> Result<Vec<u8>, StorageKeyEncodeError>
where
    Keys: IntoEncodableValues,
    Info: StorageTypeInfo,
    Info::TypeId: Clone + core::fmt::Debug,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
{
    // pre-allocate at least as many bytes as we need for the root/prefix.
    let mut out = Vec::with_capacity(32);
    encode_storage_key_suffix_to(
        pallet_name,
        storage_entry,
        keys,
        info,
        type_resolver,
        &mut out,
    )?;
    Ok(out)
}

/// Encode the end part of a storage key (ie everything except for the prefix that can be encoded
/// via [`encode_storage_key_prefix`]) for a given pallet and storage entry and a set of keys that are each
/// able to be encoded via [`scale_encode::EncodeAsType`].
///
/// This is the same as [`encode_storage_key_suffix`], but can be handed a `Vec` to encode the bytes to.
///
/// Prefer [`encode_storage_key_to`] if you need to encode the entire storage key at once and not just the suffix.
///
/// # Example
///
/// ```rust
/// use frame_decode::storage::{ encode_storage_key_prefix, encode_storage_key_suffix_to };
/// use frame_metadata::RuntimeMetadata;
/// use parity_scale_codec::Decode;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata_10000000_9180.scale").unwrap();
/// let RuntimeMetadata::V14(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
///
/// let account_id = [0u8; 32];
///
/// let mut key = Vec::new();
///
/// // Write the suffix to the provided vec:
/// let encoded_key_suffix = encode_storage_key_suffix_to(
///     "System",
///     "Account",
///     &[account_id],
///     &metadata,
///     &metadata.types,
///     &mut key
/// ).unwrap();
/// ```
pub fn encode_storage_key_suffix_to<Info, Resolver, Keys>(
    pallet_name: &str,
    storage_entry: &str,
    keys: Keys,
    info: &Info,
    type_resolver: &Resolver,
    out: &mut Vec<u8>,
) -> Result<(), StorageKeyEncodeError>
where
    Keys: IntoEncodableValues,
    Info: StorageTypeInfo,
    Info::TypeId: Clone + core::fmt::Debug,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
{
    let storage_info = info
        .storage_info(pallet_name, storage_entry)
        .map_err(|e| StorageKeyEncodeError::CannotGetInfo(e.into_owned()))?;

    encode_storage_key_suffix_with_info_to(keys, &storage_info, type_resolver, out)
}

/// Encode the end part of a storage key (ie everything except for the prefix that can be encoded
/// via [`encode_storage_key_prefix`]) for a given pallet and storage entry and a set of keys that are each
/// able to be encoded via [`scale_encode::EncodeAsType`].
///
/// This is the same as [`encode_storage_key_suffix_to`], but is instead handed storage info that has been
/// pre-computed via [`StorageTypeInfo::storage_info`]. This avoids having to retrieve this information multiple
/// times if you'd like to encode many key suffixes for the same storage entry.
///
/// Prefer [`encode_storage_key_with_info_to`] if you need to encode the entire storage key at once and not
/// just the suffix.
pub fn encode_storage_key_suffix_with_info_to<Resolver, Keys>(
    keys: Keys,
    storage_info: &StorageInfo<<Resolver as TypeResolver>::TypeId>,
    type_resolver: &Resolver,
    out: &mut Vec<u8>,
) -> Result<(), StorageKeyEncodeError>
where
    Keys: IntoEncodableValues,
    Resolver: TypeResolver,
    <Resolver as TypeResolver>::TypeId: Clone + core::fmt::Debug,
{
    let num_encodable_values = keys.num_encodable_values();

    // If we provide more encodable values than there are keys, bail.
    // If we provide less, that's ok and we just don't encode every part of the key
    // (useful if eg iterating a bunch of entries under some prefix of a key).
    if num_encodable_values > storage_info.keys.len() {
        return Err(StorageKeyEncodeError::TooManyKeysProvided {
            max_keys_expected: storage_info.keys.len(),
        });
    }

    // Encode the keys:
    let mut keys = keys.into_encodable_values();
    let mut temp = Vec::with_capacity(32);
    let iter = (0..num_encodable_values)
        .zip(&*storage_info.keys)
        .map(|(_, k)| k);

    for key_info in iter {
        keys.encode_next_value_to(key_info.key_id.clone(), type_resolver, &mut temp)
            .map_err(StorageKeyEncodeError::EncodeError)?;

        match key_info.hasher {
            StorageHasher::Blake2_128 => {
                let hash = sp_crypto_hashing::blake2_128(&temp);
                out.extend_from_slice(&hash);
            }
            StorageHasher::Blake2_256 => {
                let hash = sp_crypto_hashing::blake2_256(&temp);
                out.extend_from_slice(&hash);
            }
            StorageHasher::Blake2_128Concat => {
                let hash = sp_crypto_hashing::blake2_128(&temp);
                out.extend_from_slice(&hash);
                out.extend_from_slice(&temp);
            }
            StorageHasher::Twox128 => {
                let hash = sp_crypto_hashing::twox_128(&temp);
                out.extend_from_slice(&hash);
            }
            StorageHasher::Twox256 => {
                let hash = sp_crypto_hashing::twox_256(&temp);
                out.extend_from_slice(&hash);
            }
            StorageHasher::Twox64Concat => {
                let hash = sp_crypto_hashing::twox_64(&temp);
                out.extend_from_slice(&hash);
                out.extend_from_slice(&temp);
            }
            StorageHasher::Identity => {
                out.extend_from_slice(&temp);
            }
        }

        // Clear our temp space ready for the next key.
        temp.clear();
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_encode_storage_key() {
        use crate::storage::encode_storage_key;
        use frame_metadata::RuntimeMetadata;
        use parity_scale_codec::Decode;

        let metadata_bytes = std::fs::read("artifacts/metadata_10000000_9180.scale").unwrap();
        let RuntimeMetadata::V14(metadata) =
            RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap()
        else {
            return;
        };

        let account_id = [0u8; 32];

        // System.Account needs only one key to point at a specific value; the account ID.
        // We just fake an account ID for this example by providing 32 0 bytes, but anything
        // which would `scale_encode::EncodeAsType` into 32 bytes should work.
        encode_storage_key(
            "System",
            "Account",
            [account_id],
            &metadata,
            &metadata.types,
        )
        .expect("Encoding should work");
        encode_storage_key(
            "System",
            "Account",
            (account_id,),
            &metadata,
            &metadata.types,
        )
        .expect("Encoding should work");

        // We provide no additional keys, so we should get the prefix only.
        let out = encode_storage_key("System", "Account", (), &metadata, &metadata.types)
            .expect("Encoding should work");
        assert_eq!(&out, &encode_storage_key_prefix("System", "Account"));

        // We provide too many additional keys so should get an error.
        let err = encode_storage_key(
            "System",
            "Account",
            (account_id, 123u16),
            &metadata,
            &metadata.types,
        );
        assert!(matches!(
            err,
            Err(StorageKeyEncodeError::TooManyKeysProvided {
                max_keys_expected: 1
            })
        ));
    }
}
