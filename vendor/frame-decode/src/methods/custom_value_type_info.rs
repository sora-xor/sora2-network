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

use alloc::borrow::Cow;
use alloc::string::String;

/// This can be implemented for anything capable of providing Custom Value information.
pub trait CustomValueTypeInfo {
    /// The type of type IDs that we are using to obtain type information.
    type TypeId: Clone;
    /// Get information about a constant
    fn custom_value_info(
        &self,
        name: &str,
    ) -> Result<CustomValueInfo<'_, Self::TypeId>, CustomValueInfoError>;
}

/// This can be implemented for anything capable of providing information about the available Custom Values
pub trait CustomValueEntryInfo {
    /// Iterate over all of the available Custom Values, returning [`CustomValue`] as we go.
    fn custom_values(&self) -> impl Iterator<Item = CustomValue<'_>>;
}

/// An error returned trying to access Custom Value information.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("Custom Value `{not_found}` not found.")]
pub struct CustomValueInfoError {
    /// The custom value that was not found:
    pub not_found: String,
}

/// Information about a Custom Value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomValueInfo<'info, TypeId: Clone> {
    /// The bytes representing this custom value.
    ///
    /// It is expected that these bytes are in the metadata
    /// and can be borrowed here.
    pub bytes: &'info [u8],
    /// The type of this custom value. Because custom values
    /// can be arbitrarily inserted, this has a higher likelihood
    /// of being invalid than many type IDs.
    pub type_id: TypeId,
}

/// The identifier for a single Custom Value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomValue<'info> {
    /// The name of this Custom Value.
    pub name: Cow<'info, str>,
}

macro_rules! impl_custom_value_type_info_for_v15_to_v16 {
    ($path:path, $name:ident) => {
        const _: () = {
            use $path as path;
            impl CustomValueTypeInfo for path::$name {
                type TypeId = u32;
                fn custom_value_info(
                    &self,
                    name: &str,
                ) -> Result<CustomValueInfo<'_, Self::TypeId>, CustomValueInfoError> {
                    let custom_value =
                        self.custom
                            .map
                            .get(name)
                            .ok_or_else(move || CustomValueInfoError {
                                not_found: name.into(),
                            })?;

                    Ok(CustomValueInfo {
                        bytes: &custom_value.value,
                        type_id: custom_value.ty.id,
                    })
                }
            }
            impl CustomValueEntryInfo for path::$name {
                fn custom_values(&self) -> impl Iterator<Item = CustomValue<'_>> {
                    self.custom.map.iter().map(|(name, _)| CustomValue {
                        name: Cow::Borrowed(name),
                    })
                }
            }
        };
    };
}

impl_custom_value_type_info_for_v15_to_v16!(frame_metadata::v15, RuntimeMetadataV15);
impl_custom_value_type_info_for_v15_to_v16!(frame_metadata::v16, RuntimeMetadataV16);
