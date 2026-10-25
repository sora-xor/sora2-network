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

use crate::methods::constant_type_info::{ConstantInfo, ConstantInfoError, ConstantTypeInfo};
use crate::utils::{DecodeErrorTrace, decode_with_error_tracing};
use alloc::vec::Vec;
use scale_type_resolver::TypeResolver;

/// An error returned trying to decode a constant.
#[non_exhaustive]
#[allow(missing_docs)]
#[derive(Clone, Debug, thiserror::Error)]
pub enum ConstantDecodeError<TypeId> {
    #[error("Cannot get constant info: {0}")]
    CannotGetInfo(ConstantInfoError<'static>),
    #[error("Cannot decode constant: {reason}")]
    CannotDecodeValue {
        ty: TypeId,
        reason: DecodeErrorTrace,
    },
    #[error("There were leftover bytes attempting to decode the constant")]
    LeftoverBytes { bytes: Vec<u8> },
}

/// Decode a constant from the provided information.
///
/// # Examples
///
/// Decode a constant into a [`scale_value::Value`]`:
///
/// ```rust
/// use frame_decode::constants::decode_constant;
/// use frame_metadata::RuntimeMetadata;
/// use scale_value::scale::ValueVisitor;
/// use parity_scale_codec::Decode;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata_10000000_9180.scale").unwrap();
/// let RuntimeMetadata::V14(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// let ed = decode_constant(
///     "Balances",
///     "ExistentialDeposit",
///     &metadata,
///     &metadata.types,
///     ValueVisitor::new()
/// ).unwrap();
///
/// println!("{ed}");
/// ```
///
/// Or we can just take the constant info and decode it "manually" (in this case, remember to
/// also check whether the bytes are fully consumed; leftover bytes indicate a failure to
/// properly decode them):
///
/// ```rust
/// use frame_decode::constants::{ConstantTypeInfo, decode_constant_with_info};
/// use frame_metadata::RuntimeMetadata;
/// use scale_decode::DecodeAsType;
/// use scale_value::Value;
/// use parity_scale_codec::Decode;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata_10000000_9180.scale").unwrap();
/// let RuntimeMetadata::V14(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// let ed_info = metadata.constant_info("Balances", "ExistentialDeposit").unwrap();
///
/// let ed_bytes = ed_info.bytes;
/// let ed_type = ed_info.type_id;
///
/// let ed = Value::decode_as_type(&mut &*ed_bytes, ed_type, &metadata.types).unwrap();
/// println!("{ed}");
/// ```
pub fn decode_constant<'info, 'resolver, Info, Resolver, V>(
    pallet_name: &str,
    constant_name: &str,
    info: &'info Info,
    type_resolver: &'resolver V::TypeResolver,
    visitor: V,
) -> Result<V::Value<'info, 'resolver>, ConstantDecodeError<Info::TypeId>>
where
    Info: ConstantTypeInfo,
    Info::TypeId: Clone + core::fmt::Debug,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
    V: scale_decode::Visitor<TypeResolver = Resolver>,
    V::Error: core::fmt::Debug,
{
    let info = info
        .constant_info(pallet_name, constant_name)
        .map_err(|e| ConstantDecodeError::CannotGetInfo(e.into_owned()))?;

    decode_constant_with_info(&info, type_resolver, visitor)
}

/// Decode a constant given the [`ConstantInfo`] and a resolver to resolve the constant type.
///
/// # Example
///
/// ```rust
/// use frame_decode::constants::{ConstantTypeInfo, decode_constant_with_info};
/// use frame_metadata::RuntimeMetadata;
/// use scale_value::scale::ValueVisitor;
/// use parity_scale_codec::Decode;
///
/// let metadata_bytes = std::fs::read("artifacts/metadata_10000000_9180.scale").unwrap();
/// let RuntimeMetadata::V14(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { return };
///
/// let ed_info = metadata.constant_info("Balances", "ExistentialDeposit").unwrap();
///
/// let ed = decode_constant_with_info(
///     &ed_info,
///     &metadata.types,
///     ValueVisitor::new()
/// ).unwrap();
///
/// println!("{ed}");
/// ```
pub fn decode_constant_with_info<'info, 'resolver, V>(
    info: &ConstantInfo<'info, <V::TypeResolver as TypeResolver>::TypeId>,
    type_resolver: &'resolver V::TypeResolver,
    visitor: V,
) -> Result<
    V::Value<'info, 'resolver>,
    ConstantDecodeError<<V::TypeResolver as TypeResolver>::TypeId>,
>
where
    V: scale_decode::Visitor,
    V::Error: core::fmt::Debug,
{
    let type_id = info.type_id.clone();
    let cursor = &mut &*info.bytes;

    let value = decode_with_error_tracing(cursor, type_id.clone(), type_resolver, visitor)
        .map_err(|e| ConstantDecodeError::CannotDecodeValue {
            ty: type_id,
            reason: e,
        })?;

    if !cursor.is_empty() {
        Err(ConstantDecodeError::LeftoverBytes {
            bytes: cursor.to_vec(),
        })
    } else {
        Ok(value)
    }
}
