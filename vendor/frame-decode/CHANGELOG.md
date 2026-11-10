# Changelog

The format is based on [Keep a Changelog].

[Keep a Changelog]: http://keepachangelog.com/en/1.0.0/

## 0.18.1 (2026-08-06)

Note: `0.18.0` was published to crates.io by accident and yanked; this release takes its place.

- Fix V5 signer payloads to hash the transaction extension version and call, followed by the extension values and implicits of only the extensions after the last authorization extension (such as `VerifyMultiSignature`), matching FRAME's transaction extension pipeline semantics.
- Add `TransactionExtension::is_authorization_extension` (and the corresponding `TransactionExtensions` method), which extensions like `VerifyMultiSignature` should override to return `true`. It defaults to `false`.
- Remove `TransactionExtension::encode_value_for_signer_payload_to` and `TransactionExtensions::encode_extension_value_for_signer_payload_to`; extension values are now encoded identically in transactions and signer payloads, with authorization extensions excluded via `is_authorization_extension` instead.
- Change `encode_v5_signer_payload_with_info` to require the transaction extension version needed to construct a valid V5 signer payload.

## 0.17.2 (2026-03-12)

- When encoding extrinsics, unknown Transaction Extensions that are `Option<SomeType>` will now be accepted, and encoded as `0u8` (ie the `None` variant) by default if an extension with the corresponding name is not provided by the user. This allows transaction encoding to succeed in more cases, such as when a bunch of optional extensions exist for a chian, but still allows the user to properly implement and provide those extensions if they wish.

## 0.17.1 (2026-02-23)

- Fixed: update some Kusama types to fix decode issues ([#94](https://github.com/paritytech/frame-decode/pull/94)).

## 0.17.0 (2025-12-03)

- Add extrinsic encode methods in `frame_decode::extrinsics`. This covers encoding v4 and v5 unsigned and signed extrinsics, new traits for providing transaction extensions, encoding v4 and v5 signer payloads and encoding call data.

## 0.16.1 (2025-12-03)

- Expose the `crate::helpers::ToTypeRegistry` trait so that inputs to `crate::helpers::type_registry_from_metadata` can be named. Make it sealed so that others cannot rely on it.

## 0.16.0 (2025-11-26)

- Add a flag to `StorageInfo` which can be set in order to tell `frame-decode` to use the old version of V9 storage hashers when decoding storage keys. We need to manually toggle this flag when using metadata produced by runtimes prior to [this change](https://github.com/paritytech/substrate/commit/bbb363f4320b4a72e059c0fca96af42296d5a6bf#diff-aa7bc120d701816def0f2a5eb469212d2b7021d2fc9d3b284f843f3f8089e91a), which altered the storage hashers (and thus their encoding/decoding). Kusama prior to spec version 1032 is one such case when this needs to be toggled.

## 0.15.0 (2025-11-20)

- Update to `scale-info-legacy` 0.4.0.
- Tweak `AccountId32` types to be unnamed structs rather than tuple types, so that they get a path.
- Add `legacy_types::from_bytes` to convert provided bytes into a `ChainTypeRegistry`, negating the need to include `serde_yaml` or whatever in other crates.

## 0.14.0 (2025-11-19)

- Add Kusama RC types capable of decoding historic blocks.
- Enforce that type definitions have sane variable names and use **snake case** rather than **camel case** for field names. If field names were relied on, then note that some of them will change as a result of this.

## 0.13.0 (2025-11-14)

- Separate the iterating over entries from the core `frame-decode` `*Info` traits; One only needs to implement `*Info` traits to work with `frame-decode`; the other traits are convenience traits.
- Make `Entry` type generic so that it can potentially be used in more places, expose it, and expose concrete versions from each module.
- Expose Kusama RC and AH types (though keep these hidden until the types are more complete).
- Remove `helpers::list_storage_entries_any`: it was a bit of an anomaly to have this and not a version for any other thing. Better to keep this upstream for now.

## 0.12.1 (2025-11-12)

- Add `map_ids()` functions to `RuntimeApiInfo`, `StorageInfo` and `ViewFunctionInfo` to make translating the `TypeId` parameter simpler.
  This adds a `'static` bound to `StorageInfo` type IDs, but it is not expected that this will break anything as this is already the case for `u32` and `LookupName` IDs (and is required in many other places).

## 0.12.0 (2025-11-10)

- Bump to scale-info-legacy 0.3.0.
- It's now easier to iterate over items handed back in `*Info` traits, and to do so in only one pallet/trait where that is applicable. 

## 0.11.1 (2025-10-24)

Fix storage info logic in the case of one hasher and a tuple of keys.

## 0.11.0 (2025-10-08)

This release adds encode/decode logic for Runtime APIs, Constants and Custom Values, and removes some unused bits and pieces. Additionally we merge the trait functions for getting lists of entries into the main `*TypeInfo` traits, and do a little renaming for consistency. See [#46](https://github.com/paritytech/frame-decode/pull/46) for more information.

## 0.10.0 (2025-08-29)

- Provide information about the default value in `StorageInfo`, if one exists. This may be borrowed, and so adds a lifetime to the `StorageInfo` type (which `.into_owned()` can handle if necessary).

## 0.9.0 (2025-07-24)

- Remove the `_legacy` functions; just use the non suffixed versions which are identical.
- Adds storage key encoding via `frame_decode::storage::encode_storage_key`, `frame_decode::storage::encode_storage_key_to`, and`frame_decode::storage::encode_storage_key_with_info_to`, with supporting traits.
- Rename `prefix` to `encode_prefix` to align with the above.

## 0.8.3 (2025-07-17)

- Make a couple of methods in `crate::extrinsics` return `impl ExactSizeIterator` rather than `impl Iterator`, enabling them to be used with `scale_decode::DecodeAsFields`.

## 0.8.2 (2025-07-16)

- Make the `crate::extrinsics::NamedArg` type public, since it's in the public interface.`

## 0.8.1 (2025-07-15)

- Expose a `crate::legacy_types` module which provides `crate::legacy_types::polkadot::relay_chain()` to access the relay chain types. This is gated behind the "legacy-types" feature which is disabled by default.

## 0.8.0 (2025-05-07)

- Support `frame-metadata` v23. That stabilized V16 metadata, so we implement the relevant traits for that here to support it.

## 0.7.1 (2025-04-23)

- Support `frame-metadata` v20-v21.

## 0.7.0 (2025-03-05)

- Bump `frame-metadata` to latest: 20.0.0

## 0.6.1 (2025-01-30)

- Fix a decoding error where the ranges end at 0 if an extrinsic is 0 bytes in length ([#30](https://github.com/paritytech/frame-decode/pull/30))

## 0.6.0 (2024-11-18)

- Bump frame-metadata to 18.0, scale-decode to 0.16 and scale-value to 0.18 (latest versions at time of release).

## 0.5.0 (2024-10-23)

- Bump scale-decode 0.14, scale-value 0.17 and scale-info v2.11.4 ([#7](https://github.com/paritytech/frame-decode/pull/7))
- Remove unused dependency hex ([#8](https://github.com/paritytech/frame-decode/pull/8))
- Bump frame-metadata from 16.0.0 to 17.0.0 ([#9](https://github.com/paritytech/frame-decode/pull/8))
- Bump scale-info-legacy from 0.2.1 to 0.2.2 ([#10](https://github.com/paritytech/frame-decode/pull/10))

## 0.4.0 (2024-10-21)

- Split `ExtrinsicTypeInfo` trait to get signature and extensions info separately, and support being given an extension version in the latter.
- Remove support for V5 signed extrinsics, which are no longer a thing (see [#3685](https://github.com/paritytech/polkadot-sdk/pull/3685) for context).

## 0.3.0 (2024-09-30)

- Fix `extrinsic.call_range()` and `extensions.range()` functions, and clarify descriptions. 
- Add `extrinsic.call_args_range()` to return the call data arguments.

## 0.2.0 (2024-09-30)

- Consistify helper functions; have `list_storage_entries{_any}` and `type_registry_from_metadata{_any}`, where
  the `any` versions take a `RuntimeMetadata` enum and the others take the specific metadata versions contained within.
- Improve the top level docs.

## 0.1.0 (2024-09-27)

Initial release.
