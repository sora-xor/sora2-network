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

/// [`frame_metadata::RuntimeMetadata`] contains information about runtime calls and events. This
/// function adds this information into a [`scale_info_legacy::TypeRegistry`] which can then be referenced
/// by other types. The main types you'll be able to reference from this set are:
///
/// - `builtin::Call` - A variant which describes the shape of a `RuntimeCall`, ie any call.
/// - `builtin::Event` - A variant which describes the shape of a `RuntimeEvent`, ie any event.
///
/// These are composed from:
///
/// - `builtin::module::event::$PALLET` - A variant containing the events in a specific pallet.
/// - `builtin::module::call::$PALLET` - A variant containing the calls in a specific pallet.
pub fn type_registry_from_metadata<Md: ToTypeRegistry>(
    metadata: &Md,
) -> Result<scale_info_legacy::TypeRegistry, scale_info_legacy::lookup_name::ParseError> {
    metadata.to_type_registry()
}

/// This is like [`type_registry_from_metadata`], except it can be handed the outer [`frame_metadata::RuntimeMetadata`]
/// enum and will extract types from it where appropriate (handing back no types for deprecated or modern metadata).
pub fn type_registry_from_metadata_any(
    metadata: &frame_metadata::RuntimeMetadata,
) -> Result<scale_info_legacy::TypeRegistry, scale_info_legacy::lookup_name::ParseError> {
    use frame_metadata::RuntimeMetadata;
    match metadata {
        RuntimeMetadata::V0(_d) => Ok(scale_info_legacy::TypeRegistry::empty()),
        RuntimeMetadata::V1(_d) => Ok(scale_info_legacy::TypeRegistry::empty()),
        RuntimeMetadata::V2(_d) => Ok(scale_info_legacy::TypeRegistry::empty()),
        RuntimeMetadata::V3(_d) => Ok(scale_info_legacy::TypeRegistry::empty()),
        RuntimeMetadata::V4(_d) => Ok(scale_info_legacy::TypeRegistry::empty()),
        RuntimeMetadata::V5(_d) => Ok(scale_info_legacy::TypeRegistry::empty()),
        RuntimeMetadata::V6(_d) => Ok(scale_info_legacy::TypeRegistry::empty()),
        RuntimeMetadata::V7(_d) => Ok(scale_info_legacy::TypeRegistry::empty()),
        RuntimeMetadata::V8(m) => m.to_type_registry(),
        RuntimeMetadata::V9(m) => m.to_type_registry(),
        RuntimeMetadata::V10(m) => m.to_type_registry(),
        RuntimeMetadata::V11(m) => m.to_type_registry(),
        RuntimeMetadata::V12(m) => m.to_type_registry(),
        RuntimeMetadata::V13(m) => m.to_type_registry(),
        RuntimeMetadata::V14(_m) => Ok(scale_info_legacy::TypeRegistry::empty()),
        RuntimeMetadata::V15(_m) => Ok(scale_info_legacy::TypeRegistry::empty()),
        RuntimeMetadata::V16(_m) => Ok(scale_info_legacy::TypeRegistry::empty()),
    }
}

/// This is used with the [`type_registry_from_metadata`] helper function to extract types from the
/// metadata. It is not intended to be implemented on anything else.
pub trait ToTypeRegistry: sealed::Sealed {
    /// Return a type registry.
    fn to_type_registry(
        &self,
    ) -> Result<scale_info_legacy::TypeRegistry, scale_info_legacy::lookup_name::ParseError>;
}

mod sealed {
    pub trait Sealed {}
}

const _: () = {
    use super::as_decoded;
    use alloc::borrow::ToOwned;
    use alloc::format;
    use alloc::vec;
    use alloc::vec::Vec;
    use scale_info_legacy::InsertName;
    use scale_info_legacy::type_shape::{Field, TypeShape, Variant, VariantDesc};
    use scale_info_legacy::{LookupName, TypeRegistry};

    macro_rules! impl_for_v8_to_v13 {
        ($path:path $(, $builtin_index:ident)?) => {
            impl sealed::Sealed for $path {}
            impl ToTypeRegistry for $path {
                fn to_type_registry(&self) -> Result<scale_info_legacy::TypeRegistry, scale_info_legacy::lookup_name::ParseError> {
                    let metadata = self;
                    let mut new_types = TypeRegistry::empty();
                    let modules = as_decoded(&metadata.modules);

                    let mut call_module_variants: Vec<Variant> = vec![];
                    let mut event_module_variants: Vec<Variant> = vec![];
                    let mut error_module_variants: Vec<Variant> = vec![];

                    let mut calls_index = 0u8;
                    let mut events_index = 0u8;
                    let mut errors_index = 0u8;

                    for module in modules {
                        // In older metadatas, calls and event enums can have different indexes
                        // in a given pallet. Pallets without calls or events don't increment
                        // the respective index for them.
                        //
                        // We assume since errors are non optional, that the pallet index _always_
                        // increments for errors (no `None`s to skip).
                        let (calls_index, events_index, errors_index) = {
                            let out = (calls_index, events_index, errors_index);
                            if module.calls.is_some() {
                                calls_index += 1;
                            }
                            if module.event.is_some() {
                                events_index += 1;
                            }
                            errors_index += 1;

                            out
                        };

                        // For v12 and v13 metadata, there is a builtin index for everything in a pallet.
                        // If we pass an ident as second arg to this macro, we'll trigger
                        // using this builtin index instead.
                        $(
                            let $builtin_index = true;
                            let (calls_index, events_index, errors_index) = if $builtin_index {
                                (module.index, module.index, module.index)
                            } else {
                                (calls_index, events_index, errors_index)
                            };
                        )?

                        // as_ref to work when scale-info returns `&static str`
                        // instead of `String` in no-std mode.
                        let module_name: &str = as_decoded(&module.name).as_ref();

                        //// 1. Add calls to the type registry
                        if let Some(calls) = &module.calls.as_ref() {
                            let calls = as_decoded(calls);

                            // Iterate over each call in the module and turn into variants:
                            let mut call_variants: Vec<Variant> = vec![];
                            for (c_idx, call) in calls.iter().enumerate() {
                                let call_name: &str = as_decoded(&call.name).as_ref();
                                let args = as_decoded(&call.arguments)
                                    .iter()
                                    .map(|arg| {
                                        let name: &str = as_decoded(&arg.name).as_ref();
                                        Ok(Field {
                                            name: name.to_owned(),
                                            value: LookupName::parse(&as_decoded(&arg.ty))?.in_pallet(module_name),
                                        })
                                    })
                                    .collect::<Result<_,_>>()?;

                                call_variants.push(Variant {
                                    index: c_idx as u8,
                                    name: call_name.to_owned(),
                                    fields: VariantDesc::NamedStructOf(args)
                                });
                            }

                            // Store these call variants in the types:
                            let call_enum_name_str = format!("builtin::module::call::{module_name}");
                            let call_enum_insert_name = InsertName::parse(&call_enum_name_str).unwrap();
                            new_types.insert(call_enum_insert_name, TypeShape::EnumOf(call_variants));

                            // Reference it in the modules enum we're building:
                            let call_enum_lookup_name = LookupName::parse(&call_enum_name_str).unwrap();
                            call_module_variants.push(Variant {
                                index: calls_index,
                                name: module_name.to_owned(),
                                fields: VariantDesc::UnnamedStructOf(vec![call_enum_lookup_name])
                            });
                        }

                        //// 2. Add events to the type registry
                        if let Some(events) = &module.event.as_ref() {
                            let events = as_decoded(events);

                            let mut event_variants: Vec<Variant> = vec![];
                            for (e_idx, event)in events.iter().enumerate() {
                                let event_name: &str = as_decoded(&event.name).as_ref();
                                let args = as_decoded(&event.arguments)
                                    .iter()
                                    .map(|arg| {
                                        Ok(LookupName::parse(&arg)?.in_pallet(module_name))
                                    })
                                    .collect::<Result<_,_>>()?;

                                event_variants.push(Variant {
                                    index: e_idx as u8,
                                    name: event_name.to_owned(),
                                    fields: VariantDesc::UnnamedStructOf(args)
                                });
                            }

                            // Store event variants in the types:
                            let event_enum_name_str = format!("builtin::module::event::{module_name}");
                            let event_enum_insert_name = InsertName::parse(&event_enum_name_str).unwrap();
                            new_types.insert(event_enum_insert_name, TypeShape::EnumOf(event_variants));

                            // Reference it in the modules enum we're building:
                            let event_enum_lookup_name = LookupName::parse(&event_enum_name_str).unwrap();
                            event_module_variants.push(Variant {
                                index: events_index,
                                name: module_name.to_owned(),
                                fields: VariantDesc::UnnamedStructOf(vec![event_enum_lookup_name])
                            });
                        }

                        //// 3. Add errors to the type registry. Each error is historically a variant without any data
                        //// (this changed sometime after V14 metadata). We assume that variant indexes start from 0 and
                        //// increment. I'm not sure how to test this at the time of writing.
                        {
                            let error_variants = as_decoded(&module.errors).iter().enumerate().map(|(e_idx, error)| {
                                let event_name: &str = as_decoded(&error.name).as_ref();
                                Variant {
                                    index: e_idx as u8,
                                    name: event_name.to_owned(),
                                    fields: VariantDesc::UnnamedStructOf(Vec::new())
                                }
                            }).collect();

                            // Store error variants in the types:
                            let error_enum_name_str = format!("builtin::module::error::{module_name}");
                            let error_enum_insert_name = InsertName::parse(&error_enum_name_str).unwrap();
                            new_types.insert(error_enum_insert_name, TypeShape::EnumOf(error_variants));

                            // Reference it in the modules enum we're building:
                            let error_enum_lookup_name = LookupName::parse(&error_enum_name_str).unwrap();
                            error_module_variants.push(Variant {
                                index: errors_index,
                                name: module_name.to_owned(),
                                fields: VariantDesc::UnnamedStructOf(vec![error_enum_lookup_name])
                            });
                        }
                    }

                    // Store the module call variants in the types:
                    let calls_enum_name_str = "builtin::Call";
                    let calls_enum_insert_name = InsertName::parse(&calls_enum_name_str).unwrap();
                    new_types.insert(calls_enum_insert_name, TypeShape::EnumOf(call_module_variants));

                    // Store the module event variants in the types:
                    let events_enum_name_str = "builtin::Event";
                    let events_enum_insert_name = InsertName::parse(&events_enum_name_str).unwrap();
                    new_types.insert(events_enum_insert_name, TypeShape::EnumOf(event_module_variants));

                    // Store the module error variants in the types:
                    let errors_enum_name_str = "builtin::Error";
                    let errors_enum_insert_name = InsertName::parse(&errors_enum_name_str).unwrap();
                    new_types.insert(errors_enum_insert_name, TypeShape::EnumOf(error_module_variants));

                    Ok(new_types)
                }
            }
        }
    }

    impl_for_v8_to_v13!(frame_metadata::v8::RuntimeMetadataV8);
    impl_for_v8_to_v13!(frame_metadata::v9::RuntimeMetadataV9);
    impl_for_v8_to_v13!(frame_metadata::v10::RuntimeMetadataV10);
    impl_for_v8_to_v13!(frame_metadata::v11::RuntimeMetadataV11);
    impl_for_v8_to_v13!(frame_metadata::v12::RuntimeMetadataV12, use_builtin_index);
    impl_for_v8_to_v13!(frame_metadata::v13::RuntimeMetadataV13, use_builtin_index);
};
