//! Complete finite cancellation of checkpoint-known native bridge signing hashes.
//! Original signatures, pending flags and latest commitments remain immutable.
//! No arbitrary hash or inferred batch/message relationship is admitted.
use crate::{
    hex_bytes,
    inventory::{Descriptor, Hasher, MetadataSchema},
    typed, Result,
};
use bridge_types::{GenericCommitment, GenericNetworkId, SubNetworkId, H256};
use codec::{Decode, DecodeWithMemLimit, DecodeWithMemTracking, Encode};
use common::recovery::{
    self, GuardedStorageEdit, StoragePrefixWitness, StorageScope, StorageWitness,
};
use serde::{Deserialize, Serialize};
use sp_core::{ecdsa, storage::Storage};
use std::collections::{BTreeMap, BTreeSet};
const METADATA: &str = "0x0c1d4bf5c3bc5037ad854932443c94faae6c327ecf1f38b3e77d38668cc2b4b8";
const MAX_ROWS: usize = 100_000;
const MAX_BYTES: usize = 64 * 1024 * 1024;
pub const ITEMS: [&str; 4] = [
    "BridgeDataSigner.Approvals",
    "BridgeDataSigner.PendingApprovals",
    "BridgeOutboundChannel.LatestCommitment",
    "SubstrateBridgeOutboundChannel.LatestCommitment",
];
type SigningSources = BTreeMap<(Vec<u8>, H256), Vec<(Vec<u8>, [u8; 32])>>;
type Approvals =
    sp_runtime::BoundedBTreeMap<ecdsa::Public, ecdsa::Signature, framenode_runtime::BridgeMaxPeers>;
type Latest = bridge_types::types::GenericCommitmentWithBlock<
    u32,
    framenode_runtime::BridgeMaxMessagesPerCommit,
    framenode_runtime::BridgeMaxMessagePayloadSize,
>;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Review {
    pub evidence_hash: String,
    pub cancel_all_known_signing_commitments: bool,
    pub records: Vec<Record>,
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
    pub original_records: usize,
    pub approvals_records: usize,
    pub pending_approval_records: usize,
    pub latest_commitments: usize,
    pub quarantined_signing_hashes: usize,
    pub signatures_changed: bool,
    pub external_authorizations_invalidated: bool,
    pub complete_historical_batch_membership: bool,
    pub refunds_created: u32,
    pub production_ready: bool,
}
pub struct Compilation {
    pub groups: Vec<Vec<GuardedStorageEdit>>,
    pub witnesses: Vec<StorageWitness>,
    pub prefix_witnesses: Vec<StoragePrefixWitness>,
    pub report: Report,
    before: BTreeMap<Vec<u8>, Vec<u8>>,
    expected: BTreeMap<Vec<u8>, Vec<u8>>,
    prefixes: Vec<Vec<u8>>,
}
impl Compilation {
    pub fn verify_groups(&self, groups: &[Vec<GuardedStorageEdit>]) -> Result<()> {
        let mut projected = self.before.clone();
        let mut seen = BTreeSet::new();
        for edit in groups.iter().flatten().filter(|e| {
            e.scope == StorageScope::Top && self.prefixes.iter().any(|p| e.key.starts_with(p))
        }) {
            if !seen.insert(edit.key.clone())
                || self.before.get(&edit.key).map(|v| hash(v)) != edit.before
            {
                return Err("duplicate/stale bridge signing cancellation edit".into());
            }
            if let Some(value) = &edit.after {
                projected.insert(edit.key.clone(), value.clone());
            } else {
                projected.remove(&edit.key);
            }
        }
        if projected != self.expected {
            return Err("bridge signing cancellation must preserve all native evidence and every known hash tombstone".into());
        }
        Ok(())
    }
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(bytes)
}
#[cfg(test)]
fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn prefix(name: &str) -> Result<Vec<u8>> {
    let (pallet, item) = name.split_once('.').ok_or("bridge signing storage name")?;
    Ok([
        sp_io::hashing::twox_128(pallet.as_bytes()),
        sp_io::hashing::twox_128(item.as_bytes()),
    ]
    .concat())
}
fn decode<T: DecodeWithMemTracking + Encode>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > typed::MAX_TYPED_BYTES {
        return Err("bridge signing value bound".into());
    }
    let mut input = bytes;
    let value = T::decode_with_mem_limit(&mut input, typed::MAX_TYPED_BYTES * 2)
        .map_err(|e| e.to_string())?;
    if !input.is_empty() || value.encode() != bytes {
        return Err("noncanonical native bridge signing codec".into());
    }
    Ok(value)
}
fn descriptor<'a>(schema: &'a MetadataSchema, name: &str) -> Result<&'a Descriptor> {
    let d = schema
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == name)
        .ok_or("bridge signing descriptor missing")?;
    let count = if name.starts_with("BridgeDataSigner.") {
        2
    } else {
        1
    };
    if d.prefix != prefix(name)?
        || d.key_types.len() != count
        || d.hashers != vec![Hasher::Identity; count]
    {
        return Err("bridge signing exact key layout differs".into());
    }
    Ok(d)
}
pub fn compile(schema: &MetadataSchema, storage: &Storage, review: &Review) -> Result<Compilation> {
    let evidence: [u8; 32] = hex_bytes(&review.evidence_hash)?
        .try_into()
        .map_err(|_| "bridge signing evidence width")?;
    if schema.metadata_hash != METADATA
        || evidence == [0; 32]
        || !review.cancel_all_known_signing_commitments
        || review.records.len() > MAX_ROWS
    {
        return Err("bridge signing exact metadata, complete cancellation policy and bounded review required".into());
    }
    let marker_prefix = bridge_types::recovery::HISTORICAL_COMMITMENT_PREFIX.to_vec();
    if storage
        .top
        .range(marker_prefix.clone()..)
        .next()
        .is_some_and(|(k, _)| k.starts_with(&marker_prefix))
    {
        return Err("historical signing markers already present at checkpoint".into());
    }
    let mut declared = BTreeMap::new();
    for row in &review.records {
        let h: [u8; 32] = hex_bytes(&row.value_hash)?
            .try_into()
            .map_err(|_| "bridge signing value hash width")?;
        if declared
            .insert(hex_bytes(&row.key)?, (row.storage.as_str(), h))
            .is_some()
        {
            return Err("duplicate bridge signing review".into());
        }
    }
    let mut before = BTreeMap::new();
    let mut prefixes = vec![marker_prefix];
    let mut empty = Vec::new();
    let mut total = 0usize;
    let mut counts = [0usize; 4];
    let mut targets = SigningSources::new();
    for (index, name) in ITEMS.iter().enumerate() {
        let d = descriptor(schema, name)?;
        prefixes.push(d.prefix.clone());
        for (key, value) in storage
            .top
            .range(d.prefix.clone()..)
            .take_while(|(k, _)| k.starts_with(&d.prefix))
        {
            total = total
                .checked_add(value.len())
                .filter(|n| *n <= MAX_BYTES)
                .ok_or("bridge signing aggregate bound")?;
            if before.len() >= MAX_ROWS || declared.get(key) != Some(&(*name, hash(value))) {
                return Err("bridge signing complete record review mismatch".into());
            }
            typed::decode_exact(value, d.value_type, &schema.registry)?;
            crate::inventory::transform_key(key, d, &schema.registry, &BTreeMap::new(), None)?;
            let mut encoded = &key[d.prefix.len()..];
            let (network, id) = if index < 2 {
                let network = GenericNetworkId::decode(&mut encoded).map_err(|e| e.to_string())?;
                let id = decode::<H256>(encoded)?;
                if index == 0 {
                    decode::<Approvals>(value)?;
                } else {
                    decode::<bool>(value)?;
                }
                (network, id)
            } else {
                let network = if index == 2 {
                    decode::<GenericNetworkId>(encoded)?
                } else {
                    decode::<SubNetworkId>(encoded)?.into()
                };
                let latest = decode::<Latest>(value)?;
                if latest.block_number > recovery::CHECKPOINT_NUMBER
                    || !matches!(
                        (network, &latest.commitment),
                        (GenericNetworkId::Sub(_), GenericCommitment::Sub(_))
                            | (
                                GenericNetworkId::EVM(_),
                                GenericCommitment::EVM(bridge_types::evm::Commitment::Outbound(_))
                            )
                    )
                {
                    return Err(
                        "bridge signing latest commitment network/direction/block mismatch".into(),
                    );
                }
                (network, latest.commitment.hash())
            };
            targets
                .entry((network.encode(), id))
                .or_default()
                .push((key.clone(), hash(value)));
            before.insert(key.clone(), value.clone());
            counts[index] += 1;
        }
        if counts[index] == 0 {
            empty.push(StoragePrefixWitness {
                scope: StorageScope::Top,
                prefix: d.prefix.clone(),
            });
        }
    }
    if before.len() != declared.len() {
        return Err("extraneous bridge signing review record".into());
    }
    let mut expected = before.clone();
    let mut groups = Vec::new();
    for ((encoded_network, id), refs) in &targets {
        if refs.is_empty() || refs.len() > 4 {
            return Err("bridge signing source coverage bound".into());
        }
        let network = decode::<GenericNetworkId>(encoded_network)?;
        let key = bridge_types::recovery::historical_commitment_key(network, *id);
        let value = (*b"snxsign1", recovery::CHECKPOINT_HASH, evidence, refs).encode();
        expected.insert(key.clone(), value.clone());
        groups.push(vec![GuardedStorageEdit {
            scope: StorageScope::Top,
            key,
            before: None,
            after: Some(value),
        }]);
    }
    let witnesses = expected
        .iter()
        .map(|(key, value)| StorageWitness {
            scope: StorageScope::Top,
            key: key.clone(),
            value_hash: Some(hash(value)),
        })
        .collect();
    empty.sort_by(|a, b| a.prefix.cmp(&b.prefix));
    let result = Compilation {
        groups,
        witnesses,
        prefix_witnesses: empty,
        report: Report {
            evidence_hash: review.evidence_hash.clone(),
            original_records: before.len(),
            approvals_records: counts[0],
            pending_approval_records: counts[1],
            latest_commitments: counts[2] + counts[3],
            quarantined_signing_hashes: targets.len(),
            signatures_changed: false,
            external_authorizations_invalidated: false,
            complete_historical_batch_membership: false,
            refunds_created: 0,
            production_ready: false,
        },
        before,
        expected,
        prefixes,
    };
    result.verify_groups(&result.groups)?;
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, Registry};
    fn fixture() -> (MetadataSchema, Storage, Review) {
        let mut registry = Registry::new();
        let network = registry.register_type(&meta_type::<GenericNetworkId>()).id;
        let sub = registry.register_type(&meta_type::<SubNetworkId>()).id;
        let id = registry.register_type(&meta_type::<H256>()).id;
        let approvals = registry.register_type(&meta_type::<Approvals>()).id;
        let flag = registry.register_type(&meta_type::<bool>()).id;
        let latest = registry.register_type(&meta_type::<Latest>()).id;
        let mut descriptors = Vec::new();
        for (index, name) in ITEMS.iter().enumerate() {
            descriptors.push(Descriptor {
                trie: "top".into(),
                name: (*name).into(),
                prefix: prefix(name).unwrap(),
                key_types: if index < 2 {
                    vec![network, id]
                } else if index == 2 {
                    vec![network]
                } else {
                    vec![sub]
                },
                hashers: vec![Hasher::Identity; if index < 2 { 2 } else { 1 }],
                value_type: [approvals, flag, latest, latest][index],
            });
        }
        let schema = MetadataSchema {
            metadata_hash: METADATA.into(),
            registry: registry.into(),
            descriptors,
        };
        let mut storage = Storage::default();
        let network = GenericNetworkId::Sub(SubNetworkId::Liberland);
        let hash = H256::repeat_byte(7);
        storage.top.insert(
            [prefix(ITEMS[0]).unwrap(), network.encode(), hash.encode()].concat(),
            Approvals::default().encode(),
        );
        storage.top.insert(
            [prefix(ITEMS[1]).unwrap(), network.encode(), hash.encode()].concat(),
            true.encode(),
        );
        let review = review(&storage);
        (schema, storage, review)
    }
    fn review(storage: &Storage) -> Review {
        Review {
            evidence_hash: hx(&[9; 32]),
            cancel_all_known_signing_commitments: true,
            records: storage
                .top
                .iter()
                .map(|(key, value)| Record {
                    storage: ITEMS
                        .iter()
                        .find(|name| key.starts_with(&prefix(name).unwrap()))
                        .unwrap()
                        .to_string(),
                    key: hx(key),
                    value_hash: hx(&hash(value)),
                })
                .collect(),
        }
    }
    #[test]
    fn complete_native_hash_union_creates_one_stop_and_preserves_all_evidence() {
        let (s, storage, r) = fixture();
        let c = compile(&s, &storage, &r).unwrap();
        assert_eq!(c.report.original_records, 2);
        assert_eq!(c.report.quarantined_signing_hashes, 1);
        assert_eq!(c.groups.len(), 1);
        assert_eq!(c.prefix_witnesses.len(), 2);
        for (k, v) in &storage.top {
            assert_eq!(c.expected.get(k), Some(v));
        }
        assert!(c.verify_groups(&[]).is_err());
        let mut tampered = c.groups.clone();
        let (k, v) = storage.top.iter().next().unwrap();
        tampered.push(vec![GuardedStorageEdit {
            scope: StorageScope::Top,
            key: k.clone(),
            before: Some(hash(v)),
            after: None,
        }]);
        assert!(c.verify_groups(&tampered).is_err());
    }
    #[test]
    fn missing_review_unknown_hash_and_malformed_native_rows_fail() {
        let (s, mut storage, mut r) = fixture();
        r.records.pop();
        assert!(compile(&s, &storage, &r).is_err());
        let key = storage
            .top
            .keys()
            .find(|k| k.starts_with(&prefix(ITEMS[1]).unwrap()))
            .unwrap()
            .clone();
        storage.top.insert(key, vec![2]);
        assert!(compile(&s, &storage, &review(&storage)).is_err());
        let (s, storage, mut r) = fixture();
        r.cancel_all_known_signing_commitments = false;
        assert!(compile(&s, &storage, &r).is_err());
    }
    #[test]
    fn exact_latest_preimage_adds_hash_without_inventing_archive_membership() {
        let (s, mut storage, _) = fixture();
        let latest = Latest {
            block_number: recovery::CHECKPOINT_NUMBER,
            commitment: GenericCommitment::Sub(bridge_types::substrate::Commitment {
                nonce: 123,
                messages: Vec::new().try_into().unwrap(),
            }),
        };
        let id = latest.commitment.hash();
        let network = SubNetworkId::Liberland;
        storage.top.insert(
            [prefix(ITEMS[3]).unwrap(), network.encode()].concat(),
            latest.encode(),
        );
        let c = compile(&s, &storage, &review(&storage)).unwrap();
        assert_eq!(c.report.quarantined_signing_hashes, 2);
        assert!(c
            .expected
            .contains_key(&bridge_types::recovery::historical_commitment_key(
                network.into(),
                id
            )));
        assert!(!c.report.complete_historical_batch_membership);
        assert!(!c.report.external_authorizations_invalidated);
    }
}
