//! Offline storage coverage and guarded substitution candidates. Metadata proves
//! encoding, not economic meaning: every present prefix needs a reviewed semantic
//! disposition, and this module never declares a production-ready release.

use crate::{hex_bytes, typed, Result};
use codec::{Decode, Encode};
use common::recovery::{GuardedStorageEdit, StorageScope};
use frame_metadata::{v14, RuntimeMetadata, RuntimeMetadataPrefixed};
use scale_info::{form::PortableForm, PortableRegistry, TypeDef, TypeDefPrimitive};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use sp_core::storage::Storage;
use std::collections::{BTreeMap, BTreeSet};

#[path = "legacy_checkpoint.rs"]
mod legacy_checkpoint;

type Accounts = BTreeSet<[u8; 32]>;
type Mappings = BTreeMap<[u8; 32], [u8; 32]>;
type PalletStorageSchemas = Vec<(String, Option<v14::PalletStorageMetadata<PortableForm>>)>;
fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Hasher {
    Blake2_128,
    Blake2_256,
    Blake2_128Concat,
    Twox128,
    Twox256,
    Twox64Concat,
    Identity,
}
impl From<v14::StorageHasher> for Hasher {
    fn from(value: v14::StorageHasher) -> Self {
        match value {
            v14::StorageHasher::Blake2_128 => Self::Blake2_128,
            v14::StorageHasher::Blake2_256 => Self::Blake2_256,
            v14::StorageHasher::Blake2_128Concat => Self::Blake2_128Concat,
            v14::StorageHasher::Twox128 => Self::Twox128,
            v14::StorageHasher::Twox256 => Self::Twox256,
            v14::StorageHasher::Twox64Concat => Self::Twox64Concat,
            v14::StorageHasher::Identity => Self::Identity,
        }
    }
}
impl Hasher {
    fn hash_len(self) -> usize {
        match self {
            Self::Blake2_128 | Self::Blake2_128Concat | Self::Twox128 => 16,
            Self::Blake2_256 | Self::Twox256 => 32,
            Self::Twox64Concat => 8,
            Self::Identity => 0,
        }
    }
    fn reversible(self) -> bool {
        matches!(
            self,
            Self::Blake2_128Concat | Self::Twox64Concat | Self::Identity
        )
    }
    pub(crate) fn apply(self, bytes: &[u8]) -> Vec<u8> {
        let mut result = match self {
            Self::Blake2_128 | Self::Blake2_128Concat => sp_io::hashing::blake2_128(bytes).to_vec(),
            Self::Blake2_256 => sp_io::hashing::blake2_256(bytes).to_vec(),
            Self::Twox128 => sp_io::hashing::twox_128(bytes).to_vec(),
            Self::Twox256 => sp_io::hashing::twox_256(bytes).to_vec(),
            Self::Twox64Concat => sp_io::hashing::twox_64(bytes).to_vec(),
            Self::Identity => Vec::new(),
        };
        if self.reversible() {
            result.extend_from_slice(bytes);
        }
        result
    }
}

#[derive(Clone, Debug)]
pub struct Descriptor {
    pub trie: String,
    pub name: String,
    pub prefix: Vec<u8>,
    pub key_types: Vec<u32>,
    pub hashers: Vec<Hasher>,
    pub value_type: u32,
}

#[derive(Clone)]
pub struct MetadataSchema {
    pub registry: PortableRegistry,
    pub descriptors: Vec<Descriptor>,
    pub metadata_hash: String,
}

impl MetadataSchema {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut input = bytes;
        let metadata = RuntimeMetadataPrefixed::decode(&mut input).map_err(|e| e.to_string())?;
        if !input.is_empty()
            || metadata.0 != frame_metadata::META_RESERVED
            || metadata.encode() != bytes
        {
            return Err("runtime metadata did not roundtrip exactly".into());
        }
        let (registry, pallets): (_, PalletStorageSchemas) = match metadata.1 {
            RuntimeMetadata::V14(metadata) => (
                metadata.types,
                metadata
                    .pallets
                    .into_iter()
                    .map(|p| (p.name, p.storage))
                    .collect(),
            ),
            RuntimeMetadata::V15(metadata) => (
                metadata.types,
                metadata
                    .pallets
                    .into_iter()
                    .map(|p| (p.name, p.storage))
                    .collect(),
            ),
            _ => {
                return Err(
                    "only explicitly supported portable metadata V14/V15 can inventory storage"
                        .into(),
                )
            }
        };
        let mut descriptors = Vec::new();
        let u16_type = registry
            .types
            .iter()
            .find(|t| matches!(t.ty.type_def, TypeDef::Primitive(TypeDefPrimitive::U16)))
            .map(|t| t.id)
            .ok_or("metadata lacks native u16 storage-version type")?;
        for (pallet, storage) in pallets {
            // FRAME's StorageVersion key is not exposed as a pallet storage
            // entry. Its prefix and u16 codec are fixed by frame-support's
            // traits/metadata.rs STORAGE_VERSION_STORAGE_KEY_POSTFIX.
            let mut version_prefix = sp_io::hashing::twox_128(pallet.as_bytes()).to_vec();
            version_prefix.extend_from_slice(&sp_io::hashing::twox_128(b":__STORAGE_VERSION__:"));
            descriptors.push(Descriptor {
                trie: "top".into(),
                name: format!("{pallet}.__STORAGE_VERSION__"),
                prefix: version_prefix,
                key_types: vec![],
                hashers: vec![],
                value_type: u16_type,
            });
            let Some(storage) = storage else {
                continue;
            };
            for entry in storage.entries {
                let mut prefix = sp_io::hashing::twox_128(storage.prefix.as_bytes()).to_vec();
                prefix.extend_from_slice(&sp_io::hashing::twox_128(entry.name.as_bytes()));
                let (key_types, hashers, value_type) = match entry.ty {
                    v14::StorageEntryType::Plain(value) => (vec![], vec![], value.id),
                    v14::StorageEntryType::Map {
                        hashers,
                        key,
                        value,
                    } => {
                        if hashers.is_empty() {
                            return Err("map metadata has no hashers".into());
                        }
                        let key_types = if hashers.len() == 1 {
                            vec![key.id]
                        } else {
                            match &registry
                                .resolve(key.id)
                                .ok_or("map key type missing")?
                                .type_def
                            {
                                TypeDef::Tuple(tuple) if tuple.fields.len() == hashers.len() => {
                                    tuple.fields.iter().map(|t| t.id).collect()
                                }
                                _ => {
                                    return Err(
                                        "multi-key map metadata does not match its hashers".into()
                                    )
                                }
                            }
                        };
                        (
                            key_types,
                            hashers.into_iter().map(Into::into).collect(),
                            value.id,
                        )
                    }
                };
                descriptors.push(Descriptor {
                    trie: "top".into(),
                    name: format!("{pallet}.{}", entry.name),
                    prefix,
                    key_types,
                    hashers,
                    value_type,
                });
            }
        }
        validate_descriptors(&descriptors, &registry)?;
        Ok(Self {
            registry,
            descriptors,
            metadata_hash: hx(&sp_io::hashing::blake2_256(bytes)),
        })
    }
}

/// Diagnostic schemas for legacy/child storage; type IDs refer to the pinned
/// registry. Their presence supplies encoding only, never semantic approval.
/// Production composition requires a built-in source-pinned legacy adapter.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AdditionalStorage {
    /// `top`, or the full :child_storage:default: key as lowercase hex.
    pub trie: String,
    pub name: String,
    pub prefix: String,
    pub key_types: Vec<u32>,
    pub hashers: Vec<Hasher>,
    pub value_type: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    MutableOwnership,
    RetainHistorical,
    /// Requires a pallet-specific bookkeeping cancellation adapter. This tool
    /// deliberately does not delete pending entries or invent nominal refunds.
    CancelPending,
    Manual,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StorageDisposition {
    pub trie: String,
    pub storage: String,
    pub action: Disposition,
    pub reason: String,
    pub evidence_hash: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct KeyPreimage {
    pub trie: String,
    pub key: String,
    /// One exact SCALE preimage per map component, including reversible ones.
    pub components: Vec<String>,
}

/// Admission for immutable opaque artifacts, never mutable ownership records.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OpaqueRetention {
    pub trie: String,
    pub key: String,
    pub value_hash: String,
    pub evidence_hash: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InventoryOptions {
    /// Source-proven removed codecs, accepted only for the exact executed
    /// checkpoint registry. This never supplies semantic authorization.
    #[serde(default)]
    pub include_pinned_checkpoint_legacy: bool,
    #[serde(default)]
    pub additional_storage: Vec<AdditionalStorage>,
    #[serde(default)]
    pub dispositions: Vec<StorageDisposition>,
    #[serde(default)]
    pub key_preimages: Vec<KeyPreimage>,
    #[serde(default)]
    pub opaque_retentions: Vec<OpaqueRetention>,
    /// Canonical AccountId32 or H160 bytes to locate without assuming ownership.
    #[serde(default)]
    pub watched_accounts: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefixReport {
    pub trie: String,
    pub storage: String,
    pub prefix: String,
    pub entries: usize,
    pub decoded_entries: usize,
    pub affected_entries: usize,
    pub adapter_resolved_entries: usize,
    pub disposition: Option<Disposition>,
    pub issues: BTreeMap<String, usize>,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterId {
    StakingPayeeV1,
    BridgeMultisigCancellationV1,
    GovernanceV1,
    ReferralSuspensionV1,
    StakingControlV1,
    SessionControlV1,
    PermissionControlV1,
    IrohaPendingCancellationV1,
    StakingHistoryV1,
    DemocracyV1,
    SchedulerCancellationV1,
    StakingRolesV1,
    BridgeMultisigMembershipV1,
    ElectionsV1,
    OffencesV1,
    BridgeProxyHistoryV1,
    HashiCancellationV1,
    HashiIncomingOwnershipV1,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionAction {
    ReplaceControl,
    CancelPending,
}

/// Constructed by reviewed typed adapters, never deserialized from user JSON.
/// A resolution applies to one captured key and its exact compiler-produced
/// edits, not every row sharing a pallet or prefix.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterResolution {
    pub storage: String,
    pub key: String,
    pub expected_before_hash: String,
    pub expected_after_hash: Option<String>,
    pub adapter: AdapterId,
    pub action: ResolutionAction,
    #[serde(skip)]
    pub(crate) validated_edits: Vec<GuardedStorageEdit>,
    #[serde(skip)]
    pub(crate) observed_accounts: BTreeSet<[u8; 32]>,
}
impl PrefixReport {
    fn issue(&mut self, issue: impl Into<String>, key: &[u8]) {
        *self.issues.entry(issue.into()).or_default() += 1;
        if self.examples.len() < 8 {
            self.examples.push(hx(key));
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardedCandidate {
    pub trie: String,
    pub storage: String,
    pub source_key: String,
    pub destination_key: String,
    pub expected_source_value_hash: String,
    pub require_destination_absent: bool,
    pub replacement_value: String,
    pub disposition_evidence_hash: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryReport {
    pub schema: &'static str,
    pub metadata_hash: String,
    pub top_entries: usize,
    pub child_entries: usize,
    pub child_tries: usize,
    pub all_entries_decoded: bool,
    pub all_entries_classified: bool,
    pub opaque_retained_entries: usize,
    pub semantic_dispositions_complete: bool,
    /// False whenever any opaque/unknown storage remains, even if no matching
    /// AccountId happened to be visible in the decodable portion.
    pub unused_destinations_verified: bool,
    pub used_destinations: BTreeMap<String, String>,
    pub watched_accounts: Vec<String>,
    pub watched_references: Vec<WatchedStorageReference>,
    pub prefixes: Vec<PrefixReport>,
    pub candidates: Vec<GuardedCandidate>,
    pub production_ready: bool,
    pub remaining: &'static str,
    #[serde(skip)]
    native_watch: NativeWatchCertificate,
}

/// Constructed only during this module's complete native traversal. Public JSON
/// reports cannot be promoted back into this in-memory authority.
#[derive(Debug)]
struct NativeWatchCertificate {
    report_hash: [u8; 32],
    state_sha256: [u8; 32],
    schema_sha256: [u8; 32],
    registry: BTreeSet<Vec<u8>>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeWatchSummary {
    pub inventory_report_sha256: String,
    pub complete_storage_sha256: String,
    pub decoding_schema_sha256: String,
    pub top_entries: usize,
    pub child_entries: usize,
    pub child_tries: usize,
    pub opaque_retained_entries: usize,
}
impl InventoryReport {
    pub(crate) fn require_native_watch(
        &self,
        identities: &BTreeSet<Vec<u8>>,
    ) -> Result<NativeWatchSummary> {
        let digest: [u8; 32] =
            Sha256::digest(serde_json::to_vec(self).map_err(|e| e.to_string())?).into();
        if digest != self.native_watch.report_hash
            || !identities.is_subset(&self.native_watch.registry)
            || !self.all_entries_classified
            || !self.semantic_dispositions_complete
            || self.watched_references.iter().any(|record| {
                record.key_decoding_error.is_some() || record.value_decoding_error.is_some()
            })
        {
            return Err("attacker coverage requires the intact complete native typed inventory and registry".into());
        }
        Ok(NativeWatchSummary {
            inventory_report_sha256: hx(&digest),
            complete_storage_sha256: hx(&self.native_watch.state_sha256),
            decoding_schema_sha256: hx(&self.native_watch.schema_sha256),
            top_entries: self.top_entries,
            child_entries: self.child_entries,
            child_tries: self.child_tries,
            opaque_retained_entries: self.opaque_retained_entries,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchedStorageReference {
    pub trie: String,
    pub storage: String,
    pub key: String,
    pub key_decoded: Vec<serde_json::Value>,
    pub value_hash: String,
    pub value_scale: Option<String>,
    pub value_decoded: serde_json::Value,
    pub key_decoding_error: Option<String>,
    pub value_decoding_error: Option<String>,
    pub references: Vec<typed::IdentityReference>,
    pub interpretation: &'static str,
}

fn key_references(
    key: &[u8],
    descriptor: &Descriptor,
    schema: &MetadataSchema,
    preimages: Option<&[Vec<u8>]>,
    watched: &BTreeSet<Vec<u8>>,
) -> Result<(Vec<typed::IdentityReference>, Vec<Vec<u8>>)> {
    // Verify all hash material and trailing bytes before reporting provenance.
    transform_key(
        key,
        descriptor,
        &schema.registry,
        &BTreeMap::new(),
        preimages,
    )?;
    let mut input = &key[descriptor.prefix.len()..];
    let mut found = Vec::new();
    let mut components = Vec::new();
    for (index, (hasher, type_id)) in descriptor
        .hashers
        .iter()
        .zip(&descriptor.key_types)
        .enumerate()
    {
        let original = if hasher.reversible() {
            let bytes = &input[hasher.hash_len()..];
            let (_, _, consumed) =
                typed::transform_prefix(bytes, *type_id, &schema.registry, &BTreeMap::new())?;
            let original = bytes[..consumed].to_vec();
            input = &input[hasher.hash_len() + consumed..];
            original
        } else {
            input = &input[hasher.hash_len()..];
            preimages.ok_or("opaque key preimage missing")?[index].clone()
        };
        for mut reference in
            typed::watched_references(&original, *type_id, &schema.registry, watched)?
        {
            reference.path.insert(0, format!("key[{index}]"));
            found.push(reference);
        }
        components.push(original);
    }
    Ok((found, components))
}

fn watch_record(
    trie: &str,
    key: &[u8],
    value: &[u8],
    descriptor: &Descriptor,
    schema: &MetadataSchema,
    preimages: Option<&[Vec<u8>]>,
    watched: &BTreeSet<Vec<u8>>,
) -> Result<Option<WatchedStorageReference>> {
    let key_result = key_references(key, descriptor, schema, preimages, watched);
    let request_index = validated_request_index(schema, descriptor, value);
    let legacy_preimage = legacy_checkpoint::preimage(schema, descriptor, value)?;
    let value_result = match &request_index {
        Ok(Some(_)) => Ok(Vec::new()),
        Err(error) => Err(error.clone()),
        Ok(None) if legacy_preimage.is_some() => {
            let mut references = Vec::new();
            if let Some((_, provider)) = legacy_preimage.as_ref().and_then(|p| p.provider) {
                if watched.contains(provider.as_slice()) {
                    references.push(typed::IdentityReference {
                        identity: hx(&provider),
                        kind: "account_id32",
                        type_id: 0,
                        type_path: "sp_core::crypto::AccountId32".into(),
                        path: vec!["Available".into(), "provider".into()],
                    });
                }
            }
            Ok(references)
        }
        Ok(None) => {
            typed::watched_references(value, descriptor.value_type, &schema.registry, watched)
        }
    };
    let key_decoding_error = key_result.as_ref().err().cloned();
    let value_decoding_error = value_result.as_ref().err().cloned();
    let (mut references, components) = key_result.unwrap_or_default();
    // A provided identity is itself an exact typed preimage for an opaque
    // AccountId component. This targeted check cannot recover unrelated owners
    // or excuse full-map coverage, but hash-only keys must not hide a hit.
    let mut offset = descriptor.prefix.len();
    for (index, (hasher, type_id)) in descriptor
        .hashers
        .iter()
        .zip(&descriptor.key_types)
        .enumerate()
    {
        if !hasher.reversible() {
            let end = offset + hasher.hash_len();
            if end > key.len() {
                break;
            }
            if typed::account_type(&schema.registry, *type_id) {
                for identity in watched.iter().filter(|a| a.len() == 32) {
                    if hasher.apply(identity) == key[offset..end] {
                        references.push(typed::IdentityReference {
                            identity: hx(identity),
                            kind: "account_id32_hashed_key_preimage_match",
                            type_id: *type_id,
                            type_path: schema
                                .registry
                                .resolve(*type_id)
                                .ok_or("watched key type missing")?
                                .path
                                .segments
                                .join("::"),
                            path: vec![format!("key[{index}]")],
                        });
                    }
                }
            }
            offset = end;
        } else {
            let start = offset + hasher.hash_len();
            if start > key.len() {
                break;
            }
            match typed::transform_prefix(
                &key[start..],
                *type_id,
                &schema.registry,
                &BTreeMap::new(),
            ) {
                Ok((_, _, consumed)) => offset = start + consumed,
                Err(_) => break,
            }
        }
    }
    for mut reference in value_result.unwrap_or_default() {
        reference.path.insert(0, "value".into());
        references.push(reference);
    }
    if references.is_empty() && key_decoding_error.is_none() && value_decoding_error.is_none() {
        return Ok(None);
    }
    let key_decoded = components
        .iter()
        .zip(&descriptor.key_types)
        .map(|(bytes, id)| typed::decoded_json(bytes, *id, &schema.registry))
        .collect::<Result<_>>()?;
    let value_decoded = if let Some(preimage) = legacy_preimage {
        preimage.summary
    } else if let Ok(Some(count)) = request_index {
        serde_json::json!({"retainedRequestReferences":count,"elementType":"(u32,H256)","interpretation":"Request hashes are preserved, never reinterpreted as account ownership."})
    } else if value_decoding_error.is_none() {
        typed::decoded_json(value, descriptor.value_type, &schema.registry)?
    } else {
        serde_json::Value::Null
    };
    Ok(Some(WatchedStorageReference { trie: trie.into(), storage: descriptor.name.clone(), key: hx(key), key_decoded, value_hash: hx(&sp_io::hashing::blake2_256(value)), value_scale: (value.len()<=typed::MAX_TYPED_BYTES).then(||hx(value)), value_decoded, key_decoding_error, value_decoding_error, references, interpretation: "Typed identity reference only; economic ownership/control classification requires the prefix-specific reviewed adapter." }))
}

/// The deployed HASHI account index contains only fixed-width network IDs and
/// request hashes. Validate its full SCALE shape without allocating 32 Value
/// nodes per hash. The AccountId map KEY is still decoded/watched/remapped.
fn validated_request_index(
    schema: &MetadataSchema,
    descriptor: &Descriptor,
    value: &[u8],
) -> Result<Option<u32>> {
    if descriptor.trie != "top" || descriptor.name != "EthBridge.AccountRequests" {
        return Ok(None);
    }
    let registry = &schema.registry;
    let sequence = match registry.resolve(descriptor.value_type).map(|t| &t.type_def) {
        Some(TypeDef::Sequence(s)) => s,
        _ => return Err("AccountRequests no longer has reviewed sequence codec".into()),
    };
    let tuple = match registry
        .resolve(sequence.type_param.id)
        .map(|t| &t.type_def)
    {
        Some(TypeDef::Tuple(t)) if t.fields.len() == 2 => t,
        _ => return Err("AccountRequests element is not reviewed pair".into()),
    };
    if !matches!(
        registry.resolve(tuple.fields[0].id).map(|t| &t.type_def),
        Some(TypeDef::Primitive(TypeDefPrimitive::U32))
    ) {
        return Err("AccountRequests network ID codec differs".into());
    }
    let hash = registry
        .resolve(tuple.fields[1].id)
        .ok_or("AccountRequests hash type missing")?;
    if !hash.path.segments.last().is_some_and(|p| p == "H256") {
        return Err("AccountRequests value could contain mutable ownership".into());
    }
    let array_id = match &hash.type_def {
        TypeDef::Composite(c) if c.fields.len() == 1 => c.fields[0].ty.id,
        _ => return Err("AccountRequests hash wrapper differs".into()),
    };
    let array = match registry.resolve(array_id).map(|t| &t.type_def) {
        Some(TypeDef::Array(a)) if a.len == 32 => a,
        _ => return Err("AccountRequests hash width differs".into()),
    };
    if !matches!(
        registry.resolve(array.type_param.id).map(|t| &t.type_def),
        Some(TypeDef::Primitive(TypeDefPrimitive::U8))
    ) {
        return Err("AccountRequests hash bytes differ".into());
    }
    if value.len() > 16 * 1024 * 1024 {
        return Err("AccountRequests exceeds bounded 16 MiB streaming codec".into());
    }
    let mut input = value;
    let count = codec::Compact::<u32>::decode(&mut input).map_err(|e| e.to_string())?;
    let prefix = count.encode();
    if !value.starts_with(&prefix)
        || value.len() - input.len() != prefix.len()
        || input.len() != count.0 as usize * 36
    {
        return Err("AccountRequests SCALE length is noncanonical or truncated".into());
    }
    Ok(Some(count.0))
}

fn byte_vector_type(registry: &PortableRegistry, id: u32, depth: usize) -> bool {
    if depth > 16 {
        return false;
    }
    match registry.resolve(id).map(|t| &t.type_def) {
        Some(TypeDef::Sequence(sequence)) => matches!(
            registry
                .resolve(sequence.type_param.id)
                .map(|t| &t.type_def),
            Some(TypeDef::Primitive(TypeDefPrimitive::U8))
        ),
        Some(TypeDef::Composite(composite)) if composite.fields.len() == 1 => {
            byte_vector_type(registry, composite.fields[0].ty.id, depth + 1)
        }
        _ => false,
    }
}

fn safely_retained_opaque(
    schema: &MetadataSchema,
    descriptor: Option<&Descriptor>,
    key: &[u8],
    value: &[u8],
) -> Result<bool> {
    if key == b":code" {
        use sha2::{Digest as _, Sha256};
        return Ok(hex::encode(Sha256::digest(value))
            == "de7173a9e0137a32265b341354383958d85beef81103571e6b0aa09622ef2221");
    }
    let Some(descriptor) = descriptor else {
        return Ok(false);
    };
    if descriptor.trie != "top"
        || descriptor.name != "Preimage.PreimageFor"
        || !byte_vector_type(&schema.registry, descriptor.value_type, 0)
    {
        return Ok(false);
    }
    let (_, accounts) = transform_key(key, descriptor, &schema.registry, &BTreeMap::new(), None)?;
    if !accounts.is_empty() {
        return Ok(false);
    }
    let mut input = value;
    let count = codec::Compact::<u32>::decode(&mut input)
        .map_err(|e| e.to_string())?
        .0 as usize;
    Ok(input.len() == count)
}

fn validate_descriptors(descriptors: &[Descriptor], registry: &PortableRegistry) -> Result<()> {
    let mut names = BTreeSet::new();
    let mut prefixes: BTreeMap<&str, Vec<&[u8]>> = BTreeMap::new();
    for descriptor in descriptors {
        if descriptor.name.is_empty()
            || descriptor.prefix.is_empty()
            || descriptor.key_types.len() != descriptor.hashers.len()
            || !names.insert((&descriptor.trie, &descriptor.name))
        {
            return Err("invalid or duplicate storage descriptor".into());
        }
        if descriptor.trie != "top"
            && !hex_bytes(&descriptor.trie)?.starts_with(b":child_storage:default:")
        {
            return Err("additional descriptor has unsupported trie identity".into());
        }
        let seen = prefixes.entry(&descriptor.trie).or_default();
        if seen.iter().any(|prefix| {
            descriptor.prefix.starts_with(prefix) || prefix.starts_with(&descriptor.prefix)
        }) {
            return Err("ambiguous or colliding storage prefixes".into());
        }
        seen.push(&descriptor.prefix);
        for id in descriptor
            .key_types
            .iter()
            .chain(std::iter::once(&descriptor.value_type))
        {
            if registry.resolve(*id).is_none() {
                return Err("storage descriptor references missing type".into());
            }
        }
    }
    Ok(())
}

/// Reconstruct every original hasher output before rehashing transformed keys.
/// Hash-only encodings require explicit authenticated preimages, even when the
/// operator expects that no mapped AccountId appears in that key.
pub fn transform_key(
    key: &[u8],
    descriptor: &Descriptor,
    registry: &PortableRegistry,
    mappings: &Mappings,
    preimages: Option<&[Vec<u8>]>,
) -> Result<(Vec<u8>, Accounts)> {
    let mut input = key
        .strip_prefix(descriptor.prefix.as_slice())
        .ok_or("storage prefix mismatch")?;
    if descriptor.hashers.len() != descriptor.key_types.len() {
        return Err("key hasher/type count mismatch".into());
    }
    if preimages.is_some_and(|p| p.len() != descriptor.hashers.len()) {
        return Err("key preimage count mismatch".into());
    }
    let mut transformed = descriptor.prefix.clone();
    let mut accounts = BTreeSet::new();
    for (index, (hasher, type_id)) in descriptor
        .hashers
        .iter()
        .zip(&descriptor.key_types)
        .enumerate()
    {
        let hash_len = hasher.hash_len();
        if input.len() < hash_len {
            return Err("truncated map hash".into());
        }
        let (original, replacement, found, consumed) = if hasher.reversible() {
            let bytes = &input[hash_len..];
            let (replacement, found, consumed) =
                typed::transform_prefix(bytes, *type_id, registry, mappings)?;
            let original = bytes[..consumed].to_vec();
            if preimages.is_some_and(|p| p[index] != original) {
                return Err("provided reversible preimage disagrees".into());
            }
            (original, replacement, found, hash_len + consumed)
        } else {
            let original = preimages
                .ok_or("opaque hash-only map key requires explicit preimage")?[index]
                .clone();
            let (replacement, found) =
                typed::transform_value(&original, *type_id, registry, mappings)?;
            (original, replacement, found, hash_len)
        };
        if hasher.apply(&original) != input[..consumed] {
            return Err("map key hash/preimage mismatch".into());
        }
        input = &input[consumed..];
        transformed.extend_from_slice(&hasher.apply(&replacement));
        accounts.extend(found);
    }
    if !input.is_empty() {
        return Err("trailing bytes in storage key".into());
    }
    Ok((transformed, accounts))
}

fn type_contains_account(
    registry: &PortableRegistry,
    id: u32,
    visited: &mut BTreeSet<u32>,
) -> bool {
    if typed::account_type(registry, id) {
        return true;
    }
    if !visited.insert(id) {
        return false;
    }
    let Some(ty) = registry.resolve(id) else {
        return true;
    };
    let children: Vec<_> = match &ty.type_def {
        TypeDef::Composite(c) => c.fields.iter().map(|f| f.ty.id).collect(),
        TypeDef::Variant(v) => v
            .variants
            .iter()
            .flat_map(|v| v.fields.iter().map(|f| f.ty.id))
            .collect(),
        TypeDef::Sequence(s) => vec![s.type_param.id],
        TypeDef::Array(a) => vec![a.type_param.id],
        TypeDef::Tuple(t) => t.fields.iter().map(|f| f.id).collect(),
        TypeDef::Compact(c) => vec![c.type_param.id],
        _ => vec![],
    };
    children
        .into_iter()
        .any(|child| type_contains_account(registry, child, visited))
}

pub(crate) fn sensitive(descriptor: &Descriptor, registry: &PortableRegistry) -> bool {
    let name = descriptor.name.to_ascii_lowercase();
    // Scalar AccountIds can carry grants/payout instructions just as sequences
    // and signed calls do. A generic typed move must not preserve these merely
    // because the compromise's target is not itself in the replacement CSV.
    if matches!(
        name.as_str(),
        "staking.payee"
            | "staking.bonded"
            | "staking.ledger"
            | "referrals.referrers"
            | "referralsystem.referrers"
            | "democracy.votingof"
            | "democracy.voting"
            | "convictionvoting.votingfor"
    ) {
        return true;
    }
    if [
        "historical",
        "keyowner",
        "grandpa",
        "babe",
        "session",
        "multisig",
        "scheduler",
        "preimage",
        "request",
        "approv",
        "proposal",
        "referend",
        "agenda",
        "membership",
        "members",
        "council",
        "technical",
        "nominations",
        "validators",
        "signatories",
    ]
    .iter()
    .any(|word| name.contains(word))
    {
        return true;
    }
    // Farming.PoolFarmers is an unordered roster of position owners: the pallet
    // only looks farmers up with a linear find and rebuilds the list on every
    // refresh, so replacing an owner in place cannot change its meaning.
    let unordered_owner_list = name == "farming.poolfarmers";
    // Calls/signatures nested in otherwise innocent-looking structures still
    // require an authorization-aware adapter, not recursive byte replacement.
    // BTreeMap/BTreeSet encode as sequences, so the sequence rule below still
    // flags any ordered collection whose elements contain an account; ones
    // without accounts move unchanged with their key.
    let mut pending = vec![descriptor.value_type];
    let mut visited = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        let Some(ty) = registry.resolve(id) else {
            return true;
        };
        if ty.path.segments.last().is_some_and(|name| {
            name.ends_with("Call") || name.ends_with("Signature") || name.contains("Extrinsic")
        }) {
            return true;
        }
        match &ty.type_def {
            TypeDef::Composite(c) => pending.extend(c.fields.iter().map(|f| f.ty.id)),
            TypeDef::Variant(v) => pending.extend(
                v.variants
                    .iter()
                    .flat_map(|v| v.fields.iter().map(|f| f.ty.id)),
            ),
            TypeDef::Sequence(s) => {
                if !unordered_owner_list
                    && type_contains_account(registry, s.type_param.id, &mut BTreeSet::new())
                {
                    return true;
                }
                pending.push(s.type_param.id);
            }
            TypeDef::Array(a) => pending.push(a.type_param.id),
            TypeDef::Tuple(t) => pending.extend(t.fields.iter().map(|f| f.id)),
            TypeDef::Compact(c) => pending.push(c.type_param.id),
            _ => {}
        }
    }
    false
}

/// Inputs must come from the independently verified complete capture. The
/// report is a review artifact; candidates are not runtime chunks or approval.
pub fn inventory(
    schema: &MetadataSchema,
    storage: &Storage,
    mappings: &Mappings,
    options: &InventoryOptions,
) -> Result<InventoryReport> {
    inventory_with_resolutions(schema, storage, mappings, options, &[], &[])
}

pub fn inventory_with_resolutions(
    schema: &MetadataSchema,
    storage: &Storage,
    mappings: &Mappings,
    options: &InventoryOptions,
    resolutions: &[AdapterResolution],
    adapter_edits: &[GuardedStorageEdit],
) -> Result<InventoryReport> {
    let legacy_schema = options
        .include_pinned_checkpoint_legacy
        .then(|| legacy_checkpoint::apply(schema))
        .transpose()?;
    let schema = legacy_schema.as_ref().unwrap_or(schema);
    let destinations: BTreeSet<_> = mappings.values().copied().collect();
    let watched: BTreeSet<_> = options
        .watched_accounts
        .iter()
        .map(|account| {
            let bytes = hex_bytes(account)?;
            if ![20, 32].contains(&bytes.len()) {
                return Err("watch identity must be a canonical AccountId32 or H160".into());
            }
            Ok(bytes)
        })
        .collect::<Result<_>>()?;
    if watched.len() != options.watched_accounts.len() {
        return Err("duplicate watched identity".into());
    }
    let mut watched_records = Vec::new();
    if destinations.len() != mappings.len() || mappings.keys().any(|old| destinations.contains(old))
    {
        return Err("non-injective or overlapping replacement mapping".into());
    }
    let mut descriptors = schema.descriptors.clone();
    let diagnostic_schemas: BTreeSet<_> = options
        .additional_storage
        .iter()
        .map(|extra| (extra.trie.clone(), extra.name.clone()))
        .collect();
    for extra in &options.additional_storage {
        descriptors.push(Descriptor {
            trie: extra.trie.clone(),
            name: extra.name.clone(),
            prefix: hex_bytes(&extra.prefix)?,
            key_types: extra.key_types.clone(),
            hashers: extra.hashers.clone(),
            value_type: extra.value_type,
        });
    }
    validate_descriptors(&descriptors, &schema.registry)?;
    let mut resolved = BTreeMap::new();
    for resolution in resolutions {
        let key = hex_bytes(&resolution.key)?;
        let allowed = match resolution.adapter {
            AdapterId::PermissionControlV1 => {
                crate::permissions_controls::ITEMS.contains(&resolution.storage.as_str())
                    && resolution.action == ResolutionAction::ReplaceControl
            }
            AdapterId::IrohaPendingCancellationV1 => match resolution.storage.as_str() {
                "IrohaMigration.PendingMultiSigAccounts" => {
                    resolution.action == ResolutionAction::CancelPending
                        && resolution.expected_after_hash.is_none()
                }
                "IrohaMigration.PublicKeys" => {
                    resolution.action == ResolutionAction::ReplaceControl
                        && resolution.expected_after_hash.is_some()
                }
                _ => false,
            },
            AdapterId::SessionControlV1 => {
                crate::session_controls::ITEMS.contains(&resolution.storage.as_str())
                    && resolution.action == ResolutionAction::ReplaceControl
            }
            AdapterId::HashiIncomingOwnershipV1 => {
                resolution.action == ResolutionAction::ReplaceControl
                    && match resolution.storage.as_str() {
                        "EthBridge.Requests" => {
                            let mut marker = b":sora-nexus:hashi-incoming-ownership:v1:".to_vec();
                            if key.len() == 76 {
                                marker.extend_from_slice(&key[40..]);
                            }
                            key.len() == 76
                                && resolution.expected_after_hash.as_ref()
                                    == Some(&resolution.expected_before_hash)
                                && resolution.validated_edits.iter().any(|edit| {
                                    edit.scope == StorageScope::Top
                                        && edit.key == marker
                                        && edit.before.is_none()
                                        && edit
                                            .after
                                            .as_ref()
                                            .is_some_and(|value| value.starts_with(b"snxhown1"))
                                })
                        }
                        "EthBridge.AccountRequests" => resolution.expected_after_hash.is_none(),
                        _ => false,
                    }
            }
            AdapterId::HashiCancellationV1 => {
                resolution.action == ResolutionAction::CancelPending
                    && match resolution.storage.as_str() {
                        "EthBridge.Requests" | "EthBridge.RequestApprovers" => {
                            // Signed request/approval evidence is immutable.
                            // Only the exact native identity's permanent marker
                            // can justify retaining its old control references.
                            let mut marker = b":sora-nexus:historical-hashi-request:v1:".to_vec();
                            if key.len() == 76 {
                                marker.extend_from_slice(&key[40..]);
                            }
                            key.len() == 76
                                && resolution.expected_after_hash.as_ref()
                                    == Some(&resolution.expected_before_hash)
                                && resolution.validated_edits.iter().any(|edit| {
                                    edit.scope == StorageScope::Top
                                        && edit.key == marker
                                        && edit.before.is_none()
                                        && edit
                                            .after
                                            .as_ref()
                                            .is_some_and(|value| value.starts_with(b"snxhash1"))
                                })
                        }
                        // The native compiler separately proves complete
                        // same-author cancelled references and certifies the
                        // final paired move plus every required marker.
                        "EthBridge.AccountRequests" => resolution.expected_after_hash.is_none(),
                        _ => false,
                    }
            }
            AdapterId::BridgeProxyHistoryV1 => {
                ["BridgeProxy.Senders", "BridgeProxy.Transactions"]
                    .contains(&resolution.storage.as_str())
                    && resolution.action == ResolutionAction::ReplaceControl
            }
            AdapterId::ElectionsV1 => {
                crate::elections::ITEMS.contains(&resolution.storage.as_str())
                    && resolution.action == ResolutionAction::ReplaceControl
            }
            AdapterId::OffencesV1 => {
                ["Offences.Reports", "Offences.ConcurrentReportsIndex"]
                    .contains(&resolution.storage.as_str())
                    && resolution.action == ResolutionAction::ReplaceControl
            }
            AdapterId::BridgeMultisigMembershipV1 => {
                ["BridgeMultisig.Accounts", "System.Account"].contains(&resolution.storage.as_str())
                    && resolution.action == ResolutionAction::ReplaceControl
            }
            AdapterId::StakingRolesV1 => {
                [
                    "Staking.Nominators",
                    "Staking.Validators",
                    "Staking.CounterForNominators",
                    "Staking.CounterForValidators",
                    "BagsList.ListNodes",
                    "BagsList.ListBags",
                    "BagsList.CounterForListNodes",
                    "BagsList.__STORAGE_VERSION__",
                ]
                .contains(&resolution.storage.as_str())
                    && resolution.action == ResolutionAction::ReplaceControl
            }
            AdapterId::SchedulerCancellationV1 => {
                ["Scheduler.Agenda", "Scheduler.IncompleteSince"]
                    .contains(&resolution.storage.as_str())
                    && resolution.action == ResolutionAction::CancelPending
                    && resolution.expected_after_hash.is_none()
            }
            AdapterId::DemocracyV1 => {
                resolution.storage == "Democracy.VotingOf"
                    && resolution.action == ResolutionAction::ReplaceControl
            }
            AdapterId::StakingHistoryV1 => {
                resolution
                    .storage
                    .strip_prefix("Staking.")
                    .is_some_and(|item| {
                        [
                            "ErasStakers",
                            "ErasStakersClipped",
                            "ErasStakersOverview",
                            "ErasStakersPaged",
                            "ErasValidatorPrefs",
                            "ErasRewardPoints",
                            "ClaimedRewards",
                            "ValidatorSlashInEra",
                            "NominatorSlashInEra",
                            "SlashingSpans",
                            "SpanSlash",
                            "UnappliedSlashes",
                            "Invulnerables",
                            "ErasTotalStake",
                            "ErasValidatorReward",
                            "ErasStartSessionIndex",
                            "BondedEras",
                        ]
                        .contains(&item)
                    })
                    && resolution.action == ResolutionAction::ReplaceControl
            }
            AdapterId::StakingControlV1 => {
                ["Staking.Bonded", "Staking.Ledger"].contains(&resolution.storage.as_str())
                    && resolution.action == ResolutionAction::ReplaceControl
                    && resolution.expected_after_hash.is_none()
            }
            AdapterId::ReferralSuspensionV1 => {
                [
                    "Referrals.Referrers",
                    "Referrals.Referrals",
                    "System.Account",
                ]
                .contains(&resolution.storage.as_str())
                    && resolution.action == ResolutionAction::ReplaceControl
            }
            AdapterId::StakingPayeeV1 => {
                resolution.storage == "Staking.Payee"
                    && resolution.action == ResolutionAction::ReplaceControl
            }
            AdapterId::BridgeMultisigCancellationV1 => {
                [
                    "BridgeMultisig.Calls",
                    "BridgeMultisig.Multisigs",
                    "BridgeMultisig.CancellationApprovals",
                ]
                .contains(&resolution.storage.as_str())
                    && resolution.action == ResolutionAction::CancelPending
                    && resolution.expected_after_hash.is_none()
            }
            AdapterId::GovernanceV1 => {
                let (pallet, item) = resolution
                    .storage
                    .split_once('.')
                    .ok_or("invalid governance resolution name")?;
                match item {
                    "Members" | "Prime" => {
                        ["Council", "TechnicalCommittee", "TechnicalMembership"].contains(&pallet)
                            && resolution.action == ResolutionAction::ReplaceControl
                            && resolution.expected_after_hash.is_some()
                    }
                    "ProposalOf" | "Voting" => {
                        ["Council", "TechnicalCommittee"].contains(&pallet)
                            && resolution.action == ResolutionAction::CancelPending
                            && resolution.expected_after_hash.is_none()
                    }
                    "Proposals" => {
                        ["Council", "TechnicalCommittee"].contains(&pallet)
                            && resolution.action == ResolutionAction::CancelPending
                            && resolution.expected_after_hash.is_some()
                    }
                    _ => false,
                }
            }
        };
        let descriptor = descriptors
            .iter()
            .find(|d| d.trie == "top" && d.name == resolution.storage)
            .ok_or("resolution storage is unknown")?;
        let before = storage
            .top
            .get(&key)
            .ok_or("resolved key absent from checkpoint")?;
        if !allowed
            || !key.starts_with(&descriptor.prefix)
            || resolution.expected_before_hash != hx(&sp_io::hashing::blake2_256(before))
            || resolution.validated_edits.is_empty()
        {
            return Err("adapter resolution scope/preimage/action mismatch".into());
        }
        let preserved_hashi_evidence = (resolution.adapter == AdapterId::HashiCancellationV1
            && ["EthBridge.Requests", "EthBridge.RequestApprovers"]
                .contains(&resolution.storage.as_str()))
            || (resolution.adapter == AdapterId::HashiIncomingOwnershipV1
                && resolution.storage == "EthBridge.Requests");
        if preserved_hashi_evidence {
            // These signed payloads/approval authors must remain byte-identical.
            // Only their exact native cancellation/ownership marker is an edit. The
            // complete HASHI certificate checks the final storage projection;
            // committed witnesses recheck these preserved bytes before activation.
            // Do not manufacture a no-op write to satisfy a mutation interface.
            if resolution.expected_after_hash.as_ref() != Some(&resolution.expected_before_hash)
                || adapter_edits
                    .iter()
                    .any(|e| e.scope == StorageScope::Top && e.key == key)
            {
                return Err("HASHI evidence must be preserved without source edits".into());
            }
        } else {
            let source = resolution
                .validated_edits
                .iter()
                .find(|e| e.scope == StorageScope::Top && e.key == key)
                .ok_or("resolution lacks exact source edit")?;
            if source.before != Some(sp_io::hashing::blake2_256(before))
                || source
                    .after
                    .as_deref()
                    .map(|a| hx(&sp_io::hashing::blake2_256(a)))
                    != resolution.expected_after_hash
            {
                return Err("resolved effect differs from compiler edit".into());
            }
        }
        for edit in &resolution.validated_edits {
            if edit.scope != StorageScope::Top
                || !adapter_edits.contains(edit)
                || storage
                    .top
                    .get(&edit.key)
                    .map(|v| sp_io::hashing::blake2_256(v))
                    != edit.before
            {
                return Err(
                    "resolution edit missing from validated adapter output or checkpoint".into(),
                );
            }
        }
        if resolved.insert(key, resolution).is_some() {
            return Err("duplicate per-key adapter resolution".into());
        }
    }
    let mut dispositions = BTreeMap::new();
    for review in &options.dispositions {
        if review.reason.trim().is_empty()
            || hex_bytes(&review.evidence_hash)?.len() != 32
            || hex_bytes(&review.evidence_hash)?
                .iter()
                .all(|byte| *byte == 0)
            || dispositions
                .insert((review.trie.clone(), review.storage.clone()), review)
                .is_some()
        {
            return Err("duplicate or unbound semantic disposition".into());
        }
    }
    let mut preimages = BTreeMap::new();
    for item in &options.key_preimages {
        let components = item
            .components
            .iter()
            .map(|p| hex_bytes(p))
            .collect::<Result<Vec<_>>>()?;
        if preimages
            .insert((item.trie.clone(), hex_bytes(&item.key)?), components)
            .is_some()
        {
            return Err("duplicate map-key preimage".into());
        }
    }
    let mut seen_preimages = BTreeSet::new();
    let mut opaque = BTreeMap::new();
    for retained in &options.opaque_retentions {
        if hex_bytes(&retained.value_hash)?.len() != 32
            || hex_bytes(&retained.evidence_hash)?.len() != 32
            || hex_bytes(&retained.evidence_hash)?
                .iter()
                .all(|byte| *byte == 0)
            || opaque
                .insert((retained.trie.clone(), hex_bytes(&retained.key)?), retained)
                .is_some()
        {
            return Err("invalid or duplicate opaque retention".into());
        }
    }
    let mut seen_opaque = BTreeSet::new();
    let mut opaque_retained_entries = 0;
    let mut used_reviews = BTreeSet::new();
    let mut reports: BTreeMap<(String, String), PrefixReport> = BTreeMap::new();
    let mut candidates = Vec::new();
    let mut generated = BTreeSet::new();
    let mut used_destinations = BTreeMap::new();
    let mut all_entries_decoded = true;
    let mut all_entries_classified = true;
    let mut semantic_dispositions_complete = true;
    let mut tries = vec![("top".to_string(), &storage.top)];
    let mut state_digest = Sha256::new();
    state_digest.update(b"sora-nexus/native-storage-watch/v1");
    for (id, child) in &storage.children_default {
        let mut key = b":child_storage:default:".to_vec();
        key.extend_from_slice(id);
        tries.push((hx(&key), &child.data));
    }
    for (trie, entries) in tries {
        state_digest.update((trie.len() as u64).to_le_bytes());
        state_digest.update(trie.as_bytes());
        state_digest.update((entries.len() as u64).to_le_bytes());
        let matching: Vec<_> = descriptors.iter().filter(|d| d.trie == trie).collect();
        for (key, value) in entries {
            state_digest.update((key.len() as u64).to_le_bytes());
            state_digest.update(key);
            state_digest.update((value.len() as u64).to_le_bytes());
            state_digest.update(value);
            let descriptor = matching
                .iter()
                .find(|d| key.starts_with(&d.prefix))
                .copied();
            let name = descriptor
                .map(|d| d.name.clone())
                .unwrap_or_else(|| format!("unknown:{}", hx(&key[..key.len().min(32)])));
            let report_key = (trie.clone(), name.clone());
            let review = dispositions.get(&report_key).copied();
            if review.is_some() {
                used_reviews.insert(report_key.clone());
            }
            let report = reports.entry(report_key).or_insert_with(|| PrefixReport {
                trie: trie.clone(),
                storage: name.clone(),
                prefix: descriptor
                    .map(|d| hx(&d.prefix))
                    .unwrap_or_else(|| hx(&key[..key.len().min(32)])),
                entries: 0,
                decoded_entries: 0,
                affected_entries: 0,
                adapter_resolved_entries: 0,
                disposition: review.map(|r| r.action),
                issues: BTreeMap::new(),
                examples: Vec::new(),
            });
            report.entries += 1;
            if trie == "top" {
                if let Some(resolution) = resolved.get(key) {
                    if opaque.contains_key(&(trie.clone(), key.clone())) {
                        return Err("opaque retention conflicts with adapter resolution".into());
                    }
                    report.adapter_resolved_entries += 1;
                    if resolution.storage == "BridgeMultisig.Calls" {
                        all_entries_decoded = false;
                    }
                    for account in &resolution.observed_accounts {
                        if destinations.contains(account) {
                            used_destinations
                                .entry(hx(account))
                                .or_insert_with(|| format!("{trie}/{name}/{}", hx(key)));
                        }
                    }
                    if resolution
                        .observed_accounts
                        .iter()
                        .any(|a| mappings.contains_key(a))
                    {
                        report.affected_entries += 1;
                    }
                    // The typed adapter proves and removes/replaces this exact
                    // authorization row. Other records keep all normal gates.
                    if !watched.is_empty() {
                        if resolution.storage == "BridgeMultisig.Calls" {
                            // Legacy incompatible calls are cancelled, never
                            // interpreted. The adapter proves native envelope
                            // owners; opaque payloads are not asset ownership.
                            let references: Vec<_> = resolution
                                .observed_accounts
                                .iter()
                                .filter(|account| watched.contains(account.as_slice()))
                                .map(|account| typed::IdentityReference {
                                    identity: hx(account),
                                    kind: "adapter_proven_account_id32",
                                    type_id: 0,
                                    type_path: "source-proven BridgeMultisig cancellation envelope"
                                        .into(),
                                    path: vec!["envelope".into()],
                                })
                                .collect();
                            if !references.is_empty() {
                                watched_records.push(WatchedStorageReference {trie:trie.clone(),storage:name.clone(),key:hx(key),key_decoded:vec![],
                                    value_hash:hx(&sp_io::hashing::blake2_256(value)),value_scale:None,value_decoded:serde_json::Value::Null,
                                    key_decoding_error:None,value_decoding_error:None,references,
                                    interpretation:"Source-proven native envelope identity only; incompatible call is cancelled without execution or refund."});
                            }
                        } else if let Some(descriptor) = descriptor {
                            if let Some(found) = watch_record(
                                &trie,
                                key,
                                value,
                                descriptor,
                                schema,
                                preimages
                                    .get(&(trie.clone(), key.clone()))
                                    .map(Vec::as_slice),
                                &watched,
                            )? {
                                watched_records.push(found);
                            }
                        } else {
                            return Err("adapter-resolved ownership watch lacks its source-pinned descriptor".into());
                        }
                    }
                    // A resolved hashed key still consumes its supplied preimage;
                    // preimages for absent keys remain rejected below.
                    let preimage_key = (trie.clone(), key.clone());
                    if preimages.contains_key(&preimage_key) {
                        seen_preimages.insert(preimage_key);
                    }
                    continue;
                }
            }
            if let Some(retained) = opaque.get(&(trie.clone(), key.clone())) {
                if trie != "top"
                    || review.is_none_or(|r| r.action != Disposition::RetainHistorical)
                    || retained.value_hash != hx(&sp_io::hashing::blake2_256(value))
                    || !safely_retained_opaque(schema, descriptor, key, value)?
                {
                    return Err(
                        "opaque retention is not a proven immutable non-ownership artifact".into(),
                    );
                }
                seen_opaque.insert((trie.clone(), key.clone()));
                opaque_retained_entries += 1;
                all_entries_decoded = false;
                continue;
            }
            let Some(descriptor) = descriptor else {
                all_entries_decoded = false;
                all_entries_classified = false;
                semantic_dispositions_complete = false;
                report.issue(
                    "unknown encoding requires explicit legacy/child schema",
                    key,
                );
                continue;
            };
            let preimage_key = (trie.clone(), key.clone());
            let supplied = preimages.get(&preimage_key);
            if supplied.is_some() {
                seen_preimages.insert(preimage_key);
            }
            let decoded = transform_key(
                key,
                descriptor,
                &schema.registry,
                mappings,
                supplied.map(Vec::as_slice),
            )
            .and_then(|(new_key, mut accounts)| {
                legacy_checkpoint::validate_marker(descriptor, value)?;
                let (new_value, value_accounts) =
                    if validated_request_index(schema, descriptor, value)?.is_some() {
                        (value.clone(), BTreeSet::new())
                    } else if let Some(preimage) =
                        legacy_checkpoint::preimage(schema, descriptor, value)?
                    {
                        let mut updated = value.clone();
                        let mut owners = BTreeSet::new();
                        if let Some((offset, owner)) = preimage.provider {
                            owners.insert(owner);
                            if let Some(replacement) = mappings.get(&owner) {
                                updated[offset..offset + 32].copy_from_slice(replacement);
                            }
                        }
                        (updated, owners)
                    } else {
                        typed::transform_value(
                            value,
                            descriptor.value_type,
                            &schema.registry,
                            mappings,
                        )?
                    };
                accounts.extend(value_accounts);
                Ok((new_key, new_value, accounts))
            });
            let (new_key, new_value, accounts) = match decoded {
                Ok(decoded) => decoded,
                Err(error) => {
                    all_entries_decoded = false;
                    all_entries_classified = false;
                    report.issue(error, key);
                    if !watched.is_empty() {
                        if let Some(found) = watch_record(
                            &trie,
                            key,
                            value,
                            descriptor,
                            schema,
                            supplied.map(Vec::as_slice),
                            &watched,
                        )? {
                            watched_records.push(found);
                        }
                    }
                    continue;
                }
            };
            report.decoded_entries += 1;
            if !watched.is_empty() {
                if let Some(found) = watch_record(
                    &trie,
                    key,
                    value,
                    descriptor,
                    schema,
                    supplied.map(Vec::as_slice),
                    &watched,
                )? {
                    watched_records.push(found);
                }
            }
            for account in &accounts {
                if destinations.contains(account) {
                    used_destinations
                        .entry(hx(account))
                        .or_insert_with(|| format!("{trie}/{name}/{}", hx(key)));
                }
            }
            let affected = accounts
                .iter()
                .any(|account| mappings.contains_key(account));
            if affected {
                report.affected_entries += 1;
            }
            if diagnostic_schemas.contains(&(trie.clone(), name.clone())) {
                semantic_dispositions_complete = false;
                report.issue("caller-provided codec is diagnostic only; source-pinned storage adapter required", key);
                continue;
            }
            let Some(review) = review else {
                semantic_dispositions_complete = false;
                report.issue("missing reviewed semantic disposition", key);
                continue;
            };
            match review.action {
                Disposition::Manual | Disposition::CancelPending => {
                    semantic_dispositions_complete = false;
                    report.issue(
                        "pallet-specific forward repair/cancellation adapter required",
                        key,
                    );
                }
                Disposition::RetainHistorical => {
                    // A caller's label is not proof that mutable ownership is
                    // historical. Only explicit immutable evidence layouts may
                    // retain mapped identities without a per-key adapter.
                    let immutable_evidence = (descriptor.name=="System.Events" && descriptor.prefix==[sp_io::hashing::twox_128(b"System"),sp_io::hashing::twox_128(b"Events")].concat())
                        || (descriptor.name=="Authorship.Uncles" && descriptor.prefix==[sp_io::hashing::twox_128(b"Authorship"),sp_io::hashing::twox_128(b"Uncles")].concat() && schema.metadata_hash=="0x0c1d4bf5c3bc5037ad854932443c94faae6c327ecf1f38b3e77d38668cc2b4b8" && schema.registry.resolve(descriptor.value_type).is_some_and(|t|t.path.segments.last().is_some_and(|p|p=="LegacyUncles")));
                    if affected && !immutable_evidence {
                        semantic_dispositions_complete = false;
                        report.issue("mapped ownership cannot be waived by historical retention; exact semantic adapter required",key);
                    }
                }
                Disposition::MutableOwnership => {
                    if sensitive(descriptor, &schema.registry) {
                        semantic_dispositions_complete = false;
                        report.issue(
                            "historical/consensus/authorization state requires dedicated adapter",
                            key,
                        );
                        continue;
                    }
                    if !affected || (new_key == *key && new_value == *value) {
                        continue;
                    }
                    if new_key != *key && entries.contains_key(&new_key) {
                        semantic_dispositions_complete = false;
                        report.issue("destination storage key already occupied", key);
                        continue;
                    }
                    if !generated.insert((trie.clone(), new_key.clone())) {
                        return Err("transformed storage key collision".into());
                    }
                    candidates.push(GuardedCandidate {
                        trie: trie.clone(),
                        storage: name.clone(),
                        source_key: hx(key),
                        destination_key: hx(&new_key),
                        expected_source_value_hash: hx(&sp_io::hashing::blake2_256(value)),
                        require_destination_absent: new_key != *key,
                        replacement_value: hx(&new_value),
                        disposition_evidence_hash: review.evidence_hash.clone(),
                    });
                }
            }
        }
    }
    if seen_preimages.len() != preimages.len() {
        return Err("preimage refers to an absent/unknown captured key".into());
    }
    if used_reviews.len() != dispositions.len() {
        return Err("semantic disposition refers to absent or unknown storage".into());
    }
    if seen_opaque.len() != opaque.len() {
        return Err("opaque retention references an absent captured key".into());
    }
    if !all_entries_classified {
        semantic_dispositions_complete = false;
    }
    let unused_destinations_verified = all_entries_classified && used_destinations.is_empty();
    let mut report = InventoryReport { schema: "sora-nexus-storage-inventory-v1", metadata_hash: schema.metadata_hash.clone(), top_entries: storage.top.len(), child_entries: storage.children_default.values().map(|c| c.data.len()).sum(), child_tries: storage.children_default.len(), all_entries_decoded, all_entries_classified, opaque_retained_entries, semantic_dispositions_complete, unused_destinations_verified, used_destinations, watched_accounts: options.watched_accounts.clone(), watched_references: watched_records, prefixes: reports.into_values().collect(), candidates, production_ready: false, remaining: "Trusted CSV authorization, automatic funded derivative expansion, reviewed economic/pointer semantics, consensus evidence, authenticated recovery chunks and exact-artifact rehearsals remain separate release gates.",
        native_watch:NativeWatchCertificate {report_hash:[0;32],state_sha256:state_digest.finalize().into(),schema_sha256:{
            let mut digest=Sha256::new();
            digest.update(b"sora-nexus/native-storage-decoding-schema/v1");
            digest.update(schema.registry.encode());
            for descriptor in &descriptors {
                let encoded=serde_json::to_vec(&(&descriptor.trie,&descriptor.name,&descriptor.prefix,&descriptor.key_types,&descriptor.hashers,descriptor.value_type)).map_err(|e|e.to_string())?;
                digest.update((encoded.len() as u64).to_le_bytes());
                digest.update(encoded);
            }
            digest.finalize().into()
        },registry:watched} };
    report.native_watch.report_hash =
        Sha256::digest(serde_json::to_vec(&report).map_err(|e| e.to_string())?).into();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, Registry};
    use sp_core::crypto::AccountId32;

    fn fixture(hasher: Hasher) -> (MetadataSchema, Descriptor) {
        let mut registry = Registry::new();
        let account = registry.register_type(&meta_type::<AccountId32>()).id;
        let amount = registry.register_type(&meta_type::<u128>()).id;
        let descriptor = Descriptor {
            trie: "top".into(),
            name: "Balances.Positions".into(),
            prefix: vec![9; 32],
            key_types: vec![account],
            hashers: vec![hasher],
            value_type: amount,
        };
        (
            MetadataSchema {
                registry: PortableRegistry::from(registry),
                descriptors: vec![descriptor.clone()],
                metadata_hash: hx(&[0; 32]),
            },
            descriptor,
        )
    }
    fn key(descriptor: &Descriptor, account: [u8; 32]) -> Vec<u8> {
        let mut key = descriptor.prefix.clone();
        key.extend(descriptor.hashers[0].apply(&account));
        key
    }
    fn options(name: &str, action: Disposition) -> InventoryOptions {
        InventoryOptions {
            dispositions: vec![StorageDisposition {
                trie: "top".into(),
                storage: name.into(),
                action,
                reason: "reviewed fixture only".into(),
                evidence_hash: hx(&[7; 32]),
            }],
            ..Default::default()
        }
    }
    #[test]
    fn soratopia_check_in_moves_owner_without_resetting_eligibility() {
        let mut registry = Registry::new();
        let account = registry.register_type(&meta_type::<AccountId32>()).id;
        let block = registry.register_type(&meta_type::<u32>()).id;
        let descriptor = Descriptor {
            trie: "top".into(),
            name: "Soratopia.LastCheckIn".into(),
            prefix: [
                sp_io::hashing::twox_128(b"Soratopia"),
                sp_io::hashing::twox_128(b"LastCheckIn"),
            ]
            .concat(),
            key_types: vec![account],
            hashers: vec![Hasher::Blake2_128Concat],
            value_type: block,
        };
        let schema = MetadataSchema {
            registry: registry.into(),
            descriptors: vec![descriptor.clone()],
            metadata_hash: hx(&[0; 32]),
        };
        let old_key = key(&descriptor, [1; 32]);
        let new_key = key(&descriptor, [2; 32]);
        let value = 28_191_816u32.encode();
        let mut storage = Storage::default();
        storage.top.insert(old_key.clone(), value.clone());
        let mapping = BTreeMap::from([([1; 32], [2; 32])]);
        let review = options(&descriptor.name, Disposition::MutableOwnership);
        let report = inventory(&schema, &storage, &mapping, &review).unwrap();
        assert!(report.semantic_dispositions_complete);
        assert!(report.unused_destinations_verified);
        assert_eq!(report.candidates.len(), 1);
        let candidate = &report.candidates[0];
        assert_eq!(candidate.source_key, hx(&old_key));
        assert_eq!(candidate.destination_key, hx(&new_key));
        assert_eq!(candidate.replacement_value, hx(&value));
        assert_eq!(
            candidate.expected_source_value_hash,
            hx(&sp_io::hashing::blake2_256(&value))
        );
        assert!(candidate.require_destination_absent);

        storage.top.insert(new_key, 28_191_817u32.encode());
        let occupied = inventory(&schema, &storage, &mapping, &review).unwrap();
        assert!(!occupied.semantic_dispositions_complete);
        assert!(!occupied.unused_destinations_verified);
        assert!(occupied.candidates.is_empty());
    }
    #[test]
    fn scalar_payout_and_delegation_pointers_cannot_use_generic_ownership_moves() {
        for name in [
            "Staking.Payee",
            "Staking.Bonded",
            "Staking.Ledger",
            "Referrals.Referrers",
            "ReferralSystem.Referrers",
            "Democracy.VotingOf",
            "ConvictionVoting.VotingFor",
        ] {
            let (mut schema, mut d) = fixture(Hasher::Blake2_128Concat);
            d.name = name.into();
            d.value_type = d.key_types[0];
            schema.descriptors = vec![d.clone()];
            let mut storage = Storage::default();
            storage
                .top
                .insert(key(&d, [1; 32]), AccountId32::from([9; 32]).encode());
            let report = inventory(
                &schema,
                &storage,
                &BTreeMap::from([([1; 32], [2; 32])]),
                &options(name, Disposition::MutableOwnership),
            )
            .unwrap();
            assert!(!report.semantic_dispositions_complete, "{name}");
            assert!(report.candidates.is_empty(), "{name}");
        }
    }
    #[test]
    fn caller_supplied_legacy_codec_cannot_authorize_a_production_mutation() {
        let (mut schema, mut d) = fixture(Hasher::Blake2_128Concat);
        schema.descriptors.clear();
        d.name = "RemovedPallet.ScalarOwner".into();
        let mut storage = Storage::default();
        storage.top.insert(key(&d, [1; 32]), 7u128.encode());
        let mut review = options(&d.name, Disposition::MutableOwnership);
        review.additional_storage.push(AdditionalStorage {
            trie: d.trie.clone(),
            name: d.name.clone(),
            prefix: hx(&d.prefix),
            key_types: d.key_types.clone(),
            hashers: d.hashers.clone(),
            value_type: d.value_type,
        });
        let report = inventory(
            &schema,
            &storage,
            &BTreeMap::from([([1; 32], [2; 32])]),
            &review,
        )
        .unwrap();
        assert!(report.all_entries_classified);
        assert!(!report.semantic_dispositions_complete);
        assert!(report.candidates.is_empty());
        assert_eq!(report.prefixes[0].affected_entries, 1);
    }
    #[test]
    fn reversible_hashes_are_verified_and_rebuilt_not_byte_replaced() {
        for hasher in [
            Hasher::Identity,
            Hasher::Twox64Concat,
            Hasher::Blake2_128Concat,
        ] {
            let (schema, descriptor) = fixture(hasher);
            let original = key(&descriptor, [1; 32]);
            let (changed, accounts) = transform_key(
                &original,
                &descriptor,
                &schema.registry,
                &BTreeMap::from([([1; 32], [2; 32])]),
                None,
            )
            .unwrap();
            assert_eq!(changed, key(&descriptor, [2; 32]));
            assert_eq!(accounts, BTreeSet::from([[1; 32]]));
            let mut trailing = original.clone();
            trailing.push(0);
            assert!(transform_key(
                &trailing,
                &descriptor,
                &schema.registry,
                &BTreeMap::new(),
                None
            )
            .is_err());
            if hasher != Hasher::Identity {
                let mut bad = original;
                bad[32] ^= 1;
                assert!(
                    transform_key(&bad, &descriptor, &schema.registry, &BTreeMap::new(), None)
                        .is_err()
                );
            }
        }
    }
    #[test]
    fn opaque_keys_require_exact_typed_preimages() {
        for hasher in [
            Hasher::Blake2_128,
            Hasher::Blake2_256,
            Hasher::Twox128,
            Hasher::Twox256,
        ] {
            let (schema, descriptor) = fixture(hasher);
            let original = key(&descriptor, [1; 32]);
            assert!(transform_key(
                &original,
                &descriptor,
                &schema.registry,
                &BTreeMap::new(),
                None
            )
            .is_err());
            assert!(transform_key(
                &original,
                &descriptor,
                &schema.registry,
                &BTreeMap::new(),
                Some(&[vec![2; 32]])
            )
            .is_err());
            let (changed, _) = transform_key(
                &original,
                &descriptor,
                &schema.registry,
                &BTreeMap::from([([1; 32], [2; 32])]),
                Some(&[vec![1; 32]]),
            )
            .unwrap();
            assert_eq!(changed, key(&descriptor, [2; 32]));
        }
    }

    #[test]
    fn retain_historical_cannot_leave_mutable_mapped_ownership_behind() {
        let (schema, d) = fixture(Hasher::Blake2_128Concat);
        let mut storage = Storage::default();
        storage.top.insert(key(&d, [1; 32]), 7u128.encode());
        let report = inventory(
            &schema,
            &storage,
            &BTreeMap::from([([1; 32], [2; 32])]),
            &options(&d.name, Disposition::RetainHistorical),
        )
        .unwrap();
        assert!(!report.semantic_dispositions_complete);
        assert!(report.candidates.is_empty());
        assert!(report.prefixes[0]
            .issues
            .keys()
            .any(|s| s.contains("cannot be waived")));
    }
    #[test]
    fn resolutions_are_exact_per_key_effects_and_cannot_waive_a_prefix() {
        let (mut schema, mut d) = fixture(Hasher::Blake2_128Concat);
        d.name = "Staking.Payee".into();
        schema.descriptors = vec![d.clone()];
        let first = key(&d, [1; 32]);
        let second = key(&d, [2; 32]);
        let before = 7u128.encode();
        let mut storage = Storage::default();
        storage.top.insert(first.clone(), before.clone());
        storage.top.insert(second, before.clone());
        let edit = GuardedStorageEdit {
            scope: StorageScope::Top,
            key: first.clone(),
            before: Some(sp_io::hashing::blake2_256(&before)),
            after: None,
        };
        let mut resolution = AdapterResolution {
            storage: d.name.clone(),
            key: hx(&first),
            expected_before_hash: hx(&sp_io::hashing::blake2_256(&before)),
            expected_after_hash: None,
            adapter: AdapterId::StakingPayeeV1,
            action: ResolutionAction::ReplaceControl,
            validated_edits: vec![edit.clone()],
            observed_accounts: BTreeSet::from([[1; 32]]),
        };
        let opts = options(&d.name, Disposition::Manual);
        let report = inventory_with_resolutions(
            &schema,
            &storage,
            &BTreeMap::new(),
            &opts,
            std::slice::from_ref(&resolution),
            std::slice::from_ref(&edit),
        )
        .unwrap();
        assert!(!report.semantic_dispositions_complete);
        assert_eq!(report.prefixes[0].adapter_resolved_entries, 1);
        assert_eq!(report.prefixes[0].issues.values().sum::<usize>(), 1);
        assert!(inventory_with_resolutions(
            &schema,
            &storage,
            &BTreeMap::new(),
            &opts,
            std::slice::from_ref(&resolution),
            &[]
        )
        .is_err());
        resolution.expected_before_hash = hx(&[9; 32]);
        assert!(inventory_with_resolutions(
            &schema,
            &storage,
            &BTreeMap::new(),
            &opts,
            &[resolution],
            &[edit]
        )
        .is_err());
    }
    #[test]
    fn watched_identity_is_checked_against_opaque_account_key_without_excusing_coverage() {
        let (schema, descriptor) = fixture(Hasher::Blake2_256);
        let watched = BTreeSet::from([vec![1; 32]]);
        let hit = watch_record(
            "top",
            &key(&descriptor, [1; 32]),
            &0u128.encode(),
            &descriptor,
            &schema,
            None,
            &watched,
        )
        .unwrap()
        .unwrap();
        assert!(hit.key_decoding_error.is_some());
        assert_eq!(
            hit.references[0].kind,
            "account_id32_hashed_key_preimage_match"
        );
        let incomplete = watch_record(
            "top",
            &key(&descriptor, [2; 32]),
            &0u128.encode(),
            &descriptor,
            &schema,
            None,
            &watched,
        )
        .unwrap()
        .unwrap();
        assert!(incomplete.references.is_empty());
        assert!(incomplete.key_decoding_error.is_some());
    }

    #[test]
    fn account_request_index_streams_hashes_but_never_an_account_bearing_shape() {
        let mut registry = Registry::new();
        let index = registry
            .register_type(&meta_type::<Vec<(u32, sp_core::H256)>>())
            .id;
        let unsafe_index = registry
            .register_type(&meta_type::<Vec<(u32, AccountId32)>>())
            .id;
        let schema = MetadataSchema {
            registry: registry.into(),
            descriptors: vec![],
            metadata_hash: hx(&[0; 32]),
        };
        let mut descriptor = Descriptor {
            trie: "top".into(),
            name: "EthBridge.AccountRequests".into(),
            prefix: vec![1; 32],
            key_types: vec![],
            hashers: vec![],
            value_type: index,
        };
        let encoded = vec![(7u32, sp_core::H256::repeat_byte(3)); 10_000].encode();
        assert_eq!(
            validated_request_index(&schema, &descriptor, &encoded).unwrap(),
            Some(10_000)
        );
        assert!(
            validated_request_index(&schema, &descriptor, &encoded[..encoded.len() - 1]).is_err()
        );
        descriptor.value_type = unsafe_index;
        assert!(validated_request_index(&schema, &descriptor, &encoded).is_err());
    }
    #[test]
    fn missing_disposition_emits_no_writes_and_occupied_targets_are_rejected() {
        let (schema, descriptor) = fixture(Hasher::Blake2_128Concat);
        let mut storage = Storage::default();
        storage
            .top
            .insert(key(&descriptor, [1; 32]), 5u128.encode());
        let map = BTreeMap::from([([1; 32], [2; 32])]);
        let report = inventory(&schema, &storage, &map, &InventoryOptions::default()).unwrap();
        assert!(report.all_entries_decoded);
        assert!(!report.semantic_dispositions_complete);
        assert!(report.candidates.is_empty());
        let options = options(&descriptor.name, Disposition::MutableOwnership);
        assert_eq!(
            inventory(&schema, &storage, &map, &options)
                .unwrap()
                .candidates
                .len(),
            1
        );
        storage
            .top
            .insert(key(&descriptor, [2; 32]), 0u128.encode());
        let report = inventory(&schema, &storage, &map, &options).unwrap();
        assert!(!report.unused_destinations_verified);
        assert!(report.candidates.is_empty());
        assert!(!report.production_ready);
    }

    #[test]
    fn ordered_collections_need_adapters_only_when_they_hold_accounts() {
        #[allow(dead_code)]
        #[derive(scale_info::TypeInfo)]
        struct Farmer {
            account: AccountId32,
            weight: u128,
        }
        let mut registry = Registry::new();
        let account = registry.register_type(&meta_type::<AccountId32>()).id;
        let asset = registry.register_type(&meta_type::<u32>()).id;
        let pools = registry.register_type(&meta_type::<BTreeSet<u32>>()).id;
        let rewards = registry
            .register_type(&meta_type::<BTreeMap<u8, u128>>())
            .id;
        let owners = registry
            .register_type(&meta_type::<BTreeSet<AccountId32>>())
            .id;
        let farmers = registry.register_type(&meta_type::<Vec<Farmer>>()).id;
        let registry = PortableRegistry::from(registry);
        let descriptor = |name: &str, value_type| Descriptor {
            trie: "top".into(),
            name: name.into(),
            prefix: vec![9; 32],
            key_types: vec![account, asset],
            hashers: vec![Hasher::Identity, Hasher::Blake2_128Concat],
            value_type,
        };
        assert!(!sensitive(
            &descriptor("PoolXYK.AccountPools", pools),
            &registry
        ));
        assert!(!sensitive(
            &descriptor("VestedRewards.Rewards", rewards),
            &registry
        ));
        // Replacing an account inside an ordered collection can reorder it.
        assert!(sensitive(&descriptor("Example.Owners", owners), &registry));
        assert!(!sensitive(
            &descriptor("Farming.PoolFarmers", farmers),
            &registry
        ));
        assert!(sensitive(
            &descriptor("Example.Farmers", farmers),
            &registry
        ));
    }

    #[test]
    fn liquidity_pool_index_moves_with_its_owner() {
        let mut registry = Registry::new();
        let account = registry.register_type(&meta_type::<AccountId32>()).id;
        let asset = registry.register_type(&meta_type::<u32>()).id;
        let pools = registry.register_type(&meta_type::<BTreeSet<u32>>()).id;
        let descriptor = Descriptor {
            trie: "top".into(),
            name: "PoolXYK.AccountPools".into(),
            prefix: vec![9; 32],
            key_types: vec![account, asset],
            hashers: vec![Hasher::Identity, Hasher::Blake2_128Concat],
            value_type: pools,
        };
        let schema = MetadataSchema {
            registry: PortableRegistry::from(registry),
            descriptors: vec![descriptor.clone()],
            metadata_hash: hx(&[0; 32]),
        };
        let row = |owner: [u8; 32]| {
            let mut key = descriptor.prefix.clone();
            key.extend(Hasher::Identity.apply(&owner));
            key.extend(Hasher::Blake2_128Concat.apply(&5u32.encode()));
            key
        };
        let mut storage = Storage::default();
        storage
            .top
            .insert(row([1; 32]), BTreeSet::from([7u32, 9]).encode());
        let map = BTreeMap::from([([1; 32], [2; 32])]);
        let options = options(&descriptor.name, Disposition::MutableOwnership);
        let report = inventory(&schema, &storage, &map, &options).unwrap();
        assert_eq!(report.prefixes[0].affected_entries, 1);
        assert!(report.prefixes[0].issues.is_empty());
        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].destination_key, hx(&row([2; 32])));
    }

    #[test]
    fn watched_key_reports_exact_typed_value_and_provenance_without_implying_ownership() {
        let (schema, descriptor) = fixture(Hasher::Blake2_128Concat);
        let mut storage = Storage::default();
        let key = key(&descriptor, [1; 32]);
        storage.top.insert(key.clone(), u128::MAX.encode());
        let options = InventoryOptions {
            watched_accounts: vec![hx(&[1; 32])],
            ..Default::default()
        };
        let report = inventory(&schema, &storage, &BTreeMap::new(), &options).unwrap();
        assert_eq!(report.watched_references.len(), 1);
        let found = &report.watched_references[0];
        assert_eq!(found.key, hx(&key));
        assert_eq!(
            found.value_decoded,
            serde_json::Value::String(u128::MAX.to_string())
        );
        assert_eq!(found.references[0].path, vec!["key[0]"]);
        assert_eq!(
            found.key_decoded,
            vec![serde_json::Value::String(hx(&[1; 32]))]
        );
        assert!(!report.production_ready);
    }

    #[test]
    fn resolved_control_rows_still_participate_in_complete_identity_watch() {
        let (mut schema, mut descriptor) = fixture(Hasher::Blake2_128Concat);
        descriptor.name = "Staking.Payee".into();
        descriptor.value_type = descriptor.key_types[0];
        schema.descriptors = vec![descriptor.clone()];
        let mut storage = Storage::default();
        let key = key(&descriptor, [1; 32]);
        let before = AccountId32::from([1; 32]).encode();
        let after = AccountId32::from([2; 32]).encode();
        storage.top.insert(key.clone(), before.clone());
        let edit = GuardedStorageEdit {
            scope: StorageScope::Top,
            key: key.clone(),
            before: Some(sp_io::hashing::blake2_256(&before)),
            after: Some(after.clone()),
        };
        let resolution = AdapterResolution {
            storage: descriptor.name.clone(),
            key: hx(&key),
            expected_before_hash: hx(&sp_io::hashing::blake2_256(&before)),
            expected_after_hash: Some(hx(&sp_io::hashing::blake2_256(&after))),
            adapter: AdapterId::StakingPayeeV1,
            action: ResolutionAction::ReplaceControl,
            validated_edits: vec![edit.clone()],
            observed_accounts: BTreeSet::from([[1; 32]]),
        };
        let report = inventory_with_resolutions(
            &schema,
            &storage,
            &BTreeMap::new(),
            &InventoryOptions {
                watched_accounts: vec![hx(&[1; 32])],
                ..Default::default()
            },
            &[resolution],
            &[edit],
        )
        .unwrap();
        assert!(report.all_entries_classified);
        assert!(report.semantic_dispositions_complete);
        assert_eq!(report.watched_references.len(), 1);
        assert_eq!(report.watched_references[0].references.len(), 2);
        assert!(report
            .require_native_watch(&BTreeSet::from([vec![1; 32]]))
            .is_ok());
    }

    #[test]
    fn hashi_signed_evidence_resolution_requires_exact_identity_marker_and_keeps_watches() {
        let mut registry = Registry::new();
        let network = registry.register_type(&meta_type::<u32>()).id;
        let request = registry.register_type(&meta_type::<sp_core::H256>()).id;
        let owner = registry.register_type(&meta_type::<AccountId32>()).id;
        let descriptor = Descriptor {
            trie: "top".into(),
            name: "EthBridge.Requests".into(),
            prefix: [
                sp_io::hashing::twox_128(b"EthBridge"),
                sp_io::hashing::twox_128(b"Requests"),
            ]
            .concat(),
            key_types: vec![network, request],
            hashers: vec![Hasher::Twox64Concat, Hasher::Identity],
            value_type: owner,
        };
        let schema = MetadataSchema {
            registry: registry.into(),
            descriptors: vec![descriptor.clone()],
            metadata_hash: hx(&[1; 32]),
        };
        let key = [
            descriptor.prefix.clone(),
            Hasher::Twox64Concat.apply(&0u32.encode()),
            vec![9; 32],
        ]
        .concat();
        let value = common::recovery::ATTACKER_ACCOUNT.to_vec();
        let marker = [
            b":sora-nexus:historical-hashi-request:v1:".to_vec(),
            0u32.encode(),
            vec![9; 32],
        ]
        .concat();
        let edits = vec![GuardedStorageEdit {
            scope: StorageScope::Top,
            key: marker,
            before: None,
            after: Some((*b"snxhash1", common::recovery::CHECKPOINT_HASH, [7u8; 32]).encode()),
        }];
        let mut storage = Storage::default();
        storage.top.insert(key.clone(), value.clone());
        let resolution = AdapterResolution {
            storage: descriptor.name.clone(),
            key: hx(&key),
            expected_before_hash: hx(&sp_io::hashing::blake2_256(&value)),
            expected_after_hash: Some(hx(&sp_io::hashing::blake2_256(&value))),
            adapter: AdapterId::HashiCancellationV1,
            action: ResolutionAction::CancelPending,
            validated_edits: edits.clone(),
            observed_accounts: BTreeSet::from([common::recovery::ATTACKER_ACCOUNT]),
        };
        let options = InventoryOptions {
            watched_accounts: vec![hx(&common::recovery::ATTACKER_ACCOUNT)],
            ..Default::default()
        };
        let report = inventory_with_resolutions(
            &schema,
            &storage,
            &BTreeMap::new(),
            &options,
            std::slice::from_ref(&resolution),
            &edits,
        )
        .unwrap();
        assert_eq!(report.watched_references.len(), 1);
        assert_eq!(
            report.watched_references[0].references[0].identity,
            hx(&common::recovery::ATTACKER_ACCOUNT)
        );
        for mutate in 0..5 {
            let mut changed = resolution.clone();
            let mut bad_edits = edits.clone();
            match mutate {
                0 => {
                    bad_edits.pop();
                }
                1 => {
                    *bad_edits[0].key.last_mut().unwrap() = 8;
                }
                2 => {
                    changed.expected_after_hash = Some(hx(&sp_io::hashing::blake2_256(&[2; 32])));
                }
                3 => bad_edits.push(GuardedStorageEdit {
                    scope: StorageScope::Top,
                    key: key.clone(),
                    before: Some(sp_io::hashing::blake2_256(&value)),
                    after: Some(value.clone()),
                }),
                _ => bad_edits[0].after = Some(vec![2; 32]),
            }
            changed.validated_edits = bad_edits.clone();
            assert!(inventory_with_resolutions(
                &schema,
                &storage,
                &BTreeMap::new(),
                &options,
                &[changed],
                &bad_edits
            )
            .is_err());
        }
    }

    #[test]
    fn opaque_key_does_not_hide_known_value_identity_references() {
        let (mut schema, mut descriptor) = fixture(Hasher::Blake2_256);
        descriptor.value_type = descriptor.key_types[0];
        schema.descriptors = vec![descriptor.clone()];
        let mut storage = Storage::default();
        storage.top.insert(key(&descriptor, [8; 32]), vec![1; 32]);
        let options = InventoryOptions {
            watched_accounts: vec![hx(&[1; 32])],
            ..Default::default()
        };
        let report = inventory(&schema, &storage, &BTreeMap::new(), &options).unwrap();
        assert!(!report.all_entries_classified);
        assert_eq!(report.watched_references.len(), 1);
        let found = &report.watched_references[0];
        assert!(found.key_decoding_error.is_some());
        assert!(found.value_decoding_error.is_none());
        assert_eq!(found.references[0].path, vec!["value"]);
    }

    #[test]
    fn account_collections_need_ordering_semantics_not_just_roundtrip() {
        let mut registry = Registry::new();
        let account = registry.register_type(&meta_type::<AccountId32>()).id;
        let members = registry.register_type(&meta_type::<Vec<AccountId32>>()).id;
        let descriptor = Descriptor {
            trie: "top".into(),
            name: "Example.Positions".into(),
            prefix: vec![9; 32],
            key_types: vec![account],
            hashers: vec![Hasher::Identity],
            value_type: members,
        };
        let schema = MetadataSchema {
            registry: PortableRegistry::from(registry),
            descriptors: vec![descriptor.clone()],
            metadata_hash: hx(&[0; 32]),
        };
        let mut storage = Storage::default();
        storage.top.insert(
            key(&descriptor, [1; 32]),
            vec![AccountId32::from([1; 32]), AccountId32::from([2; 32])].encode(),
        );
        let report = inventory(
            &schema,
            &storage,
            &BTreeMap::from([([1; 32], [3; 32])]),
            &options(&descriptor.name, Disposition::MutableOwnership),
        )
        .unwrap();
        assert!(report.candidates.is_empty());
        assert!(!report.semantic_dispositions_complete);
    }
    #[test]
    fn historical_and_pending_authorization_entries_never_get_generic_rewrites() {
        let (mut schema, mut descriptor) = fixture(Hasher::Identity);
        descriptor.name = "BridgeMultisig.PendingCalls".into();
        schema.descriptors = vec![descriptor.clone()];
        let mut storage = Storage::default();
        storage
            .top
            .insert(key(&descriptor, [1; 32]), 5u128.encode());
        let map = BTreeMap::from([([1; 32], [2; 32])]);
        for action in [
            Disposition::MutableOwnership,
            Disposition::CancelPending,
            Disposition::RetainHistorical,
        ] {
            let report =
                inventory(&schema, &storage, &map, &options(&descriptor.name, action)).unwrap();
            assert!(report.candidates.is_empty());
            assert!(!report.semantic_dispositions_complete);
        }
    }
    #[test]
    fn unknown_and_child_entries_prevent_unused_destination_certification() {
        use sp_core::storage::{ChildInfo, StorageChild};
        let (schema, _) = fixture(Hasher::Identity);
        let mut storage = Storage::default();
        storage.top.insert(vec![0xff; 32], vec![0]);
        storage.children_default.insert(
            b"child".to_vec(),
            StorageChild {
                data: BTreeMap::from([(vec![1], vec![2])]),
                child_info: ChildInfo::new_default(b"child"),
            },
        );
        let report = inventory(
            &schema,
            &storage,
            &BTreeMap::new(),
            &InventoryOptions::default(),
        )
        .unwrap();
        assert_eq!(report.child_entries, 1);
        assert_eq!(report.prefixes.len(), 2);
        assert!(!report.all_entries_decoded);
        assert!(!report.unused_destinations_verified);
    }
    #[test]
    fn malformed_values_and_prefix_collisions_are_rejected() {
        let (mut schema, descriptor) = fixture(Hasher::Identity);
        let mut storage = Storage::default();
        storage.top.insert(key(&descriptor, [1; 32]), vec![1]);
        assert!(
            !inventory(
                &schema,
                &storage,
                &BTreeMap::new(),
                &InventoryOptions::default()
            )
            .unwrap()
            .all_entries_decoded
        );
        let mut overlapping = descriptor;
        overlapping.name = "Other".into();
        overlapping.prefix.push(1);
        schema.descriptors.push(overlapping);
        assert!(inventory(
            &schema,
            &storage,
            &BTreeMap::new(),
            &InventoryOptions::default()
        )
        .is_err());
    }

    #[test]
    fn opaque_retention_requires_exact_hash_and_non_ownership_shape() {
        let mut registry = Registry::new();
        let bytes_id = registry.register_type(&meta_type::<Vec<u8>>()).id;
        let hash_id = registry.register_type(&meta_type::<[u8; 32]>()).id;
        let descriptor = Descriptor {
            trie: "top".into(),
            name: "Preimage.PreimageFor".into(),
            prefix: vec![9; 32],
            key_types: vec![hash_id],
            hashers: vec![Hasher::Identity],
            value_type: bytes_id,
        };
        let schema = MetadataSchema {
            registry: PortableRegistry::from(registry),
            descriptors: vec![descriptor.clone()],
            metadata_hash: hx(&[0; 32]),
        };
        let key = key(&descriptor, [1; 32]);
        let value = vec![7u8; typed::MAX_TYPED_BYTES + 1].encode();
        let mut storage = Storage::default();
        storage.top.insert(key.clone(), value.clone());
        let mut options = options(&descriptor.name, Disposition::RetainHistorical);
        assert!(
            !inventory(&schema, &storage, &BTreeMap::new(), &options)
                .unwrap()
                .all_entries_classified
        );
        options.opaque_retentions.push(OpaqueRetention {
            trie: "top".into(),
            key: hx(&key),
            value_hash: hx(&sp_io::hashing::blake2_256(&value)),
            evidence_hash: hx(&[7; 32]),
        });
        let report = inventory(&schema, &storage, &BTreeMap::new(), &options).unwrap();
        assert!(report.all_entries_classified);
        assert!(!report.all_entries_decoded);
        assert_eq!(report.opaque_retained_entries, 1);
        options.opaque_retentions[0].value_hash = hx(&[0; 32]);
        assert!(inventory(&schema, &storage, &BTreeMap::new(), &options).is_err());
    }
}
