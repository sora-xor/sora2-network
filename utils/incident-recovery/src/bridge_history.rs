//! Coupled local ownership of terminal bridge history. External addresses and
//! delivered payload evidence are immutable; completed transfers create no refund.

use crate::{
    hex_bytes,
    inventory::{
        AdapterId, AdapterResolution, Descriptor, Hasher, MetadataSchema, ResolutionAction,
    },
    typed, Result,
};
use bridge_types::{
    recovery::HistoricalMessageTombstone,
    types::{MessageDirection, MessageStatus},
    GenericAccount, GenericNetworkId, GenericTimepoint, H256,
};
use codec::{Decode, DecodeWithMemLimit, DecodeWithMemTracking, Encode};
use common::recovery::{
    self, AccountReplacement, GuardedStorageEdit, StorageScope, StorageWitness,
};
use framenode_runtime::AssetId;
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, storage::Storage};
use std::collections::{BTreeMap, BTreeSet};

#[path = "bridge_pending.rs"]
mod pending;

const METADATA: &str = "0x0c1d4bf5c3bc5037ad854932443c94faae6c327ecf1f38b3e77d38668cc2b4b8";
const MARKERS: &[u8] = b":sora-nexus:historical-outgoing-message:v1:";
const MAX_ROWS: usize = 200_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Review {
    pub evidence_hash: String,
    /// Explicit policy action. False suppresses affected-owner terminal refunds
    /// only, and never claims complete cancellation of all checkpoint outgoing.
    pub cancel_all_terminal_outgoing: bool,
    /// Explicitly stop and quarantine every nonterminal outgoing liability;
    /// this conveys no refund or counterpart redemption evidence.
    #[serde(default)]
    pub quarantine_all_nonterminal_outgoing: bool,
    pub records: Vec<Record>,
    /// Exact, untouched historical bytes whose apparent native owner is invalid.
    /// These records never establish a direction, financial claim or refund.
    #[serde(default)]
    pub preserved_anomalies: Vec<PreservedAnomaly>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreservedAnomaly {
    pub sender_key: String,
    pub sender_value_hash: String,
    pub transaction_key: String,
    pub transaction_value_hash: String,
    pub source_evidence_hash: String,
    pub reason: AnomalyReason,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnomalyReason {
    NativeOwnerMismatch,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnresolvedAnomaly {
    pub network_scale: String,
    pub message_id: String,
    pub sender_key: String,
    pub transaction_key: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Record {
    pub storage: String,
    pub key: String,
    pub value_hash: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub evidence_hash: String,
    pub paired_requests: usize,
    pub remapped_requests: usize,
    pub terminal_outgoing_tombstones: usize,
    pub nonterminal_outgoing_requests: usize,
    pub quarantined_nonterminal_outgoing: usize,
    pub remapped_nonterminal_outgoing: usize,
    pub extinguished_attacker_pending_entitlements: usize,
    pub quarantine_all_nonterminal_outgoing: bool,
    pub cancel_all_terminal_outgoing: bool,
    pub external_addresses_changed: bool,
    pub refunds_created: u32,
    pub batch_authorizations_reconciled: bool,
    pub unresolved_quarantined_anomalies: Vec<UnresolvedAnomaly>,
    pub complete_financial_interpretation: bool,
    /// This storage compiler does not execute native admission, status or credit
    /// paths. Their guard regressions and exact-artifact runs are separate evidence.
    pub anomaly_status_and_inbound_guards_complete: bool,
    pub production_ready: bool,
}
pub struct Compilation {
    pub groups: Vec<Vec<GuardedStorageEdit>>,
    pub edits: Vec<GuardedStorageEdit>,
    pub resolutions: Vec<AdapterResolution>,
    pub witnesses: Vec<StorageWitness>,
    pub report: Report,
    before: BTreeMap<Vec<u8>, Vec<u8>>,
    expected: BTreeMap<Vec<u8>, Vec<u8>>,
    prefixes: Vec<Vec<u8>>,
    mapping_hash: [u8; 32],
    atomic_sets: Vec<BTreeSet<Vec<u8>>>,
}
impl Compilation {
    pub fn verify_groups(
        &self,
        groups: &[Vec<GuardedStorageEdit>],
        replacements: &[AccountReplacement],
    ) -> Result<()> {
        if hash(&replacements.encode()) != self.mapping_hash {
            return Err("bridge history mapping certificate changed".into());
        }
        let group_for_key: BTreeMap<_, _> = groups
            .iter()
            .enumerate()
            .flat_map(|(index, group)| {
                group
                    .iter()
                    .filter(|e| e.scope == StorageScope::Top)
                    .map(move |e| (&e.key, index))
            })
            .collect();
        for required in &self.atomic_sets {
            let mut indexes = required.iter().map(|key| group_for_key.get(key));
            let first = indexes.next().flatten();
            if first.is_none() || indexes.any(|index| index != first) {
                return Err(
                    "bridge ownership and refund suppression must remain one atomic group".into(),
                );
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
                return Err("duplicate/stale bridge history edits".into());
            }
            if let Some(after) = &edit.after {
                projected.insert(edit.key.clone(), after.clone());
            } else {
                projected.remove(&edit.key);
            }
        }
        if projected != self.expected {
            return Err(
                "incomplete bridge ownership or permanent-refund-suppression projection".into(),
            );
        }
        Ok(())
    }
}

/// Exact deployed BridgeRequest field order; additionally decoded through the
/// executed checkpoint metadata, with complete re-encoding equality required.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, scale_info::TypeInfo,
)]
struct Request {
    source: GenericAccount,
    dest: GenericAccount,
    asset_id: AssetId,
    amount: u128,
    status: MessageStatus,
    start_timepoint: GenericTimepoint,
    end_timepoint: GenericTimepoint,
    direction: MessageDirection,
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(bytes)
}
fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn bytes32(text: &str) -> Result<[u8; 32]> {
    hex_bytes(text)?
        .try_into()
        .map_err(|_| "bridge evidence/account hash width".into())
}
fn prefix(item: &str) -> Vec<u8> {
    [
        sp_io::hashing::twox_128(b"BridgeProxy"),
        sp_io::hashing::twox_128(item.as_bytes()),
    ]
    .concat()
}
fn desc<'a>(schema: &'a MetadataSchema, item: &str, hashers: &[Hasher]) -> Result<&'a Descriptor> {
    let d = schema
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == format!("BridgeProxy.{item}"))
        .ok_or("missing bridge history metadata")?;
    if d.prefix != prefix(item) || d.hashers != hashers || d.key_types.len() != 2 {
        return Err("bridge history key layout changed".into());
    }
    Ok(d)
}
fn sender_key(network: GenericNetworkId, message: H256) -> Vec<u8> {
    [
        prefix("Senders"),
        sp_io::hashing::twox_64(&network.encode()).to_vec(),
        network.encode(),
        sp_io::hashing::blake2_128(&message.encode()).to_vec(),
        message.encode(),
    ]
    .concat()
}
fn transaction_key(network: GenericNetworkId, owner: &AccountId32, message: H256) -> Vec<u8> {
    let first = (network, owner).encode();
    [
        prefix("Transactions"),
        sp_io::hashing::blake2_128(&first).to_vec(),
        first,
        sp_io::hashing::blake2_128(&message.encode()).to_vec(),
        message.encode(),
    ]
    .concat()
}
fn decode<T: DecodeWithMemTracking + Encode>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > 32 * 1024 {
        return Err("bridge historical record bound".into());
    }
    let mut input = bytes;
    let value = T::decode_with_mem_limit(&mut input, 128 * 1024).map_err(|e| e.to_string())?;
    if !input.is_empty() || value.encode() != bytes {
        return Err("noncanonical bridge historical encoding".into());
    }
    Ok(value)
}
fn sender_identity(key: &[u8]) -> Result<(GenericNetworkId, H256)> {
    let mut bytes = key.get(40..).ok_or("short sender key")?;
    let network = GenericNetworkId::decode(&mut bytes).map_err(|e| e.to_string())?;
    let message = decode::<H256>(bytes.get(16..).ok_or("short sender message key")?)?;
    if sender_key(network, message) != key {
        return Err("invalid bridge sender key hash".into());
    }
    Ok((network, message))
}
fn transaction_identity(key: &[u8]) -> Result<(GenericNetworkId, AccountId32, H256)> {
    let mut bytes = key.get(48..).ok_or("short transaction key")?;
    let (network, owner) =
        <(GenericNetworkId, AccountId32)>::decode(&mut bytes).map_err(|e| e.to_string())?;
    let message = decode::<H256>(bytes.get(16..).ok_or("short transaction message key")?)?;
    if transaction_key(network, &owner, message) != key {
        return Err("invalid bridge transaction key hash".into());
    }
    Ok((network, owner, message))
}

pub fn compile(
    schema: &MetadataSchema,
    storage: &Storage,
    replacements: &[AccountReplacement],
    review: &Review,
) -> Result<Compilation> {
    recovery::validate_replacements(replacements)
        .map_err(|e| format!("bridge history mapping: {e:?}"))?;
    let evidence = bytes32(&review.evidence_hash)?;
    if schema.metadata_hash != METADATA
        || evidence == [0; 32]
        || review.records.len() > MAX_ROWS
        || review.preserved_anomalies.len() > 256
    {
        return Err("bridge history requires exact metadata and bounded review".into());
    }
    let sd = desc(
        schema,
        "Senders",
        &[Hasher::Twox64Concat, Hasher::Blake2_128Concat],
    )?;
    let td = desc(
        schema,
        "Transactions",
        &[Hasher::Blake2_128Concat, Hasher::Blake2_128Concat],
    )?;
    if storage
        .top
        .range(MARKERS.to_vec()..)
        .next()
        .is_some_and(|(k, _)| k.starts_with(MARKERS))
    {
        return Err("checkpoint unexpectedly contains historical recovery markers".into());
    }
    let mapping: BTreeMap<_, _> = replacements
        .iter()
        .map(|p| (p.old.clone(), p.new.clone()))
        .collect();
    let mut reviewed = BTreeMap::new();
    for row in &review.records {
        if reviewed
            .insert(
                hex_bytes(&row.key)?,
                (row.storage.clone(), bytes32(&row.value_hash)?),
            )
            .is_some()
        {
            return Err("duplicate bridge history evidence".into());
        }
    }
    let mut before = BTreeMap::new();
    let mut observed = BTreeMap::new();
    let mut names = BTreeMap::new();
    let mut senders = BTreeMap::new();
    let mut transactions = BTreeMap::new();
    let mut owner_mismatches = BTreeSet::new();
    let mut total = 0usize;
    for d in [sd, td] {
        for (key, value) in storage
            .top
            .range(d.prefix.clone()..)
            .take_while(|(k, _)| k.starts_with(&d.prefix))
        {
            total = total
                .checked_add(value.len())
                .ok_or("bridge history size overflow")?;
            if before.len() >= MAX_ROWS || total > 64 * 1024 * 1024 {
                return Err("bridge history inventory bound".into());
            }
            typed::decode_exact(value, d.value_type, &schema.registry)?;
            crate::inventory::transform_key(key, d, &schema.registry, &BTreeMap::new(), None)?;
            observed.insert(key.clone(), (d.name.clone(), hash(value)));
            names.insert(key.clone(), d.name.clone());
            before.insert(key.clone(), value.clone());
            if d.name == sd.name {
                let (network, message) = sender_identity(key)?;
                let identity = (network.encode(), message);
                let owner = decode::<AccountId32>(value)?;
                if senders.insert(identity, owner).is_some() {
                    return Err("duplicate bridge sender identity".into());
                }
            } else {
                let (network, owner, message) = transaction_identity(key)?;
                let record = decode::<Request>(value)?;
                let local = match record.direction {
                    MessageDirection::Inbound => &record.dest,
                    MessageDirection::Outbound => &record.source,
                };
                if local != &GenericAccount::Sora(owner.clone()) {
                    owner_mismatches.insert(key.clone());
                }
                if transactions
                    .insert((network.encode(), message), (network, owner, record))
                    .is_some()
                {
                    return Err("multiple bridge owners share one message identity".into());
                }
            }
        }
    }
    if reviewed != observed {
        return Err("missing/extraneous bridge history evidence".into());
    }
    if senders.len() != transactions.len()
        || transactions
            .iter()
            .any(|(identity, (_, owner, _))| senders.get(identity) != Some(owner))
    {
        return Err("bridge sender/transaction pair graph is incomplete".into());
    }
    let mut anomalies = BTreeMap::new();
    let protected: Vec<&[u8]> = replacements
        .iter()
        .flat_map(|p| [p.old.as_ref(), p.new.as_ref()])
        .chain([
            recovery::ATTACKER_ACCOUNT.as_slice(),
            recovery::ATTACKER_EVM.as_slice(),
        ])
        .collect();
    for anomaly in &review.preserved_anomalies {
        let transaction = hex_bytes(&anomaly.transaction_key)?;
        let sender = hex_bytes(&anomaly.sender_key)?;
        let (network, owner, message) = transaction_identity(&transaction)?;
        let source_evidence = bytes32(&anomaly.source_evidence_hash)?;
        if !owner_mismatches.contains(&transaction)
            || sender != sender_key(network, message)
            || before.get(&sender).map(|v| hash(v)) != Some(bytes32(&anomaly.sender_value_hash)?)
            || before.get(&transaction).map(|v| hash(v))
                != Some(bytes32(&anomaly.transaction_value_hash)?)
            || source_evidence == [0; 32]
            || mapping.contains_key(&owner)
            || anomalies
                .insert(transaction.clone(), source_evidence)
                .is_some()
        {
            return Err(
                "anomalous bridge history requires exact unrelated paired preservation evidence"
                    .into(),
            );
        }
        for bytes in [
            &sender,
            &transaction,
            &before[&sender],
            &before[&transaction],
        ] {
            if protected
                .iter()
                .any(|account| bytes.windows(account.len()).any(|part| part == *account))
            {
                return Err(
                    "ambiguous bridge record intersects a replacement or attacker identity".into(),
                );
            }
        }
    }
    if anomalies.len() != owner_mismatches.len() {
        return Err(
            "bridge record local owner differs from tuple index without exact quarantine review"
                .into(),
        );
    }
    let mut expected = before.clone();
    let mut groups = Vec::new();
    let mut resolutions = Vec::new();
    let mut remapped = 0;
    let mut tombstones = 0;
    let mut nonterminal = 0;
    let mut pending_quarantined = 0;
    let mut pending_remapped = 0;
    let mut attacker_extinguished = 0;
    let mut unresolved = Vec::new();
    for ((_, message), (network, owner, record)) in &transactions {
        let source = transaction_key(*network, owner, *message);
        let sender = sender_key(*network, *message);
        if let Some(source_evidence) = anomalies.get(&source) {
            // Opaque execution stop; not the snxhist1 owner/liability codec.
            // Existing native guards reject every marker, regardless of bytes.
            let marker = (
                *b"snxanom1",
                recovery::CHECKPOINT_HASH,
                hash(&before[&sender]),
                hash(&before[&source]),
                *source_evidence,
                evidence,
            )
                .encode();
            let key = bridge_types::recovery::historical_message_key(*network, *message);
            expected.insert(key.clone(), marker.clone());
            groups.push(vec![GuardedStorageEdit {
                scope: StorageScope::Top,
                key,
                before: None,
                after: Some(marker),
            }]);
            unresolved.push(UnresolvedAnomaly {
                network_scale: hx(&network.encode()),
                message_id: hx(message.as_bytes()),
                sender_key: hx(&sender),
                transaction_key: hx(&source),
            });
            continue;
        }
        let mapped = mapping.get(owner);
        let terminal = matches!(record.status, MessageStatus::Done | MessageStatus::Refunded);
        if record.direction == MessageDirection::Outbound && !terminal {
            nonterminal += 1;
        }
        let quarantine_pending = record.direction == MessageDirection::Outbound
            && !terminal
            && review.quarantine_all_nonterminal_outgoing;
        if mapped.is_some() && !terminal && !quarantine_pending {
            return Err("mapped nonterminal bridge request requires explicit outgoing quarantine or authenticated incoming reconciliation".into());
        }
        let cancel = record.direction == MessageDirection::Outbound
            && terminal
            && (mapped.is_some() || review.cancel_all_terminal_outgoing);
        if mapped.is_none() && !cancel && !quarantine_pending {
            continue;
        }
        let effective = mapped.unwrap_or(owner);
        let target = transaction_key(*network, effective, *message);
        let mut edits = Vec::new();
        if mapped.is_some() {
            if storage.top.contains_key(&target) {
                return Err("replacement bridge request destination occupied".into());
            }
            remapped += 1;
            let mut changed = record.clone();
            match changed.direction {
                MessageDirection::Inbound => changed.dest = GenericAccount::Sora(effective.clone()),
                MessageDirection::Outbound => {
                    changed.source = GenericAccount::Sora(effective.clone())
                }
            };
            edits.push(GuardedStorageEdit {
                scope: StorageScope::Top,
                key: sender.clone(),
                before: Some(hash(&before[&sender])),
                after: Some(effective.encode()),
            });
            edits.push(GuardedStorageEdit {
                scope: StorageScope::Top,
                key: source.clone(),
                before: Some(hash(&before[&source])),
                after: None,
            });
            edits.push(GuardedStorageEdit {
                scope: StorageScope::Top,
                key: target.clone(),
                before: None,
                after: Some(changed.encode()),
            });
        }
        if quarantine_pending {
            let marker = pending::marker(
                record,
                owner,
                effective,
                &before[&sender],
                &before[&source],
                evidence,
            )?;
            edits.push(GuardedStorageEdit {
                scope: StorageScope::Top,
                key: bridge_types::recovery::historical_message_key(*network, *message),
                before: None,
                after: Some(marker.encode()),
            });
            pending_quarantined += 1;
            pending_remapped += usize::from(mapped.is_some());
            attacker_extinguished += usize::from(marker.attacker_entitlement_extinguished);
        }
        if cancel {
            let marker = HistoricalMessageTombstone {
                tag: HistoricalMessageTombstone::TAG,
                source_record_hash: hash(&before[&source]),
                original_owner: *AsRef::<[u8; 32]>::as_ref(owner),
                effective_owner: *AsRef::<[u8; 32]>::as_ref(effective),
                evidence,
                refund_authorized: false,
            };
            if !marker.is_valid() {
                return Err("invalid historical no-refund marker".into());
            }
            let key = bridge_types::recovery::historical_message_key(*network, *message);
            if storage.top.contains_key(&key) {
                return Err("historical tombstone already exists".into());
            }
            edits.push(GuardedStorageEdit {
                scope: StorageScope::Top,
                key,
                before: None,
                after: Some(marker.encode()),
            });
            tombstones += 1;
        }
        edits.sort_by(|a, b| a.key.cmp(&b.key));
        recovery::RecoveryChunk {
            index: 0,
            edits: edits.clone(),
        }
        .validate()
        .map_err(|e| format!("bridge coupled group bounds: {e:?}"))?;
        for edit in &edits {
            if let Some(after) = &edit.after {
                expected.insert(edit.key.clone(), after.clone());
            } else {
                expected.remove(&edit.key);
            }
        }
        if mapped.is_some() {
            for k in [&sender, &source] {
                let d = if *k == sender { sd } else { td };
                let mut accounts = crate::inventory::transform_key(
                    k,
                    d,
                    &schema.registry,
                    &BTreeMap::new(),
                    None,
                )?
                .1;
                accounts.extend(
                    typed::transform_value(
                        &before[k],
                        d.value_type,
                        &schema.registry,
                        &BTreeMap::new(),
                    )?
                    .1,
                );
                resolutions.push(AdapterResolution {
                    storage: names[k].clone(),
                    key: hx(k),
                    expected_before_hash: hx(&hash(&before[k])),
                    expected_after_hash: expected.get(k).map(|v| hx(&hash(v))),
                    adapter: AdapterId::BridgeProxyHistoryV1,
                    action: ResolutionAction::ReplaceControl,
                    validated_edits: edits.clone(),
                    observed_accounts: accounts,
                });
            }
        }
        groups.push(edits);
    }
    if groups.is_empty() {
        return Err("unused bridge history review".into());
    }
    let keys: BTreeSet<_> = before.keys().chain(expected.keys()).cloned().collect();
    let witnesses = keys
        .into_iter()
        .map(|key| StorageWitness {
            scope: StorageScope::Top,
            value_hash: expected.get(&key).map(|v| hash(v)),
            key,
        })
        .collect();
    let atomic_sets = groups
        .iter()
        .map(|group| group.iter().map(|e| e.key.clone()).collect())
        .collect();
    let compilation = Compilation {
        edits: groups.iter().flatten().cloned().collect(),
        groups,
        resolutions,
        witnesses,
        report: Report {
            evidence_hash: review.evidence_hash.clone(),
            paired_requests: transactions.len(),
            remapped_requests: remapped,
            terminal_outgoing_tombstones: tombstones,
            nonterminal_outgoing_requests: nonterminal,
            quarantined_nonterminal_outgoing: pending_quarantined,
            remapped_nonterminal_outgoing: pending_remapped,
            extinguished_attacker_pending_entitlements: attacker_extinguished,
            quarantine_all_nonterminal_outgoing: review.quarantine_all_nonterminal_outgoing,
            cancel_all_terminal_outgoing: review.cancel_all_terminal_outgoing,
            external_addresses_changed: false,
            refunds_created: 0,
            batch_authorizations_reconciled: false,
            complete_financial_interpretation: unresolved.is_empty(),
            anomaly_status_and_inbound_guards_complete: unresolved.is_empty(),
            unresolved_quarantined_anomalies: unresolved,
            production_ready: false,
        },
        before,
        expected,
        prefixes: vec![sd.prefix.clone(), td.prefix.clone(), MARKERS.to_vec()],
        mapping_hash: hash(&replacements.encode()),
        atomic_sets,
    };
    compilation.verify_groups(&compilation.groups, replacements)?;
    Ok(compilation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, Registry};
    pub(super) fn account(n: u8) -> AccountId32 {
        AccountId32::new([n; 32])
    }
    pub(super) fn fixture() -> (MetadataSchema, Storage, Review, Vec<AccountReplacement>) {
        let mut r = Registry::new();
        let network = r.register_type(&meta_type::<GenericNetworkId>()).id;
        let message = r.register_type(&meta_type::<H256>()).id;
        let owner = r.register_type(&meta_type::<AccountId32>()).id;
        let tuple = r
            .register_type(&meta_type::<(GenericNetworkId, AccountId32)>())
            .id;
        let request = r.register_type(&meta_type::<Request>()).id;
        let schema = MetadataSchema {
            metadata_hash: METADATA.into(),
            registry: r.into(),
            descriptors: vec![
                Descriptor {
                    trie: "top".into(),
                    name: "BridgeProxy.Senders".into(),
                    prefix: prefix("Senders"),
                    key_types: vec![network, message],
                    hashers: vec![Hasher::Twox64Concat, Hasher::Blake2_128Concat],
                    value_type: owner,
                },
                Descriptor {
                    trie: "top".into(),
                    name: "BridgeProxy.Transactions".into(),
                    prefix: prefix("Transactions"),
                    key_types: vec![tuple, message],
                    hashers: vec![Hasher::Blake2_128Concat, Hasher::Blake2_128Concat],
                    value_type: request,
                },
            ],
        };
        let mut storage = Storage::default();
        let network = GenericNetworkId::Sub(bridge_types::SubNetworkId::Liberland);
        for (n, who, direction, status) in [
            (
                1,
                account(1),
                MessageDirection::Outbound,
                MessageStatus::Done,
            ),
            (
                2,
                account(1),
                MessageDirection::Inbound,
                MessageStatus::Done,
            ),
            (
                3,
                account(3),
                MessageDirection::Outbound,
                MessageStatus::Refunded,
            ),
        ] {
            // Same 32 bytes on another network are still external evidence.
            let external = GenericAccount::Liberland(account(1));
            let (source, dest) = if direction == MessageDirection::Outbound {
                (GenericAccount::Sora(who.clone()), external)
            } else {
                (external, GenericAccount::Sora(who.clone()))
            };
            let record = Request {
                source,
                dest,
                asset_id: common::XOR,
                amount: 123,
                status,
                start_timepoint: GenericTimepoint::Sora(9),
                end_timepoint: GenericTimepoint::Sora(10),
                direction,
            };
            storage
                .top
                .insert(sender_key(network, H256::repeat_byte(n)), who.encode());
            storage.top.insert(
                transaction_key(network, &who, H256::repeat_byte(n)),
                record.encode(),
            );
        }
        let review = review(&storage);
        (
            schema,
            storage,
            review,
            vec![AccountReplacement {
                old: account(1),
                new: account(2),
            }],
        )
    }
    pub(super) fn review(storage: &Storage) -> Review {
        Review {
            evidence_hash: hx(&[8; 32]),
            cancel_all_terminal_outgoing: true,
            quarantine_all_nonterminal_outgoing: false,
            preserved_anomalies: vec![],
            records: storage
                .top
                .iter()
                .map(|(k, v)| Record {
                    storage: if k.starts_with(&prefix("Senders")) {
                        "BridgeProxy.Senders"
                    } else {
                        "BridgeProxy.Transactions"
                    }
                    .into(),
                    key: hx(k),
                    value_hash: hx(&hash(v)),
                })
                .collect(),
        }
    }
    #[test]
    fn paired_terminal_ownership_preserves_external_evidence_and_suppresses_refunds() {
        let (s, storage, review, map) = fixture();
        let c = compile(&s, &storage, &map, &review).unwrap();
        assert_eq!(c.report.remapped_requests, 2);
        assert_eq!(c.report.terminal_outgoing_tombstones, 2);
        let network = GenericNetworkId::Sub(bridge_types::SubNetworkId::Liberland);
        let original: Request =
            decode(&storage.top[&transaction_key(network, &account(1), H256::repeat_byte(1))])
                .unwrap();
        let after: Request =
            decode(&c.expected[&transaction_key(network, &account(2), H256::repeat_byte(1))])
                .unwrap();
        assert_eq!(after.source, GenericAccount::Sora(account(2)));
        assert_eq!(after.dest, original.dest);
        assert_eq!(after.status, original.status);
        assert_eq!(after.amount, original.amount);
        let marker: HistoricalMessageTombstone = decode(
            &c.expected
                [&bridge_types::recovery::historical_message_key(network, H256::repeat_byte(1))],
        )
        .unwrap();
        assert!(marker.is_valid());
        assert!(!marker.refund_authorized);
        c.verify_groups(&c.groups, &map).unwrap();
        let mut incomplete = c.groups.clone();
        incomplete[0].retain(|e| !e.key.starts_with(MARKERS));
        assert!(c.verify_groups(&incomplete, &map).is_err());
        let split: Vec<_> = c
            .groups
            .iter()
            .flatten()
            .map(|edit| vec![edit.clone()])
            .collect();
        assert!(c.verify_groups(&split, &map).is_err());
    }
    #[test]
    fn incomplete_pairs_missing_review_nonterminal_and_backdoor_edits_are_rejected() {
        let (s, mut storage, mut r, map) = fixture();
        r.records.pop();
        assert!(compile(&s, &storage, &map, &r).is_err());
        let network = GenericNetworkId::Sub(bridge_types::SubNetworkId::Liberland);
        let key = transaction_key(network, &account(1), H256::repeat_byte(1));
        let mut request: Request = decode(&storage.top[&key]).unwrap();
        request.status = MessageStatus::InQueue;
        storage.top.insert(key.clone(), request.encode());
        assert!(compile(&s, &storage, &map, &review(&storage)).is_err());
        request.status = MessageStatus::Done;
        storage.top.insert(key, request.encode());
        storage
            .top
            .remove(&sender_key(network, H256::repeat_byte(1)));
        assert!(compile(&s, &storage, &map, &review(&storage)).is_err());
    }
    #[test]
    fn unconfirmed_all_terminal_action_cannot_claim_full_cancellation() {
        let (s, storage, mut r, map) = fixture();
        r.cancel_all_terminal_outgoing = false;
        let c = compile(&s, &storage, &map, &r).unwrap();
        assert_eq!(c.report.terminal_outgoing_tombstones, 1);
        assert!(!c.report.cancel_all_terminal_outgoing);
        assert!(!c.report.batch_authorizations_reconciled);
        assert!(!c.report.production_ready);
    }
    fn anomaly_fixture() -> (
        MetadataSchema,
        Storage,
        Review,
        Vec<AccountReplacement>,
        Vec<u8>,
        Vec<u8>,
    ) {
        let (schema, mut storage, _, map) = fixture();
        let network = GenericNetworkId::Sub(bridge_types::SubNetworkId::Liberland);
        let message = H256::repeat_byte(3);
        let transaction = transaction_key(network, &account(3), message);
        let sender = sender_key(network, message);
        let mut record: Request = decode(&storage.top[&transaction]).unwrap();
        record.direction = MessageDirection::Inbound;
        record.dest = GenericAccount::Unknown;
        // Avoid accidentally including a mapped account in unrelated evidence.
        record.source = GenericAccount::Sora(account(3));
        storage.top.insert(transaction.clone(), record.encode());
        let mut review = review(&storage);
        review.preserved_anomalies.push(PreservedAnomaly {
            sender_key: hx(&sender),
            sender_value_hash: hx(&hash(&storage.top[&sender])),
            transaction_key: hx(&transaction),
            transaction_value_hash: hx(&hash(&storage.top[&transaction])),
            source_evidence_hash: hx(&[9; 32]),
            reason: AnomalyReason::NativeOwnerMismatch,
        });
        (schema, storage, review, map, sender, transaction)
    }
    #[test]
    fn exact_unrelated_anomaly_preserves_raw_pair_and_never_claims_financial_completion() {
        let (schema, storage, review, map, sender, transaction) = anomaly_fixture();
        let c = compile(&schema, &storage, &map, &review).unwrap();
        assert_eq!(c.expected[&sender], storage.top[&sender]);
        assert_eq!(c.expected[&transaction], storage.top[&transaction]);
        assert_eq!(c.report.unresolved_quarantined_anomalies.len(), 1);
        assert!(!c.report.complete_financial_interpretation);
        assert!(!c.report.anomaly_status_and_inbound_guards_complete);
        assert_eq!(c.report.terminal_outgoing_tombstones, 1);
        let marker_key = bridge_types::recovery::historical_message_key(
            GenericNetworkId::Sub(bridge_types::SubNetworkId::Liberland),
            H256::repeat_byte(3),
        );
        assert!(c.expected[&marker_key].starts_with(b"snxanom1"));
        assert!(decode::<HistoricalMessageTombstone>(&c.expected[&marker_key]).is_err());
        c.verify_groups(&c.groups, &map).unwrap();
        let mut tampered = c.groups.clone();
        tampered.push(vec![GuardedStorageEdit {
            scope: StorageScope::Top,
            key: transaction.clone(),
            before: Some(hash(&storage.top[&transaction])),
            after: None,
        }]);
        assert!(c.verify_groups(&tampered, &map).is_err());
    }
    #[test]
    fn anomaly_exception_rejects_missing_wrong_duplicate_or_protected_identity_evidence() {
        let (schema, storage, mut review, map, _, transaction) = anomaly_fixture();
        let exception = review.preserved_anomalies.pop().unwrap();
        assert!(compile(&schema, &storage, &map, &review).is_err());
        review.preserved_anomalies.push(exception);
        review.preserved_anomalies[0].transaction_value_hash = hx(&[7; 32]);
        assert!(compile(&schema, &storage, &map, &review).is_err());
        let (schema, storage, mut review, map, _, _) = anomaly_fixture();
        review.preserved_anomalies[0].sender_key = hx(&transaction);
        assert!(compile(&schema, &storage, &map, &review).is_err());
        let (schema, storage, mut review, map, _, _) = anomaly_fixture();
        review
            .preserved_anomalies
            .push(anomaly_fixture().2.preserved_anomalies.pop().unwrap());
        assert!(compile(&schema, &storage, &map, &review).is_err());
        for identity in [
            map[0].old.clone(),
            map[0].new.clone(),
            recovery::ATTACKER_ACCOUNT.into(),
        ] {
            let (schema, mut storage, mut review, map, _, transaction) = anomaly_fixture();
            let mut record: Request = decode(&storage.top[&transaction]).unwrap();
            record.source = GenericAccount::Sora(identity);
            storage.top.insert(transaction.clone(), record.encode());
            review.records = super::tests::review(&storage).records;
            review.preserved_anomalies[0].transaction_value_hash =
                hx(&hash(&storage.top[&transaction]));
            assert!(compile(&schema, &storage, &map, &review).is_err());
        }
        let (schema, mut storage, mut review, map, _, transaction) = anomaly_fixture();
        let mut record: Request = decode(&storage.top[&transaction]).unwrap();
        record.source = GenericAccount::EVM(recovery::ATTACKER_EVM.into());
        storage.top.insert(transaction.clone(), record.encode());
        review.records = super::tests::review(&storage).records;
        review.preserved_anomalies[0].transaction_value_hash =
            hx(&hash(&storage.top[&transaction]));
        assert!(compile(&schema, &storage, &map, &review).is_err());
    }
}
