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

use super::runtime_api_type_info::{RuntimeApiInfo, RuntimeApiInfoError, RuntimeApiTypeInfo};
use crate::utils::{DecodeErrorTrace, decode_with_error_tracing};
use scale_type_resolver::TypeResolver;

/// An error returned trying to decode a Runtime API responses.
#[non_exhaustive]
#[allow(missing_docs)]
#[derive(Clone, Debug, thiserror::Error)]
pub enum RuntimeApiDecodeError<TypeId> {
    #[error("Cannot get Runtime API info: {0}")]
    CannotGetInfo(RuntimeApiInfoError<'static>),
    #[error("Cannot decode Runtime API response: {reason}")]
    CannotDecodeValue {
        ty: TypeId,
        reason: DecodeErrorTrace,
    },
}

/// Decode a runtime API response.
pub fn decode_runtime_api_response<'scale, 'resolver, Info, Resolver, V>(
    trait_name: &str,
    method_name: &str,
    cursor: &mut &'scale [u8],
    info: &Info,
    type_resolver: &'resolver Resolver,
    visitor: V,
) -> Result<V::Value<'scale, 'resolver>, RuntimeApiDecodeError<Info::TypeId>>
where
    Info: RuntimeApiTypeInfo,
    Info::TypeId: Clone + core::fmt::Debug,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
    V: scale_decode::Visitor<TypeResolver = Resolver>,
    V::Error: core::fmt::Debug,
{
    let runtime_api_info = info
        .runtime_api_info(trait_name, method_name)
        .map_err(|e| RuntimeApiDecodeError::CannotGetInfo(e.into_owned()))?;

    decode_runtime_api_response_with_info(cursor, &runtime_api_info, type_resolver, visitor)
}

/// Decode a runtime API response.
///
/// Unlike [`decode_runtime_api_response`], which obtains the Runtime API information internally given the trait and
/// method names, this function takes the Runtime API info as an argument. This is useful if you already have the
/// Runtime API info available, for exampel if you are making multiple calls to the same API and wish to decode each one.
pub fn decode_runtime_api_response_with_info<'scale, 'resolver, V>(
    cursor: &mut &'scale [u8],
    runtime_api_info: &RuntimeApiInfo<<V::TypeResolver as TypeResolver>::TypeId>,
    type_resolver: &'resolver V::TypeResolver,
    visitor: V,
) -> Result<
    V::Value<'scale, 'resolver>,
    RuntimeApiDecodeError<<V::TypeResolver as TypeResolver>::TypeId>,
>
where
    V: scale_decode::Visitor,
    V::Error: core::fmt::Debug,
{
    let response_id = runtime_api_info.output_id.clone();

    decode_with_error_tracing(cursor, response_id.clone(), type_resolver, visitor).map_err(|e| {
        RuntimeApiDecodeError::CannotDecodeValue {
            ty: response_id,
            reason: e,
        }
    })
}
