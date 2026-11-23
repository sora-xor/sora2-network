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
use alloc::vec::Vec;
use core::write;

/// Implementations of this are responsible for handing back the information we need to
/// encode and decode extrinsics. This is expected to be implemented for runtime metadata
/// types or derivatives thereof where needed.
pub trait ExtrinsicTypeInfo {
    /// The type of type IDs that we are using to obtain type information.
    type TypeId;

    /// Get the information about the call data of a given extrinsic, given u8 indexes.
    fn extrinsic_call_info_by_index(
        &self,
        pallet_index: u8,
        call_index: u8,
    ) -> Result<ExtrinsicCallInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>>;

    /// Get the information about the call data of a given extrinsic, given pallet and call name.
    fn extrinsic_call_info_by_name(
        &self,
        pallet_name: &str,
        call_name: &str,
    ) -> Result<ExtrinsicCallInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>>;

    /// Get the information needed to decode or encode the extrinsic signature bytes.
    fn extrinsic_signature_info(
        &self,
    ) -> Result<ExtrinsicSignatureInfo<Self::TypeId>, ExtrinsicInfoError<'_>>;

    /// Get the information needed to decode the transaction extensions.
    fn extrinsic_extension_info(
        &self,
        extension_version: Option<u8>,
    ) -> Result<ExtrinsicExtensionInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>>;

    /// Get the available transaction extension versions. Prior to runtimes supporting
    /// V5 extrinsics this should not return any entries. In runtimes supporting V5 extrinsics,
    /// 1 or more versions should be returned.
    ///
    /// Versions should be returned in order from highest to lowest preference of
    /// whether they should be used for encoding extrinsics. The highest will be tried first,
    /// but if the relevant transaction extension information is not given then we will fall
    /// back to the next version until we find one we can satisfy.
    ///
    /// All versions returned here should be passable to [`ExtrinsicTypeInfo::extrinsic_extension_info`]
    /// and return valid extensions information from that.
    fn extrinsic_extension_version_info(
        &self,
    ) -> Result<impl Iterator<Item = u8>, ExtrinsicInfoError<'_>>;
}

/// An error returned trying to access extrinsic type information.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
#[allow(missing_docs)]
pub enum ExtrinsicInfoError<'a> {
    PalletNotFound {
        index: u8,
    },
    PalletNotFoundByName {
        name: Cow<'a, str>,
    },
    CallNotFound {
        index: u8,
        pallet_index: u8,
        pallet_name: Cow<'a, str>,
    },
    CallNotFoundByName {
        call_name: Cow<'a, str>,
        pallet_index: u8,
        pallet_name: Cow<'a, str>,
    },
    #[cfg(feature = "legacy")]
    CannotParseTypeName {
        name: Cow<'a, str>,
        reason: scale_info_legacy::lookup_name::ParseError,
    },
    CallsTypeNotFound {
        id: u32,
        pallet_index: u8,
        pallet_name: Cow<'a, str>,
    },
    CallsTypeShouldBeVariant {
        id: u32,
        pallet_index: u8,
        pallet_name: Cow<'a, str>,
    },
    ExtrinsicTypeNotFound {
        id: u32,
    },
    ExtrinsicAddressTypeNotFound,
    ExtrinsicSignatureTypeNotFound,
    ExtrinsicExtensionVersionNotSupported {
        extension_version: u8,
    },
    ExtrinsicExtensionVersionNotFound {
        extension_version: u8,
    },
}

impl core::error::Error for ExtrinsicInfoError<'_> {}

impl core::fmt::Display for ExtrinsicInfoError<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ExtrinsicInfoError::PalletNotFound { index } => {
                write!(f, "Pallet with index {index} not found")
            }
            ExtrinsicInfoError::PalletNotFoundByName { name } => {
                write!(f, "Pallet with name '{name}' not found")
            }
            ExtrinsicInfoError::CallNotFound {
                index,
                pallet_index,
                pallet_name,
            } => {
                write!(
                    f,
                    "Call with index {index} not found in pallet '{pallet_name}' (pallet index {pallet_index})."
                )
            }
            ExtrinsicInfoError::CallNotFoundByName {
                call_name,
                pallet_index,
                pallet_name,
            } => {
                write!(
                    f,
                    "Call with name '{call_name}' not found in pallet '{pallet_name}' (pallet index {pallet_index})."
                )
            }
            #[cfg(feature = "legacy")]
            ExtrinsicInfoError::CannotParseTypeName { name, reason } => {
                write!(f, "Cannot parse type name '{name}':\n\n{reason}")
            }
            ExtrinsicInfoError::CallsTypeNotFound {
                id,
                pallet_index,
                pallet_name,
            } => {
                write!(
                    f,
                    "Cannot find calls type with id {id} in pallet '{pallet_name}' (pallet index {pallet_index})."
                )
            }
            ExtrinsicInfoError::CallsTypeShouldBeVariant {
                id,
                pallet_index,
                pallet_name,
            } => {
                write!(
                    f,
                    "Calls type with id {id} should be a variant in pallet '{pallet_name}' (pallet index {pallet_index})."
                )
            }
            ExtrinsicInfoError::ExtrinsicTypeNotFound { id } => {
                write!(f, "Could not find the extrinsic type with id {id}.")
            }
            ExtrinsicInfoError::ExtrinsicAddressTypeNotFound => {
                write!(f, "Could not find the extrinsic address type.")
            }
            ExtrinsicInfoError::ExtrinsicSignatureTypeNotFound => {
                write!(f, "Could not find the extrinsic signature type.")
            }
            ExtrinsicInfoError::ExtrinsicExtensionVersionNotSupported { extension_version } => {
                // Dev note: If we see a V5 General extrinsic, it will contain a byte for the version of the transaction extensions.
                // In V15 or below metadata, we don't know which version of the transaction extensions we're being told about. Thus,
                // We can't be sure that we can decode a given extrinsic with V15 or below metadata.
                write!(
                    f,
                    "The extrinsic contains an extension version (here, version {extension_version}), but in metadata <=V15 it's not obvious how to decode this."
                )
            }
            ExtrinsicInfoError::ExtrinsicExtensionVersionNotFound { extension_version } => {
                write!(
                    f,
                    "Could not find information about extensions with version {extension_version} in the metadata. Note: Metadata <=V15 only supports version 0."
                )
            }
        }
    }
}

impl ExtrinsicInfoError<'_> {
    /// Take ownership of this error.
    pub fn into_owned(self) -> ExtrinsicInfoError<'static> {
        match self {
            ExtrinsicInfoError::PalletNotFound { index } => {
                ExtrinsicInfoError::PalletNotFound { index }
            }
            ExtrinsicInfoError::PalletNotFoundByName { name } => {
                ExtrinsicInfoError::PalletNotFoundByName {
                    name: Cow::Owned(name.into_owned()),
                }
            }
            ExtrinsicInfoError::CallNotFound {
                index,
                pallet_index,
                pallet_name,
            } => ExtrinsicInfoError::CallNotFound {
                index,
                pallet_index,
                pallet_name: Cow::Owned(pallet_name.into_owned()),
            },
            ExtrinsicInfoError::CallNotFoundByName {
                call_name,
                pallet_index,
                pallet_name,
            } => ExtrinsicInfoError::CallNotFoundByName {
                call_name: Cow::Owned(call_name.into_owned()),
                pallet_index,
                pallet_name: Cow::Owned(pallet_name.into_owned()),
            },
            #[cfg(feature = "legacy")]
            ExtrinsicInfoError::CannotParseTypeName { name, reason } => {
                ExtrinsicInfoError::CannotParseTypeName {
                    name: Cow::Owned(name.into_owned()),
                    reason,
                }
            }
            ExtrinsicInfoError::CallsTypeNotFound {
                id,
                pallet_index,
                pallet_name,
            } => ExtrinsicInfoError::CallsTypeNotFound {
                id,
                pallet_index,
                pallet_name: Cow::Owned(pallet_name.into_owned()),
            },
            ExtrinsicInfoError::CallsTypeShouldBeVariant {
                id,
                pallet_index,
                pallet_name,
            } => ExtrinsicInfoError::CallsTypeShouldBeVariant {
                id,
                pallet_index,
                pallet_name: Cow::Owned(pallet_name.into_owned()),
            },
            ExtrinsicInfoError::ExtrinsicTypeNotFound { id } => {
                ExtrinsicInfoError::ExtrinsicTypeNotFound { id }
            }
            ExtrinsicInfoError::ExtrinsicAddressTypeNotFound => {
                ExtrinsicInfoError::ExtrinsicAddressTypeNotFound
            }
            ExtrinsicInfoError::ExtrinsicSignatureTypeNotFound => {
                ExtrinsicInfoError::ExtrinsicSignatureTypeNotFound
            }
            ExtrinsicInfoError::ExtrinsicExtensionVersionNotSupported { extension_version } => {
                ExtrinsicInfoError::ExtrinsicExtensionVersionNotSupported { extension_version }
            }
            ExtrinsicInfoError::ExtrinsicExtensionVersionNotFound { extension_version } => {
                ExtrinsicInfoError::ExtrinsicExtensionVersionNotFound { extension_version }
            }
        }
    }
}

/// An argument with a name and type ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtrinsicExtensionInfoArg<'info, TypeId> {
    /// Argument name.
    pub name: Cow<'info, str>,
    /// Argument type ID.
    pub id: TypeId,
    /// The type ID for implicit arguments.
    pub implicit_id: TypeId,
}

/// Extrinsic call data information given pallet and call names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtrinsicCallInfo<'info, TypeId> {
    /// Index of the pallet.
    pub pallet_index: u8,
    /// Index of the call.
    pub call_index: u8,
    /// Name of the pallet.
    pub pallet_name: Cow<'info, str>,
    /// Name of the call.
    pub call_name: Cow<'info, str>,
    /// Names and types of each of the extrinsic arguments.
    pub args: Vec<ExtrinsicCallInfoArg<'info, TypeId>>,
}

/// An argument in some extrinsic call data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtrinsicCallInfoArg<'info, TypeId> {
    /// Argument name.
    pub name: Cow<'info, str>,
    /// Argument type ID.
    pub id: TypeId,
}

/// Extrinsic signature information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtrinsicSignatureInfo<TypeId> {
    /// Type ID of the address.
    pub address_id: TypeId,
    /// Type ID of the signature.
    pub signature_id: TypeId,
}

/// Extrinsic extension information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtrinsicExtensionInfo<'info, TypeId> {
    /// Names and type IDs of the transaction extensions.
    pub extension_ids: Vec<ExtrinsicExtensionInfoArg<'info, TypeId>>,
}

macro_rules! impl_call_info_by_name_body_for_v14_to_v16 {
    ($self:ident, $pallet_name:ident, $call_name:ident) => {{
        use alloc::string::ToString;

        let pallet = $self
            .pallets
            .iter()
            .find(|p| p.name == $pallet_name)
            .ok_or_else(|| ExtrinsicInfoError::PalletNotFoundByName {
                name: Cow::Owned($pallet_name.to_string()),
            })?;

        let pallet_index = pallet.index;

        let calls_id = pallet
            .calls
            .as_ref()
            .ok_or_else(|| ExtrinsicInfoError::CallNotFoundByName {
                call_name: Cow::Owned($call_name.to_string()),
                pallet_index,
                pallet_name: Cow::Borrowed(&pallet.name),
            })?
            .ty
            .id;

        let calls_ty =
            $self
                .types
                .resolve(calls_id)
                .ok_or_else(|| ExtrinsicInfoError::CallsTypeNotFound {
                    id: calls_id,
                    pallet_index,
                    pallet_name: Cow::Borrowed(&pallet.name),
                })?;

        let calls_enum = match &calls_ty.type_def {
            scale_info::TypeDef::Variant(v) => v,
            _ => {
                return Err(ExtrinsicInfoError::CallsTypeShouldBeVariant {
                    id: calls_id,
                    pallet_index,
                    pallet_name: Cow::Borrowed(&pallet.name),
                });
            }
        };

        let call_variant = calls_enum
            .variants
            .iter()
            .find(|v| v.name == $call_name)
            .ok_or_else(|| ExtrinsicInfoError::CallNotFoundByName {
                call_name: Cow::Owned($call_name.to_string()),
                pallet_index,
                pallet_name: Cow::Borrowed(&pallet.name),
            })?;

        let args = call_variant
            .fields
            .iter()
            .map(|f| {
                let id = f.ty.id;
                let name = f
                    .name
                    .as_ref()
                    .map(|n| Cow::Borrowed(&**n))
                    .unwrap_or(Cow::Owned(String::new()));
                ExtrinsicCallInfoArg { id, name }
            })
            .collect();

        Ok(ExtrinsicCallInfo {
            pallet_index,
            call_index: call_variant.index,
            pallet_name: Cow::Borrowed(&pallet.name),
            call_name: Cow::Borrowed(&call_variant.name),
            args,
        })
    }};
}

macro_rules! impl_call_info_by_index_body_for_v14_to_v16 {
    ($self:ident, $pallet_index:ident, $call_index:ident) => {{
        let pallet = $self
            .pallets
            .iter()
            .find(|p| p.index == $pallet_index)
            .ok_or_else(|| ExtrinsicInfoError::PalletNotFound {
                index: $pallet_index,
            })?;

        let pallet_name = &pallet.name;

        let calls_id = pallet
            .calls
            .as_ref()
            .ok_or_else(|| ExtrinsicInfoError::CallNotFound {
                index: $call_index,
                pallet_index: $pallet_index,
                pallet_name: Cow::Borrowed(pallet_name),
            })?
            .ty
            .id;

        let calls_ty =
            $self
                .types
                .resolve(calls_id)
                .ok_or_else(|| ExtrinsicInfoError::CallsTypeNotFound {
                    id: calls_id,
                    pallet_index: $pallet_index,
                    pallet_name: Cow::Borrowed(pallet_name),
                })?;

        let calls_enum = match &calls_ty.type_def {
            scale_info::TypeDef::Variant(v) => v,
            _ => {
                return Err(ExtrinsicInfoError::CallsTypeShouldBeVariant {
                    id: calls_id,
                    pallet_index: $pallet_index,
                    pallet_name: Cow::Borrowed(pallet_name),
                });
            }
        };

        let call_variant = calls_enum
            .variants
            .iter()
            .find(|v| v.index == $call_index)
            .ok_or_else(|| ExtrinsicInfoError::CallNotFound {
                index: $call_index,
                pallet_index: $pallet_index,
                pallet_name: Cow::Borrowed(pallet_name),
            })?;

        let args = call_variant
            .fields
            .iter()
            .map(|f| {
                let id = f.ty.id;
                let name = f
                    .name
                    .as_ref()
                    .map(|n| Cow::Borrowed(&**n))
                    .unwrap_or(Cow::Owned(String::new()));
                ExtrinsicCallInfoArg { id, name }
            })
            .collect();

        Ok(ExtrinsicCallInfo {
            pallet_index: $pallet_index,
            call_index: $call_index,
            pallet_name: Cow::Borrowed(pallet_name),
            call_name: Cow::Borrowed(&call_variant.name),
            args,
        })
    }};
}

impl ExtrinsicTypeInfo for frame_metadata::v14::RuntimeMetadataV14 {
    type TypeId = u32;
    fn extrinsic_call_info_by_index(
        &self,
        pallet_index: u8,
        call_index: u8,
    ) -> Result<ExtrinsicCallInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>> {
        impl_call_info_by_index_body_for_v14_to_v16!(self, pallet_index, call_index)
    }
    fn extrinsic_call_info_by_name(
        &self,
        pallet_name: &str,
        call_name: &str,
    ) -> Result<ExtrinsicCallInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>> {
        impl_call_info_by_name_body_for_v14_to_v16!(self, pallet_name, call_name)
    }
    fn extrinsic_signature_info(
        &self,
    ) -> Result<ExtrinsicSignatureInfo<Self::TypeId>, ExtrinsicInfoError<'_>> {
        let extrinsic_parts = get_v14_extrinsic_parts(self)?;

        Ok(ExtrinsicSignatureInfo {
            address_id: extrinsic_parts.address,
            signature_id: extrinsic_parts.signature,
        })
    }
    fn extrinsic_extension_info(
        &self,
        extension_version: Option<u8>,
    ) -> Result<ExtrinsicExtensionInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>> {
        err_if_bad_extension_version(extension_version)?;

        let extension_ids = self
            .extrinsic
            .signed_extensions
            .iter()
            .map(|e| ExtrinsicExtensionInfoArg {
                id: e.ty.id,
                implicit_id: e.additional_signed.id,
                name: Cow::Borrowed(&e.identifier),
            })
            .collect();

        Ok(ExtrinsicExtensionInfo { extension_ids })
    }
    fn extrinsic_extension_version_info(
        &self,
    ) -> Result<impl Iterator<Item = u8>, ExtrinsicInfoError<'_>> {
        Ok(core::iter::empty())
    }
}

impl ExtrinsicTypeInfo for frame_metadata::v15::RuntimeMetadataV15 {
    type TypeId = u32;
    fn extrinsic_call_info_by_index(
        &self,
        pallet_index: u8,
        call_index: u8,
    ) -> Result<ExtrinsicCallInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>> {
        impl_call_info_by_index_body_for_v14_to_v16!(self, pallet_index, call_index)
    }
    fn extrinsic_call_info_by_name(
        &self,
        pallet_name: &str,
        call_name: &str,
    ) -> Result<ExtrinsicCallInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>> {
        impl_call_info_by_name_body_for_v14_to_v16!(self, pallet_name, call_name)
    }
    fn extrinsic_signature_info(
        &self,
    ) -> Result<ExtrinsicSignatureInfo<Self::TypeId>, ExtrinsicInfoError<'_>> {
        Ok(ExtrinsicSignatureInfo {
            address_id: self.extrinsic.address_ty.id,
            signature_id: self.extrinsic.signature_ty.id,
        })
    }
    fn extrinsic_extension_info(
        &self,
        extension_version: Option<u8>,
    ) -> Result<ExtrinsicExtensionInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>> {
        err_if_bad_extension_version(extension_version)?;

        let extension_ids = self
            .extrinsic
            .signed_extensions
            .iter()
            .map(|e| ExtrinsicExtensionInfoArg {
                id: e.ty.id,
                implicit_id: e.additional_signed.id,
                name: Cow::Borrowed(&e.identifier),
            })
            .collect();

        Ok(ExtrinsicExtensionInfo { extension_ids })
    }
    fn extrinsic_extension_version_info(
        &self,
    ) -> Result<impl Iterator<Item = u8>, ExtrinsicInfoError<'_>> {
        Ok(core::iter::empty())
    }
}

impl ExtrinsicTypeInfo for frame_metadata::v16::RuntimeMetadataV16 {
    type TypeId = u32;
    fn extrinsic_call_info_by_index(
        &self,
        pallet_index: u8,
        call_index: u8,
    ) -> Result<ExtrinsicCallInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>> {
        impl_call_info_by_index_body_for_v14_to_v16!(self, pallet_index, call_index)
    }
    fn extrinsic_call_info_by_name(
        &self,
        pallet_name: &str,
        call_name: &str,
    ) -> Result<ExtrinsicCallInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>> {
        impl_call_info_by_name_body_for_v14_to_v16!(self, pallet_name, call_name)
    }
    fn extrinsic_signature_info(
        &self,
    ) -> Result<ExtrinsicSignatureInfo<Self::TypeId>, ExtrinsicInfoError<'_>> {
        Ok(ExtrinsicSignatureInfo {
            address_id: self.extrinsic.address_ty.id,
            signature_id: self.extrinsic.signature_ty.id,
        })
    }
    fn extrinsic_extension_info(
        &self,
        extension_version: Option<u8>,
    ) -> Result<ExtrinsicExtensionInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>> {
        // If no extension version is provided (ie we are decoding a v4 transaction),
        // we always use version 0 of the transaction extensions. Else, we try to find
        // the version of transaction extensions that the transaction declared it's
        // using.
        let extension_version = extension_version.unwrap_or(0);

        let extension_indexes = self
            .extrinsic
            .transaction_extensions_by_version
            .get(&extension_version)
            .ok_or(ExtrinsicInfoError::ExtrinsicExtensionVersionNotSupported {
                extension_version,
            })?;

        let extension_ids = extension_indexes
            .iter()
            .map(|idx| {
                let ext = self
                    .extrinsic
                    .transaction_extensions
                    .get(idx.0 as usize)
                    .expect("Index in transaction_extensions_by_version should exist in transaction_extensions");

                ExtrinsicExtensionInfoArg {
                    id: ext.ty.id,
                    implicit_id: ext.implicit.id,
                    name: Cow::Borrowed(&ext.identifier),
                }
            })
            .collect();

        Ok(ExtrinsicExtensionInfo { extension_ids })
    }
    fn extrinsic_extension_version_info(
        &self,
    ) -> Result<impl Iterator<Item = u8>, ExtrinsicInfoError<'_>> {
        Ok(self
            .extrinsic
            .transaction_extensions_by_version
            .keys()
            .copied())
    }
}

fn get_v14_extrinsic_parts(
    metadata: &frame_metadata::v14::RuntimeMetadataV14,
) -> Result<ExtrinsicParts, ExtrinsicInfoError<'_>> {
    const ADDRESS: &str = "Address";
    const SIGNATURE: &str = "Signature";

    let extrinsic_id = metadata.extrinsic.ty.id;
    let extrinsic_ty = metadata
        .types
        .resolve(extrinsic_id)
        .ok_or(ExtrinsicInfoError::ExtrinsicTypeNotFound { id: extrinsic_id })?;

    let address_ty = extrinsic_ty
        .type_params
        .iter()
        .find_map(|param| {
            if param.name == ADDRESS {
                param.ty
            } else {
                None
            }
        })
        .ok_or(ExtrinsicInfoError::ExtrinsicAddressTypeNotFound)?;

    let signature_ty = extrinsic_ty
        .type_params
        .iter()
        .find_map(|param| {
            if param.name == SIGNATURE {
                param.ty
            } else {
                None
            }
        })
        .ok_or(ExtrinsicInfoError::ExtrinsicSignatureTypeNotFound)?;

    Ok(ExtrinsicParts {
        address: address_ty.id,
        signature: signature_ty.id,
    })
}

struct ExtrinsicParts {
    address: u32,
    signature: u32,
}

fn err_if_bad_extension_version<'a>(
    extension_version: Option<u8>,
) -> Result<(), ExtrinsicInfoError<'a>> {
    if let Some(extension_version) = extension_version {
        // Dev note: at the time of writing this comment, there is only one
        // possible extensions version (0), so any metadata is capable of decoding
        // transactions that we see with this version.
        //
        // As soon as there is more than one extension version in the wild, we should
        // change this to always fail if not >=V16 metadata, since at that point we'll
        // no longer know if the extension_version given lines up with the extensions
        // we're told about in the metadata or are for some older version of them.
        if extension_version != 0 {
            return Err(ExtrinsicInfoError::ExtrinsicExtensionVersionNotSupported {
                extension_version,
            });
        }
    }
    Ok(())
}

#[cfg(feature = "legacy")]
const _: () = {
    use crate::utils::as_decoded;
    use alloc::format;
    use scale_info_legacy::LookupName;

    macro_rules! impl_extrinsic_info_by_index_body_for_v8_to_v11 {
        ($self:ident, $pallet_index:ident, $call_index:ident) => {{
            let modules = as_decoded(&$self.modules);

            let m = modules
                .iter()
                .filter(|m| m.calls.is_some())
                .nth($pallet_index as usize)
                .ok_or_else(|| ExtrinsicInfoError::PalletNotFound {
                    index: $pallet_index,
                })?;

            // as_ref to work when scale-info returns `&static str`
            // instead of `String` in no-std mode.
            let m_name = as_decoded(&m.name).as_ref();

            let calls = m
                .calls
                .as_ref()
                .ok_or_else(|| ExtrinsicInfoError::CallNotFound {
                    index: $call_index,
                    pallet_index: $pallet_index,
                    pallet_name: Cow::Borrowed(m_name),
                })?;

            let calls = as_decoded(calls);

            let call = calls.get($call_index as usize).ok_or_else(|| {
                ExtrinsicInfoError::CallNotFound {
                    index: $call_index,
                    pallet_index: $pallet_index,
                    pallet_name: Cow::Borrowed(m_name),
                }
            })?;

            let c_name = as_decoded(&call.name);

            let args = as_decoded(&call.arguments);

            let args = args
                .iter()
                .map(|a| {
                    let ty = as_decoded(&a.ty);
                    let id = parse_lookup_name(ty)?.in_pallet(m_name);
                    let name = as_decoded(&a.name);
                    Ok(ExtrinsicCallInfoArg {
                        id,
                        name: Cow::Borrowed(name),
                    })
                })
                .collect::<Result<_, ExtrinsicInfoError>>()?;

            Ok(ExtrinsicCallInfo {
                pallet_index: $pallet_index,
                call_index: $call_index,
                pallet_name: Cow::Borrowed(m_name),
                call_name: Cow::Borrowed(c_name),
                args,
            })
        }};
    }

    macro_rules! impl_extrinsic_info_by_name_body_for_v8_to_v11 {
        ($self:ident, $pallet_name_arg:ident, $call_name_arg:ident) => {{
            use alloc::string::ToString;

            let modules = as_decoded(&$self.modules);

            let (pallet_index, m) = modules
                .iter()
                .filter(|m| m.calls.is_some())
                .enumerate()
                .find(|(_, m)| {
                    let name: &str = as_decoded(&m.name).as_ref();
                    name == $pallet_name_arg
                })
                .ok_or_else(|| ExtrinsicInfoError::PalletNotFoundByName {
                    name: Cow::Owned($pallet_name_arg.to_string()),
                })?;

            let pallet_index = pallet_index as u8;

            // as_ref to work when scale-info returns `&static str`
            // instead of `String` in no-std mode.
            let m_name: &str = as_decoded(&m.name).as_ref();

            let calls = m
                .calls
                .as_ref()
                .ok_or_else(|| ExtrinsicInfoError::CallNotFoundByName {
                    call_name: Cow::Owned($call_name_arg.to_string()),
                    pallet_index,
                    pallet_name: Cow::Borrowed(m_name),
                })?;

            let calls = as_decoded(calls);

            let (call_index, call) = calls
                .iter()
                .enumerate()
                .find(|(_, c)| {
                    let name: &str = as_decoded(&c.name).as_ref();
                    name == $call_name_arg
                })
                .ok_or_else(|| ExtrinsicInfoError::CallNotFoundByName {
                    call_name: Cow::Owned($call_name_arg.to_string()),
                    pallet_index,
                    pallet_name: Cow::Borrowed(m_name),
                })?;

            let c_name: &str = as_decoded(&call.name).as_ref();

            let args = as_decoded(&call.arguments);

            let args = args
                .iter()
                .map(|a| {
                    let ty: &str = as_decoded(&a.ty).as_ref();
                    let id = parse_lookup_name(ty)?.in_pallet(m_name);
                    let name: &str = as_decoded(&a.name).as_ref();
                    Ok(ExtrinsicCallInfoArg {
                        id,
                        name: Cow::Borrowed(name),
                    })
                })
                .collect::<Result<_, ExtrinsicInfoError>>()?;

            Ok(ExtrinsicCallInfo {
                pallet_index,
                call_index: call_index as u8,
                pallet_name: Cow::Borrowed(m_name),
                call_name: Cow::Borrowed(c_name),
                args,
            })
        }};
    }

    macro_rules! impl_for_v8_to_v10 {
        ($path:path) => {
            impl ExtrinsicTypeInfo for $path {
                type TypeId = LookupName;
                fn extrinsic_call_info_by_index(
                    &self,
                    pallet_index: u8,
                    call_index: u8,
                ) -> Result<ExtrinsicCallInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>> {
                    impl_extrinsic_info_by_index_body_for_v8_to_v11!(self, pallet_index, call_index)
                }
                fn extrinsic_call_info_by_name(
                    &self,
                    pallet_name: &str,
                    call_name: &str,
                ) -> Result<ExtrinsicCallInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>> {
                    impl_extrinsic_info_by_name_body_for_v8_to_v11!(self, pallet_name, call_name)
                }
                fn extrinsic_signature_info(
                    &self,
                ) -> Result<ExtrinsicSignatureInfo<Self::TypeId>, ExtrinsicInfoError<'_>> {
                    Ok(ExtrinsicSignatureInfo {
                        address_id: parse_lookup_name("hardcoded::ExtrinsicAddress")?,
                        signature_id: parse_lookup_name("hardcoded::ExtrinsicSignature")?,
                    })
                }
                fn extrinsic_extension_info(
                    &self,
                    extension_version: Option<u8>,
                ) -> Result<ExtrinsicExtensionInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>> {
                    err_if_bad_extension_version(extension_version)?;

                    Ok(ExtrinsicExtensionInfo {
                        extension_ids: Vec::from_iter([ExtrinsicExtensionInfoArg {
                            name: Cow::Borrowed("ExtrinsicSignedExtensions"),
                            id: parse_lookup_name("hardcoded::ExtrinsicSignedExtensions")?,
                            implicit_id: parse_lookup_name(
                                "hardcoded::ExtrinsicSignedExtensionsImplicit",
                            )?,
                        }]),
                    })
                }
                fn extrinsic_extension_version_info(
                    &self,
                ) -> Result<impl Iterator<Item = u8>, ExtrinsicInfoError<'_>> {
                    Ok(core::iter::empty())
                }
            }
        };
    }

    impl_for_v8_to_v10!(frame_metadata::v8::RuntimeMetadataV8);
    impl_for_v8_to_v10!(frame_metadata::v9::RuntimeMetadataV9);
    impl_for_v8_to_v10!(frame_metadata::v10::RuntimeMetadataV10);

    impl ExtrinsicTypeInfo for frame_metadata::v11::RuntimeMetadataV11 {
        type TypeId = LookupName;
        fn extrinsic_call_info_by_index(
            &self,
            pallet_index: u8,
            call_index: u8,
        ) -> Result<ExtrinsicCallInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>> {
            impl_extrinsic_info_by_index_body_for_v8_to_v11!(self, pallet_index, call_index)
        }
        fn extrinsic_call_info_by_name(
            &self,
            pallet_name: &str,
            call_name: &str,
        ) -> Result<ExtrinsicCallInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>> {
            impl_extrinsic_info_by_name_body_for_v8_to_v11!(self, pallet_name, call_name)
        }
        fn extrinsic_signature_info(
            &self,
        ) -> Result<ExtrinsicSignatureInfo<Self::TypeId>, ExtrinsicInfoError<'_>> {
            Ok(ExtrinsicSignatureInfo {
                address_id: parse_lookup_name("hardcoded::ExtrinsicAddress")?,
                signature_id: parse_lookup_name("hardcoded::ExtrinsicSignature")?,
            })
        }
        fn extrinsic_extension_info(
            &self,
            extension_version: Option<u8>,
        ) -> Result<ExtrinsicExtensionInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>> {
            err_if_bad_extension_version(extension_version)?;

            // In V11 metadata we start exposing signed extension names, so we use those directly instead of
            // a hardcoded ExtrinsicSignedExtensions type that the user is expected to define.
            let extension_ids = self
                .extrinsic
                .signed_extensions
                .iter()
                .map(|e| {
                    let signed_ext_name = as_decoded(e);
                    let signed_ext_id = parse_lookup_name(signed_ext_name)?;
                    let signed_ext_implicit_id =
                        parse_lookup_name(&format!("{signed_ext_name}Implicit"))
                            .map_err(|e| e.into_owned())?;

                    Ok(ExtrinsicExtensionInfoArg {
                        id: signed_ext_id,
                        implicit_id: signed_ext_implicit_id,
                        name: Cow::Borrowed(signed_ext_name),
                    })
                })
                .collect::<Result<Vec<_>, ExtrinsicInfoError<'_>>>()?;

            Ok(ExtrinsicExtensionInfo { extension_ids })
        }
        fn extrinsic_extension_version_info(
            &self,
        ) -> Result<impl Iterator<Item = u8>, ExtrinsicInfoError<'_>> {
            Ok(core::iter::empty())
        }
    }

    macro_rules! impl_for_v12_to_v13 {
        ($path:path) => {
            impl ExtrinsicTypeInfo for $path {
                type TypeId = LookupName;
                fn extrinsic_call_info_by_index(
                    &self,
                    pallet_index: u8,
                    call_index: u8,
                ) -> Result<ExtrinsicCallInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>> {
                    let modules = as_decoded(&self.modules);

                    let m = modules
                        .iter()
                        .find(|m| m.index == pallet_index)
                        .ok_or_else(|| ExtrinsicInfoError::PalletNotFound {
                            index: pallet_index,
                        })?;

                    // as_ref to work when scale-info returns `&static str`
                    // instead of `String` in no-std mode.
                    let m_name = as_decoded(&m.name).as_ref();

                    let calls =
                        m.calls
                            .as_ref()
                            .ok_or_else(|| ExtrinsicInfoError::CallNotFound {
                                index: call_index,
                                pallet_index,
                                pallet_name: Cow::Borrowed(m_name),
                            })?;

                    let calls = as_decoded(calls);

                    let call = calls.get(call_index as usize).ok_or_else(|| {
                        ExtrinsicInfoError::CallNotFound {
                            index: call_index,
                            pallet_index,
                            pallet_name: Cow::Borrowed(m_name),
                        }
                    })?;

                    let c_name = as_decoded(&call.name);

                    let args = as_decoded(&call.arguments);

                    let args = args
                        .iter()
                        .map(|a| {
                            let ty = as_decoded(&a.ty);
                            let id = parse_lookup_name(ty)?.in_pallet(m_name);
                            let name = as_decoded(&a.name);
                            Ok(ExtrinsicCallInfoArg {
                                id,
                                name: Cow::Borrowed(name),
                            })
                        })
                        .collect::<Result<_, ExtrinsicInfoError>>()?;

                    Ok(ExtrinsicCallInfo {
                        pallet_index,
                        call_index,
                        pallet_name: Cow::Borrowed(m_name),
                        call_name: Cow::Borrowed(c_name),
                        args,
                    })
                }
                fn extrinsic_call_info_by_name(
                    &self,
                    pallet_name: &str,
                    call_name: &str,
                ) -> Result<ExtrinsicCallInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>> {
                    use alloc::string::ToString;

                    let modules = as_decoded(&self.modules);

                    let m = modules
                        .iter()
                        .find(|m| {
                            let name: &str = as_decoded(&m.name).as_ref();
                            name == pallet_name
                        })
                        .ok_or_else(|| ExtrinsicInfoError::PalletNotFoundByName {
                            name: Cow::Owned(pallet_name.to_string()),
                        })?;

                    let pallet_index = m.index;

                    // as_ref to work when scale-info returns `&static str`
                    // instead of `String` in no-std mode.
                    let m_name: &str = as_decoded(&m.name).as_ref();

                    let calls =
                        m.calls
                            .as_ref()
                            .ok_or_else(|| ExtrinsicInfoError::CallNotFoundByName {
                                call_name: Cow::Owned(call_name.to_string()),
                                pallet_index,
                                pallet_name: Cow::Borrowed(m_name),
                            })?;

                    let calls = as_decoded(calls);

                    let (call_index, call) = calls
                        .iter()
                        .enumerate()
                        .find(|(_, c)| {
                            let name: &str = as_decoded(&c.name).as_ref();
                            name == call_name
                        })
                        .ok_or_else(|| ExtrinsicInfoError::CallNotFoundByName {
                            call_name: Cow::Owned(call_name.to_string()),
                            pallet_index,
                            pallet_name: Cow::Borrowed(m_name),
                        })?;

                    let c_name: &str = as_decoded(&call.name).as_ref();

                    let args = as_decoded(&call.arguments);

                    let args = args
                        .iter()
                        .map(|a| {
                            let ty: &str = as_decoded(&a.ty).as_ref();
                            let id = parse_lookup_name(ty)?.in_pallet(m_name);
                            let name: &str = as_decoded(&a.name).as_ref();
                            Ok(ExtrinsicCallInfoArg {
                                id,
                                name: Cow::Borrowed(name),
                            })
                        })
                        .collect::<Result<_, ExtrinsicInfoError>>()?;

                    Ok(ExtrinsicCallInfo {
                        pallet_index,
                        call_index: call_index as u8,
                        pallet_name: Cow::Borrowed(m_name),
                        call_name: Cow::Borrowed(c_name),
                        args,
                    })
                }
                fn extrinsic_signature_info(
                    &self,
                ) -> Result<ExtrinsicSignatureInfo<Self::TypeId>, ExtrinsicInfoError<'_>> {
                    Ok(ExtrinsicSignatureInfo {
                        address_id: parse_lookup_name("hardcoded::ExtrinsicAddress")?,
                        signature_id: parse_lookup_name("hardcoded::ExtrinsicSignature")?,
                    })
                }
                fn extrinsic_extension_info(
                    &self,
                    extension_version: Option<u8>,
                ) -> Result<ExtrinsicExtensionInfo<'_, Self::TypeId>, ExtrinsicInfoError<'_>> {
                    err_if_bad_extension_version(extension_version)?;

                    // In V12 metadata we are exposing signed extension names, so we use those directly instead of
                    // a hardcoded ExtrinsicSignedExtensions type that the user is expected to define.
                    let extension_ids = self
                        .extrinsic
                        .signed_extensions
                        .iter()
                        .map(|e| {
                            let signed_ext_name = as_decoded(e);
                            let signed_ext_id = parse_lookup_name(signed_ext_name)?;
                            let signed_ext_implicit_id =
                                parse_lookup_name(&format!("{signed_ext_name}Implicit"))
                                    .map_err(|e| e.into_owned())?;

                            Ok(ExtrinsicExtensionInfoArg {
                                id: signed_ext_id,
                                implicit_id: signed_ext_implicit_id,
                                name: Cow::Borrowed(signed_ext_name),
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;

                    Ok(ExtrinsicExtensionInfo { extension_ids })
                }
                fn extrinsic_extension_version_info(
                    &self,
                ) -> Result<impl Iterator<Item = u8>, ExtrinsicInfoError<'_>> {
                    Ok(core::iter::empty())
                }
            }
        };
    }

    impl_for_v12_to_v13!(frame_metadata::v12::RuntimeMetadataV12);
    impl_for_v12_to_v13!(frame_metadata::v13::RuntimeMetadataV13);

    fn parse_lookup_name(name: &str) -> Result<LookupName, ExtrinsicInfoError<'_>> {
        LookupName::parse(name).map_err(|e| ExtrinsicInfoError::CannotParseTypeName {
            name: Cow::Borrowed(name),
            reason: e,
        })
    }
};
