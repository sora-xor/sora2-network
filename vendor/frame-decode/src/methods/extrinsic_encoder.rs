// Copyright (C) 2022-2026 Parity Technologies (UK) Ltd. (admin@parity.io)
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

mod transaction_extension;
mod transaction_extensions;
use super::extrinsic_type_info::{
    ExtrinsicCallInfo, ExtrinsicExtensionInfo, ExtrinsicExtensionInfoArg, ExtrinsicInfoError,
    ExtrinsicSignatureInfo, ExtrinsicTypeInfo,
};
use alloc::vec::Vec;
use parity_scale_codec::Encode;
use scale_encode::{EncodeAsFields, EncodeAsType};
use scale_type_resolver::{Field, TypeResolver};

pub use transaction_extension::{TransactionExtension, TransactionExtensionError};
pub use transaction_extensions::{TransactionExtensions, TransactionExtensionsError};

/// An error returned trying to encode extrinsic call data.
#[non_exhaustive]
#[allow(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum ExtrinsicEncodeError {
    #[error("Cannot get extrinsic info: {0}")]
    CannotGetInfo(ExtrinsicInfoError<'static>),
    #[error("Extrinsic encoding failed: cannot encode call data: {0}")]
    CannotEncodeCallData(scale_encode::Error),
    #[error("Extrinsic encoding failed: cannot encode address: {0}")]
    CannotEncodeAddress(scale_encode::Error),
    #[error("Extrinsic encoding failed: cannot encode signature: {0}")]
    CannotEncodeSignature(scale_encode::Error),
    #[error("Extrinsic encoding failed: cannot encode transaction extensions: {0}")]
    TransactionExtensions(TransactionExtensionsError),
    #[error(
        "Extrinsic encoding failed: cannot find a transaction extensions version which relies only on the transaction extensions we were given."
    )]
    CannotFindGoodExtensionVersion,
}

/// Encode a V4 unsigned extrinsic (also known as an inherent).
///
/// This is the same as [`encode_v4_unsigned_to`], but returns the encoded extrinsic as a `Vec<u8>`,
/// rather than accepting a mutable output buffer.
///
/// # Example
///
/// ```rust
/// use frame_decode::extrinsics::encode_v4_unsigned;
/// use frame_metadata::RuntimeMetadata;
/// use parity_scale_codec::Decode;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata_10000000_9180.scale").unwrap();
/// let RuntimeMetadata::V14(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// // Encode a call to Timestamp.set with an argument.
/// // The call_data type must implement `scale_encode::EncodeAsFields`.
/// let call_data = scale_value::value!({
///     now: 1234567890u64,
/// });
///
/// let encoded = encode_v4_unsigned(
///     "Timestamp",
///     "set",
///     &call_data,
///     &metadata,
///     &metadata.types,
/// ).unwrap();
/// ```
pub fn encode_v4_unsigned<CallData, Info, Resolver>(
    pallet_name: &str,
    call_name: &str,
    call_data: &CallData,
    info: &Info,
    type_resolver: &Resolver,
) -> Result<Vec<u8>, ExtrinsicEncodeError>
where
    CallData: EncodeAsFields,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
    Info: ExtrinsicTypeInfo,
{
    let mut out = Vec::new();
    encode_v4_unsigned_to(
        pallet_name,
        call_name,
        call_data,
        info,
        type_resolver,
        &mut out,
    )?;
    Ok(out)
}

/// Encode a V4 unsigned extrinsic (also known as an inherent) to a provided output buffer.
///
/// This is the same as [`encode_v4_unsigned`], but writes the encoded extrinsic to the provided
/// `Vec<u8>` rather than returning a new one.
///
/// # Example
///
/// ```rust
/// use frame_decode::extrinsics::encode_v4_unsigned_to;
/// use frame_metadata::RuntimeMetadata;
/// use parity_scale_codec::Decode;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata_10000000_9180.scale").unwrap();
/// let RuntimeMetadata::V14(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// // Encode a call to Timestamp.set with an argument.
/// let call_data = scale_value::value!({
///     now: 1234567890u64,
/// });
///
/// let mut encoded = Vec::new();
/// encode_v4_unsigned_to(
///     "Timestamp",
///     "set",
///     &call_data,
///     &metadata,
///     &metadata.types,
///     &mut encoded,
/// ).unwrap();
/// ```
pub fn encode_v4_unsigned_to<CallData, Info, Resolver>(
    pallet_name: &str,
    call_name: &str,
    call_data: &CallData,
    info: &Info,
    type_resolver: &Resolver,
    out: &mut Vec<u8>,
) -> Result<(), ExtrinsicEncodeError>
where
    CallData: EncodeAsFields,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
    Info: ExtrinsicTypeInfo,
{
    let call_info = info
        .extrinsic_call_info_by_name(pallet_name, call_name)
        .map_err(|i| i.into_owned())
        .map_err(ExtrinsicEncodeError::CannotGetInfo)?;

    encode_v4_unsigned_with_info_to(call_data, type_resolver, &call_info, out)
}

/// Encode a V4 unsigned extrinsic (also known as an inherent) to a provided output buffer,
/// using pre-computed call information.
///
/// Unlike [`encode_v4_unsigned_to`], which obtains the call info internally given the pallet and call names,
/// this function takes the call info as an argument. This is useful if you already have the call info available,
/// for example if you are encoding multiple extrinsics for the same call.
pub fn encode_v4_unsigned_with_info_to<CallData, Resolver>(
    call_data: &CallData,
    type_resolver: &Resolver,
    call_info: &ExtrinsicCallInfo<Resolver::TypeId>,
    out: &mut Vec<u8>,
) -> Result<(), ExtrinsicEncodeError>
where
    CallData: EncodeAsFields,
    Resolver: TypeResolver,
{
    encode_unsigned_at_version_with_info_to(
        call_data,
        call_info,
        type_resolver,
        TransactionVersion::V4,
        out,
    )
}

/// Encode a V4 signed extrinsic, ready to submit.
///
/// A signed V4 extrinsic includes an address, signature, and transaction extensions (such as
/// nonce and tip) alongside the call data. The signature should be computed over the signer
/// payload, which can be obtained via [`encode_v4_signer_payload`].
///
/// This is the same as [`encode_v4_signed_to`], but returns the encoded extrinsic as a `Vec<u8>`,
/// rather than accepting a mutable output buffer.
///
/// # Example
///
/// ```rust,ignore
/// use frame_decode::extrinsics::{encode_v4_signed, TransactionExtensions};
/// use frame_metadata::RuntimeMetadata;
/// use parity_scale_codec::Decode;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata_10000000_9180.scale").unwrap();
/// let RuntimeMetadata::V14(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// // The call data, address, signature, and transaction extensions must implement
/// // the appropriate scale_encode traits.
/// let call_data = /* ... */;
/// let address = /* your address type */;
/// let signature = /* your signature type */;
/// let transaction_extensions = /* your TransactionExtensions impl */;
///
/// let encoded = encode_v4_signed(
///     "Balances",
///     "transfer_keep_alive",
///     &call_data,
///     &transaction_extensions,
///     &address,
///     &signature,
///     &metadata,
///     &metadata.types,
/// ).unwrap();
/// ```
#[allow(clippy::too_many_arguments)]
pub fn encode_v4_signed<CallData, Info, Resolver, Exts, Address, Signature>(
    pallet_name: &str,
    call_name: &str,
    call_data: &CallData,
    transaction_extensions: &Exts,
    address: &Address,
    signature: &Signature,
    info: &Info,
    type_resolver: &Resolver,
) -> Result<Vec<u8>, ExtrinsicEncodeError>
where
    CallData: EncodeAsFields,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
    Info: ExtrinsicTypeInfo,
    Exts: TransactionExtensions<Resolver>,
    Address: EncodeAsType,
    Signature: EncodeAsType,
{
    let mut out = Vec::new();
    encode_v4_signed_to(
        pallet_name,
        call_name,
        call_data,
        transaction_extensions,
        address,
        signature,
        info,
        type_resolver,
        &mut out,
    )?;
    Ok(out)
}

/// Encode a V4 signed extrinsic to a provided output buffer.
///
/// A signed extrinsic includes an address, signature, and transaction extensions (such as
/// nonce and tip) alongside the call data. The signature should be computed over the signer
/// payload, which can be obtained via [`encode_v4_signer_payload`].
///
/// This is the same as [`encode_v4_signed`], but writes the encoded extrinsic to the provided
/// `Vec<u8>` rather than returning a new one.
///
/// # Example
///
/// ```rust,ignore
/// use frame_decode::extrinsics::{encode_v4_signed_to, TransactionExtensions};
/// use frame_metadata::RuntimeMetadata;
/// use parity_scale_codec::Decode;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata_10000000_9180.scale").unwrap();
/// let RuntimeMetadata::V14(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// let call_data = /* ... */;
/// let address = /* your address type */;
/// let signature = /* your signature type */;
/// let transaction_extensions = /* your TransactionExtensions impl */;
///
/// let mut encoded = Vec::new();
/// encode_v4_signed_to(
///     "Balances",
///     "transfer_keep_alive",
///     &call_data,
///     &transaction_extensions,
///     &address,
///     &signature,
///     &metadata,
///     &metadata.types,
///     &mut encoded,
/// ).unwrap();
/// ```
#[allow(clippy::too_many_arguments)]
pub fn encode_v4_signed_to<CallData, Info, Resolver, Exts, Address, Signature>(
    pallet_name: &str,
    call_name: &str,
    call_data: &CallData,
    transaction_extensions: &Exts,
    address: &Address,
    signature: &Signature,
    info: &Info,
    type_resolver: &Resolver,
    out: &mut Vec<u8>,
) -> Result<(), ExtrinsicEncodeError>
where
    CallData: EncodeAsFields,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
    Info: ExtrinsicTypeInfo,
    Exts: TransactionExtensions<Resolver>,
    Address: EncodeAsType,
    Signature: EncodeAsType,
{
    let call_info = info
        .extrinsic_call_info_by_name(pallet_name, call_name)
        .map_err(|i| i.into_owned())
        .map_err(ExtrinsicEncodeError::CannotGetInfo)?;

    let ext_info = info
        .extrinsic_extension_info(None)
        .map_err(|i| i.into_owned())
        .map_err(ExtrinsicEncodeError::CannotGetInfo)?;

    let sig_info = info
        .extrinsic_signature_info()
        .map_err(|i| i.into_owned())
        .map_err(ExtrinsicEncodeError::CannotGetInfo)?;

    encode_v4_signed_with_info_to(
        call_data,
        transaction_extensions,
        address,
        signature,
        type_resolver,
        &call_info,
        &sig_info,
        &ext_info,
        out,
    )
}

/// Encode a V4 signed extrinsic to a provided output buffer, using pre-computed type information.
///
/// Unlike [`encode_v4_signed_to`], which obtains the call, signature, and extension info internally
/// given the pallet and call names, this function takes these as arguments. This is useful if you
/// already have this information available, for example if you are encoding multiple extrinsics.
#[allow(clippy::too_many_arguments)]
pub fn encode_v4_signed_with_info_to<CallData, Resolver, Exts, Address, Signature>(
    call_data: &CallData,
    transaction_extensions: &Exts,
    address: &Address,
    signature: &Signature,
    type_resolver: &Resolver,
    call_info: &ExtrinsicCallInfo<Resolver::TypeId>,
    sig_info: &ExtrinsicSignatureInfo<Resolver::TypeId>,
    ext_info: &ExtrinsicExtensionInfo<Resolver::TypeId>,
    out: &mut Vec<u8>,
) -> Result<(), ExtrinsicEncodeError>
where
    CallData: EncodeAsFields,
    Resolver: TypeResolver,
    Exts: TransactionExtensions<Resolver>,
    Address: EncodeAsType,
    Signature: EncodeAsType,
{
    // Encode the "inner" bytes
    let mut encoded_inner = Vec::new();

    // "is signed" + transaction protocol version (4)
    (0b10000000 + 4u8).encode_to(&mut encoded_inner);

    // Who is this transaction from (corresponds to public key of signature)
    address
        .encode_as_type_to(
            sig_info.address_id.clone(),
            type_resolver,
            &mut encoded_inner,
        )
        .map_err(ExtrinsicEncodeError::CannotEncodeAddress)?;

    // Signature for the above identity
    signature
        .encode_as_type_to(
            sig_info.signature_id.clone(),
            type_resolver,
            &mut encoded_inner,
        )
        .map_err(ExtrinsicEncodeError::CannotEncodeSignature)?;

    // Signed extensions (now Transaction Extensions)
    encode_transaction_extension_values(
        &ext_info.extension_ids,
        transaction_extensions,
        type_resolver,
        &mut encoded_inner,
    )
    .map_err(ExtrinsicEncodeError::TransactionExtensions)?;

    // And now the actual call data, ie the arguments we're passing to the call
    encode_call_data_with_info_to(call_data, call_info, type_resolver, &mut encoded_inner)?;

    // Now, encoding these inner bytes prefixes the compact length to the beginning:
    encoded_inner.encode_to(out);
    Ok(())
}

/// Encode the signer payload for a V4 signed extrinsic.
///
/// The signer payload is the data that should be signed to produce the signature for
/// a signed extrinsic. It consists of the encoded call data, the transaction extension
/// values, and the transaction extension implicit data. If the resulting payload exceeds
/// 256 bytes, it is hashed using Blake2-256.
///
/// Use this function to obtain the bytes that should be signed, then pass the resulting
/// signature to [`encode_v4_signed`] to construct the final extrinsic.
///
/// # Example
///
/// ```rust,ignore
/// use frame_decode::extrinsics::{encode_v4_signer_payload, TransactionExtensions};
/// use frame_metadata::RuntimeMetadata;
/// use parity_scale_codec::Decode;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata_10000000_9180.scale").unwrap();
/// let RuntimeMetadata::V14(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// let call_data = /* ... */;
/// let transaction_extensions = /* your TransactionExtensions impl */;
///
/// // Get the payload to sign
/// let signer_payload = encode_v4_signer_payload(
///     "Balances",
///     "transfer_keep_alive",
///     &call_data,
///     &transaction_extensions,
///     &metadata,
///     &metadata.types,
/// ).unwrap();
///
/// // Sign the payload with your signing key, then use encode_v4_signed
/// // to construct the final extrinsic.
/// ```
pub fn encode_v4_signer_payload<CallData, Info, Resolver, Exts>(
    pallet_name: &str,
    call_name: &str,
    call_data: &CallData,
    transaction_extensions: &Exts,
    info: &Info,
    type_resolver: &Resolver,
) -> Result<Vec<u8>, ExtrinsicEncodeError>
where
    CallData: EncodeAsFields,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
    Info: ExtrinsicTypeInfo,
    Exts: TransactionExtensions<Resolver>,
    Info::TypeId: Clone,
{
    let call_info = info
        .extrinsic_call_info_by_name(pallet_name, call_name)
        .map_err(|i| i.into_owned())
        .map_err(ExtrinsicEncodeError::CannotGetInfo)?;

    let ext_info = info
        .extrinsic_extension_info(None)
        .map_err(|i| i.into_owned())
        .map_err(ExtrinsicEncodeError::CannotGetInfo)?;

    encode_v4_signer_payload_with_info(
        call_data,
        transaction_extensions,
        type_resolver,
        &call_info,
        &ext_info,
    )
}

/// Encode the signer payload for a V4 signed extrinsic, using pre-computed type information.
///
/// Unlike [`encode_v4_signer_payload`], which obtains the call and extension info internally
/// given the pallet and call names, this function takes these as arguments. This is useful if you
/// already have this information available.
///
/// The signer payload consists of the encoded call data, the transaction extension values,
/// and the transaction extension implicit data. If the resulting payload exceeds 256 bytes,
/// it is hashed using Blake2-256.
pub fn encode_v4_signer_payload_with_info<CallData, Resolver, Exts>(
    call_data: &CallData,
    transaction_extensions: &Exts,
    type_resolver: &Resolver,
    call_info: &ExtrinsicCallInfo<Resolver::TypeId>,
    ext_info: &ExtrinsicExtensionInfo<Resolver::TypeId>,
) -> Result<Vec<u8>, ExtrinsicEncodeError>
where
    CallData: EncodeAsFields,
    Resolver: TypeResolver,
    Exts: TransactionExtensions<Resolver>,
{
    let mut out = Vec::new();

    // First encode call data
    encode_call_data_with_info_to(call_data, call_info, type_resolver, &mut out)?;

    // Then the signer payload value (ie roughly the bytes that will appear in the tx)
    encode_transaction_extension_values(
        &ext_info.extension_ids,
        transaction_extensions,
        type_resolver,
        &mut out,
    )
    .map_err(ExtrinsicEncodeError::TransactionExtensions)?;

    // Then the signer payload implicits (ie data we want to verify that is NOT in the tx)
    encode_transaction_extension_implicits(
        &ext_info.extension_ids,
        transaction_extensions,
        type_resolver,
        &mut out,
    )
    .map_err(ExtrinsicEncodeError::TransactionExtensions)?;

    // Finally we need to hash it if it's too long
    if out.len() > 256 {
        out = sp_crypto_hashing::blake2_256(&out).to_vec();
    }

    Ok(out)
}

/// Encode a V5 bare extrinsic (also known as an inherent), ready to submit.
///
/// V5 bare extrinsics contain only call data with no transaction extensions or signature.
/// They are functionally equivalent to V4 unsigned extrinsics and are typically used for
/// inherents (data provided by block authors).
///
/// This is the same as [`encode_v5_bare_to`], but returns the encoded extrinsic as a `Vec<u8>`,
/// rather than accepting a mutable output buffer.
///
/// # Example
///
/// ```rust
/// use frame_decode::extrinsics::encode_v5_bare;
/// use frame_metadata::RuntimeMetadata;
/// use parity_scale_codec::Decode;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata_10000000_9180.scale").unwrap();
/// let RuntimeMetadata::V14(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// // Encode a call to Timestamp.set with an argument.
/// let call_data = scale_value::value!({
///     now: 1234567890u64,
/// });
///
/// let encoded = encode_v5_bare(
///     "Timestamp",
///     "set",
///     &call_data,
///     &metadata,
///     &metadata.types,
/// ).unwrap();
/// ```
pub fn encode_v5_bare<CallData, Info, Resolver>(
    pallet_name: &str,
    call_name: &str,
    call_data: &CallData,
    info: &Info,
    type_resolver: &Resolver,
) -> Result<Vec<u8>, ExtrinsicEncodeError>
where
    CallData: EncodeAsFields,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
    Info: ExtrinsicTypeInfo,
{
    let mut out = Vec::new();
    encode_v5_bare_to(
        pallet_name,
        call_name,
        call_data,
        info,
        type_resolver,
        &mut out,
    )?;
    Ok(out)
}

/// Encode a V5 bare extrinsic (also known as an inherent) to a provided output buffer.
///
/// This is the same as [`encode_v5_bare`], but writes the encoded extrinsic to the provided
/// `Vec<u8>` rather than returning a new one.
///
/// # Example
///
/// ```rust
/// use frame_decode::extrinsics::encode_v5_bare_to;
/// use frame_metadata::RuntimeMetadata;
/// use parity_scale_codec::Decode;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata_10000000_9180.scale").unwrap();
/// let RuntimeMetadata::V14(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// // Encode a call to Timestamp.set with an argument.
/// let call_data = scale_value::value!({
///     now: 1234567890u64,
/// });
///
/// let mut encoded = Vec::new();
/// encode_v5_bare_to(
///     "Timestamp",
///     "set",
///     &call_data,
///     &metadata,
///     &metadata.types,
///     &mut encoded,
/// ).unwrap();
/// ```
pub fn encode_v5_bare_to<CallData, Info, Resolver>(
    pallet_name: &str,
    call_name: &str,
    call_data: &CallData,
    info: &Info,
    type_resolver: &Resolver,
    out: &mut Vec<u8>,
) -> Result<(), ExtrinsicEncodeError>
where
    CallData: EncodeAsFields,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
    Info: ExtrinsicTypeInfo,
{
    let call_info = info
        .extrinsic_call_info_by_name(pallet_name, call_name)
        .map_err(|i| i.into_owned())
        .map_err(ExtrinsicEncodeError::CannotGetInfo)?;

    encode_v5_bare_with_info_to(call_data, type_resolver, &call_info, out)
}

/// Encode a V5 bare extrinsic (also known as an inherent) to a provided output buffer,
/// using pre-computed call information.
///
/// Unlike [`encode_v5_bare_to`], which obtains the call info internally given the pallet and call names,
/// this function takes the call info as an argument. This is useful if you already have the call info available,
/// for example if you are encoding multiple extrinsics for the same call.
pub fn encode_v5_bare_with_info_to<CallData, Resolver>(
    call_data: &CallData,
    type_resolver: &Resolver,
    call_info: &ExtrinsicCallInfo<Resolver::TypeId>,
    out: &mut Vec<u8>,
) -> Result<(), ExtrinsicEncodeError>
where
    CallData: EncodeAsFields,
    Resolver: TypeResolver,
{
    encode_unsigned_at_version_with_info_to(
        call_data,
        call_info,
        type_resolver,
        TransactionVersion::V5,
        out,
    )
}

/// Determine the best transaction extension version to use for a V5 general extrinsic.
///
/// V5 general extrinsics support multiple versions of transaction extensions. This function
/// iterates through the available extension versions and returns the first version for which
/// all required extension data is provided.
///
/// Use this function to determine which `transaction_extension_version` to pass to
/// [`encode_v5_general`] or [`encode_v5_general_to`].
///
/// # Errors
///
/// Returns [`ExtrinsicEncodeError::CannotFindGoodExtensionVersion`] if no extension version
/// can be found for which all required data is available.
///
/// # Example
///
/// ```rust,ignore
/// use frame_decode::extrinsics::{best_v5_general_transaction_extension_version, encode_v5_general};
/// use frame_metadata::RuntimeMetadata;
/// use parity_scale_codec::Decode;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata.scale").unwrap();
/// let RuntimeMetadata::V16(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// let transaction_extensions = /* your TransactionExtensions impl */;
///
/// // Find the best extension version for your provided extensions
/// let ext_version = best_v5_general_transaction_extension_version(
///     &transaction_extensions,
///     &metadata,
/// ).unwrap();
///
/// // Use this version when encoding the extrinsic
/// let encoded = encode_v5_general(
///     "Balances",
///     "transfer_keep_alive",
///     &call_data,
///     ext_version,
///     &transaction_extensions,
///     &metadata,
///     &metadata.types,
/// ).unwrap();
/// ```
pub fn best_v5_general_transaction_extension_version<Exts, Info, Resolver>(
    transaction_extensions: &Exts,
    info: &Info,
    type_resolver: &Resolver,
) -> Result<u8, ExtrinsicEncodeError>
where
    Info: ExtrinsicTypeInfo,
    Exts: TransactionExtensions<Resolver>,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
    Info::TypeId: Clone,
{
    let extension_versions = info
        .extrinsic_extension_version_info()
        .map_err(|i| i.into_owned())
        .map_err(ExtrinsicEncodeError::CannotGetInfo)?;

    for ext_version in extension_versions {
        // get extension info for each version.
        let ext_info = info
            .extrinsic_extension_info(Some(ext_version))
            .map_err(|i| i.into_owned())
            .map_err(ExtrinsicEncodeError::CannotGetInfo)?;

        // Do we have all of the extension data for this version?
        let have_data = ext_info.extension_ids.iter().all(|e| {
            let is_value_empty = is_type_empty(e.id.clone(), type_resolver);
            let is_value_option = is_type_option(e.id.clone(), type_resolver);
            let is_implicit_empty = is_type_empty(e.implicit_id.clone(), type_resolver);
            ((is_value_empty || is_value_option) && is_implicit_empty)
                || transaction_extensions.contains_extension(&e.name)
        });

        // If we have all of the data we need, encode the extrinsic,
        // else loop and try the next extension version.
        if have_data {
            return Ok(ext_version);
        }
    }

    Err(ExtrinsicEncodeError::CannotFindGoodExtensionVersion)
}

/// Encode a V5 general extrinsic, ready to submit.
///
/// V5 general extrinsics include transaction extensions but no separate signature field.
/// Instead, the signature (if needed) is provided as part of one of the transaction extensions.
/// This is the new extrinsic format introduced in newer Substrate runtimes.
///
/// Use [`best_v5_general_transaction_extension_version`] to determine which extension version
/// to use based on the extensions you have available.
///
/// This is the same as [`encode_v5_general_to`], but returns the encoded extrinsic as a `Vec<u8>`,
/// rather than accepting a mutable output buffer.
///
/// # Example
///
/// ```rust,ignore
/// use frame_decode::extrinsics::{encode_v5_general, best_v5_general_transaction_extension_version};
/// use frame_metadata::RuntimeMetadata;
/// use parity_scale_codec::Decode;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata.scale").unwrap();
/// let RuntimeMetadata::V16(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// let call_data = /* ... */;
/// let transaction_extensions = /* your TransactionExtensions impl */;
///
/// let ext_version = best_v5_general_transaction_extension_version(
///     &transaction_extensions,
///     &metadata,
/// ).unwrap();
///
/// let encoded = encode_v5_general(
///     "Balances",
///     "transfer_keep_alive",
///     &call_data,
///     ext_version,
///     &transaction_extensions,
///     &metadata,
///     &metadata.types,
/// ).unwrap();
/// ```
pub fn encode_v5_general<CallData, Info, Resolver, Exts>(
    pallet_name: &str,
    call_name: &str,
    call_data: &CallData,
    transaction_extension_version: u8,
    transaction_extensions: &Exts,
    info: &Info,
    type_resolver: &Resolver,
) -> Result<Vec<u8>, ExtrinsicEncodeError>
where
    CallData: EncodeAsFields,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
    Info: ExtrinsicTypeInfo,
    Exts: TransactionExtensions<Resolver>,
{
    let mut out = Vec::new();
    encode_v5_general_to(
        pallet_name,
        call_name,
        call_data,
        transaction_extension_version,
        transaction_extensions,
        info,
        type_resolver,
        &mut out,
    )?;
    Ok(out)
}

/// Encode a V5 general extrinsic to a provided output buffer.
///
/// V5 general extrinsics include transaction extensions but no separate signature field.
/// Instead, the signature (if needed) is provided as part of one of the transaction extensions.
///
/// This is the same as [`encode_v5_general`], but writes the encoded extrinsic to the provided
/// `Vec<u8>` rather than returning a new one.
///
/// # Example
///
/// ```rust,ignore
/// use frame_decode::extrinsics::{encode_v5_general_to, best_v5_general_transaction_extension_version};
/// use frame_metadata::RuntimeMetadata;
/// use parity_scale_codec::Decode;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata.scale").unwrap();
/// let RuntimeMetadata::V16(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// let call_data = /* ... */;
/// let transaction_extensions = /* your TransactionExtensions impl */;
///
/// let ext_version = best_v5_general_transaction_extension_version(
///     &transaction_extensions,
///     &metadata,
/// ).unwrap();
///
/// let mut encoded = Vec::new();
/// encode_v5_general_to(
///     "Balances",
///     "transfer_keep_alive",
///     &call_data,
///     ext_version,
///     &transaction_extensions,
///     &metadata,
///     &metadata.types,
///     &mut encoded,
/// ).unwrap();
/// ```
#[allow(clippy::too_many_arguments)]
pub fn encode_v5_general_to<CallData, Info, Resolver, Exts>(
    pallet_name: &str,
    call_name: &str,
    call_data: &CallData,
    transaction_extension_version: u8,
    transaction_extensions: &Exts,
    info: &Info,
    type_resolver: &Resolver,
    out: &mut Vec<u8>,
) -> Result<(), ExtrinsicEncodeError>
where
    CallData: EncodeAsFields,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
    Info: ExtrinsicTypeInfo,
    Exts: TransactionExtensions<Resolver>,
{
    let call_info = info
        .extrinsic_call_info_by_name(pallet_name, call_name)
        .map_err(|i| i.into_owned())
        .map_err(ExtrinsicEncodeError::CannotGetInfo)?;

    let ext_info = info
        .extrinsic_extension_info(Some(transaction_extension_version))
        .map_err(|i| i.into_owned())
        .map_err(ExtrinsicEncodeError::CannotGetInfo)?;

    encode_v5_general_with_info_to(
        call_data,
        transaction_extension_version,
        transaction_extensions,
        type_resolver,
        &call_info,
        &ext_info,
        out,
    )
}

/// Encode a V5 general extrinsic to a provided output buffer, using pre-computed type information.
///
/// Unlike [`encode_v5_general_to`], which obtains the call and extension info internally
/// given the pallet and call names, this function takes these as arguments. This is useful if you
/// already have this information available, for example if you are encoding multiple extrinsics.
pub fn encode_v5_general_with_info_to<CallData, Resolver, Exts>(
    call_data: &CallData,
    transaction_extension_version: u8,
    transaction_extensions: &Exts,
    type_resolver: &Resolver,
    call_info: &ExtrinsicCallInfo<Resolver::TypeId>,
    ext_info: &ExtrinsicExtensionInfo<Resolver::TypeId>,
    out: &mut Vec<u8>,
) -> Result<(), ExtrinsicEncodeError>
where
    CallData: EncodeAsFields,
    Resolver: TypeResolver,
    Exts: TransactionExtensions<Resolver>,
{
    // Encode the "inner" bytes
    let mut encoded_inner = Vec::new();

    // "is signed" (2 bits now) + transaction protocol version (5)
    (0b01000000 + 5u8).encode_to(&mut encoded_inner);

    // Version of the transaction extensions.
    transaction_extension_version.encode_to(&mut encoded_inner);

    // Transaction Extensions next. These may include a signature/address
    encode_transaction_extension_values(
        &ext_info.extension_ids,
        transaction_extensions,
        type_resolver,
        &mut encoded_inner,
    )
    .map_err(ExtrinsicEncodeError::TransactionExtensions)?;

    // And now the actual call data, ie the arguments we're passing to the call
    encode_call_data_with_info_to(call_data, call_info, type_resolver, &mut encoded_inner)?;

    // Now, encoding these inner bytes prefixes the compact length to the beginning:
    encoded_inner.encode_to(out);
    Ok(())
}

/// Encode the signer payload for a V5 general extrinsic.
///
/// The signer payload is the data that should be signed to produce the signature for
/// a general extrinsic. It consists of the transaction extension version and encoded call data,
/// followed by the transaction extension values and implicit data of the extensions after the
/// last authorization extension (see [`TransactionExtension::is_authorization_extension`]);
/// an authorization extension signs only the implications which follow it.
///
/// Unlike [`encode_v4_signer_payload`], which conditionally hashes the payload if it exceeds
/// 256 bytes, V5 signer payloads are always hashed using Blake2-256, returning a fixed 32-byte
/// array.
///
/// Use this function to obtain the bytes that should be signed, then include the resulting
/// signature in the appropriate transaction extension when calling [`encode_v5_general`].
///
/// # Example
///
/// ```rust,ignore
/// use frame_decode::extrinsics::{encode_v5_signer_payload, best_v5_general_transaction_extension_version};
/// use frame_metadata::RuntimeMetadata;
/// use parity_scale_codec::Decode;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata.scale").unwrap();
/// let RuntimeMetadata::V16(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// let call_data = /* ... */;
/// let transaction_extensions = /* your TransactionExtensions impl */;
///
/// let ext_version = best_v5_general_transaction_extension_version(
///     &transaction_extensions,
///     &metadata,
/// ).unwrap();
///
/// // Get the 32-byte payload hash to sign
/// let signer_payload = encode_v5_signer_payload(
///     "Balances",
///     "transfer_keep_alive",
///     &call_data,
///     ext_version,
///     &transaction_extensions,
///     &metadata,
///     &metadata.types,
/// ).unwrap();
///
/// // Sign the payload with your signing key, then include the signature
/// // in your transaction extensions when calling encode_v5_general.
/// ```
pub fn encode_v5_signer_payload<CallData, Info, Resolver, Exts>(
    pallet_name: &str,
    call_name: &str,
    call_data: &CallData,
    transaction_extension_version: u8,
    transaction_extensions: &Exts,
    info: &Info,
    type_resolver: &Resolver,
) -> Result<[u8; 32], ExtrinsicEncodeError>
where
    CallData: EncodeAsFields,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
    Info: ExtrinsicTypeInfo,
    Exts: TransactionExtensions<Resolver>,
    Info::TypeId: Clone,
{
    let call_info = info
        .extrinsic_call_info_by_name(pallet_name, call_name)
        .map_err(|i| i.into_owned())
        .map_err(ExtrinsicEncodeError::CannotGetInfo)?;

    let ext_info = info
        .extrinsic_extension_info(Some(transaction_extension_version))
        .map_err(|i| i.into_owned())
        .map_err(ExtrinsicEncodeError::CannotGetInfo)?;

    encode_v5_signer_payload_with_info(
        call_data,
        transaction_extension_version,
        transaction_extensions,
        type_resolver,
        &call_info,
        &ext_info,
    )
}

/// Encode the signer payload for a V5 general extrinsic, using pre-computed type information.
///
/// Unlike [`encode_v5_signer_payload`], which obtains the call and extension info internally
/// given the pallet and call names, this function takes these as arguments. This is useful if you
/// already have this information available.
///
/// The signer payload consists of the transaction extension version and encoded call data,
/// followed by the transaction extension values and implicit data of the extensions after the
/// last authorization extension (see [`TransactionExtension::is_authorization_extension`]).
/// The result is always hashed using Blake2-256, returning a fixed 32-byte array.
pub fn encode_v5_signer_payload_with_info<CallData, Resolver, Exts>(
    call_data: &CallData,
    transaction_extension_version: u8,
    transaction_extensions: &Exts,
    type_resolver: &Resolver,
    call_info: &ExtrinsicCallInfo<Resolver::TypeId>,
    ext_info: &ExtrinsicExtensionInfo<Resolver::TypeId>,
) -> Result<[u8; 32], ExtrinsicEncodeError>
where
    CallData: EncodeAsFields,
    Resolver: TypeResolver,
    Exts: TransactionExtensions<Resolver>,
{
    let mut payload = Vec::new();

    // First, the base implication: the transaction extension version and call data.
    // This is always signed, regardless of any authorization extension below.
    transaction_extension_version.encode_to(&mut payload);
    encode_call_data_with_info_to(call_data, call_info, type_resolver, &mut payload)?;

    // An authorization extension (eg VerifyMultiSignature) signs only the implications of the
    // extensions following it, so it and every extension before it contribute no bytes to the
    // signer payload.
    let signed_extension_ids = ext_info
        .extension_ids
        .iter()
        .rposition(|e| transaction_extensions.is_authorization_extension(&e.name))
        .map(|last_auth| &ext_info.extension_ids[last_auth + 1..])
        .unwrap_or(&ext_info.extension_ids);

    // Then the signer payload values (ie roughly the bytes that will appear in the tx)
    encode_transaction_extension_values(
        signed_extension_ids,
        transaction_extensions,
        type_resolver,
        &mut payload,
    )
    .map_err(ExtrinsicEncodeError::TransactionExtensions)?;

    // Then the signer payload implicits (ie data we want to verify that is NOT in the tx)
    encode_transaction_extension_implicits(
        signed_extension_ids,
        transaction_extensions,
        type_resolver,
        &mut payload,
    )
    .map_err(ExtrinsicEncodeError::TransactionExtensions)?;

    // V5 signer payloads are always hashed, regardless of length.
    Ok(sp_crypto_hashing::blake2_256(&payload))
}

/// Encode the call data for an extrinsic.
///
/// This is basically an alias for [`scale_encode::EncodeAsFields::encode_as_fields()`].
pub fn encode_call_data<CallData, Info, Resolver>(
    pallet_name: &str,
    call_name: &str,
    call_data: &CallData,
    info: &Info,
    type_resolver: &Resolver,
) -> Result<Vec<u8>, ExtrinsicEncodeError>
where
    CallData: EncodeAsFields,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
    Info: ExtrinsicTypeInfo,
{
    let mut out = Vec::new();
    encode_call_data_to(
        pallet_name,
        call_name,
        call_data,
        info,
        type_resolver,
        &mut out,
    )?;
    Ok(out)
}

/// Encode the call data for an extrinsic to the given Vec.
///
/// This is basically an alias for [`scale_encode::EncodeAsFields::encode_as_fields()`], but
/// with a byte for the pallet index and call index prepended.
pub fn encode_call_data_to<CallData, Info, Resolver>(
    pallet_name: &str,
    call_name: &str,
    call_data: &CallData,
    info: &Info,
    type_resolver: &Resolver,
    out: &mut Vec<u8>,
) -> Result<(), ExtrinsicEncodeError>
where
    CallData: EncodeAsFields,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
    Info: ExtrinsicTypeInfo,
{
    let call_info = info
        .extrinsic_call_info_by_name(pallet_name, call_name)
        .map_err(|i| i.into_owned())
        .map_err(ExtrinsicEncodeError::CannotGetInfo)?;

    encode_call_data_with_info_to(call_data, &call_info, type_resolver, out)
}

/// Encode the call data for an extrinsic, given some already-computed [`ExtrinsicCallInfo`].
///
/// This is basically an alias for [`scale_encode::EncodeAsFields::encode_as_fields()`], but
/// with a byte for the pallet index and call index prepended.
pub fn encode_call_data_with_info<CallData, Info, Resolver>(
    call_data: &CallData,
    call_info: &ExtrinsicCallInfo<Resolver::TypeId>,
    type_resolver: &Resolver,
) -> Result<Vec<u8>, ExtrinsicEncodeError>
where
    Resolver: TypeResolver,
    CallData: EncodeAsFields,
{
    let mut out = Vec::new();
    encode_call_data_with_info_to(call_data, call_info, type_resolver, &mut out)?;
    Ok(out)
}

/// Encode the call data for an extrinsic, given some already-computed [`ExtrinsicCallInfo`],
/// to the given Vec.
///
/// This is basically an alias for [`scale_encode::EncodeAsFields::encode_as_fields()`], but
/// with a byte for the pallet index and call index prepended.
pub fn encode_call_data_with_info_to<CallData, Resolver>(
    call_data: &CallData,
    call_info: &ExtrinsicCallInfo<Resolver::TypeId>,
    type_resolver: &Resolver,
    out: &mut Vec<u8>,
) -> Result<(), ExtrinsicEncodeError>
where
    Resolver: TypeResolver,
    CallData: EncodeAsFields,
{
    // Pallet and call index to identify the call:
    call_info.pallet_index.encode_to(out);
    call_info.call_index.encode_to(out);

    // Arguments to this call:
    let mut fields = call_info.args.iter().map(|arg| Field {
        name: Some(&*arg.name),
        id: arg.id.clone(),
    });
    call_data
        .encode_as_fields_to(&mut fields, type_resolver, out)
        .map_err(ExtrinsicEncodeError::CannotEncodeCallData)?;

    Ok(())
}

// V4 unsigned and V5 bare extrinsics are basically encoded
// in the same way; this helper can do either.
fn encode_unsigned_at_version_with_info_to<CallData, Resolver>(
    call_data: &CallData,
    call_info: &ExtrinsicCallInfo<Resolver::TypeId>,
    type_resolver: &Resolver,
    tx_version: TransactionVersion,
    out: &mut Vec<u8>,
) -> Result<(), ExtrinsicEncodeError>
where
    Resolver: TypeResolver,
    CallData: EncodeAsFields,
{
    // Build our inner, non-length-prefixed extrinsic:
    let inner = {
        let mut out = Vec::new();
        // Transaction version (4):
        (tx_version as u8).encode_to(&mut out);
        // Then the arguments for the call:
        encode_call_data_with_info_to(call_data, call_info, type_resolver, &mut out)?;
        out
    };

    // Encode the inner vec to prefix the compact length to it:
    inner.encode_to(out);
    Ok(())
}

#[derive(Copy, Clone)]
#[repr(u8)]
enum TransactionVersion {
    V4 = 4u8,
    V5 = 5u8,
}

/// Encode the transaction extension values to the provided output in the order given by
/// the extension args from an [`ExtrinsicExtensionInfo`]. We skip missing extensions that
/// would encode as 0 bytes, and we encode a 0u8 for any missing extensions whose value is
/// an `Option<T>`, which carries the assumption that the default value for such an extension
/// is `None`.
fn encode_transaction_extension_values<Resolver, Exts>(
    extension_ids: &[ExtrinsicExtensionInfoArg<'_, <Resolver as TypeResolver>::TypeId>],
    transaction_extensions: &Exts,
    type_resolver: &Resolver,
    out: &mut Vec<u8>,
) -> Result<(), TransactionExtensionsError>
where
    Resolver: TypeResolver,
    Exts: TransactionExtensions<Resolver>,
{
    let nonempty_values = extension_ids
        .iter()
        .filter(|arg| !is_type_empty(arg.id.clone(), type_resolver))
        .map(|arg| (&*arg.name, arg.id.clone()));

    for (name, id) in nonempty_values {
        let res =
            transaction_extensions.encode_extension_value_to(name, id.clone(), type_resolver, out);

        match res {
            // All ok
            Ok(()) => {}
            // Extension not found. As a fallback, if it contains an "Option" then default it to None and encode that.
            Err(TransactionExtensionsError::NotFound(name)) => {
                if is_type_option(id, type_resolver) {
                    0u8.encode_to(out);
                } else {
                    return Err(TransactionExtensionsError::NotFound(name));
                }
            }
            // Some other error was encountered; return it immediately.
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Encode the transaction extension implicits to the provided output in the order given by
/// the extension args from an [`ExtrinsicExtensionInfo`]. We skip missing extensions whose
/// implicit would encode as 0 bytes.
fn encode_transaction_extension_implicits<Resolver, Exts>(
    extension_ids: &[ExtrinsicExtensionInfoArg<'_, <Resolver as TypeResolver>::TypeId>],
    transaction_extensions: &Exts,
    type_resolver: &Resolver,
    out: &mut Vec<u8>,
) -> Result<(), TransactionExtensionsError>
where
    Resolver: TypeResolver,
    Exts: TransactionExtensions<Resolver>,
{
    let nonempty_implicits = extension_ids
        .iter()
        .filter(|arg| !is_type_empty(arg.implicit_id.clone(), type_resolver))
        .map(|arg| (&*arg.name, arg.implicit_id.clone()));

    for (name, id) in nonempty_implicits {
        transaction_extensions.encode_extension_implicit_to(name, id, type_resolver, out)?;
    }
    Ok(())
}

/// Checks to see whether the type being given is empty, ie would require
/// 0 bytes to encode. We use this to skip 0 byte transaction extensions; ones
/// that are mentioned in the metadata but only used in the node side and require
/// no bytes to be given.
fn is_type_empty<Resolver: TypeResolver>(type_id: Resolver::TypeId, types: &Resolver) -> bool {
    struct IsEmptyVisitor<'r, R> {
        types: &'r R,
    }
    impl<'r, R: TypeResolver> scale_type_resolver::ResolvedTypeVisitor<'r> for IsEmptyVisitor<'r, R> {
        type TypeId = R::TypeId;
        type Value = bool;

        // The default ans safe assumption is that a type is _not_ empty.
        fn visit_unhandled(self, _: scale_type_resolver::UnhandledKind) -> Self::Value {
            false
        }
        // Arrays are empty if they are 0 length or the type inside is empty.
        fn visit_array(self, type_id: Self::TypeId, len: usize) -> Self::Value {
            len == 0 || is_type_empty(type_id, self.types)
        }
        // Composites are empty if all of their fields are empty.
        fn visit_composite<Path, Fields>(self, _path: Path, mut fields: Fields) -> Self::Value
        where
            Path: scale_type_resolver::PathIter<'r>,
            Fields: scale_decode::FieldIter<'r, Self::TypeId>,
        {
            fields.all(|f| is_type_empty(f.id, self.types))
        }
        // Tuples are empty if all of their fields are empty.
        fn visit_tuple<TypeIds>(self, mut type_ids: TypeIds) -> Self::Value
        where
            TypeIds: ExactSizeIterator<Item = Self::TypeId>,
        {
            type_ids.all(|id| is_type_empty(id, self.types))
        }
    }

    types
        .resolve_type(type_id, IsEmptyVisitor { types })
        .unwrap_or_default()
}

/// Checks to see whether a type (or the type inside) resolves to being an Option<T>.
fn is_type_option<Resolver: TypeResolver>(type_id: Resolver::TypeId, types: &Resolver) -> bool {
    struct IsOptionVisitor<'r, R> {
        types: &'r R,
    }
    impl<'r, R: TypeResolver> scale_type_resolver::ResolvedTypeVisitor<'r> for IsOptionVisitor<'r, R> {
        type TypeId = R::TypeId;
        type Value = bool;

        // The default ans safe assumption is that a type is _not_ an option.
        fn visit_unhandled(self, _: scale_type_resolver::UnhandledKind) -> Self::Value {
            false
        }
        // Composites are options if they contain exactly one field which is_type_option
        fn visit_composite<Path, Fields>(self, _path: Path, mut fields: Fields) -> Self::Value
        where
            Path: scale_type_resolver::PathIter<'r>,
            Fields: scale_decode::FieldIter<'r, Self::TypeId>,
        {
            match (fields.next(), fields.next()) {
                (Some(f), None) => is_type_option(f.id, self.types),
                _ => false,
            }
        }
        // Tuples are options if they contain exactly one field which is_type_option
        fn visit_tuple<TypeIds>(self, mut type_ids: TypeIds) -> Self::Value
        where
            TypeIds: ExactSizeIterator<Item = Self::TypeId>,
        {
            match (type_ids.next(), type_ids.next()) {
                (Some(id), None) => is_type_option(id, self.types),
                _ => false,
            }
        }
        // Variants are Options if the path is exactly ["Option"] and they contain a None variant at index 0.
        fn visit_variant<Path, Fields, Var>(self, mut path: Path, mut variants: Var) -> Self::Value
        where
            Path: scale_type_resolver::PathIter<'r>,
            Fields: scale_decode::FieldIter<'r, Self::TypeId>,
            Var: scale_type_resolver::VariantIter<'r, Fields>,
        {
            match (path.next(), path.next()) {
                // If the path exactly ["Option"]?
                (Some("Option"), None) => {
                    // For a bit more safety: does the option have 2 variants, and a None variant at index 0?
                    match (variants.next(), variants.next(), variants.next()) {
                        (Some(v1), Some(v2), None) => {
                            (v1.name == "None" && v1.index == 0)
                                || (v2.name == "None" && v2.index == 0)
                        }
                        _ => false,
                    }
                }
                _ => false,
            }
        }
    }

    types
        .resolve_type(type_id, IsOptionVisitor { types })
        .unwrap_or_default()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::methods::extrinsic_type_info::{ExtrinsicExtensionInfo, ExtrinsicExtensionInfoArg};
    use scale_info::PortableRegistry;

    /// A test type that implements [`TransactionExtension`] for specific types.
    struct TestExtension<Value, Implicit> {
        value: Value,
        implicit: Implicit,
    }

    /// A small helper to extract the Value and Implicit type from the above where needed.
    trait GetTestExtensionTypes {
        type Value;
        type Implicit;
    }

    impl<Value, Implicit> GetTestExtensionTypes for TestExtension<Value, Implicit> {
        type Value = Value;
        type Implicit = Implicit;
    }

    /// Create a concrete [`TestExtension`] which implements [`TransactionExtension`].
    macro_rules! make_test_extension {
        ( $name:ident value=$value:ty, implicit=$implicit:ty ) => {
            type $name = TestExtension<$value, $implicit>;
            impl TransactionExtension<PortableRegistry> for TestExtension<$value, $implicit> {
                const NAME: &str = stringify!($name);
                fn encode_value_to(
                    &self,
                    type_id: u32,
                    type_resolver: &PortableRegistry,
                    out: &mut Vec<u8>,
                ) -> Result<(), TransactionExtensionError> {
                    self.value.encode_as_type_to(type_id, type_resolver, out)?;
                    Ok(())
                }
                fn encode_implicit_to(
                    &self,
                    type_id: u32,
                    type_resolver: &PortableRegistry,
                    out: &mut Vec<u8>,
                ) -> Result<(), TransactionExtensionError> {
                    self.implicit
                        .encode_as_type_to(type_id, type_resolver, out)?;
                    Ok(())
                }
            }
        };
    }

    #[derive(scale_encode::EncodeAsType, scale_info::TypeInfo, Debug, Clone, PartialEq)]
    struct NestedOption<T>(Option<T>);

    make_test_extension!(ExtensionContainingOption       value=Option<bool>,          implicit=u64);
    make_test_extension!(ExtensionContainingNestedOption value=(NestedOption<bool>,), implicit=u64);
    make_test_extension!(ExtensionContainingNothing      value=(),                    implicit=());
    make_test_extension!(Extension1                      value=(bool, u64),           implicit=bool);
    make_test_extension!(Extension2                      value=String,                implicit=());

    /// A test extension which authorizes the transaction, standing in for
    /// something like the real `VerifyMultiSignature` extension.
    struct VerifyMultiSignature;

    impl GetTestExtensionTypes for VerifyMultiSignature {
        type Value = ();
        type Implicit = ();
    }

    impl TransactionExtension<PortableRegistry> for VerifyMultiSignature {
        const NAME: &str = "VerifyMultiSignature";

        fn is_authorization_extension(&self) -> bool {
            true
        }

        fn encode_value_to(
            &self,
            _type_id: u32,
            _type_resolver: &PortableRegistry,
            _out: &mut Vec<u8>,
        ) -> Result<(), TransactionExtensionError> {
            Ok(())
        }

        fn encode_implicit_to(
            &self,
            _type_id: u32,
            _type_resolver: &PortableRegistry,
            _out: &mut Vec<u8>,
        ) -> Result<(), TransactionExtensionError> {
            Ok(())
        }
    }

    /// Returns an ExtrinsicExtensionInfo vec given some set of test transaction extensions, as well
    /// as the PortableRegistry needed to resolve those types properly.
    macro_rules! make_extension_info {
        ( $($ident:ident),* $(,)? ) => {{
            use scale_info::meta_type;
            let mut registry = scale_info::Registry::new();
            let extension_info = vec![
                $(
                    ExtrinsicExtensionInfoArg {
                        name: <$ident as TransactionExtension<PortableRegistry>>::NAME.into(),
                        id: registry.register_type(&meta_type::<<$ident as GetTestExtensionTypes>::Value>()).id,
                        implicit_id: registry.register_type(&meta_type::<<$ident as GetTestExtensionTypes>::Implicit>()).id,
                    }
                ),*
            ];

            let portable_registry: PortableRegistry = registry.into();
            let info = ExtrinsicExtensionInfo { extension_ids: extension_info };
            (info, portable_registry)
        }}
    }

    fn assert_decodes_into<T: parity_scale_codec::Decode + std::fmt::Debug + PartialEq>(
        bytes: &[u8],
        target: T,
    ) {
        let cursor = &mut &*bytes;
        let actual = T::decode(cursor).expect("decoding should succeed");
        assert_eq!(actual, target, "actual does not match target");
        if !cursor.is_empty() {
            panic!("Leftvoer bytes after decoding");
        }
    }

    // -------------------------- And now the actual tests --------------------------

    #[test]
    fn v5_signer_payload_encodes_the_runtime_implication() {
        let (extension_info, types) = make_extension_info![
            Extension1,
            ExtensionContainingNothing,
            VerifyMultiSignature,
            ExtensionContainingOption,
        ];
        let extensions = (
            Extension1 {
                value: (true, 123),
                implicit: false,
            },
            VerifyMultiSignature,
            ExtensionContainingOption {
                value: Some(true),
                implicit: 12345,
            },
        );
        let call_info = ExtrinsicCallInfo {
            pallet_index: 1,
            call_index: 2,
            pallet_name: "Test".into(),
            call_name: "call".into(),
            args: Vec::new(),
        };

        let actual = encode_v5_signer_payload_with_info(
            &(),
            7,
            &extensions,
            &types,
            &call_info,
            &extension_info,
        )
        .unwrap();

        // Mirrors TxBaseImplication((extension_version, call)) followed by the explicit and
        // implicit implications strictly after VerifyMultiSignature.
        let expected_preimage = (7u8, 1u8, 2u8, Some(true), 12345u64).encode();
        let expected = sp_crypto_hashing::blake2_256(&expected_preimage);

        assert_eq!(actual, expected);
    }

    #[test]
    fn encode_transaction_extension_values_basic() {
        let (info, types) = make_extension_info![Extension1, Extension2,];

        let exts = (
            Extension1 {
                value: (true, 123),
                implicit: false,
            },
            Extension2 {
                value: "Hello".to_owned(),
                implicit: (),
            },
        );

        let mut out = vec![];
        encode_transaction_extension_values(&info.extension_ids, &exts, &types, &mut out)
            .expect("Encoding should succeed");
        assert_decodes_into(&out, (true, 123u64, "Hello".to_owned()));
    }

    #[test]
    fn encode_transaction_extension_values_order_irrelevant() {
        let (info, types) = make_extension_info![Extension1, Extension2,];

        let exts = (
            Extension2 {
                value: "Hello".to_owned(),
                implicit: (),
            },
            Extension1 {
                value: (true, 123),
                implicit: false,
            },
        );

        let mut out = vec![];
        encode_transaction_extension_values(&info.extension_ids, &exts, &types, &mut out)
            .expect("Encoding should succeed");
        assert_decodes_into(&out, (true, 123u64, "Hello".to_owned()));
    }

    #[test]
    fn encode_transaction_extension_values_skips_empty() {
        let (info, types) =
            make_extension_info![Extension1, ExtensionContainingNothing, Extension2,];

        let exts = (
            Extension1 {
                value: (true, 123),
                implicit: false,
            },
            Extension2 {
                value: "Hello".to_owned(),
                implicit: (),
            },
        );

        let mut out = vec![];
        encode_transaction_extension_values(&info.extension_ids, &exts, &types, &mut out)
            .expect("Encoding should succeed despite Option");
        assert_decodes_into(&out, (true, 123u64, "Hello".to_owned()));
    }

    #[test]
    fn encode_transaction_extension_values_skips_option() {
        let (info, types) = make_extension_info![
            Extension1,
            ExtensionContainingOption,
            ExtensionContainingNestedOption,
            Extension2,
        ];

        let exts = (
            Extension1 {
                value: (true, 123),
                implicit: false,
            },
            Extension2 {
                value: "Hello".to_owned(),
                implicit: (),
            },
        );

        let mut out = vec![];
        encode_transaction_extension_values(&info.extension_ids, &exts, &types, &mut out)
            .expect("Encoding should succeed despite Option");
        assert_decodes_into(&out, (true, 123u64, 0u8, 0u8, "Hello".to_owned()));
    }

    #[test]
    fn encode_transaction_extension_implicits_skips_empty() {
        let (info, types) =
            make_extension_info![Extension1, Extension2, ExtensionContainingOption,];

        let exts = (
            Extension1 {
                value: (true, 123),
                implicit: false,
            },
            Extension2 {
                value: "Hello".to_owned(),
                implicit: (),
            },
            ExtensionContainingOption {
                value: None,
                implicit: 12345,
            },
        );

        let mut out = vec![];
        encode_transaction_extension_implicits(&info.extension_ids, &exts, &types, &mut out)
            .expect("Encoding should succeed");
        assert_decodes_into(&out, (false, 12345u64));
    }

    #[test]
    fn encode_transaction_extension_implicits_order_irrelevant() {
        let (info, types) =
            make_extension_info![Extension1, Extension2, ExtensionContainingOption,];

        let exts = (
            ExtensionContainingOption {
                value: None,
                implicit: 12345,
            },
            Extension2 {
                value: "Hello".to_owned(),
                implicit: (),
            },
            Extension1 {
                value: (true, 123),
                implicit: false,
            },
        );

        let mut out = vec![];
        encode_transaction_extension_implicits(&info.extension_ids, &exts, &types, &mut out)
            .expect("Encoding should succeed");
        assert_decodes_into(&out, (false, 12345u64));
    }
}
