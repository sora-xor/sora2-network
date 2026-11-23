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

use crate::methods::custom_value_type_info::{
    CustomValueInfo, CustomValueInfoError, CustomValueTypeInfo,
};
use crate::utils::{DecodeErrorTrace, decode_with_error_tracing};
use alloc::vec::Vec;
use scale_type_resolver::TypeResolver;

/// An error returned trying to decode a custom value.
#[non_exhaustive]
#[allow(missing_docs)]
#[derive(Clone, Debug, thiserror::Error)]
pub enum CustomValueDecodeError<TypeId> {
    #[error("Cannot get custom value info: {0}")]
    CannotGetInfo(CustomValueInfoError),
    #[error("Cannot decode custom value: {reason}")]
    CannotDecodeValue {
        ty: TypeId,
        reason: DecodeErrorTrace,
    },
    #[error("There were leftover bytes attempting to decode the custom value")]
    LeftoverBytes { bytes: Vec<u8> },
}

/// Decode a custom value from the provided information.
pub fn decode_custom_value<'info, 'resolver, Info, Resolver, V>(
    name: &str,
    info: &'info Info,
    type_resolver: &'resolver V::TypeResolver,
    visitor: V,
) -> Result<V::Value<'info, 'resolver>, CustomValueDecodeError<Info::TypeId>>
where
    Info: CustomValueTypeInfo,
    Info::TypeId: Clone + core::fmt::Debug,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
    V: scale_decode::Visitor<TypeResolver = Resolver>,
    V::Error: core::fmt::Debug,
{
    let info = info
        .custom_value_info(name)
        .map_err(CustomValueDecodeError::CannotGetInfo)?;

    decode_custom_value_with_info(&info, type_resolver, visitor)
}

/// Decode a custom value given the [`CustomValueInfo`] and a resolver to resolve the custom value type.
pub fn decode_custom_value_with_info<'info, 'resolver, V>(
    info: &CustomValueInfo<'info, <V::TypeResolver as TypeResolver>::TypeId>,
    type_resolver: &'resolver V::TypeResolver,
    visitor: V,
) -> Result<
    V::Value<'info, 'resolver>,
    CustomValueDecodeError<<V::TypeResolver as TypeResolver>::TypeId>,
>
where
    V: scale_decode::Visitor,
    V::Error: core::fmt::Debug,
{
    let type_id = info.type_id.clone();
    let cursor = &mut &*info.bytes;

    let value = decode_with_error_tracing(cursor, type_id.clone(), type_resolver, visitor)
        .map_err(|e| CustomValueDecodeError::CannotDecodeValue {
            ty: type_id,
            reason: e,
        })?;

    if !cursor.is_empty() {
        Err(CustomValueDecodeError::LeftoverBytes {
            bytes: cursor.to_vec(),
        })
    } else {
        Ok(value)
    }
}
