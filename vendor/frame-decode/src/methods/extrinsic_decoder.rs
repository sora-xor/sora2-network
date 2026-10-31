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

use crate::methods::extrinsic_type_info::ExtrinsicInfoError;
use crate::methods::extrinsic_type_info::ExtrinsicTypeInfo;
use crate::utils::{DecodeErrorTrace, decode_with_error_tracing};
use alloc::borrow::Cow;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;
use parity_scale_codec::{Compact, Decode};
use scale_type_resolver::TypeResolver;

/// An error returned trying to decode extrinsic bytes.
#[non_exhaustive]
#[allow(missing_docs)]
#[derive(Debug, Clone, thiserror::Error)]
pub enum ExtrinsicDecodeError {
    #[error("Cannot decode the compact-encoded extrinsic length.")]
    CannotDecodeLength,
    #[error(
        "The actual number of bytes does not match the compact-encoded extrinsic length; expected {expected_len} bytes but got {actual_len} bytes."
    )]
    WrongLength {
        expected_len: usize,
        actual_len: usize,
    },
    #[error("Not enough bytes to decode a valid extrinsic.")]
    NotEnoughBytes,
    #[error("This extrinsic version ({0}) is not supported.")]
    VersionNotSupported(u8),
    #[error(
        "The extrinsic type 0b{extrinsic_type:02b} is not supported (given extrinsic version {version})."
    )]
    ExtrinsicTypeNotSupported { version: u8, extrinsic_type: u8 },
    #[error("Cannot get extrinsic info:\n\n{0}")]
    CannotGetInfo(ExtrinsicInfoError<'static>),
    #[error("Cannot decode signature:\n\n{0}")]
    CannotDecodeSignature(DecodeErrorTrace),
    #[error("Cannot decode pallet index byte:\n\n{0}")]
    CannotDecodePalletIndex(parity_scale_codec::Error),
    #[error("Cannot decode call index byte:\n\n{0}")]
    CannotDecodeCallIndex(parity_scale_codec::Error),
    #[error("Cannot decode transaction extensions version byte:\n\n{0}")]
    CannotDecodeExtensionsVersion(parity_scale_codec::Error),
    #[error(
        "Cannot decode call data for argument {argument_name} in {pallet_name}.{call_name}:\n\n{reason}"
    )]
    CannotDecodeCallData {
        pallet_name: String,
        call_name: String,
        argument_name: String,
        reason: DecodeErrorTrace,
    },
}

/// An owned variant of an Extrinsic (note: this may still contain
/// references if the visitor used to decode the extrinsic contents holds
/// onto any)
pub type ExtrinsicOwned<TypeId> = Extrinsic<'static, TypeId>;

/// Information about the extrinsic.
#[derive(Clone, Debug)]
pub struct Extrinsic<'info, TypeId> {
    compact_prefix_len: u8,
    version: u8,
    version_ty: ExtrinsicType,
    byte_len: u32,
    signature: Option<ExtrinsicSignature<TypeId>>,
    extensions: Option<ExtrinsicExtensions<'info, TypeId>>,
    pallet_name: Cow<'info, str>,
    pallet_index: u8,
    pallet_index_idx: u32,
    call_name: Cow<'info, str>,
    call_index: u8,
    call_data: Vec<NamedArg<'info, TypeId>>,
}

impl<'info, TypeId> Extrinsic<'info, TypeId> {
    /// Take ownership of the extrinsic, so that it no longer references
    /// the extrinsic info or bytes.
    pub fn into_owned(self) -> ExtrinsicOwned<TypeId> {
        Extrinsic {
            compact_prefix_len: self.compact_prefix_len,
            version: self.version,
            version_ty: self.version_ty,
            byte_len: self.byte_len,
            signature: self.signature,
            extensions: self.extensions.map(|e| e.into_owned()),
            pallet_name: Cow::Owned(self.pallet_name.into_owned()),
            pallet_index: self.pallet_index,
            pallet_index_idx: self.pallet_index_idx,
            call_name: Cow::Owned(self.call_name.into_owned()),
            call_index: self.call_index,
            call_data: self.call_data.into_iter().map(|e| e.into_owned()).collect(),
        }
    }

    /// The extrinsic version.
    pub fn version(&self) -> u8 {
        self.version
    }

    /// The type of the extrinsic.
    pub fn ty(&self) -> ExtrinsicType {
        self.version_ty
    }

    /// The length of the extrinsic payload, excluding the prefixed compact-encoded length bytes.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.byte_len as usize
    }

    /// The name of the pallet that this extrinsic is calling into.
    pub fn pallet_name(&self) -> &str {
        &self.pallet_name
    }

    /// the index of the pallet that this extrinsic is calling into.
    pub fn pallet_index(&self) -> u8 {
        self.pallet_index
    }

    /// The name of the call that the extrinsic is making.
    pub fn call_name(&self) -> &str {
        &self.call_name
    }

    /// the index of the call that the extrinsic is making.
    pub fn call_index(&self) -> u8 {
        self.call_index
    }

    /// Does the extrinsic have a signature.
    pub fn is_signed(&self) -> bool {
        self.signature.is_some()
    }

    /// Return the extrinsic signature payload, if present. This contains the
    /// address and signature information.
    pub fn signature_payload(&self) -> Option<&ExtrinsicSignature<TypeId>> {
        self.signature.as_ref()
    }

    /// Return the transaction extension payload, if present. This contains the
    /// transaction extensions.
    pub fn transaction_extension_payload(&self) -> Option<&ExtrinsicExtensions<'info, TypeId>> {
        self.extensions.as_ref()
    }

    /// Iterate over the call data argument names and types.
    pub fn call_data(&self) -> impl ExactSizeIterator<Item = &NamedArg<'info, TypeId>> {
        self.call_data.iter()
    }

    /// Return a range denoting the call data bytes. This includes the pallet index and
    /// call index bytes and then any encoded arguments for the call.
    pub fn call_data_range(&self) -> Range<usize> {
        let start = self.pallet_index_idx as usize;
        let end = self
            .call_data()
            .map(|a| a.range.end as usize)
            .max()
            .unwrap_or(start);

        Range { start, end }
    }

    /// Return a range denoting the arguments given to the call. This does *not* include
    /// the pallet index and call index bytes.
    pub fn call_data_args_range(&self) -> Range<usize> {
        let start = (self.pallet_index_idx + 2) as usize;
        let end = self
            .call_data()
            .map(|a| a.range.end as usize)
            .max()
            .unwrap_or(start);

        Range { start, end }
    }

    /// Map the signature type IDs to something else.
    pub fn map_type_id<NewTypeId, F>(self, mut f: F) -> Extrinsic<'info, NewTypeId>
    where
        F: FnMut(TypeId) -> NewTypeId,
    {
        Extrinsic {
            compact_prefix_len: self.compact_prefix_len,
            version: self.version,
            version_ty: self.version_ty,
            byte_len: self.byte_len,
            signature: self.signature.map(|s| s.map_type_id(&mut f)),
            extensions: self.extensions.map(|e| e.map_type_id(&mut f)),
            pallet_name: self.pallet_name,
            pallet_index: self.pallet_index,
            pallet_index_idx: self.pallet_index_idx,
            call_name: self.call_name,
            call_index: self.call_index,
            call_data: self
                .call_data
                .into_iter()
                .map(|s| s.map_type_id(&mut f))
                .collect(),
        }
    }
}

/// The type of the extrinsic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum ExtrinsicType {
    /// Only call data
    Bare,
    /// Transaction extensions and call data
    General,
    /// Address, signature, transaction extensions and call data
    Signed,
}

/// Information about the extrinsic signature.
#[derive(Clone, Debug)]
pub struct ExtrinsicSignature<TypeId> {
    // Store byte offsets so people can ask for raw
    // bytes to do their own thing.
    address_start_idx: u32,
    address_end_idx: u32,
    signature_end_idx: u32,
    // Also decode and store actual values. We could
    // do this more "on demand" but it complicates
    // everything. Ultimately just a couple of vec allocs
    // that we could perhaps optimise into SmallVecs or
    // something if desired.
    address_ty: TypeId,
    signature_ty: TypeId,
}

impl<TypeId> ExtrinsicSignature<TypeId> {
    /// Return a range denoting the address bytes.
    pub fn address_range(&self) -> Range<usize> {
        Range {
            start: self.address_start_idx as usize,
            end: self.address_end_idx as usize,
        }
    }

    /// The decoded address.
    pub fn address_type(&self) -> &TypeId {
        &self.address_ty
    }

    /// Return a range denoting the signature bytes.
    pub fn signature_range(&self) -> Range<usize> {
        Range {
            start: self.address_end_idx as usize,
            end: self.signature_end_idx as usize,
        }
    }

    /// The decoded signature.
    pub fn signature_type(&self) -> &TypeId {
        &self.signature_ty
    }

    /// Map the signature type IDs to something else.
    pub fn map_type_id<NewTypeId, F>(self, mut f: F) -> ExtrinsicSignature<NewTypeId>
    where
        F: FnMut(TypeId) -> NewTypeId,
    {
        ExtrinsicSignature {
            address_start_idx: self.address_start_idx,
            address_end_idx: self.address_end_idx,
            signature_end_idx: self.signature_end_idx,
            address_ty: f(self.address_ty),
            signature_ty: f(self.signature_ty),
        }
    }
}

/// Information about the extrinsic signed extensions.
#[derive(Clone, Debug)]
pub struct ExtrinsicExtensions<'info, TypeId> {
    transaction_extensions_version: u8,
    transaction_extensions: Vec<NamedArg<'info, TypeId>>,
}

impl<'info, TypeId> ExtrinsicExtensions<'info, TypeId> {
    /// Take ownership of the signature.
    pub fn into_owned(self) -> ExtrinsicExtensions<'static, TypeId> {
        ExtrinsicExtensions {
            transaction_extensions_version: self.transaction_extensions_version,
            transaction_extensions: self
                .transaction_extensions
                .into_iter()
                .map(|e| e.into_owned())
                .collect(),
        }
    }

    /// The version of the transaction extensions
    pub fn version(&self) -> u8 {
        self.transaction_extensions_version
    }

    /// Iterate over the signed extension argument names and types.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &NamedArg<'info, TypeId>> {
        self.transaction_extensions.iter()
    }

    /// Return a range denoting the transaction extension bytes. This does
    /// *not* include any version byte.
    pub fn range(&self) -> Range<usize> {
        let start = self
            .iter()
            .map(|a| a.range.start as usize)
            .min()
            .unwrap_or(0);
        let end = self
            .iter()
            .map(|a| a.range.end as usize)
            .max()
            .unwrap_or(start);

        Range { start, end }
    }

    /// Map the extensions type IDs to something else.
    pub fn map_type_id<NewTypeId, F>(self, mut f: F) -> ExtrinsicExtensions<'info, NewTypeId>
    where
        F: FnMut(TypeId) -> NewTypeId,
    {
        ExtrinsicExtensions {
            transaction_extensions_version: self.transaction_extensions_version,
            transaction_extensions: self
                .transaction_extensions
                .into_iter()
                .map(|s| s.map_type_id(&mut f))
                .collect(),
        }
    }
}

/// A single named argument.
#[derive(Clone, Debug)]
pub struct NamedArg<'info, TypeId> {
    name: Cow<'info, str>,
    range: Range<u32>,
    ty: TypeId,
}

impl<'info, TypeId> NamedArg<'info, TypeId> {
    /// Map the type ID to something else.
    pub fn map_type_id<NewTypeId, F>(self, mut f: F) -> NamedArg<'info, NewTypeId>
    where
        F: FnMut(TypeId) -> NewTypeId,
    {
        NamedArg {
            name: self.name,
            range: self.range,
            ty: f(self.ty),
        }
    }
}

impl<TypeId> NamedArg<'_, TypeId> {
    /// Take ownership of this named argument.
    pub fn into_owned(self) -> NamedArg<'static, TypeId> {
        NamedArg {
            name: Cow::Owned(self.name.into_owned()),
            range: self.range,
            ty: self.ty,
        }
    }

    /// The name of this argument.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return a range denoting the bytes associated with this argument value.
    pub fn range(&self) -> Range<usize> {
        Range {
            start: self.range.start as usize,
            end: self.range.end as usize,
        }
    }

    /// The type ID associated with this argument value.
    pub fn ty(&self) -> &TypeId {
        &self.ty
    }
}

/// Decode an extrinsic, returning information about it.
///
/// This information can be used to then decode the various parts of the extrinsic (address,
/// signature, transaction extensions and call data) to concrete types.
///
/// # Example
///
/// Here, we decode all of the extrinsics in a block to a [`scale_value::Value`] type.
///
/// ```rust
/// use frame_decode::extrinsics::decode_extrinsic;
/// use frame_decode::helpers::decode_with_visitor;
/// use frame_metadata::RuntimeMetadata;
/// use parity_scale_codec::Decode;
/// use scale_value::scale::ValueVisitor;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata_10000000_9180.scale").unwrap();
/// let RuntimeMetadata::V14(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// let extrinsics_bytes = std::fs::read("artifacts/exts_10000000_9180.json").unwrap();
/// let extrinsics_hex: Vec<String> = serde_json::from_slice(&extrinsics_bytes).unwrap();
///
/// for ext_hex in extrinsics_hex {
///     let ext_bytes = hex::decode(ext_hex.trim_start_matches("0x")).unwrap();
///
///     // Decode the extrinsic, returning information about it:
///     let ext_info = decode_extrinsic(&mut &*ext_bytes, &metadata, &metadata.types).unwrap();
///
///     // Decode the signature details to scale_value::Value's.
///     if let Some(sig) = ext_info.signature_payload() {
///         let address_bytes =  &ext_bytes[sig.address_range()];
///         let address_value = decode_with_visitor(
///             &mut &*address_bytes,
///             *sig.address_type(),
///             &metadata.types,
///             ValueVisitor::new()
///         ).unwrap();
///
///         let signature_bytes = &ext_bytes[sig.signature_range()];
///         let signature_value = decode_with_visitor(
///             &mut &*signature_bytes,
///             *sig.signature_type(),
///             &metadata.types,
///             ValueVisitor::new()
///         ).unwrap();
///     }
///
///     // Decode the transaction extensions to scale_value::Value's.
///     if let Some(exts) = ext_info.transaction_extension_payload() {
///         for ext in exts.iter() {
///             let ext_name = ext.name();
///             let ext_bytes = &ext_bytes[ext.range()];
///             let ext_value = decode_with_visitor(
///                 &mut &*ext_bytes,
///                 *ext.ty(),
///                 &metadata.types,
///                 ValueVisitor::new()
///             ).unwrap();
///         }
///     }
///
///     // Decode the call data args to scale_value::Value's.
///     for arg in ext_info.call_data() {
///         let arg_name = arg.name();
///         let arg_bytes = &ext_bytes[arg.range()];
///         let arg_value = decode_with_visitor(
///             &mut &*arg_bytes,
///             *arg.ty(),
///             &metadata.types,
///             ValueVisitor::new()
///         ).unwrap();
///     }
/// }
/// ```
pub fn decode_extrinsic<'info, Info, Resolver>(
    cursor: &mut &[u8],
    info: &'info Info,
    type_resolver: &Resolver,
) -> Result<Extrinsic<'info, Info::TypeId>, ExtrinsicDecodeError>
where
    Info: ExtrinsicTypeInfo,
    Info::TypeId: core::fmt::Debug + Clone,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
{
    let bytes = *cursor;
    let ext_len = Compact::<u64>::decode(cursor)
        .map_err(|_| ExtrinsicDecodeError::CannotDecodeLength)?
        .0 as usize;

    let compact_prefix_len = (bytes.len() - cursor.len()) as u8;

    if cursor.len() != ext_len {
        return Err(ExtrinsicDecodeError::WrongLength {
            expected_len: ext_len,
            actual_len: cursor.len(),
        });
    }

    if cursor.is_empty() {
        return Err(ExtrinsicDecodeError::NotEnoughBytes);
    }

    // Decide how to decode the extrinsic based on the version.
    // As of https://github.com/paritytech/polkadot-sdk/pull/3685,
    // only 6 bits used for version. Shouldn't break old impls.
    let version = cursor[0] & 0b0011_1111;
    let version_type = cursor[0] >> 6;
    *cursor = &cursor[1..];

    // We only know how to decode v4 and v5 extrinsics.
    if version != 4 && version != 5 {
        return Err(ExtrinsicDecodeError::VersionNotSupported(version));
    }

    // We know about the following types of extrinsics:
    // - "Bare": no signature or extensions. V4 inherents encode the same as V5 bare.
    // - "Signed": an address, signature and extensions. Only exists in V4.
    // - "General": no signature, just extensions (one of which can include sig). Only exists in V5.
    let version_ty = match version_type {
        0b00 => ExtrinsicType::Bare,
        0b10 if version == 4 => ExtrinsicType::Signed,
        0b01 if version == 5 => ExtrinsicType::General,
        _ => {
            return Err(ExtrinsicDecodeError::ExtrinsicTypeNotSupported {
                version,
                extrinsic_type: version_type,
            });
        }
    };

    let curr_idx = |cursor: &mut &[u8]| (bytes.len() - cursor.len()) as u32;

    // Signature part. Present for V4 signed extrinsics
    let signature = (version_ty == ExtrinsicType::Signed)
        .then(|| {
            let signature_info = info
                .extrinsic_signature_info()
                .map_err(|e| ExtrinsicDecodeError::CannotGetInfo(e.into_owned()))?;

            let address_start_idx = curr_idx(cursor);
            decode_with_error_tracing(
                cursor,
                signature_info.address_id.clone(),
                type_resolver,
                scale_decode::visitor::IgnoreVisitor::new(),
            )
            .map_err(ExtrinsicDecodeError::CannotDecodeSignature)?;
            let address_end_idx = curr_idx(cursor);

            decode_with_error_tracing(
                cursor,
                signature_info.signature_id.clone(),
                type_resolver,
                scale_decode::visitor::IgnoreVisitor::new(),
            )
            .map_err(ExtrinsicDecodeError::CannotDecodeSignature)?;
            let signature_end_idx = curr_idx(cursor);

            Ok(ExtrinsicSignature {
                address_start_idx,
                address_end_idx,
                signature_end_idx,
                address_ty: signature_info.address_id,
                signature_ty: signature_info.signature_id,
            })
        })
        .transpose()?;

    // "General" extensions now have a single byte representing the extension version.
    let extension_version = (version_ty == ExtrinsicType::General)
        .then(|| u8::decode(cursor).map_err(ExtrinsicDecodeError::CannotDecodeExtensionsVersion))
        .transpose()?;

    // Signed and General extrinsics both now have a set of transaction extensions.
    let extensions = (version_ty == ExtrinsicType::General || version_ty == ExtrinsicType::Signed)
        .then(|| {
            let extension_info = info
                .extrinsic_extension_info(extension_version)
                .map_err(|e| ExtrinsicDecodeError::CannotGetInfo(e.into_owned()))?;

            let mut transaction_extensions = vec![];
            for ext in extension_info.extension_ids {
                let start_idx = curr_idx(cursor);
                decode_with_error_tracing(
                    cursor,
                    ext.id.clone(),
                    type_resolver,
                    scale_decode::visitor::IgnoreVisitor::new(),
                )
                .map_err(ExtrinsicDecodeError::CannotDecodeSignature)?;
                let end_idx = curr_idx(cursor);

                transaction_extensions.push(NamedArg {
                    name: ext.name,
                    range: Range {
                        start: start_idx,
                        end: end_idx,
                    },
                    ty: ext.id,
                });
            }

            Ok::<_, ExtrinsicDecodeError>(ExtrinsicExtensions {
                transaction_extensions_version: extension_version.unwrap_or(0),
                transaction_extensions,
            })
        })
        .transpose()?;

    // All extrinsics now have the encoded call data.
    let pallet_index_idx = curr_idx(cursor);
    let pallet_index: u8 =
        Decode::decode(cursor).map_err(ExtrinsicDecodeError::CannotDecodePalletIndex)?;
    let call_index: u8 =
        Decode::decode(cursor).map_err(ExtrinsicDecodeError::CannotDecodeCallIndex)?;
    let call_info = info
        .extrinsic_call_info_by_index(pallet_index, call_index)
        .map_err(|e| ExtrinsicDecodeError::CannotGetInfo(e.into_owned()))?;

    let mut call_data = vec![];
    for arg in call_info.args {
        let start_idx = curr_idx(cursor);
        decode_with_error_tracing(
            cursor,
            arg.id.clone(),
            type_resolver,
            scale_decode::visitor::IgnoreVisitor::new(),
        )
        .map_err(|e| ExtrinsicDecodeError::CannotDecodeCallData {
            pallet_name: call_info.pallet_name.to_string(),
            call_name: call_info.call_name.to_string(),
            argument_name: arg.name.to_string(),
            reason: e,
        })?;
        let end_idx = curr_idx(cursor);

        call_data.push(NamedArg {
            name: arg.name,
            range: Range {
                start: start_idx,
                end: end_idx,
            },
            ty: arg.id,
        })
    }

    let ext = Extrinsic {
        compact_prefix_len,
        version,
        version_ty,
        byte_len: bytes.len() as u32,
        signature,
        extensions,
        pallet_name: call_info.pallet_name,
        pallet_index,
        pallet_index_idx,
        call_name: call_info.call_name,
        call_index,
        call_data,
    };

    Ok(ext)
}
