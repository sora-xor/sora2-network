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

//! Decode extrinsics and storage values from substrate based networks which expose `frame-metadata::RuntimeMetadata`
//! like Polkadot.
//!
//! - See [`extrinsics`] for decoding Extrinsics.
//! - See [`storage`] for encoding/decoding storage keys and decoding values.
//! - See [`runtime_apis`] for encoding Runtime API inputs and decoding Runtime API responses
//! - See [`legacy_types`] to access historic type information for certain chains.
//!
#![deny(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod methods;
mod utils;

pub mod extrinsics {
    //! This module contains functions for decoding extrinsics.
    //!
    //! - See [`decode_extrinsic`] for a general function to decode modern or historic extrinsics.
    //! - See [`ExtrinsicTypeInfo`] for the underlying trait which extracts the relevant information.

    pub use crate::methods::extrinsic_decoder::{
        Extrinsic, ExtrinsicDecodeError, ExtrinsicExtensions, ExtrinsicOwned, ExtrinsicSignature,
        ExtrinsicType, NamedArg, decode_extrinsic,
    };
    pub use crate::methods::extrinsic_encoder::{
        ExtrinsicEncodeError, TransactionExtension, TransactionExtensionError,
        TransactionExtensions, TransactionExtensionsError,
        best_v5_general_transaction_extension_version, encode_call_data, encode_call_data_to,
        encode_call_data_with_info, encode_call_data_with_info_to, encode_v4_signed,
        encode_v4_signed_to, encode_v4_signed_with_info_to, encode_v4_signer_payload,
        encode_v4_signer_payload_with_info, encode_v4_unsigned, encode_v4_unsigned_to,
        encode_v4_unsigned_with_info_to, encode_v5_bare, encode_v5_bare_to,
        encode_v5_bare_with_info_to, encode_v5_general, encode_v5_general_to,
        encode_v5_general_with_info_to, encode_v5_signer_payload,
        encode_v5_signer_payload_with_info,
    };
    pub use crate::methods::extrinsic_type_info::{
        ExtrinsicCallInfo, ExtrinsicCallInfoArg, ExtrinsicExtensionInfo, ExtrinsicExtensionInfoArg,
        ExtrinsicInfoError, ExtrinsicSignatureInfo, ExtrinsicTypeInfo,
    };
}

pub mod storage {
    //! This module contains functions for decoding storage keys and values.
    //!
    //! - See [`decode_storage_key`] and [`decode_storage_value`] to decode storage keys or values
    //!   from modern or historic runtimes.
    //! - See [`encode_storage_key_prefix`] to encode storage prefixes, and [`encode_storage_key`] to encode
    //!   storage keys.
    //! - See [`StorageTypeInfo`] for the underlying trait which provides storage entry information.
    //! - See [`StorageEntryInfo`] for a underlying trait which provides information about the available
    //!   storage entries.

    pub use crate::methods::storage_decoder::{
        StorageKey, StorageKeyDecodeError, StorageKeyPart, StorageKeyPartValue,
        StorageKeyValueDecodeError, StorageValueDecodeError,
        decode_default_storage_value_with_info, decode_storage_key, decode_storage_key_values,
        decode_storage_key_with_info, decode_storage_value, decode_storage_value_with_info,
    };
    pub use crate::methods::storage_encoder::{
        StorageKeyEncodeError, encode_storage_key, encode_storage_key_prefix,
        encode_storage_key_suffix, encode_storage_key_suffix_to,
        encode_storage_key_suffix_with_info_to, encode_storage_key_to,
        encode_storage_key_with_info, encode_storage_key_with_info_to,
    };
    pub use crate::methods::storage_type_info::{
        StorageEntry, StorageEntryInfo, StorageHasher, StorageInfo, StorageInfoError,
        StorageKeyInfo, StorageTypeInfo,
    };
    pub use crate::utils::{
        DecodableValues, EncodableValues, IntoDecodableValues, IntoEncodableValues,
    };
}

pub mod constants {
    //! This module contains types and functions for working with constants.
    //!
    //! - See [`decode_constant`] and [`decode_constant_with_info`] to decode constants
    //! - See [`ConstantTypeInfo`] for the underlying trait which extracts constant
    //!   information from metadata.
    //! - See [`ConstantEntryInfo`] for a underlying trait which provides information about the available
    //!   constants.

    pub use crate::methods::constant_decoder::{
        ConstantDecodeError, decode_constant, decode_constant_with_info,
    };
    pub use crate::methods::constant_type_info::{
        ConstantEntry, ConstantEntryInfo, ConstantInfo, ConstantInfoError, ConstantTypeInfo,
    };
}

pub mod runtime_apis {
    //! This module contains types and functions for working with Runtime APIs.
    //!
    //! - See [`encode_runtime_api_name`] and [`encode_runtime_api_inputs`] to encode
    //!   the name and inputs to make a Runtime API call.
    //! - See [`decode_runtime_api_response`] to decode Runtime API responses.
    //! - See [`RuntimeApiTypeInfo`] for the underlying trait which extracts the relevant information.
    //! - See [`RuntimeApiEntryInfo`] for a underlying trait which provides information about the available
    //!   Runtime APIs.

    pub use crate::methods::runtime_api_decoder::{
        RuntimeApiDecodeError, decode_runtime_api_response, decode_runtime_api_response_with_info,
    };
    pub use crate::methods::runtime_api_encoder::{
        RuntimeApiInputsEncodeError, encode_runtime_api_inputs, encode_runtime_api_inputs_to,
        encode_runtime_api_inputs_with_info_to, encode_runtime_api_name,
    };
    pub use crate::methods::runtime_api_type_info::{
        RuntimeApiEntry, RuntimeApiEntryInfo, RuntimeApiInfo, RuntimeApiInfoError, RuntimeApiInput,
        RuntimeApiTypeInfo,
    };
    pub use crate::utils::{EncodableValues, IntoEncodableValues};
}

pub mod view_functions {
    //! This module contains types and functions for working with View Functions.
    //!
    //! - See [`RUNTIME_API_NAME`] and [`encode_view_function_inputs`] to obtain the Runtime API name
    //!   and the encoded input data required to call a given View Function.
    //! - See [`decode_view_function_response`] to decode View Function responses.
    //! - See [`ViewFunctionTypeInfo`] for the underlying trait which extracts the relevant information.
    //! - See [`ViewFunctionEntryInfo`] for a underlying trait which provides information about the available
    //!   View Functions.

    pub use crate::methods::view_function_decoder::{
        ViewFunctionDecodeError, decode_view_function_response,
        decode_view_function_response_with_info,
    };
    pub use crate::methods::view_function_encoder::{
        RUNTIME_API_NAME, ViewFunctionInputsEncodeError, encode_view_function_inputs,
        encode_view_function_inputs_to, encode_view_function_inputs_with_info_to,
    };
    pub use crate::methods::view_function_type_info::{
        ViewFunctionEntry, ViewFunctionEntryInfo, ViewFunctionInfo, ViewFunctionInfoError,
        ViewFunctionInput, ViewFunctionTypeInfo,
    };
    pub use crate::utils::{EncodableValues, IntoEncodableValues};
}

pub mod custom_values {
    //! This module contains types and functions for working with custom values.
    //!
    //! - See [`decode_custom_value`] and [`decode_custom_value_with_info`] to decode custom values
    //! - See [`CustomValueTypeInfo`] for the underlying trait which extracts custom value
    //!   information from metadata.
    //! - See [`CustomValueEntryInfo`] for a underlying trait which provides information about the available
    //!   custom values.

    pub use crate::methods::custom_value_decoder::{
        CustomValueDecodeError, decode_custom_value, decode_custom_value_with_info,
    };
    pub use crate::methods::custom_value_type_info::{
        CustomValue, CustomValueEntryInfo, CustomValueInfo, CustomValueInfoError,
        CustomValueTypeInfo,
    };
}

#[cfg(feature = "legacy-types")]
pub mod legacy_types {
    //! This module contains legacy types that can be used to decode pre-V14 blocks and storage.

    use alloc::boxed::Box;
    type GenericError = Box<dyn core::error::Error + Send + Sync + 'static>;

    /// Deserialize bytes into a [`scale_info_legacy::ChainTypeRegistry`].
    pub fn from_bytes(bytes: &[u8]) -> Result<scale_info_legacy::ChainTypeRegistry, GenericError> {
        let types = serde_saphyr::from_slice(bytes)?;
        Ok(types)
    }

    pub mod polkadot {
        //! Legacy types for Polkadot chains.

        /// Legacy types for the Polkadot Relay Chain.
        pub fn relay_chain() -> scale_info_legacy::ChainTypeRegistry {
            // This is a convenience function to load the Polkadot relay chain types.
            // It is used in the examples in this crate.
            let bytes = include_bytes!("../types/polkadot_relay_types.yaml");
            serde_saphyr::from_slice(bytes).expect("Polkadot RC types are valid YAML")
        }
    }

    pub mod kusama {
        //! Legacy types for Kusama chains.

        /// Legacy types for the Kusama Relay Chain.
        pub fn relay_chain() -> scale_info_legacy::ChainTypeRegistry {
            // This is a convenience function to load the Polkadot relay chain types.
            // It is used in the examples in this crate.
            let bytes = include_bytes!("../types/kusama_relay_types.yaml");
            serde_saphyr::from_slice(bytes).expect("Kusama RC types are valid YAML")
        }

        /// Legacy types for the Kusama Asset Hub.
        pub fn asset_hub() -> scale_info_legacy::ChainTypeRegistry {
            // This is a convenience function to load the Polkadot relay chain types.
            // It is used in the examples in this crate.
            let bytes = include_bytes!("../types/kusama_assethub_types.yaml");
            serde_saphyr::from_slice(bytes).expect("Kusama AssetHub types are valid YAML")
        }
    }
}

pub mod helpers {
    //! Helper functions and types to assist with decoding.
    //!
    //! - [`type_registry_from_metadata`] is expected to be used when decoding things from historic
    //!   runtimes, adding the ability to decode some types from information in the metadata.
    //! - [`decode_with_error_tracing`] is like [`decode_with_visitor`], but
    //!   will use a tracing visitor (if the `error-tracing` feature is enabled) to provide more
    //!   information in the event that decoding fails.

    pub use crate::methods::Entry;

    pub use crate::utils::{
        DecodableValues, DecodeErrorTrace, EncodableValues, IntoDecodableValues,
        IntoEncodableValues, decode_with_error_tracing,
    };
    #[cfg(feature = "legacy")]
    pub use crate::utils::{
        ToTypeRegistry, type_registry_from_metadata, type_registry_from_metadata_any,
    };

    /// An alias to [`scale_decode::visitor::decode_with_visitor`]. This can be used to decode the byte ranges
    /// given back from functions like [`crate::extrinsics::decode_extrinsic`] or
    /// [`crate::storage::decode_storage_key`].
    pub use scale_decode::visitor::decode_with_visitor;

    /// An alias to the underlying [`scale_decode`] crate.
    pub use scale_decode;
}

#[cfg(test)]
mod test {
    use crate::methods::extrinsic_type_info::ExtrinsicTypeInfo;
    use crate::methods::runtime_api_type_info::RuntimeApiTypeInfo;
    use crate::methods::storage_type_info::StorageTypeInfo;
    use crate::methods::view_function_type_info::ViewFunctionTypeInfo;
    use crate::utils::ToTypeRegistry;
    use scale_info_legacy::type_registry::TypeRegistryResolveError;
    use scale_info_legacy::{ChainTypeRegistry, LookupName};
    use scale_type_resolver::Field;

    // This will panic if there is any issue decoding the legacy types we provide.
    #[test]
    fn test_deserializing_legacy_types() {
        let _ = crate::legacy_types::polkadot::relay_chain();
        let _ = crate::legacy_types::kusama::relay_chain();
        let _ = crate::legacy_types::kusama::asset_hub();
    }

    #[test]
    fn test_legacy_yaml_rejects_duplicate_definitions() {
        for second in ["u16", "u32"] {
            let yaml = format!("global:\n  types:\n    Example: u16\n    Example: {second}\n");
            let error = crate::legacy_types::from_bytes(yaml.as_bytes()).unwrap_err();
            assert!(error.to_string().contains("duplicate mapping key"));
        }
    }

    #[test]
    fn test_embedded_legacy_yaml_preserves_final_duplicate_definitions() {
        use serde_json::json;

        let polkadot: serde_json::Value =
            serde_saphyr::from_slice(include_bytes!("../types/polkadot_relay_types.yaml")).unwrap();
        let kusama: serde_json::Value =
            serde_saphyr::from_slice(include_bytes!("../types/kusama_relay_types.yaml")).unwrap();
        let asset_hub: serde_json::Value =
            serde_saphyr::from_slice(include_bytes!("../types/kusama_assethub_types.yaml"))
                .unwrap();
        // The former HashMap deserializer retained each final definition, even
        // when an earlier definition disagreed. Keep those resolved meanings.
        for chain in [&polkadot, &kusama, &asset_hub] {
            assert_eq!(chain["global"]["types"]["ValidatorIndex"], "u16");
        }
        let kusama = &kusama["global"]["types"];
        assert_eq!(
            kusama["OpaqueNetworkState"],
            json!({"peer_id": "OpaquePeerId", "external_addresses": "Vec<OpaqueMultiaddr>"})
        );
        assert_eq!(
            kusama["BitVec"],
            "bitvec::vec::BitVec<u8, bitvec::order::Msb0>"
        );
        assert_eq!(
            kusama["VestingSchedule<T::Balance, T::BlockNumber>"],
            json!({"locked": "T::Balance", "per_block": "T::Balance", "starting_block": "T::BlockNumber"})
        );
        assert_eq!(
            kusama["VestingSchedule<Balance, BlockNumber>"],
            json!({"locked": "Balance", "per_block": "Balance", "starting_block": "BlockNumber"})
        );
        let asset_hub = &asset_hub["global"]["types"];
        for (name, expected) in [
            (
                "<T::Lookup as StaticLookup>::Source",
                "MultiAddress<AccountId32,u32>",
            ),
            ("RefCount", "u32"),
            ("T::ClassId", "u32"),
            ("T::InstanceId", "u32"),
        ] {
            assert_eq!(asset_hub[name], expected);
        }
        assert_eq!(
            asset_hub["AccountInfo<Index, AccountData>"],
            json!({"nonce": "Index", "consumers": "RefCount", "providers": "RefCount", "sufficients": "RefCount", "data": "AccountData"})
        );
        assert_eq!(
            asset_hub["Outcome"],
            json!({"_enum": {"Complete": "Weight", "Incomplete": "(Weight, XcmError)", "Error": "XcmError"}})
        );
        assert_eq!(asset_hub["XcmError"]["_enum"]["Trap"], "u64");
    }

    #[test]
    fn test_embedded_legacy_duplicate_winners_preserve_field_and_variant_order() {
        let registry = crate::legacy_types::kusama::asset_hub();
        let types = registry.for_spec_version(0);
        let fields = types
            .resolve_type_str(
                "AccountInfo<u32, u128>",
                scale_type_resolver::visitor::new((), |_, _| Vec::<String>::new()).visit_composite(
                    |_, _, fields| fields.map(|field| field.name.unwrap().to_owned()).collect(),
                ),
            )
            .unwrap();
        assert_eq!(
            fields,
            ["nonce", "consumers", "providers", "sufficients", "data"]
        );
        for (name, expected) in [
            ("Outcome", vec!["Complete", "Incomplete", "Error"]),
            (
                "XcmError",
                vec![
                    "Undefined",
                    "Unimplemented",
                    "UnhandledXcmMessage",
                    "UnhandledEffect",
                    "EscalationOfPrivilege",
                    "UntrustedReserveLocation",
                    "UntrustedTeleportLocation",
                    "DestinationNotInvertible",
                    "MultiLocationFull",
                    "MultiLocationNotInvertible",
                    "BadOrigin",
                    "InvalidLocation",
                    "AssetNotFound",
                    "FailedToTransactAsset",
                    "NotWithdrawable",
                    "LocationCannotHold",
                    "ExceedsMaxMessageSize",
                    "DestinationUnsupported",
                    "Locked",
                    "OrdersNotEmpty",
                    "DuplicateOrder",
                    "NotDepositable",
                    "Unwitnessed",
                    "UnknownClaim",
                    "Trap",
                    "Overflow",
                    "UnimplementedXcmVersion",
                    "Incomplete",
                    "NetworkNotSupported",
                    "BadXcm",
                    "WeightLimitReached",
                    "WeightNotComputable",
                    "Barrier",
                    "SendFailed",
                    "Transport",
                    "Unroutable",
                    "UnknownError",
                ],
            ),
        ] {
            let variants = types
                .resolve_type_str(
                    name,
                    scale_type_resolver::visitor::new((), |_, _| Vec::<(u8, String)>::new())
                        .visit_variant(|_, _, variants| {
                            variants
                                .map(|variant| (variant.index, variant.name.to_owned()))
                                .collect()
                        }),
                )
                .unwrap();
            let expected: Vec<_> = expected
                .into_iter()
                .enumerate()
                .map(|(index, name)| (index as u8, name.to_owned()))
                .collect();
            assert_eq!(variants, expected, "{name}");
        }
    }

    #[test]
    fn test_embedded_legacy_reserved_variant_names_keep_indices_and_payloads() {
        fn variants(
            types: &scale_info_legacy::TypeRegistrySet<'_>,
            name: &str,
        ) -> Vec<(u8, String, usize)> {
            types
                .resolve_type_str(
                    name,
                    scale_type_resolver::visitor::new((), |_, _| Vec::new()).visit_variant(
                        |_, _, variants| {
                            variants
                                .map(|variant| {
                                    (
                                        variant.index,
                                        variant.name.to_owned(),
                                        variant.fields.count(),
                                    )
                                })
                                .collect()
                        },
                    ),
                )
                .unwrap()
        }
        for (chain, registry) in legacy_types() {
            let types = registry.for_spec_version(0);
            assert_eq!(
                variants(&types, "Phase<u32>")[0],
                (0, "Off".into(), 0),
                "{chain}"
            );
            if chain == "Polkadot RC" {
                continue;
            }
            let location = variants(&types, "MultiLocation");
            assert_eq!(location[0], (0, "Null".into(), 0), "{chain}");
            assert_eq!(
                location
                    .into_iter()
                    .map(|(index, name, _)| (index, name))
                    .collect::<Vec<_>>(),
                ["Null", "X1", "X2", "X3", "X4", "X5", "X6", "X7", "X8"]
                    .into_iter()
                    .enumerate()
                    .map(|(index, name)| (index as u8, name.to_owned()))
                    .collect::<Vec<_>>(),
                "{chain}"
            );
            assert_eq!(
                variants(&types, "Order<()>")[0],
                (0, "Null".into(), 0),
                "{chain}"
            );
            if chain == "Kusama AH" {
                assert_eq!(variants(&types, "ResponseV1")[0], (0, "Null".into(), 0));
                let junctions = variants(&types, "JunctionV1");
                // A Null *value* denotes a unit payload and must not become a
                // quoted alias while normalizing the String-valued enum keys.
                assert_eq!(junctions[7], (7, "OnlyChild".into(), 0));
            }
        }
    }

    fn legacy_types() -> [(&'static str, ChainTypeRegistry); 3] {
        [
            ("Polkadot RC", crate::legacy_types::polkadot::relay_chain()),
            ("Kusama RC", crate::legacy_types::kusama::relay_chain()),
            ("Kusama AH", crate::legacy_types::kusama::asset_hub()),
        ]
    }

    fn all_type_registry_sets(
        registry: &scale_info_legacy::ChainTypeRegistry,
    ) -> impl Iterator<Item = scale_info_legacy::TypeRegistrySet<'_>> {
        let all_spec_versions = core::iter::once(u64::MAX)
            .chain(registry.spec_version_ranges().map(|(low, _high)| low));
        all_spec_versions.map(|version| registry.for_spec_version(version))
    }

    #[test]
    fn test_legacy_types_have_sane_type_names() {
        // We ignore these ones:
        let builtins = &[
            "u8", "u16", "u32", "u64", "u128", "u256", "i8", "i16", "i32", "i64", "i128", "i256",
            "char", "bool", "str",
        ];

        for (chain, types) in legacy_types() {
            for types in all_type_registry_sets(&types) {
                for ty in types.keys() {
                    if let Some(path_and_name) = ty.name() {
                        let name = path_and_name.split("::").last().unwrap();

                        if builtins.contains(&name) {
                            continue;
                        }

                        if name.starts_with(|c: char| !c.is_uppercase() || !c.is_ascii_alphabetic())
                        {
                            panic!("{chain}: {ty} does not begin with an uppercase letter");
                        }
                        if name.contains(|c: char| !c.is_ascii_alphanumeric()) {
                            panic!("{chain}: {ty} contains a non-ascii-alphanumeric character");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_legacy_types_have_sane_field_names() {
        fn assert_sane<'a>(
            chain: &str,
            ty: &LookupName,
            fields: impl Iterator<Item = Field<'a, LookupName>>,
        ) {
            let field_names: Vec<_> = fields.map(|f| f.name).collect();

            let all_fields_named = field_names.iter().all(|n| n.is_some());
            let all_fields_unnamed = field_names.iter().all(|n| n.is_none());

            if !(all_fields_named || all_fields_unnamed) {
                panic!("{chain}: All fields must be named or unnamed, but aren't in '{ty}'");
            }
            if all_fields_named {
                for name in field_names.into_iter().map(|n| n.unwrap()) {
                    let Some(fst) = name.chars().next() else {
                        panic!("{chain}: {ty} has a present but empty field name");
                    };
                    if !fst.is_ascii_alphabetic() {
                        panic!(
                            "{chain}: {ty} field name '{name}' is invalid (does not start with ascii letter)"
                        );
                    }
                    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                        panic!(
                            "{chain}: {ty} field name '{name}' is invalid (non-ascii-letter-or-number-or-underscore present in the name"
                        );
                    }
                    if name.contains(|c: char| c.is_uppercase()) {
                        panic!(
                            "{chain}: {ty} field name '{name}' contains uppercase. Field names should be lowercase only"
                        );
                    }
                }
            }
        }

        for (chain, types) in legacy_types() {
            for types in all_type_registry_sets(&types) {
                for ty in types.keys() {
                    let visitor = scale_type_resolver::visitor::new((), |_, _| ())
                        .visit_variant(|_ctx, _path, vars| {
                            for variant in vars {
                                assert_sane(chain, &ty, variant.fields);
                            }
                        })
                        .visit_composite(|_ctx, _path, fields| {
                            assert_sane(chain, &ty, fields);
                        });

                    if let Err(e) = types.resolve_type(ty.clone(), visitor) {
                        match e {
                            TypeRegistryResolveError::UnexpectedBitOrderType
                            | TypeRegistryResolveError::UnexpectedBitStoreType => {
                                /* Ignore these */
                            }
                            e => panic!("{chain}: Cannot resolve type '{ty}': {e}"),
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_legacy_types_have_sane_variant_names() {
        fn assert_sane(chain: &str, ty: &LookupName, variant_name: &str) {
            let Some(fst) = variant_name.chars().next() else {
                panic!("{chain}: Enum {ty} has an empty variant");
            };

            if !fst.is_uppercase() {
                panic!(
                    "{chain}: Enum {ty} variant name '{variant_name}' should start with an uppercase letter"
                );
            }
            if !fst.is_ascii_alphabetic() {
                panic!(
                    "{chain}: Enum {ty} variant name '{variant_name}' should start with an ASCII letter"
                );
            }
            if !variant_name.chars().all(|c| c.is_ascii_alphanumeric()) {
                panic!(
                    "{chain}: Enum {ty} variant name '{variant_name}' is invalid (non-ascii-letter-or-number present in the name"
                );
            }
        }

        for (chain, types) in legacy_types() {
            for types in all_type_registry_sets(&types) {
                for ty in types.keys() {
                    let visitor = scale_type_resolver::visitor::new((), |_, _| ()).visit_variant(
                        |_ctx, _path, vars| {
                            for variant in vars {
                                assert_sane(chain, &ty, variant.name);
                            }
                        },
                    );

                    if let Err(e) = types.resolve_type(ty.clone(), visitor) {
                        match e {
                            TypeRegistryResolveError::UnexpectedBitOrderType
                            | TypeRegistryResolveError::UnexpectedBitStoreType => {
                                /* Ignore these */
                            }
                            e => panic!("{chain}: Cannot resolve type '{ty}': {e}"),
                        }
                    }
                }
            }
        }
    }

    macro_rules! impls_trait {
        ($type:ty, $trait:path) => {
            const _: () = {
                const fn assert_impl<T: $trait>() {}
                assert_impl::<$type>();
            };
        };
    }

    // Just a sanity check that all of the metadata versions we expect implement
    // all of the key traits. Makes it harder to miss something when adding a new metadata
    // version; just add it below and implement the traits until everything compiles.
    #[rustfmt::skip]
    const _: () = {
        impls_trait!(frame_metadata::v8::RuntimeMetadataV8, ExtrinsicTypeInfo);
        impls_trait!(frame_metadata::v9::RuntimeMetadataV9, ExtrinsicTypeInfo);
        impls_trait!(frame_metadata::v10::RuntimeMetadataV10, ExtrinsicTypeInfo);
        impls_trait!(frame_metadata::v11::RuntimeMetadataV11, ExtrinsicTypeInfo);
        impls_trait!(frame_metadata::v12::RuntimeMetadataV12, ExtrinsicTypeInfo);
        impls_trait!(frame_metadata::v13::RuntimeMetadataV13, ExtrinsicTypeInfo);
        impls_trait!(frame_metadata::v14::RuntimeMetadataV14, ExtrinsicTypeInfo);
        impls_trait!(frame_metadata::v15::RuntimeMetadataV15, ExtrinsicTypeInfo);
        impls_trait!(frame_metadata::v16::RuntimeMetadataV16, ExtrinsicTypeInfo);

        impls_trait!(frame_metadata::v8::RuntimeMetadataV8, StorageTypeInfo);
        impls_trait!(frame_metadata::v9::RuntimeMetadataV9, StorageTypeInfo);
        impls_trait!(frame_metadata::v10::RuntimeMetadataV10, StorageTypeInfo);
        impls_trait!(frame_metadata::v11::RuntimeMetadataV11, StorageTypeInfo);
        impls_trait!(frame_metadata::v12::RuntimeMetadataV12, StorageTypeInfo);
        impls_trait!(frame_metadata::v13::RuntimeMetadataV13, StorageTypeInfo);
        impls_trait!(frame_metadata::v14::RuntimeMetadataV14, StorageTypeInfo);
        impls_trait!(frame_metadata::v15::RuntimeMetadataV15, StorageTypeInfo);
        impls_trait!(frame_metadata::v16::RuntimeMetadataV16, StorageTypeInfo);

        // Only V16+ metadata contains any view function information. Prior to this,
        // hardly any view functions existed. We _could_ extend our legacy type information
        // to support them if necessary, but it's unlikely it will be.
        impls_trait!(frame_metadata::v16::RuntimeMetadataV16, ViewFunctionTypeInfo);

        // Only V15+ metadata has Runtime API info in. For earlier, we lean on
        // our scale-Info-legacy type registry to provide the information.
        impls_trait!(scale_info_legacy::TypeRegistry, RuntimeApiTypeInfo);
        impls_trait!(scale_info_legacy::TypeRegistrySet, RuntimeApiTypeInfo);
        impls_trait!(frame_metadata::v15::RuntimeMetadataV15, RuntimeApiTypeInfo);
        impls_trait!(frame_metadata::v16::RuntimeMetadataV16, RuntimeApiTypeInfo);

        // This is a legacy trait and so only legacy metadata versions implement it:
        impls_trait!(frame_metadata::v8::RuntimeMetadataV8, ToTypeRegistry);
        impls_trait!(frame_metadata::v9::RuntimeMetadataV9, ToTypeRegistry);
        impls_trait!(frame_metadata::v10::RuntimeMetadataV10, ToTypeRegistry);
        impls_trait!(frame_metadata::v11::RuntimeMetadataV11, ToTypeRegistry);
        impls_trait!(frame_metadata::v12::RuntimeMetadataV12, ToTypeRegistry);
        impls_trait!(frame_metadata::v13::RuntimeMetadataV13, ToTypeRegistry);
    };
}
