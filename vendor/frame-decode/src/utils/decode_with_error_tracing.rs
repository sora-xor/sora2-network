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

use alloc::format;
use alloc::string::String;

/// Decode some bytes given a type ID and type resolver, and a visitor which decides the output value.
///
/// If the decoding fails and the `error-tracing` feature is enabled, we try to decode again using
/// a tracing visitor in order to return a more detailed error message.
pub fn decode_with_error_tracing<'scale, 'resolver, Resolver, Id, V>(
    cursor: &mut &'scale [u8],
    type_id: Id,
    types: &'resolver Resolver,
    visitor: V,
) -> Result<V::Value<'scale, 'resolver>, DecodeErrorTrace>
where
    Resolver: scale_type_resolver::TypeResolver<TypeId = Id>,
    Id: core::fmt::Debug + Clone,
    V: scale_decode::Visitor<TypeResolver = Resolver>,
    V::Error: core::fmt::Debug,
{
    let initial = *cursor;
    match scale_decode::visitor::decode_with_visitor(cursor, type_id.clone(), types, visitor) {
        Ok(value) => Ok(value),
        // Don't use scale-value; return the error as-is.
        #[cfg(not(feature = "error-tracing"))]
        Err(e) => {
            *cursor = initial;

            Err(DecodeErrorTrace {
                original_error: format!("{e:?}"),
                tracing_error: String::new(),
            })
        }
        // Use scale-value tracing visitor to return a better error
        #[cfg(feature = "error-tracing")]
        Err(e) => {
            // Reset cursor incase it's been consumed by the above call, and decode using the
            // tracing visitor to hopefully return a better error.
            *cursor = initial;
            let res = scale_value::scale::tracing::decode_as_type(cursor, type_id.clone(), types)
                .map(|v| v.map_context(|id| format!("{id:?}")))
                .map_err(|te| DecodeErrorTrace {
                    original_error: format!("{e:?}"),
                    tracing_error: alloc::string::ToString::to_string(&te),
                })?;

            // If the above succeeds (we're expecting it to fail), then print the value out here.
            use core::fmt::Write;
            let mut res_string = String::new();
            write!(
                &mut res_string,
                "Failed to decode value with custom visitor (but tracing decoded it):\n\n"
            )
            .unwrap();

            scale_value::stringify::to_writer_custom()
                .pretty()
                .format_context(|type_id, w: &mut &mut String| write!(w, "{type_id}"))
                .add_custom_formatter(|v, w| {
                    scale_value::stringify::custom_formatters::format_hex(v, w)
                })
                .add_custom_formatter(|v, w| {
                    // don't space unnamed composites over multiple lines if lots of primitive values.
                    if let scale_value::ValueDef::Composite(scale_value::Composite::Unnamed(vals)) =
                        &v.value
                    {
                        let are_primitive = vals
                            .iter()
                            .all(|val| matches!(val.value, scale_value::ValueDef::Primitive(_)));
                        if are_primitive {
                            return Some(write!(w, "{v}"));
                        }
                    }
                    None
                })
                .write(&res, &mut res_string)
                .expect("writing to string should always succeed");

            Err(DecodeErrorTrace {
                original_error: format!("{e:?}"),
                tracing_error: res_string,
            })
        }
    }
}

/// A tracing decode error.
#[derive(Clone, Debug, thiserror::Error)]
pub struct DecodeErrorTrace {
    original_error: String,
    tracing_error: String,
}

impl core::fmt::Display for DecodeErrorTrace {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let DecodeErrorTrace {
            original_error,
            tracing_error,
        } = self;

        write!(f, "{original_error}")?;
        if !tracing_error.is_empty() {
            write!(f, ":\n\n{tracing_error}")?;
        }
        Ok(())
    }
}
