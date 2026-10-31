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
use crate::utils::{EncodableValues, IntoEncodableValues};
use alloc::vec::Vec;
use scale_type_resolver::TypeResolver;

/// An error returned trying to encode View Function inputs.
#[non_exhaustive]
#[allow(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum ViewFunctionInputsEncodeError {
    #[error("Cannot get View Function info: {0}")]
    CannotGetInfo(ViewFunctionInfoError<'static>),
    #[error("Failed to encode View Function info: {0}")]
    EncodeError(#[from] scale_encode::Error),
    #[error("Wrong number of inputs provided; expected {num_inputs_expected}")]
    WrongNumberOfInputsProvided {
        /// The number of input parameters that were expected.
        num_inputs_expected: usize,
    },
}

/// The default name of the Runtime API that you must call to query a View Function, where
/// the arguments to this Runtime API can be encoded using [`encode_view_function_inputs`].
pub const RUNTIME_API_NAME: &str = "RuntimeViewFunction_execute_view_function";

/// Encode the Runtime API input data necessary to call a View Function.
pub fn encode_view_function_inputs<Info, Resolver, Inputs>(
    pallet_name: &str,
    function_name: &str,
    inputs: Inputs,
    info: &Info,
    type_resolver: &Resolver,
) -> Result<Vec<u8>, ViewFunctionInputsEncodeError>
where
    Inputs: IntoEncodableValues,
    Info: ViewFunctionTypeInfo,
    Info::TypeId: Clone + core::fmt::Debug,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
{
    // We'll need at least enough space to encode the query ID for the view function
    let mut out = Vec::with_capacity(32);
    encode_view_function_inputs_to(
        pallet_name,
        function_name,
        inputs,
        info,
        type_resolver,
        &mut out,
    )?;
    Ok(out)
}

/// Encode to a provided output Vec the Runtime API input data necessary to call a View Function.
pub fn encode_view_function_inputs_to<Info, Resolver, Inputs>(
    pallet_name: &str,
    function_name: &str,
    inputs: Inputs,
    info: &Info,
    type_resolver: &Resolver,
    out: &mut Vec<u8>,
) -> Result<(), ViewFunctionInputsEncodeError>
where
    Inputs: IntoEncodableValues,
    Info: ViewFunctionTypeInfo,
    Info::TypeId: Clone + core::fmt::Debug,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
{
    let view_function_api_info = info
        .view_function_info(pallet_name, function_name)
        .map_err(|e| ViewFunctionInputsEncodeError::CannotGetInfo(e.into_owned()))?;

    encode_view_function_inputs_with_info_to(inputs, &view_function_api_info, type_resolver, out)
}

/// Encode to a provided output Vec the Runtime API input data necessary to call a View Function.
///
/// Unlike [`encode_view_function_inputs_to`], which obtains the View Function info internally given trait and method names,
/// this function takes the View Function info as an argument. This is useful if you already have the info available,
/// for example if you are encoding multiple inputs for a given View Function.
pub fn encode_view_function_inputs_with_info_to<Resolver, Inputs>(
    inputs: Inputs,
    view_function_api_info: &ViewFunctionInfo<<Resolver as TypeResolver>::TypeId>,
    type_resolver: &Resolver,
    out: &mut Vec<u8>,
) -> Result<(), ViewFunctionInputsEncodeError>
where
    Inputs: IntoEncodableValues,
    Resolver: TypeResolver,
    <Resolver as TypeResolver>::TypeId: Clone + core::fmt::Debug,
{
    // If wrong number of inputs provided, bail early.
    if view_function_api_info.inputs.len() != inputs.num_encodable_values() {
        return Err(ViewFunctionInputsEncodeError::WrongNumberOfInputsProvided {
            num_inputs_expected: view_function_api_info.inputs.len(),
        });
    }

    // Encode the query ID first.
    out.extend_from_slice(&view_function_api_info.query_id);

    // Then encode each input next.
    let mut inputs = inputs.into_encodable_values();
    for input in &*view_function_api_info.inputs {
        inputs
            .encode_next_value_to(input.id.clone(), type_resolver, out)
            .map_err(ViewFunctionInputsEncodeError::EncodeError)?;
    }

    Ok(())
}
