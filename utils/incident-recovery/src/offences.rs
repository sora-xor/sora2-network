//! Coupled offence report/index migration. Report IDs commit the entire historical
//! identification tuple, so account substitution must rehash and reindex together.
use crate::{
    hex_bytes,
    inventory::{
        AdapterId, AdapterResolution, Descriptor, Hasher, MetadataSchema, ResolutionAction,
    },
    typed, Result,
};
use codec::{Decode, Encode};
use common::recovery::{
    self, AccountReplacement, GuardedStorageEdit, StoragePrefixWitness, StorageScope,
    StorageWitness,
};
use scale_value::{Composite, ValueDef};
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, storage::Storage};
use std::collections::{BTreeMap, BTreeSet};
const METADATA: &str = "0x0c1d4bf5c3bc5037ad854932443c94faae6c327ecf1f38b3e77d38668cc2b4b8";
const MAX_ROWS: usize = 100_000;
const MAX_BYTES: usize = 64 * 1024 * 1024;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Review {
    pub evidence_hash: String,
    pub records: Vec<Record>,
    /// Only required when an affected historical report was denominated in place.
    #[serde(default)]
    pub original_offenders: Vec<OriginalOffender>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OriginalOffender {
    pub report_id: String,
    pub offender: String,
    /// Product of source denomination factors applied after this report was hashed.
    pub denominator: String,
    pub source_evidence_hash: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Record {
    pub storage: String,
    pub key: String,
    pub value_hash: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub evidence_hash: String,
    pub reports: usize,
    pub concurrent_groups: usize,
    pub changed_reports: usize,
    pub rekeyed_reports: usize,
    pub changed_indices: usize,
    pub preserved_historical_ids: usize,
    pub authenticated_original_preimages: usize,
    pub offence_counts_preserved: bool,
    pub native_slashing_executed: bool,
    pub production_ready: bool,
}
pub struct Compilation {
    pub groups: Vec<Vec<GuardedStorageEdit>>,
    pub edits: Vec<GuardedStorageEdit>,
    pub resolutions: Vec<AdapterResolution>,
    pub witnesses: Vec<StorageWitness>,
    pub prefix_witnesses: Vec<StoragePrefixWitness>,
    pub report: Report,
    prefixes: Vec<Vec<u8>>,
    before: BTreeMap<Vec<u8>, Vec<u8>>,
    expected: BTreeMap<Vec<u8>, Vec<u8>>,
    mapping_hash: [u8; 32],
    denomination: Option<(Vec<u8>, Vec<u8>)>,
}
impl Compilation {
    pub fn verify_groups(
        &self,
        groups: &[Vec<GuardedStorageEdit>],
        replacements: &[AccountReplacement],
    ) -> Result<()> {
        if hash(&replacements.encode()) != self.mapping_hash {
            return Err("offences mapping certificate mismatch".into());
        }
        if let Some((key, value)) = &self.denomination {
            for edit in groups
                .iter()
                .flatten()
                .filter(|e| e.scope == StorageScope::Top && e.key == *key)
            {
                if edit.before != Some(hash(value)) || edit.after.as_ref() != Some(value) {
                    return Err(
                        "offences authenticated denomination basis must be preserved".into(),
                    );
                }
            }
        }
        let mut projected = self.before.clone();
        let mut seen = BTreeSet::new();
        for edit in groups.iter().flatten().filter(|e| {
            e.scope == StorageScope::Top && self.prefixes.iter().any(|p| e.key.starts_with(p))
        }) {
            if !seen.insert(edit.key.clone())
                || self.before.get(&edit.key).map(|v| hash(v)) != edit.before
            {
                return Err("offences stale or overlapping coupled edit".into());
            }
            if let Some(v) = &edit.after {
                projected.insert(edit.key.clone(), v.clone());
            } else {
                projected.remove(&edit.key);
            }
        }
        if projected != self.expected {
            return Err(
                "offences report/index graph requires its complete typed transformation".into(),
            );
        }
        Ok(())
    }
}
fn hash(v: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(v)
}
fn hx(v: &[u8]) -> String {
    format!("0x{}", hex::encode(v))
}
fn prefix(item: &str) -> Vec<u8> {
    [
        sp_io::hashing::twox_128(b"Offences"),
        sp_io::hashing::twox_128(item.as_bytes()),
    ]
    .concat()
}
fn descriptor<'a>(s: &'a MetadataSchema, name: &str) -> Result<&'a Descriptor> {
    let d = s
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == format!("Offences.{name}"))
        .ok_or("offences descriptor missing")?;
    if d.prefix != prefix(name) {
        return Err("offences descriptor prefix changed".into());
    }
    Ok(d)
}
fn report_key(d: &Descriptor, id: &[u8; 32]) -> Vec<u8> {
    [d.prefix.clone(), Hasher::Twox64Concat.apply(id)].concat()
}
fn index_components(s: &MetadataSchema, d: &Descriptor, key: &[u8]) -> Result<([u8; 16], Vec<u8>)> {
    crate::inventory::transform_key(key, d, &s.registry, &BTreeMap::new(), None)?;
    let mut input = key
        .get(d.prefix.len() + 8..)
        .ok_or("offence index key truncated")?;
    let kind = <[u8; 16]>::decode(&mut input).map_err(|e| e.to_string())?;
    input = input.get(8..).ok_or("offence slot hasher truncated")?;
    let slot = Vec::<u8>::decode(&mut input).map_err(|e| e.to_string())?;
    if !input.is_empty()
        || slot.is_empty()
        || slot.len() > 1024
        || key
            != [
                d.prefix.clone(),
                Hasher::Twox64Concat.apply(&kind),
                Hasher::Twox64Concat.apply(&slot.encode()),
            ]
            .concat()
    {
        return Err("offence index kind/opaque slot codec differs".into());
    }
    Ok((kind, slot))
}
fn offender_value(
    s: &MetadataSchema,
    d: &Descriptor,
    value: &[u8],
) -> Result<scale_value::Value<u32>> {
    let decoded = typed::decode_exact(value, d.value_type, &s.registry)?;
    let ValueDef::Composite(Composite::Named(fields)) = decoded.value else {
        return Err("offence details must be named offender/reporters".into());
    };
    if fields.len() != 2 || fields[0].0 != "offender" || fields[1].0 != "reporters" {
        return Err("offence details shape changed".into());
    }
    Ok(fields.into_iter().next().expect("two fields checked").1)
}
fn offender_bytes(s: &MetadataSchema, d: &Descriptor, value: &[u8]) -> Result<Vec<u8>> {
    typed::encode(&offender_value(s, d, value)?, &s.registry)
}
#[derive(Encode, Decode)]
struct OriginalExposure {
    #[codec(compact)]
    total: u128,
    #[codec(compact)]
    own: u128,
    others: Vec<OriginalIndividualExposure>,
}
#[derive(Encode, Decode)]
struct OriginalIndividualExposure {
    who: AccountId32,
    #[codec(compact)]
    value: u128,
}
fn denominator_key() -> Vec<u8> {
    [
        sp_io::hashing::twox_128(b"Denomination"),
        sp_io::hashing::twox_128(b"Denominator"),
    ]
    .concat()
}
/// The checkpoint ReportId itself authenticates this original full identification.
/// Replaying the source's division/recomputed-total operation must yield the exact
/// checkpoint bytes, preserving every account and its order. The original amounts
/// are used only for hashing; financial storage keeps its denominated amounts.
fn authenticated_original(
    schema: &MetadataSchema,
    type_id: u32,
    current: &[u8],
    identity: ([u8; 16], &[u8], [u8; 32]),
    proof: &OriginalOffender,
    cumulative_denominator: u128,
) -> Result<Vec<u8>> {
    let (kind, slot, old_id) = identity;
    let evidence = hex_bytes(&proof.source_evidence_hash)?;
    let denominator = proof
        .denominator
        .parse::<u128>()
        .map_err(|_| "offence denomination factor must be canonical u128")?;
    if denominator <= 1
        || denominator.to_string() != proof.denominator
        || !cumulative_denominator.is_multiple_of(denominator)
        || evidence.len() != 32
        || evidence.iter().all(|b| *b == 0)
        || proof.offender.len() > 2 + typed::MAX_TYPED_BYTES * 2
    {
        return Err("offence original preimage evidence/denomination bound".into());
    }
    let original = hex_bytes(&proof.offender)?;
    typed::decode_exact(&original, type_id, &schema.registry)?;
    if report_id(kind, slot, &original) != old_id {
        return Err("offence original preimage does not match checkpoint ReportId".into());
    }
    let mut input = original.as_slice();
    let (owner, mut exposure) =
        <(AccountId32, OriginalExposure)>::decode(&mut input).map_err(|e| e.to_string())?;
    if !input.is_empty() || (owner.clone(), &exposure).encode() != original {
        return Err("offence original exposure codec differs".into());
    }
    exposure.own /= denominator;
    let mut total = exposure.own;
    for nominator in &mut exposure.others {
        nominator.value /= denominator;
        total = total
            .checked_add(nominator.value)
            .ok_or("offence denominated exposure total overflow")?;
    }
    exposure.total = total;
    if (owner, exposure).encode() != current {
        return Err("offence original preimage does not reproduce checkpoint denomination and account order".into());
    }
    Ok(original)
}
fn report_id(kind: [u8; 16], slot: &[u8], offender: &[u8]) -> [u8; 32] {
    let mut encoded = (kind, slot.to_vec()).encode();
    encoded.extend(offender);
    hash(&encoded)
}
pub fn compile(
    schema: &MetadataSchema,
    storage: &Storage,
    replacements: &[AccountReplacement],
    review: &Review,
) -> Result<Compilation> {
    recovery::validate_replacements(replacements)
        .map_err(|e| format!("offences mapping: {e:?}"))?;
    let evidence = hex_bytes(&review.evidence_hash)?;
    if schema.metadata_hash != METADATA
        || evidence.len() != 32
        || evidence.iter().all(|b| *b == 0)
        || review.records.len() > MAX_ROWS
        || review.original_offenders.len() > MAX_ROWS
    {
        return Err("offences exact metadata/evidence bounds".into());
    }
    let reports = descriptor(schema, "Reports")?;
    let indices = descriptor(schema, "ConcurrentReportsIndex")?;
    if reports.hashers != [Hasher::Twox64Concat]
        || reports.key_types.len() != 1
        || indices.hashers != [Hasher::Twox64Concat, Hasher::Twox64Concat]
        || indices.key_types.len() != 2
    {
        return Err("unreviewed offence graph key codec".into());
    }
    let mappings: BTreeMap<_, _> = replacements
        .iter()
        .map(|p| {
            (
                *AsRef::<[u8; 32]>::as_ref(&p.old),
                *AsRef::<[u8; 32]>::as_ref(&p.new),
            )
        })
        .collect();
    let mut declared = BTreeMap::new();
    for r in &review.records {
        let k = hex_bytes(&r.key)?;
        let h = hex_bytes(&r.value_hash)?;
        if h.len() != 32 || declared.insert(k, (r.storage.as_str(), h)).is_some() {
            return Err("duplicate offence evidence row".into());
        }
    }
    let mut before = BTreeMap::new();
    let mut bytes = 0usize;
    let mut empty = Vec::new();
    for d in [reports, indices] {
        let mut count = 0;
        for (k, v) in storage
            .top
            .range(d.prefix.clone()..)
            .take_while(|(k, _)| k.starts_with(&d.prefix))
        {
            count += 1;
            bytes = bytes
                .checked_add(v.len())
                .filter(|n| *n <= MAX_BYTES)
                .ok_or("offences aggregate bound")?;
            if before.len() >= MAX_ROWS
                || v.len() > typed::MAX_TYPED_BYTES
                || declared
                    .get(k)
                    .is_none_or(|(name, h)| *name != d.name || h.as_slice() != hash(v))
            {
                return Err("offences complete exact record review mismatch".into());
            }
            crate::inventory::transform_key(k, d, &schema.registry, &BTreeMap::new(), None)?;
            typed::decode_exact(v, d.value_type, &schema.registry)?;
            before.insert(k.clone(), v.clone());
        }
        if count == 0 {
            empty.push(StoragePrefixWitness {
                scope: StorageScope::Top,
                prefix: d.prefix.clone(),
            });
        }
    }
    if before.len() != declared.len() {
        return Err("extraneous offence review row".into());
    }
    let mut originals = BTreeMap::new();
    let mut original_bytes = 0usize;
    for proof in &review.original_offenders {
        original_bytes = original_bytes
            .checked_add(proof.offender.len())
            .filter(|v| *v <= MAX_BYTES * 2)
            .ok_or("offence original preimage aggregate bound")?;
        let id: [u8; 32] = hex_bytes(&proof.report_id)?
            .try_into()
            .map_err(|_| "offence original report ID width")?;
        if originals.insert(id, proof).is_some() {
            return Err("duplicate offence original preimage".into());
        }
    }
    let denomination = if originals.is_empty() {
        None
    } else {
        let key = denominator_key();
        let value = storage
            .top
            .get(&key)
            .ok_or("checkpoint denomination basis missing")?
            .clone();
        let d = schema
            .descriptors
            .iter()
            .find(|d| d.name == "Denomination.Denominator" && d.trie == "top")
            .ok_or("checkpoint denomination descriptor missing")?;
        if d.prefix != key || !d.hashers.is_empty() || !d.key_types.is_empty() || value.len() != 16
        {
            return Err("checkpoint denomination codec changed".into());
        }
        typed::decode_exact(&value, d.value_type, &schema.registry)?;
        Some((key, value))
    };
    let cumulative_denominator = denomination
        .as_ref()
        .map(|(_, v)| u128::from_le_bytes(v.as_slice().try_into().expect("16 bytes checked")))
        .unwrap_or(1);
    let mut used_originals = BTreeSet::new();
    let mut preserved_historical_ids = 0;
    let mut expected = BTreeMap::new();
    let mut visited = BTreeSet::new();
    let mut new_ids = BTreeSet::new();
    let mut groups = Vec::new();
    let mut resolutions = Vec::new();
    let (mut changed_reports, mut rekeyed_reports, mut changed_indices, mut concurrent_groups) =
        (0, 0, 0, 0);
    for (index_key, index_value) in before
        .range(indices.prefix.clone()..)
        .take_while(|(k, _)| k.starts_with(&indices.prefix))
    {
        concurrent_groups += 1;
        let (kind, slot) = index_components(schema, indices, index_key)?;
        let mut input = index_value.as_slice();
        let ids = Vec::<[u8; 32]>::decode(&mut input).map_err(|e| e.to_string())?;
        if !input.is_empty() || ids.encode() != *index_value || ids.is_empty() || ids.len() > 1000 {
            return Err("offence concurrent report count/codec invalid".into());
        }
        let mut group = Vec::new();
        let mut after_ids = Vec::with_capacity(ids.len());
        let mut resolved = Vec::new();
        let mut index_accounts = BTreeSet::new();
        for old_id in ids {
            if !visited.insert(old_id) {
                return Err("offence report appears more than once in index graph".into());
            }
            let old_key = report_key(reports, &old_id);
            let old_value = before
                .get(&old_key)
                .ok_or("offence index refers to missing report")?;
            let offender_value = offender_value(schema, reports, old_value)?;
            let offender = typed::encode(&offender_value, &schema.registry)?;
            let (after, accounts) =
                typed::transform_value(old_value, reports.value_type, &schema.registry, &mappings)?;
            let changed_offender = offender_bytes(schema, reports, &after)?;
            let current_matches_id = report_id(kind, &slot, &offender) == old_id;
            let new_id = if changed_offender == offender {
                // Native denomination deliberately leaves prior ReportIds and index
                // membership intact. Do not normalize unrelated historical IDs.
                if !current_matches_id {
                    preserved_historical_ids += 1;
                }
                old_id
            } else if current_matches_id {
                report_id(kind, &slot, &changed_offender)
            } else {
                let proof = originals.get(&old_id).ok_or_else(|| format!("affected historical offence {} requires an authenticated original offender preimage", hx(&old_id)))?;
                let original = authenticated_original(
                    schema,
                    offender_value.context,
                    &offender,
                    (kind, &slot, old_id),
                    proof,
                    cumulative_denominator,
                )?;
                let (remapped_original, _) = typed::transform_value(
                    &original,
                    offender_value.context,
                    &schema.registry,
                    &mappings,
                )?;
                used_originals.insert(old_id);
                report_id(kind, &slot, &remapped_original)
            };
            let new_key = report_key(reports, &new_id);
            if !new_ids.insert(new_id) || (new_id != old_id && before.contains_key(&new_key)) {
                return Err("offence replacement report collision".into());
            }
            expected.insert(new_key.clone(), after.clone());
            after_ids.push(new_id);
            index_accounts.extend(accounts.iter().copied());
            if after != *old_value || new_id != old_id {
                changed_reports += 1;
                let effect = if new_id == old_id {
                    Some(after.clone())
                } else {
                    rekeyed_reports += 1;
                    None
                };
                group.push(GuardedStorageEdit {
                    scope: StorageScope::Top,
                    key: old_key.clone(),
                    before: Some(hash(old_value)),
                    after: effect.clone(),
                });
                if new_id != old_id {
                    group.push(GuardedStorageEdit {
                        scope: StorageScope::Top,
                        key: new_key,
                        before: None,
                        after: Some(after),
                    });
                }
                resolved.push((
                    reports.name.clone(),
                    old_key,
                    hash(old_value),
                    effect.as_deref().map(hash),
                    accounts,
                ));
            }
        }
        let index_after = after_ids.encode();
        expected.insert(index_key.clone(), index_after.clone());
        if index_after != *index_value {
            changed_indices += 1;
            group.push(GuardedStorageEdit {
                scope: StorageScope::Top,
                key: index_key.clone(),
                before: Some(hash(index_value)),
                after: Some(index_after.clone()),
            });
            resolved.push((
                indices.name.clone(),
                index_key.clone(),
                hash(index_value),
                Some(hash(&index_after)),
                index_accounts,
            ));
        }
        if !group.is_empty() {
            // One bounded group preserves each concurrent severity list atomically.
            recovery::RecoveryChunk {
                index: 0,
                edits: group.clone(),
            }
            .validate()
            .map_err(|e| {
                format!("offence concurrent group exceeds atomic recovery bounds: {e:?}")
            })?;
            for (storage, key, before_hash, after_hash, accounts) in resolved {
                resolutions.push(AdapterResolution {
                    storage,
                    key: hx(&key),
                    expected_before_hash: hx(&before_hash),
                    expected_after_hash: after_hash.map(|h| hx(&h)),
                    adapter: AdapterId::OffencesV1,
                    action: ResolutionAction::ReplaceControl,
                    validated_edits: group.clone(),
                    observed_accounts: accounts,
                });
            }
            groups.push(group);
        }
    }
    let reports_count = before
        .keys()
        .filter(|k| k.starts_with(&reports.prefix))
        .count();
    if visited.len() != reports_count {
        return Err("orphan offence reports require explicit disposition".into());
    }
    if used_originals.len() != originals.len() {
        return Err("extraneous offence original preimage".into());
    }
    let mut witnesses: Vec<_> = before
        .keys()
        .chain(expected.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|key| StorageWitness {
            scope: StorageScope::Top,
            value_hash: expected.get(&key).map(|v| hash(v)),
            key,
        })
        .collect();
    if let Some((key, value)) = &denomination {
        witnesses.push(StorageWitness {
            scope: StorageScope::Top,
            key: key.clone(),
            value_hash: Some(hash(value)),
        });
        witnesses.sort_by(|a, b| a.key.cmp(&b.key));
    }
    empty.sort_by(|a, b| a.prefix.cmp(&b.prefix));
    let edits = groups.iter().flatten().cloned().collect();
    let compiled = Compilation {
        groups,
        edits,
        resolutions,
        witnesses,
        prefix_witnesses: empty,
        report: Report {
            evidence_hash: review.evidence_hash.clone(),
            reports: reports_count,
            concurrent_groups,
            changed_reports,
            rekeyed_reports,
            changed_indices,
            preserved_historical_ids,
            authenticated_original_preimages: used_originals.len(),
            offence_counts_preserved: true,
            native_slashing_executed: false,
            production_ready: false,
        },
        prefixes: vec![reports.prefix.clone(), indices.prefix.clone()],
        before,
        expected,
        mapping_hash: hash(&replacements.encode()),
        denomination,
    };
    compiled.verify_groups(&compiled.groups, replacements)?;
    Ok(compiled)
}
#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, Registry, TypeInfo};
    use sp_core::crypto::AccountId32;
    #[derive(Encode, TypeInfo)]
    struct Exposure {
        #[codec(compact)]
        total: u128,
        #[codec(compact)]
        own: u128,
        others: Vec<IndividualExposure>,
    }
    #[derive(Encode, TypeInfo)]
    struct IndividualExposure {
        who: AccountId32,
        #[codec(compact)]
        value: u128,
    }
    #[derive(Encode, TypeInfo)]
    struct Details {
        offender: (AccountId32, Exposure),
        reporters: Vec<AccountId32>,
    }
    fn fixture() -> (MetadataSchema, Storage) {
        let mut r = Registry::new();
        let id = r.register_type(&meta_type::<[u8; 32]>()).id;
        let kind = r.register_type(&meta_type::<[u8; 16]>()).id;
        let slot = r.register_type(&meta_type::<Vec<u8>>()).id;
        let details = r.register_type(&meta_type::<Details>()).id;
        let ids = r.register_type(&meta_type::<Vec<[u8; 32]>>()).id;
        let balance = r.register_type(&meta_type::<u128>()).id;
        let schema = MetadataSchema {
            registry: r.into(),
            metadata_hash: METADATA.into(),
            descriptors: vec![
                Descriptor {
                    trie: "top".into(),
                    name: "Denomination.Denominator".into(),
                    prefix: denominator_key(),
                    key_types: vec![],
                    hashers: vec![],
                    value_type: balance,
                },
                Descriptor {
                    trie: "top".into(),
                    name: "Offences.Reports".into(),
                    prefix: prefix("Reports"),
                    key_types: vec![id],
                    hashers: vec![Hasher::Twox64Concat],
                    value_type: details,
                },
                Descriptor {
                    trie: "top".into(),
                    name: "Offences.ConcurrentReportsIndex".into(),
                    prefix: prefix("ConcurrentReportsIndex"),
                    key_types: vec![kind, slot],
                    hashers: vec![Hasher::Twox64Concat, Hasher::Twox64Concat],
                    value_type: ids,
                },
            ],
        };
        let mut storage = Storage::default();
        let kind = *b"babe:equivocatio";
        let slot = 99u64.encode();
        let mut ids = Vec::new();
        for (owner, reporter) in [(1u8, 5u8), (3, 6)] {
            let details = Details {
                offender: (
                    [owner; 32].into(),
                    Exposure {
                        total: 30,
                        own: 10,
                        others: vec![
                            IndividualExposure {
                                who: [4; 32].into(),
                                value: 10,
                            },
                            IndividualExposure {
                                who: [2; 32].into(),
                                value: 10,
                            },
                        ],
                    },
                ),
                reporters: vec![[reporter; 32].into()],
            };
            let value = details.encode();
            let id = report_id(kind, &slot, &details.offender.encode());
            ids.push(id);
            storage.top.insert(
                report_key(descriptor(&schema, "Reports").unwrap(), &id),
                value,
            );
        }
        storage.top.insert(
            [
                prefix("ConcurrentReportsIndex"),
                Hasher::Twox64Concat.apply(&kind),
                Hasher::Twox64Concat.apply(&slot.encode()),
            ]
            .concat(),
            ids.encode(),
        );
        (schema, storage)
    }
    fn review(s: &MetadataSchema, storage: &Storage) -> Review {
        Review {
            evidence_hash: hx(&[7; 32]),
            original_offenders: Vec::new(),
            records: storage
                .top
                .iter()
                .filter(|(k, _)| {
                    k.starts_with(&prefix("Reports"))
                        || k.starts_with(&prefix("ConcurrentReportsIndex"))
                })
                .map(|(k, v)| Record {
                    storage: s
                        .descriptors
                        .iter()
                        .find(|d| k.starts_with(&d.prefix))
                        .unwrap()
                        .name
                        .clone(),
                    key: hx(k),
                    value_hash: hx(&hash(v)),
                })
                .collect(),
        }
    }
    fn replacements() -> Vec<AccountReplacement> {
        vec![
            AccountReplacement {
                old: [1; 32].into(),
                new: [9; 32].into(),
            },
            AccountReplacement {
                old: [4; 32].into(),
                new: [8; 32].into(),
            },
            AccountReplacement {
                old: [5; 32].into(),
                new: [7; 32].into(),
            },
        ]
    }
    #[test]
    fn rehashes_full_exposure_identity_and_reindexes_without_severity_count_change() {
        let (s, storage) = fixture();
        let map = replacements();
        let c = compile(&s, &storage, &map, &review(&s, &storage)).unwrap();
        assert_eq!(c.report.reports, 2);
        assert_eq!(c.report.concurrent_groups, 1);
        assert_eq!(c.report.rekeyed_reports, 2);
        assert_eq!(c.report.changed_indices, 1);
        assert_eq!(c.groups.len(), 1);
        assert_eq!(c.edits.len(), 5);
        c.verify_groups(&c.groups, &map).unwrap();
        let mut projected = storage.clone();
        for e in &c.edits {
            if let Some(v) = &e.after {
                projected.top.insert(e.key.clone(), v.clone());
            } else {
                projected.top.remove(&e.key);
            }
        }
        let again = compile(&s, &projected, &[], &review(&s, &projected)).unwrap();
        assert_eq!(again.report.reports, 2);
        assert!(again.edits.is_empty());
        let mut omitted = c.groups.clone();
        omitted[0].pop();
        assert!(c.verify_groups(&omitted, &map).is_err());
        assert!(c.verify_groups(&c.groups, &[]).is_err());
    }
    #[test]
    fn reporter_only_replacement_preserves_report_ids_and_index_bytes() {
        let (s, storage) = fixture();
        let map = vec![AccountReplacement {
            old: [5; 32].into(),
            new: [7; 32].into(),
        }];
        let c = compile(&s, &storage, &map, &review(&s, &storage)).unwrap();
        assert_eq!(c.report.changed_reports, 1);
        assert_eq!(c.report.rekeyed_reports, 0);
        assert_eq!(c.report.changed_indices, 0);
        assert_eq!(c.edits.len(), 1);
        assert!(c.edits[0].after.is_some());
    }
    #[test]
    fn rejects_orphans_missing_reports_duplicate_references_and_affected_missing_preimages() {
        let (s, mut storage) = fixture();
        let index = storage
            .top
            .keys()
            .find(|k| k.starts_with(&prefix("ConcurrentReportsIndex")))
            .unwrap()
            .clone();
        storage.top.remove(&index);
        assert!(compile(&s, &storage, &[], &review(&s, &storage)).is_err());
        let (s, mut storage) = fixture();
        let report = storage
            .top
            .keys()
            .find(|k| k.starts_with(&prefix("Reports")))
            .unwrap()
            .clone();
        storage.top.remove(&report);
        assert!(compile(&s, &storage, &[], &review(&s, &storage)).is_err());
        let (s, mut storage) = fixture();
        let index = storage
            .top
            .keys()
            .find(|k| k.starts_with(&prefix("ConcurrentReportsIndex")))
            .unwrap()
            .clone();
        let mut ids = Vec::<[u8; 32]>::decode(&mut storage.top[&index].as_slice()).unwrap();
        ids.push(ids[0]);
        storage.top.insert(index, ids.encode());
        assert!(compile(&s, &storage, &[], &review(&s, &storage)).is_err());
        let (s, mut storage) = fixture();
        let report = storage
            .top
            .keys()
            .find(|k| k.starts_with(&prefix("Reports")))
            .unwrap()
            .clone();
        storage.top.get_mut(&report).unwrap()[0] ^= 1;
        assert!(compile(&s, &storage, &replacements(), &review(&s, &storage)).is_err());
    }
    #[test]
    fn rejects_identity_collision_missing_evidence_and_wrong_source_metadata() {
        let (mut s, storage) = fixture();
        let map = vec![AccountReplacement {
            old: [1; 32].into(),
            new: [3; 32].into(),
        }];
        assert!(compile(&s, &storage, &map, &review(&s, &storage)).is_err());
        let mut r = review(&s, &storage);
        r.records.pop();
        assert!(compile(&s, &storage, &[], &r).is_err());
        s.metadata_hash = hx(&[1; 32]);
        assert!(compile(&s, &storage, &[], &review(&s, &storage)).is_err());
    }
    fn denominated_fixture() -> (MetadataSchema, Storage, Review) {
        let (schema, mut storage) = fixture();
        let descriptor = descriptor(&schema, "Reports").unwrap();
        let mut originals = Vec::new();
        for (key, bytes) in storage
            .top
            .iter_mut()
            .filter(|(k, _)| k.starts_with(&descriptor.prefix))
        {
            let original = offender_bytes(&schema, descriptor, bytes).unwrap();
            let mut input = original.as_slice();
            let (owner, mut exposure) =
                <(AccountId32, OriginalExposure)>::decode(&mut input).unwrap();
            exposure.own /= 100;
            exposure.total = exposure.own;
            for nominator in &mut exposure.others {
                nominator.value /= 100;
                exposure.total += nominator.value;
            }
            let mut after = (owner, exposure).encode();
            after.extend_from_slice(&bytes[original.len()..]);
            *bytes = after;
            originals.push(OriginalOffender {
                report_id: hx(&key[key.len() - 32..]),
                offender: hx(&original),
                denominator: "100".into(),
                source_evidence_hash: hx(&[8; 32]),
            });
        }
        storage.top.insert(denominator_key(), 10_000u128.encode());
        let mut review = review(&schema, &storage);
        review.original_offenders = originals;
        (schema, storage, review)
    }
    #[test]
    fn untouched_denominated_reports_preserve_historical_ids_and_exact_index() {
        let (s, storage, _) = denominated_fixture();
        let c = compile(&s, &storage, &[], &review(&s, &storage)).unwrap();
        assert!(c.edits.is_empty());
        assert_eq!(c.report.preserved_historical_ids, 2);
        assert_eq!(c.report.authenticated_original_preimages, 0);
        let map = vec![AccountReplacement {
            old: [5; 32].into(),
            new: [7; 32].into(),
        }];
        let c = compile(&s, &storage, &map, &review(&s, &storage)).unwrap();
        assert_eq!(c.edits.len(), 1);
        assert_eq!(c.report.rekeyed_reports, 0);
        assert_eq!(c.report.preserved_historical_ids, 2);
    }
    #[test]
    fn denominated_identity_rehashes_authenticated_original_but_keeps_checkpoint_financial_values()
    {
        let (s, storage, review) = denominated_fixture();
        let map = replacements();
        let c = compile(&s, &storage, &map, &review).unwrap();
        assert_eq!(c.report.authenticated_original_preimages, 2);
        assert_eq!(c.report.rekeyed_reports, 2);
        assert_eq!(c.report.changed_indices, 1);
        let mappings = map
            .iter()
            .map(|p| {
                (
                    *AsRef::<[u8; 32]>::as_ref(&p.old),
                    *AsRef::<[u8; 32]>::as_ref(&p.new),
                )
            })
            .collect();
        for proof in &review.original_offenders {
            let original = hex_bytes(&proof.offender).unwrap();
            let old_id: [u8; 32] = hex_bytes(&proof.report_id).unwrap().try_into().unwrap();
            let descriptor = descriptor(&s, "Reports").unwrap();
            let old_value = &storage.top[&report_key(descriptor, &old_id)];
            let ty = offender_value(&s, descriptor, old_value).unwrap().context;
            let (remapped, _) =
                typed::transform_value(&original, ty, &s.registry, &mappings).unwrap();
            let new_id = report_id(*b"babe:equivocatio", &99u64.encode(), &remapped);
            let value = c.expected.get(&report_key(descriptor, &new_id)).unwrap();
            let offender = offender_bytes(&s, descriptor, value).unwrap();
            let (_, exposure) =
                <(AccountId32, OriginalExposure)>::decode(&mut offender.as_slice()).unwrap();
            assert_eq!(exposure.total, 0);
            assert_eq!(exposure.own, 0);
            assert!(exposure.others.iter().all(|n| n.value == 0));
            assert_ne!(
                report_id(*b"babe:equivocatio", &99u64.encode(), &offender),
                new_id
            );
        }
        c.verify_groups(&c.groups, &map).unwrap();
        let mut illegal = c.groups.clone();
        illegal.push(vec![GuardedStorageEdit {
            scope: StorageScope::Top,
            key: denominator_key(),
            before: Some(hash(&10_000u128.encode())),
            after: Some(1u128.encode()),
        }]);
        assert!(c.verify_groups(&illegal, &map).is_err());
    }
    #[test]
    fn denominated_affected_reports_reject_missing_false_unused_or_noncanonical_preimages() {
        let (s, storage, mut review) = denominated_fixture();
        let map = replacements();
        assert!(
            compile(&s, &storage, &map, &super::tests::review(&s, &storage))
                .err()
                .unwrap()
                .contains("requires an authenticated original")
        );
        assert!(compile(&s, &storage, &[], &review).is_err());
        review.original_offenders[0].denominator = "3".into();
        assert!(compile(&s, &storage, &map, &review).is_err());
        review.original_offenders[0].denominator = "0100".into();
        assert!(compile(&s, &storage, &map, &review).is_err());
        review.original_offenders[0].denominator = "100".into();
        let original = review.original_offenders[0].offender.clone();
        review.original_offenders[0].offender = original.replacen("0x", "0xff", 1);
        assert!(compile(&s, &storage, &map, &review).is_err());
        review.original_offenders[0].offender = original;
        review.original_offenders[0].denominator = "2".into();
        assert!(compile(&s, &storage, &map, &review).is_err());
        review.original_offenders[0].denominator = "100".into();
        let mut wrong_order = storage.clone();
        let key = report_key(
            descriptor(&s, "Reports").unwrap(),
            &hex_bytes(&review.original_offenders[0].report_id)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        let old_value = wrong_order.top.get(&key).unwrap();
        let old_offender =
            offender_bytes(&s, descriptor(&s, "Reports").unwrap(), old_value).unwrap();
        let (owner, mut exposure) =
            <(AccountId32, OriginalExposure)>::decode(&mut old_offender.as_slice()).unwrap();
        exposure.others.swap(0, 1);
        let mut reordered = (owner, exposure).encode();
        reordered.extend_from_slice(&old_value[old_offender.len()..]);
        wrong_order.top.insert(key, reordered);
        let mut revised = super::tests::review(&s, &wrong_order);
        revised.original_offenders = review.original_offenders;
        assert!(compile(&s, &wrong_order, &map, &revised).is_err());
    }
}
