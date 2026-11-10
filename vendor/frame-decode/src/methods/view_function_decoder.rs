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

use super::view_function_type_info::{
    ViewFunctionInfo, ViewFunctionInfoError, ViewFunctionTypeInfo,
};
use crate::utils::{DecodeErrorTrace, decode_with_error_tracing};
use scale_type_resolver::TypeResolver;

/// An error returned trying to decode a View Function responses.
#[non_exhaustive]
#[allow(missing_docs)]
#[derive(Clone, Debug, thiserror::Error)]
pub enum ViewFunctionDecodeError<TypeId> {
    #[error("Cannot get View Function info: {0}")]
    CannotGetInfo(ViewFunctionInfoError<'static>),
    #[error("Cannot decode View Function response: {reason}")]
    CannotDecodeValue {
        ty: TypeId,
        reason: DecodeErrorTrace,
    },
}

/// Decode a View Function response.
pub fn decode_view_function_response<'scale, 'resolver, Info, Resolver, V>(
    pallet_name: &str,
    function_name: &str,
    cursor: &mut &'scale [u8],
    info: &Info,
    type_resolver: &'resolver Resolver,
    visitor: V,
) -> Result<V::Value<'scale, 'resolver>, ViewFunctionDecodeError<Info::TypeId>>
where
    Info: ViewFunctionTypeInfo,
    Info::TypeId: Clone + core::fmt::Debug,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
    V: scale_decode::Visitor<TypeResolver = Resolver>,
    V::Error: core::fmt::Debug,
{
    let view_function_info = info
        .view_function_info(pallet_name, function_name)
        .map_err(|e| ViewFunctionDecodeError::CannotGetInfo(e.into_owned()))?;

    decode_view_function_response_with_info(cursor, &view_function_info, type_resolver, visitor)
}

/// Decode a View Function response.
///
/// Unlike [`decode_view_function_response`], which obtains the View Function information internally given the trait and
/// method names, this function takes the View Function info as an argument. This is useful if you already have the
/// View Function info available, for exampel if you are making multiple calls to the same API and wish to decode each one.
pub fn decode_view_function_response_with_info<'scale, 'resolver, V>(
    cursor: &mut &'scale [u8],
    view_function_info: &ViewFunctionInfo<<V::TypeResolver as TypeResolver>::TypeId>,
    type_resolver: &'resolver V::TypeResolver,
    visitor: V,
) -> Result<
    V::Value<'scale, 'resolver>,
    ViewFunctionDecodeError<<V::TypeResolver as TypeResolver>::TypeId>,
>
where
    V: scale_decode::Visitor,
    V::Error: core::fmt::Debug,
{
    let response_id = view_function_info.output_id.clone();

    decode_with_error_tracing(cursor, response_id.clone(), type_resolver, visitor).map_err(|e| {
        ViewFunctionDecodeError::CannotDecodeValue {
            ty: response_id,
            reason: e,
        }
    })
}
