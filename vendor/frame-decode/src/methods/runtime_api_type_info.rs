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

use super::Entry;
use crate::utils::Either;
use alloc::borrow::Cow;
use alloc::borrow::ToOwned;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// This can be implemented for anything capable of providing Runtime API type information.
/// It is implemented for newer versions of frame-metadata (V15 and above).
pub trait RuntimeApiTypeInfo {
    /// The type of type IDs that we are using to obtain type information.
    type TypeId: Clone;
    /// Get the information needed to encode/decode a specific Runtime API call
    fn runtime_api_info(
        &self,
        trait_name: &str,
        method_name: &str,
    ) -> Result<RuntimeApiInfo<'_, Self::TypeId>, RuntimeApiInfoError<'_>>;
}

/// This can be implemented for anything capable of providing information about the available Runtime Apis
pub trait RuntimeApiEntryInfo {
    /// Iterate over all of the available Runtime Apis, returning [`Entry`] as we go.
    fn runtime_api_entries(&self) -> impl Iterator<Item = RuntimeApiEntry<'_>>;
    /// Iterate over all of the available Runtime Apis, returning a pair of `(trait_name, method_name)` as we go.
    fn runtime_api_tuples(&self) -> impl Iterator<Item = (Cow<'_, str>, Cow<'_, str>)> {
        Entry::tuples_of(self.runtime_api_entries())
    }
    /// Iterate over all of the available Runtime Apis in a given trait.
    fn runtime_apis_in_trait(&self, pallet: &str) -> impl Iterator<Item = Cow<'_, str>> {
        Entry::entries_in(self.runtime_api_entries(), pallet)
    }
}

/// An entry denoting a Runtime API trait or method.
pub type RuntimeApiEntry<'a> = Entry<Cow<'a, str>, Cow<'a, str>>;

/// An error returned trying to access Runtime API type information.
#[non_exhaustive]
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeApiInfoError<'info> {
    #[error("Runtime API trait `{trait_name}` not found")]
    TraitNotFound { trait_name: String },
    #[error("Runtime API method `{method_name}` not found in trait `{trait_name}`")]
    MethodNotFound {
        trait_name: Cow<'info, str>,
        method_name: String,
    },
}

impl<'info> RuntimeApiInfoError<'info> {
    /// Take ownership of this error, turning any lifetimes to `'static`.
    pub fn into_owned(self) -> RuntimeApiInfoError<'static> {
        match self {
            RuntimeApiInfoError::TraitNotFound { trait_name } => {
                RuntimeApiInfoError::TraitNotFound { trait_name }
            }
            RuntimeApiInfoError::MethodNotFound {
                trait_name,
                method_name,
            } => RuntimeApiInfoError::MethodNotFound {
                trait_name: Cow::Owned(trait_name.into_owned()),
                method_name,
            },
        }
    }
}

/// Information about a Runtime API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeApiInfo<'info, TypeId: Clone> {
    /// Inputs to the runtime API.
    pub inputs: Cow<'info, [RuntimeApiInput<'info, TypeId>]>,
    /// The output type returned from the runtime API.
    pub output_id: TypeId,
}

impl<'info, TypeId: Clone + 'static> RuntimeApiInfo<'info, TypeId> {
    /// Take ownership of this info, turning any lifetimes to `'static`.
    pub fn into_owned(self) -> RuntimeApiInfo<'static, TypeId> {
        let inputs = self
            .inputs
            .iter()
            .map(|input| input.clone().into_owned())
            .collect();

        RuntimeApiInfo {
            inputs: Cow::Owned(inputs),
            output_id: self.output_id,
        }
    }

    /// Map the type IDs in this [`RuntimeApiInfo`], returning a new one or bailing early with an error if something goes wrong.
    /// This also takes ownership of the [`RuntimeApiInfo`], turning the lifetime to static.
    pub fn map_ids<NewTypeId: Clone, E, F: FnMut(TypeId) -> Result<NewTypeId, E>>(
        self,
        mut f: F,
    ) -> Result<RuntimeApiInfo<'static, NewTypeId>, E> {
        let new_output_id = f(self.output_id)?;
        let mut new_inputs = Vec::with_capacity(self.inputs.len());

        match self.inputs {
            Cow::Borrowed(inputs) => {
                for input in inputs {
                    new_inputs.push(RuntimeApiInput {
                        // We always have to allocate if inputs is borrowed:
                        name: Cow::Owned(input.name.to_string()),
                        id: f(input.id.clone())?,
                    });
                }
            }
            Cow::Owned(inputs) => {
                for input in inputs {
                    new_inputs.push(RuntimeApiInput {
                        // If inputs is owned, we only allocate if name is borrowed:
                        name: Cow::Owned(input.name.into_owned()),
                        id: f(input.id.clone())?,
                    });
                }
            }
        }

        Ok(RuntimeApiInfo {
            inputs: Cow::Owned(new_inputs),
            output_id: new_output_id,
        })
    }
}

/// Information about a specific input value to a Runtime API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeApiInput<'info, TypeId> {
    /// Name of the input.
    pub name: Cow<'info, str>,
    /// Type of the input.
    pub id: TypeId,
}

impl<'info, TypeId: Clone + 'static> RuntimeApiInput<'info, TypeId> {
    /// Take ownership of this info, turning any lifetimes to `'static`.
    fn into_owned(self) -> RuntimeApiInput<'static, TypeId> {
        RuntimeApiInput {
            name: Cow::Owned(self.name.into_owned()),
            id: self.id,
        }
    }
}

macro_rules! impl_runtime_api_info_for_v15_to_v16 {
    ($path:path, $name:ident) => {
        const _: () = {
            use $path as path;
            impl RuntimeApiTypeInfo for path::$name {
                type TypeId = u32;
                fn runtime_api_info(
                    &self,
                    trait_name: &str,
                    method_name: &str,
                ) -> Result<RuntimeApiInfo<'_, Self::TypeId>, RuntimeApiInfoError<'_>> {
                    let api = self.apis.iter().find(|api| api.name == trait_name).ok_or(
                        RuntimeApiInfoError::TraitNotFound {
                            trait_name: trait_name.to_owned(),
                        },
                    )?;

                    let method = api
                        .methods
                        .iter()
                        .find(|method| method.name == method_name)
                        .ok_or(RuntimeApiInfoError::MethodNotFound {
                            trait_name: Cow::Borrowed(&api.name),
                            method_name: method_name.to_owned(),
                        })?;

                    let inputs = method
                        .inputs
                        .iter()
                        .map(|arg| RuntimeApiInput {
                            name: Cow::Borrowed(&arg.name),
                            id: arg.ty.id,
                        })
                        .collect();

                    Ok(RuntimeApiInfo {
                        inputs,
                        output_id: method.output.id,
                    })
                }
            }
            impl RuntimeApiEntryInfo for path::$name {
                fn runtime_api_entries(&self) -> impl Iterator<Item = RuntimeApiEntry<'_>> {
                    self.apis.iter().flat_map(|api| {
                        core::iter::once(Entry::In(Cow::Borrowed(&*api.name))).chain(
                            api.methods
                                .iter()
                                .map(|m| Entry::Name(Cow::Borrowed(&*m.name))),
                        )
                    })
                }
                fn runtime_apis_in_trait(
                    &self,
                    trait_name: &str,
                ) -> impl Iterator<Item = Cow<'_, str>> {
                    let api = self.apis.iter().find(|api| api.name == trait_name);

                    let Some(api) = api else {
                        return Either::Left(core::iter::empty());
                    };

                    let method_names = api
                        .methods
                        .iter()
                        .map(|method| Cow::Borrowed(&*method.name));
                    Either::Right(method_names)
                }
            }
        };
    };
}

impl_runtime_api_info_for_v15_to_v16!(frame_metadata::v15, RuntimeMetadataV15);
impl_runtime_api_info_for_v15_to_v16!(frame_metadata::v16, RuntimeMetadataV16);

#[cfg(feature = "legacy")]
mod legacy {
    use super::*;
    use scale_info_legacy::type_registry::RuntimeApiName;
    use scale_info_legacy::{TypeRegistry, TypeRegistrySet, lookup_name};

    impl RuntimeApiTypeInfo for TypeRegistry {
        type TypeId = lookup_name::LookupName;

        fn runtime_api_info(
            &self,
            trait_name: &str,
            method_name: &str,
        ) -> Result<RuntimeApiInfo<'_, Self::TypeId>, RuntimeApiInfoError<'_>> {
            let api = self.runtime_api(trait_name, method_name).ok_or_else(|| {
                RuntimeApiInfoError::MethodNotFound {
                    trait_name: Cow::Owned(trait_name.to_owned()),
                    method_name: method_name.to_owned(),
                }
            })?;

            let inputs = api
                .inputs
                .iter()
                .map(|input| RuntimeApiInput {
                    name: Cow::Borrowed(&*input.name),
                    id: input.id.clone(),
                })
                .collect();

            Ok(RuntimeApiInfo {
                inputs,
                output_id: api.output.clone(),
            })
        }
    }
    impl RuntimeApiEntryInfo for TypeRegistry {
        fn runtime_api_entries(&self) -> impl Iterator<Item = RuntimeApiEntry<'_>> {
            self.runtime_apis().map(|api| match api {
                RuntimeApiName::Trait(name) => Entry::In(Cow::Borrowed(name)),
                RuntimeApiName::Method(name) => Entry::Name(Cow::Borrowed(name)),
            })
        }
        fn runtime_apis_in_trait(&self, trait_name: &str) -> impl Iterator<Item = Cow<'_, str>> {
            TypeRegistry::runtime_apis_in_trait(self, trait_name).map(Cow::Borrowed)
        }
    }

    impl<'a> RuntimeApiTypeInfo for TypeRegistrySet<'a> {
        type TypeId = lookup_name::LookupName;

        fn runtime_api_info(
            &self,
            trait_name: &str,
            method_name: &str,
        ) -> Result<RuntimeApiInfo<'_, Self::TypeId>, RuntimeApiInfoError<'_>> {
            let api = self.runtime_api(trait_name, method_name).ok_or_else(|| {
                RuntimeApiInfoError::MethodNotFound {
                    trait_name: Cow::Owned(trait_name.to_owned()),
                    method_name: method_name.to_owned(),
                }
            })?;

            let inputs = api
                .inputs
                .iter()
                .map(|input| RuntimeApiInput {
                    name: Cow::Borrowed(&*input.name),
                    id: input.id.clone(),
                })
                .collect();

            Ok(RuntimeApiInfo {
                inputs,
                output_id: api.output.clone(),
            })
        }
    }
    impl<'a> RuntimeApiEntryInfo for TypeRegistrySet<'a> {
        fn runtime_api_entries(&self) -> impl Iterator<Item = RuntimeApiEntry<'_>> {
            self.runtime_apis().map(|api| match api {
                RuntimeApiName::Trait(name) => Entry::In(Cow::Borrowed(name)),
                RuntimeApiName::Method(name) => Entry::Name(Cow::Borrowed(name)),
            })
        }

        fn runtime_apis_in_trait(&self, trait_name: &str) -> impl Iterator<Item = Cow<'_, str>> {
            TypeRegistrySet::runtime_apis_in_trait(self, trait_name).map(Cow::Borrowed)
        }
    }
}
