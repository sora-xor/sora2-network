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
use crate::utils::{EncodableValues, IntoEncodableValues};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use scale_type_resolver::TypeResolver;

/// An error returned trying to encode Runtime API inputs.
#[non_exhaustive]
#[allow(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum RuntimeApiInputsEncodeError {
    #[error("Cannot get Runtime API info: {0}")]
    CannotGetInfo(RuntimeApiInfoError<'static>),
    #[error("Failed to encode Runtime API info: {0}")]
    EncodeError(#[from] scale_encode::Error),
    #[error("Wrong number of inputs provided: expected {num_inputs_expected}")]
    WrongNumberOfInputsProvided {
        /// The number of input parameters that were expected.
        num_inputs_expected: usize,
    },
}

/// Encode the name/ID of a Runtime API used in RPC methods given the trait name and method name.
pub fn encode_runtime_api_name(trait_name: &str, method_name: &str) -> String {
    format!("{trait_name}_{method_name}")
}

/// Encode the inputs to a Runtime API.
pub fn encode_runtime_api_inputs<Info, Resolver, Inputs>(
    trait_name: &str,
    method_name: &str,
    keys: Inputs,
    info: &Info,
    type_resolver: &Resolver,
) -> Result<Vec<u8>, RuntimeApiInputsEncodeError>
where
    Inputs: IntoEncodableValues,
    Info: RuntimeApiTypeInfo,
    Info::TypeId: Clone + core::fmt::Debug,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
{
    let mut out = Vec::new();
    encode_runtime_api_inputs_to(trait_name, method_name, keys, info, type_resolver, &mut out)?;
    Ok(out)
}

/// Encode the inputs to a Runtime API to a provided output `Vec`.
pub fn encode_runtime_api_inputs_to<Info, Resolver, Inputs>(
    trait_name: &str,
    method_name: &str,
    keys: Inputs,
    info: &Info,
    type_resolver: &Resolver,
    out: &mut Vec<u8>,
) -> Result<(), RuntimeApiInputsEncodeError>
where
    Inputs: IntoEncodableValues,
    Info: RuntimeApiTypeInfo,
    Info::TypeId: Clone + core::fmt::Debug,
    Resolver: TypeResolver<TypeId = Info::TypeId>,
{
    let runtime_api_info = info
        .runtime_api_info(trait_name, method_name)
        .map_err(|e| RuntimeApiInputsEncodeError::CannotGetInfo(e.into_owned()))?;

    encode_runtime_api_inputs_with_info_to(keys, &runtime_api_info, type_resolver, out)
}

/// Encode the inputs to a Runtime API to a provided output `Vec`.
///
/// Unlike [`encode_runtime_api_inputs_to`], which obtains the Runtime API info internally given trait and method names,
/// this function takes the Runtime API info as an argument. This is useful if you already have the info available,
/// for example if you are encoding multiple inputs for a given Runtime API.
pub fn encode_runtime_api_inputs_with_info_to<Resolver, Inputs>(
    inputs: Inputs,
    runtime_api_info: &RuntimeApiInfo<<Resolver as TypeResolver>::TypeId>,
    type_resolver: &Resolver,
    out: &mut Vec<u8>,
) -> Result<(), RuntimeApiInputsEncodeError>
where
    Inputs: IntoEncodableValues,
    Resolver: TypeResolver,
    <Resolver as TypeResolver>::TypeId: Clone + core::fmt::Debug,
{
    // If wrong number of inputs provided, bail early.
    if runtime_api_info.inputs.len() != inputs.num_encodable_values() {
        return Err(RuntimeApiInputsEncodeError::WrongNumberOfInputsProvided {
            num_inputs_expected: runtime_api_info.inputs.len(),
        });
    }

    // Encode the inputs to our out bytes.
    let mut inputs = inputs.into_encodable_values();
    for input in &*runtime_api_info.inputs {
        inputs
            .encode_next_value_to(input.id.clone(), type_resolver, out)
            .map_err(RuntimeApiInputsEncodeError::EncodeError)?;
    }

    Ok(())
}
