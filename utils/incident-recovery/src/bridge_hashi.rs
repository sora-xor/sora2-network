//! Complete checkpoint HASHI cancellation. Stored map/wrapper identities are
//! authoritative; denomination or other historical edits never justify rehashing
//! the signed request payload. No signature, request, reserve or status is erased.
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
use eth_bridge::requests::{
    IncomingRequest, IncomingTransactionRequestKind, LoadIncomingRequest, OffchainRequest,
    OutgoingRequest, RequestStatus,
};
#[cfg(test)]
use framenode_runtime::AccountId;
use framenode_runtime::Runtime;
use serde::{Deserialize, Serialize};
use sp_core::{storage::Storage, H256};
use std::collections::{BTreeMap, BTreeSet};
const METADATA: &str = "0x0c1d4bf5c3bc5037ad854932443c94faae6c327ecf1f38b3e77d38668cc2b4b8";
const MAX_ROWS: usize = 1_000_000;
const MAX_BYTES: usize = 512 * 1024 * 1024;
const INCOMING_OWNERSHIP_PREFIX: &[u8] = b":sora-nexus:hashi-incoming-ownership:v1:";
pub const ITEMS: [&str; 7] = [
    "EthBridge.Requests",
    "EthBridge.RequestStatuses",
    "EthBridge.RequestApprovals",
    "EthBridge.RequestApprovers",
    "EthBridge.RequestSubmissionHeight",
    "EthBridge.RequestsQueue",
    "EthBridge.AccountRequests",
];
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Review {
    pub evidence_hash: String,
    pub cancel_all_native_outgoing: bool,
    pub prefixes: Vec<PrefixReview>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrefixReview {
    pub storage: String,
    pub records: usize,
    pub commitment: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub original_records: usize,
    pub native_outgoing_requests: usize,
    pub transfer_requests: usize,
    pub remapped_transfer_owners: usize,
    pub extinguished_attacker_contingent_claims: usize,
    pub remapped_account_indexes: usize,
    pub preserved_signed_request_records: usize,
    pub preserved_incoming_request_records: usize,
    pub recomputed_request_ids: usize,
    pub refunds_created: usize,
    pub external_authorizations_invalidated: bool,
    pub production_ready: bool,
}
pub struct Compilation {
    pub groups: Vec<Vec<GuardedStorageEdit>>,
    pub edits: Vec<GuardedStorageEdit>,
    pub resolutions: Vec<AdapterResolution>,
    pub witnesses: Vec<StorageWitness>,
    pub prefix_witnesses: Vec<StoragePrefixWitness>,
    pub report: Report,
    before: BTreeMap<Vec<u8>, [u8; 32]>,
    expected: BTreeMap<Vec<u8>, [u8; 32]>,
    prefixes: Vec<Vec<u8>>,
    mapping_hash: [u8; 32],
    atomic: Vec<BTreeSet<Vec<u8>>>,
}
impl Compilation {
    pub fn verify_groups(
        &self,
        groups: &[Vec<GuardedStorageEdit>],
        mapping: &[AccountReplacement],
    ) -> Result<()> {
        if hash(&mapping.encode()) != self.mapping_hash {
            return Err("HASHI mapping certificate changed".into());
        }
        let group_for_key: BTreeMap<_, _> = groups
            .iter()
            .enumerate()
            .flat_map(|(i, g)| {
                g.iter()
                    .filter(|e| e.scope == StorageScope::Top)
                    .map(move |e| (&e.key, i))
            })
            .collect();
        for keys in &self.atomic {
            let mut ids = keys.iter().map(|k| group_for_key.get(k));
            let first = ids.next().flatten();
            if first.is_none() || ids.any(|id| id != first) {
                return Err("HASHI evidence and owner cancellation must remain atomic".into());
            }
        }
        let mut projected = self.before.clone();
        let mut seen = BTreeSet::new();
        for e in groups.iter().flatten().filter(|e| {
            e.scope == StorageScope::Top && self.prefixes.iter().any(|p| e.key.starts_with(p))
        }) {
            if !seen.insert(&e.key) || self.before.get(&e.key).copied() != e.before {
                return Err("duplicate/stale HASHI cancellation edit".into());
            }
            if let Some(v) = &e.after {
                projected.insert(e.key.clone(), hash(v));
            } else {
                projected.remove(&e.key);
            }
        }
        if projected != self.expected {
            return Err("incomplete HASHI cancellation or altered native evidence".into());
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
fn h32(v: &str) -> Result<[u8; 32]> {
    hex_bytes(v)?
        .try_into()
        .map_err(|_| "HASHI hash width".into())
}
fn prefix(name: &str) -> Vec<u8> {
    let (p, i) = name.split_once('.').unwrap();
    [
        sp_io::hashing::twox_128(p.as_bytes()),
        sp_io::hashing::twox_128(i.as_bytes()),
    ]
    .concat()
}
fn desc<'a>(s: &'a MetadataSchema, name: &str) -> Result<&'a Descriptor> {
    let d = s
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == name)
        .ok_or("HASHI descriptor absent")?;
    let expected = if name == ITEMS[6] {
        vec![Hasher::Blake2_128Concat]
    } else if name == ITEMS[5] {
        vec![Hasher::Twox64Concat]
    } else {
        vec![Hasher::Twox64Concat, Hasher::Identity]
    };
    if d.prefix != prefix(name) || d.hashers != expected || d.key_types.len() != expected.len() {
        return Err("HASHI key codec differs".into());
    }
    Ok(d)
}
fn decode<T: Decode + Encode>(v: &[u8]) -> Result<T> {
    if v.len() > typed::MAX_TYPED_BYTES {
        return Err("HASHI native value bound".into());
    }
    let mut input = v;
    let x = T::decode(&mut input).map_err(|e| e.to_string())?;
    if !input.is_empty() || x.encode() != v {
        return Err("HASHI native codec noncanonical".into());
    }
    Ok(x)
}
fn identity(k: &[u8], d: &Descriptor) -> Result<(u32, H256)> {
    if k.len() != d.prefix.len() + 44 {
        return Err("HASHI request key width".into());
    }
    let suffix = &k[d.prefix.len()..];
    let network = decode::<u32>(&suffix[8..12])?;
    if suffix[..8] != sp_io::hashing::twox_64(&network.encode()) {
        return Err("HASHI network key hash".into());
    }
    Ok((network, decode(&suffix[12..])?))
}
fn request_key(d: &Descriptor, network: u32, id: H256) -> Vec<u8> {
    [
        d.prefix.clone(),
        sp_io::hashing::twox_64(&network.encode()).to_vec(),
        network.encode(),
        id.encode(),
    ]
    .concat()
}
fn witness(key: Vec<u8>, value: Option<&Vec<u8>>) -> StorageWitness {
    StorageWitness {
        scope: StorageScope::Top,
        key,
        value_hash: value.map(|v| hash(v)),
    }
}
fn edit(key: Vec<u8>, before: Option<&Vec<u8>>, after: Option<Vec<u8>>) -> GuardedStorageEdit {
    GuardedStorageEdit {
        scope: StorageScope::Top,
        key,
        before: before.map(|v| hash(v)),
        after,
    }
}
fn transfer_load_kind(
    load: &LoadIncomingRequest<Runtime>,
) -> Option<IncomingTransactionRequestKind> {
    [
        IncomingTransactionRequestKind::Transfer,
        IncomingTransactionRequestKind::TransferXOR,
    ]
    .into_iter()
    .find(|kind| {
        load == &LoadIncomingRequest::Transaction(
            eth_bridge::requests::LoadIncomingTransactionRequest::new(
                load.author().clone(),
                load.hash(),
                load.timepoint(),
                *kind,
                load.network_id(),
            ),
        )
    })
}
fn resolution(
    d: &Descriptor,
    key: &[u8],
    value: &[u8],
    after: Option<&[u8]>,
    edits: &[GuardedStorageEdit],
    schema: &MetadataSchema,
    adapter: AdapterId,
) -> Result<AdapterResolution> {
    let mut observed =
        crate::inventory::transform_key(key, d, &schema.registry, &BTreeMap::new(), None)?.1;
    observed
        .extend(typed::transform_value(value, d.value_type, &schema.registry, &BTreeMap::new())?.1);
    Ok(AdapterResolution {
        storage: d.name.clone(),
        key: hx(key),
        expected_before_hash: hx(&hash(value)),
        expected_after_hash: after.map(|v| hx(&hash(v))),
        adapter,
        action: if adapter == AdapterId::HashiCancellationV1 {
            ResolutionAction::CancelPending
        } else {
            ResolutionAction::ReplaceControl
        },
        validated_edits: edits.to_vec(),
        observed_accounts: observed,
    })
}

pub fn compile(
    schema: &MetadataSchema,
    storage: &Storage,
    replacements: &[AccountReplacement],
    review: &Review,
) -> Result<Compilation> {
    recovery::validate_replacements(replacements)
        .map_err(|e| format!("HASHI replacements: {e:?}"))?;
    let evidence = h32(&review.evidence_hash)?;
    if schema.metadata_hash != METADATA
        || evidence == [0; 32]
        || !review.cancel_all_native_outgoing
        || review.prefixes.len() != ITEMS.len()
    {
        return Err("HASHI exact metadata and complete cancellation review required".into());
    }
    let map: BTreeMap<_, _> = replacements
        .iter()
        .map(|r| (r.old.clone(), r.new.clone()))
        .collect();
    let mapping_bytes: BTreeMap<[u8; 32], [u8; 32]> = replacements
        .iter()
        .map(|r| (*r.old.as_ref(), *r.new.as_ref()))
        .collect();
    let declared: BTreeMap<_, _> = review
        .prefixes
        .iter()
        .map(|p| (p.storage.as_str(), p))
        .collect();
    if declared.len() != ITEMS.len() {
        return Err("duplicate HASHI prefix review".into());
    }
    let descriptors = ITEMS
        .iter()
        .map(|n| desc(schema, n))
        .collect::<Result<Vec<_>>>()?;
    let markers = bridge_types::recovery::HISTORICAL_HASHI_PREFIX.to_vec();
    if storage
        .top
        .range(markers.clone()..)
        .next()
        .is_some_and(|(k, _)| k.starts_with(&markers))
    {
        return Err("HASHI markers already present at checkpoint".into());
    }
    let incoming_markers = INCOMING_OWNERSHIP_PREFIX.to_vec();
    if storage
        .top
        .range(incoming_markers.clone()..)
        .next()
        .is_some_and(|(key, _)| key.starts_with(&incoming_markers))
    {
        return Err("HASHI incoming ownership markers already present at checkpoint".into());
    }
    let mut before = BTreeMap::new();
    let mut prefixes = vec![markers, incoming_markers];
    let mut prefix_witnesses = Vec::new();
    let mut bytes = 0usize;
    for d in &descriptors {
        let reviewed = declared
            .get(d.name.as_str())
            .ok_or("missing HASHI prefix review")?;
        let mut commitment =
            hash(&[b"sora-nexus-hashi-prefix-v1:".as_slice(), d.name.as_bytes()].concat());
        let mut count = 0;
        for (k, v) in storage
            .top
            .range(d.prefix.clone()..)
            .take_while(|(k, _)| k.starts_with(&d.prefix))
        {
            bytes = bytes
                .checked_add(v.len())
                .filter(|n| *n <= MAX_BYTES)
                .ok_or("HASHI aggregate bound")?;
            if before.len() >= MAX_ROWS {
                return Err("HASHI record bound".into());
            }
            if d.name != ITEMS[6] {
                typed::decode_exact(v, d.value_type, &schema.registry)?;
                crate::inventory::transform_key(k, d, &schema.registry, &BTreeMap::new(), None)?;
            }
            commitment = hash(&(commitment, k, hash(v)).encode());
            before.insert(k.clone(), hash(v));
            count += 1;
        }
        if count != reviewed.records || commitment != h32(&reviewed.commitment)? {
            return Err(format!(
                "HASHI complete prefix commitment mismatch: {}",
                d.name
            ));
        }
        if count == 0 {
            prefix_witnesses.push(StoragePrefixWitness {
                scope: StorageScope::Top,
                prefix: d.prefix.clone(),
            });
        }
        prefixes.push(d.prefix.clone());
    }
    let mut expected = before.clone();
    let mut groups = Vec::new();
    let mut resolutions = Vec::new();
    let mut witnesses = Vec::new();
    let mut owners = BTreeMap::new();
    let mut counts = [0usize; 5];
    let mut incoming_preserved = 0usize;
    for (key, value) in storage
        .top
        .range(descriptors[0].prefix.clone()..)
        .take_while(|(k, _)| k.starts_with(&descriptors[0].prefix))
    {
        let (network, id) = identity(key, descriptors[0])?;
        let native: OffchainRequest<Runtime> = decode(value)?;
        let (native_network, author) = match &native {
            OffchainRequest::Outgoing(request, _) => (request.network_id(), request.author()),
            OffchainRequest::Incoming(request, _) => (request.network_id(), request.author()),
            OffchainRequest::LoadIncoming(request) => (request.network_id(), request.author()),
        };
        if native.hash() != id || native_network != network {
            return Err("HASHI stored wrapper identity differs from its lookup key".into());
        }
        owners.insert((network, id), author.clone());
        // This native wrapper hash is retained at denomination. Re-hashing payloads
        // would invent a new request ID and fail to stop the actual lookup identity.
        let Some((outgoing, stored_hash)) = native.as_outgoing() else {
            let accounts = typed::transform_value(
                value,
                descriptors[0].value_type,
                &schema.registry,
                &BTreeMap::new(),
            )?
            .1;
            if accounts
                .iter()
                .any(|account| mapping_bytes.contains_key(account))
            {
                // Only financial transfers and their nonfinancial load records
                // have this reviewed ownership rule. Control operations require
                // their own authorization/cancellation adapter.
                let recipient = match &native {
                    OffchainRequest::Incoming(IncomingRequest::Transfer(transfer), _) => {
                        Some((&transfer.to, map.get(&transfer.to).unwrap_or(&transfer.to)))
                    }
                    OffchainRequest::LoadIncoming(load) if transfer_load_kind(load).is_some() => {
                        None
                    }
                    _ => {
                        return Err(
                            "mapped HASHI incoming control request requires its exact adapter"
                                .into(),
                        )
                    }
                };
                let status_key = request_key(descriptors[1], network, id);
                let status_value = storage
                    .top
                    .get(&status_key)
                    .ok_or("HASHI incoming status absent")?;
                let status: RequestStatus = decode(status_value)?;
                if matches!(&native, OffchainRequest::LoadIncoming(load)
                    if transfer_load_kind(load) == Some(IncomingTransactionRequestKind::TransferXOR))
                    && !matches!(status, RequestStatus::Done)
                {
                    return Err("nonterminal mapped HASHI TransferXOR requires native decommission reconciliation".into());
                }
                let marker_key =
                    [INCOMING_OWNERSHIP_PREFIX, &network.encode(), &id.encode()].concat();
                // This records effective ownership, never admission, completion,
                // cancellation, minting, or refund authorization. Native signed
                // payloads remain exact; the incoming executive alone can use
                // the committed mapping after authenticating the receipt.
                let marker = (
                    *b"snxhown1",
                    recovery::CHECKPOINT_HASH,
                    evidence,
                    hash(value),
                    hash(status_value),
                    author,
                    map.get(author).unwrap_or(author),
                    recipient,
                )
                    .encode();
                let group = vec![edit(marker_key.clone(), None, Some(marker.clone()))];
                expected.insert(marker_key.clone(), hash(&marker));
                witnesses.push(witness(marker_key, Some(&marker)));
                witnesses.push(witness(key.clone(), Some(value)));
                resolutions.push(resolution(
                    descriptors[0],
                    key,
                    value,
                    Some(value),
                    &group,
                    schema,
                    AdapterId::HashiIncomingOwnershipV1,
                )?);
                groups.push(group);
                incoming_preserved += 1;
            }
            continue;
        };
        if stored_hash != id || outgoing.network_id() != network {
            return Err("HASHI stored wrapper identity differs from its lookup key".into());
        }
        counts[0] += 1;
        let status_key = request_key(descriptors[1], network, id);
        let status_value = storage
            .top
            .get(&status_key)
            .ok_or("HASHI outgoing status absent")?;
        let status: RequestStatus = decode(status_value)?;
        let owner = outgoing.author();
        let effective = map.get(owner).unwrap_or(owner);
        let transfer = if let OutgoingRequest::Transfer(t) = outgoing {
            counts[1] += 1;
            counts[2] += usize::from(effective != owner);
            Some((t.asset_id, t.amount))
        } else {
            None
        };
        let attacker = transfer.is_some()
            && AsRef::<[u8; 32]>::as_ref(owner) == &recovery::ATTACKER_ACCOUNT
            && !matches!(status, RequestStatus::Done);
        counts[3] += usize::from(attacker);
        let marker_key = bridge_types::recovery::historical_hashi_key(network, id);
        let marker = (
            *b"snxhash1",
            recovery::CHECKPOINT_HASH,
            evidence,
            hash(value),
            hash(status_value),
            owner,
            effective,
            transfer,
            attacker,
            false,
        )
            .encode();
        let group = vec![edit(marker_key.clone(), None, Some(marker.clone()))];
        expected.insert(marker_key.clone(), hash(&marker));
        witnesses.push(witness(marker_key, Some(&marker)));
        for (i, d) in descriptors.iter().take(5).enumerate() {
            let k = request_key(d, network, id);
            let v = storage.top.get(&k);
            witnesses.push(witness(k.clone(), v));
            if let Some(v) = v {
                if i == 0 || i == 3 {
                    let accounts = typed::transform_value(
                        v,
                        d.value_type,
                        &schema.registry,
                        &BTreeMap::new(),
                    )?
                    .1;
                    if accounts.iter().any(|a| mapping_bytes.contains_key(a)) {
                        // Signed native evidence is never a storage mutation.
                        // The exact cancellation marker resolves these retained
                        // references, while the private complete-prefix
                        // certificate forbids any other adapter from changing
                        // them and the committed audit rechecks their bytes.
                        resolutions.push(resolution(
                            d,
                            &k,
                            v,
                            Some(v),
                            &group,
                            schema,
                            AdapterId::HashiCancellationV1,
                        )?);
                    }
                }
            }
        }
        groups.push(group);
    }
    // AccountRequests is a query/ownership index, not the signed payload. Move an
    // affected index only when every referenced native wrapper has this exact
    // author and network/hash identity. The index conveys no execution authority;
    // signed incoming/outgoing payloads and their replay identities stay intact.
    let d = descriptors[6];
    for old in map.keys() {
        let key = [
            d.prefix.clone(),
            sp_io::hashing::blake2_128(&old.encode()).to_vec(),
            old.encode(),
        ]
        .concat();
        let Some(value) = storage.top.get(&key) else {
            continue;
        };
        if value.len() > recovery::MAX_CHUNK_BYTES / 4 {
            return Err("mapped HASHI account index needs bounded custody partition".into());
        }
        typed::decode_exact(value, d.value_type, &schema.registry)?;
        let indexes: Vec<(u32, H256)> = decode(value)?;
        let mut unique = BTreeSet::new();
        if indexes
            .iter()
            .any(|id| owners.get(id) != Some(old) || !unique.insert(*id))
        {
            return Err(
                "mapped HASHI AccountRequests includes an unknown, duplicate or different-author request"
                    .into(),
            );
        }
        let target = [
            d.prefix.clone(),
            sp_io::hashing::blake2_128(&map[old].encode()).to_vec(),
            map[old].encode(),
        ]
        .concat();
        if storage.top.contains_key(&target) {
            return Err("HASHI replacement account index occupied".into());
        }
        let group = vec![
            edit(key.clone(), Some(value), None),
            edit(target.clone(), None, Some(value.clone())),
        ];
        resolutions.push(resolution(
            d,
            &key,
            value,
            None,
            &group,
            schema,
            AdapterId::HashiIncomingOwnershipV1,
        )?);
        expected.remove(&key);
        expected.insert(target.clone(), hash(value));
        witnesses.push(witness(key, None));
        witnesses.push(witness(target, Some(value)));
        groups.push(group);
        counts[4] += 1;
    }
    for group in &groups {
        recovery::RecoveryChunk {
            index: 0,
            edits: group.clone(),
        }
        .validate()
        .map_err(|e| format!("HASHI atomic group bounds: {e:?}"))?;
    }
    witnesses.sort_by(|a, b| a.key.cmp(&b.key));
    witnesses.dedup();
    prefix_witnesses.sort_by(|a, b| a.prefix.cmp(&b.prefix));
    let compilation = Compilation {
        edits: groups.iter().flatten().cloned().collect(),
        atomic: groups
            .iter()
            .map(|g| g.iter().map(|e| e.key.clone()).collect())
            .collect(),
        groups,
        resolutions,
        witnesses,
        prefix_witnesses,
        report: Report {
            original_records: before.len(),
            native_outgoing_requests: counts[0],
            transfer_requests: counts[1],
            remapped_transfer_owners: counts[2],
            extinguished_attacker_contingent_claims: counts[3],
            remapped_account_indexes: counts[4],
            preserved_signed_request_records: counts[0],
            preserved_incoming_request_records: incoming_preserved,
            recomputed_request_ids: 0,
            refunds_created: 0,
            external_authorizations_invalidated: false,
            production_ready: false,
        },
        before,
        expected,
        prefixes,
        mapping_hash: hash(&replacements.encode()),
    };
    compilation.verify_groups(&compilation.groups, replacements)?;
    Ok(compilation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, Registry};
    fn fixture() -> (MetadataSchema, Storage, Vec<AccountReplacement>) {
        let mut registry = Registry::new();
        let network = registry.register_type(&meta_type::<u32>()).id;
        let id = registry.register_type(&meta_type::<H256>()).id;
        let account = registry.register_type(&meta_type::<AccountId>()).id;
        let values = [
            registry
                .register_type(&meta_type::<OffchainRequest<Runtime>>())
                .id,
            registry.register_type(&meta_type::<RequestStatus>()).id,
            registry
                .register_type(&meta_type::<BTreeSet<eth_bridge::offchain::SignatureParams>>())
                .id,
            registry
                .register_type(&meta_type::<BTreeSet<AccountId>>())
                .id,
            network,
            registry.register_type(&meta_type::<Vec<H256>>()).id,
            registry.register_type(&meta_type::<Vec<(u32, H256)>>()).id,
        ];
        let descriptors = ITEMS
            .iter()
            .enumerate()
            .map(|(i, name)| Descriptor {
                trie: "top".into(),
                name: (*name).into(),
                prefix: prefix(name),
                key_types: if i == 6 {
                    vec![account]
                } else if i == 5 {
                    vec![network]
                } else {
                    vec![network, id]
                },
                hashers: if i == 6 {
                    vec![Hasher::Blake2_128Concat]
                } else if i == 5 {
                    vec![Hasher::Twox64Concat]
                } else {
                    vec![Hasher::Twox64Concat, Hasher::Identity]
                },
                value_type: values[i],
            })
            .collect();
        let schema = MetadataSchema {
            metadata_hash: METADATA.into(),
            registry: registry.into(),
            descriptors,
        };
        let mut storage = Storage::default();
        let old = AccountId::new([7; 32]);
        let new = AccountId::new([8; 32]);
        let outgoing =
            OutgoingRequest::<Runtime>::Transfer(eth_bridge::requests::OutgoingTransfer {
                from: old.clone(),
                to: sp_core::H160::repeat_byte(9),
                asset_id: common::XOR,
                amount: 300,
                nonce: 4,
                network_id: 0,
                timepoint: Default::default(),
            });
        let native = OffchainRequest::outgoing(outgoing);
        let id = native.hash();
        storage
            .top
            .insert(request_key(&schema.descriptors[0], 0, id), native.encode());
        storage.top.insert(
            request_key(&schema.descriptors[1], 0, id),
            RequestStatus::ApprovalsReady.encode(),
        );
        let index = [
            prefix(ITEMS[6]),
            sp_io::hashing::blake2_128(&old.encode()).to_vec(),
            old.encode(),
        ]
        .concat();
        storage.top.insert(index, vec![(0u32, id)].encode());
        (schema, storage, vec![AccountReplacement { old, new }])
    }
    fn review(storage: &Storage) -> Review {
        Review {
            evidence_hash: hx(&[3; 32]),
            cancel_all_native_outgoing: true,
            prefixes: ITEMS
                .iter()
                .map(|name| {
                    let p = prefix(name);
                    let mut commitment = hash(
                        &[b"sora-nexus-hashi-prefix-v1:".as_slice(), name.as_bytes()].concat(),
                    );
                    let mut count = 0;
                    for (k, v) in storage
                        .top
                        .range(p.clone()..)
                        .take_while(|(k, _)| k.starts_with(&p))
                    {
                        commitment = hash(&(commitment, k, hash(v)).encode());
                        count += 1;
                    }
                    PrefixReview {
                        storage: (*name).into(),
                        records: count,
                        commitment: hx(&commitment),
                    }
                })
                .collect(),
        }
    }
    #[test]
    fn full_native_cancellation_binds_preserved_payload_and_replacement_index() {
        let (s, storage, map) = fixture();
        let c = compile(&s, &storage, &map, &review(&storage)).unwrap();
        assert_eq!(c.report.native_outgoing_requests, 1);
        assert_eq!(c.report.remapped_transfer_owners, 1);
        assert_eq!(c.report.remapped_account_indexes, 1);
        assert_eq!(c.report.refunds_created, 0);
        assert_eq!(c.resolutions.len(), 2);
        let source = storage
            .top
            .keys()
            .find(|k| k.starts_with(&prefix(ITEMS[0])))
            .unwrap();
        assert_eq!(c.expected[source], hash(&storage.top[source]));
        let mut removed = c.groups.clone();
        for g in &mut removed {
            g.retain(|e| {
                !e.key
                    .starts_with(bridge_types::recovery::HISTORICAL_HASHI_PREFIX)
            });
        }
        assert!(c.verify_groups(&removed, &map).is_err());
        let split = c
            .groups
            .iter()
            .flatten()
            .cloned()
            .map(|e| vec![e])
            .collect::<Vec<_>>();
        assert!(c.verify_groups(&split, &map).is_err());
        let mut altered = c.groups.clone();
        altered.push(vec![edit(source.clone(), Some(&storage.top[source]), None)]);
        assert!(c.verify_groups(&altered, &map).is_err());
    }
    #[test]
    fn mapped_signed_requests_and_approvers_compose_without_noop_writes() {
        use common::recovery::invariants::{IssuanceInvariant, StoredAmount, TerminalAudit};
        let (s, mut storage, map) = fixture();
        let request_key = storage
            .top
            .keys()
            .find(|k| k.starts_with(&prefix(ITEMS[0])))
            .unwrap()
            .clone();
        let (network, id) = identity(&request_key, &s.descriptors[0]).unwrap();
        let approvers_key = super::request_key(&s.descriptors[3], network, id);
        storage.top.insert(
            approvers_key.clone(),
            BTreeSet::from([map[0].old.clone(), AccountId::new([19; 32])]).encode(),
        );
        let c = compile(&s, &storage, &map, &review(&storage)).unwrap();
        c.verify_groups(&c.groups, &map).unwrap();
        let inventory = crate::inventory::inventory_with_resolutions(
            &s,
            &storage,
            &map.iter()
                .map(|r| (*r.old.as_ref(), *r.new.as_ref()))
                .collect(),
            &crate::inventory::InventoryOptions::default(),
            &c.resolutions,
            &c.edits,
        )
        .unwrap();
        assert!(inventory.candidates.is_empty());
        assert_eq!(c.resolutions.len(), 3);
        for key in [&request_key, &approvers_key] {
            assert!(!c.edits.iter().any(|e| &e.key == key));
            assert!(c
                .witnesses
                .iter()
                .any(|w| { &w.key == key && w.value_hash == Some(hash(&storage.top[key])) }));
            let mut invalid = c.groups.clone();
            invalid.push(vec![edit(key.clone(), Some(&storage.top[key]), None)]);
            assert!(c.verify_groups(&invalid, &map).is_err());
        }
        let supply_key = b"test:unchanged-issuance".to_vec();
        let supply = 1u128.encode();
        storage.top.insert(supply_key.clone(), supply.clone());
        let work = crate::composer::compose_with_audits(
            &storage,
            map,
            vec![AccountId::new([29; 32])],
            [3; 32],
            c.groups,
            crate::composer::AuditRequests {
                keys: c.witnesses,
                empty_prefixes: c.prefix_witnesses,
            },
            TerminalAudit {
                issuance: vec![IssuanceInvariant {
                    asset: [1; 32],
                    current: StoredAmount {
                        scope: StorageScope::Top,
                        key: supply_key,
                        record_hash: hash(&supply),
                        offset: 0,
                    },
                    checkpoint_issuance: 1,
                    authenticated_mints: 0,
                    burned: 0,
                }],
                collateral: vec![],
                zero_claims: vec![],
                absent: vec![],
            },
        )
        .unwrap();
        assert!(work.report.storage_engine_rehearsed);
        assert_eq!(work.report.changed_keys, 3);
    }
    #[test]
    fn stored_native_identity_survives_historical_amount_change_without_rehashing() {
        let (s, mut storage, map) = fixture();
        let key = storage
            .top
            .keys()
            .find(|k| k.starts_with(&prefix(ITEMS[0])))
            .unwrap()
            .clone();
        let mut native: OffchainRequest<Runtime> = decode(&storage.top[&key]).unwrap();
        let id = native.hash();
        if let OffchainRequest::Outgoing(OutgoingRequest::Transfer(ref mut t), _) = native {
            t.amount = 1;
        }
        assert_ne!(
            OffchainRequest::outgoing(native.as_outgoing().unwrap().0.clone()).hash(),
            id
        );
        storage.top.insert(key.clone(), native.encode());
        let c = compile(&s, &storage, &map, &review(&storage)).unwrap();
        assert_eq!(c.report.recomputed_request_ids, 0);
        assert!(c
            .expected
            .contains_key(&bridge_types::recovery::historical_hashi_key(0u32, id)));
        if let OffchainRequest::Outgoing(_, ref mut hash) = native {
            *hash = H256::repeat_byte(33);
        }
        storage.top.insert(key, native.encode());
        assert!(compile(&s, &storage, &map, &review(&storage)).is_err());
    }
    #[test]
    fn missing_prefix_status_or_ambiguous_account_index_fails_closed() {
        let (s, storage, map) = fixture();
        let mut r = review(&storage);
        r.prefixes.pop();
        assert!(compile(&s, &storage, &map, &r).is_err());
        let mut missing = storage.clone();
        let key = missing
            .top
            .keys()
            .find(|k| k.starts_with(&prefix(ITEMS[1])))
            .unwrap()
            .clone();
        missing.top.remove(&key);
        assert!(compile(&s, &missing, &map, &review(&missing)).is_err());
        let mut changed = storage.clone();
        let key = changed
            .top
            .keys()
            .find(|k| k.starts_with(&prefix(ITEMS[6])))
            .unwrap()
            .clone();
        changed
            .top
            .insert(key, vec![(0u32, H256::repeat_byte(44))].encode());
        assert!(compile(&s, &changed, &map, &review(&changed)).is_err());
    }
    #[test]
    fn mapped_mixed_incoming_index_preserves_payload_and_resolves_only_query_ownership() {
        use eth_bridge::requests::{
            IncomingTransactionRequestKind, LoadIncomingRequest, LoadIncomingTransactionRequest,
        };
        let (s, mut storage, map) = fixture();
        let incoming_id = H256::repeat_byte(44);
        let native = OffchainRequest::<Runtime>::LoadIncoming(LoadIncomingRequest::Transaction(
            LoadIncomingTransactionRequest::new(
                map[0].old.clone(),
                incoming_id,
                Default::default(),
                IncomingTransactionRequestKind::Transfer,
                0,
            ),
        ));
        storage.top.insert(
            request_key(&s.descriptors[0], 0, incoming_id),
            native.encode(),
        );
        storage.top.insert(
            request_key(&s.descriptors[1], 0, incoming_id),
            RequestStatus::Pending.encode(),
        );
        let index_key = storage
            .top
            .keys()
            .find(|k| k.starts_with(&prefix(ITEMS[6])))
            .unwrap()
            .clone();
        let mut indexes: Vec<(u32, H256)> = decode(&storage.top[&index_key]).unwrap();
        indexes.push((0, incoming_id));
        storage.top.insert(index_key.clone(), indexes.encode());
        let c = compile(&s, &storage, &map, &review(&storage)).unwrap();
        assert_eq!(c.report.preserved_incoming_request_records, 1);
        assert_eq!(c.report.refunds_created, 0);
        let source = request_key(&s.descriptors[0], 0, incoming_id);
        assert!(!c.edits.iter().any(|edit| edit.key == source));
        assert!(c
            .witnesses
            .iter()
            .any(|w| w.key == source && w.value_hash == Some(hash(&native.encode()))));
        let target = [
            prefix(ITEMS[6]),
            sp_io::hashing::blake2_128(&map[0].new.encode()).to_vec(),
            map[0].new.encode(),
        ]
        .concat();
        assert!(c
            .edits
            .iter()
            .any(|edit| edit.key == index_key && edit.after.is_none()));
        assert!(c
            .edits
            .iter()
            .any(|edit| edit.key == target && edit.after == Some(indexes.encode())));
        assert!(!c.edits.iter().any(
            |edit| edit.key == bridge_types::recovery::historical_hashi_key(0u32, incoming_id)
        ));
        let options = crate::inventory::InventoryOptions {
            // The native adapter resolves request/index ownership. Retained
            // status scalars still require their independent explicit review.
            dispositions: vec![crate::inventory::StorageDisposition {
                trie: "top".into(),
                storage: ITEMS[1].into(),
                action: crate::inventory::Disposition::RetainHistorical,
                reason: "Exact unchanged fixture statuses; no completion or refund authorized"
                    .into(),
                evidence_hash: hx(&[3; 32]),
            }],
            ..Default::default()
        };
        let inventory = crate::inventory::inventory_with_resolutions(
            &s,
            &storage,
            &map.iter()
                .map(|r| (*r.old.as_ref(), *r.new.as_ref()))
                .collect(),
            &options,
            &c.resolutions,
            &c.edits,
        )
        .unwrap();
        assert!(inventory.candidates.is_empty());
        assert!(inventory.semantic_dispositions_complete);
        c.verify_groups(&c.groups, &map).unwrap();
        let mut altered = c.groups.clone();
        altered.push(vec![edit(source.clone(), storage.top.get(&source), None)]);
        assert!(c.verify_groups(&altered, &map).is_err());
        let without_marker: Vec<_> = c
            .edits
            .iter()
            .filter(|e| !e.key.starts_with(INCOMING_OWNERSHIP_PREFIX))
            .cloned()
            .collect();
        assert!(crate::inventory::inventory_with_resolutions(
            &s,
            &storage,
            &map.iter()
                .map(|r| (*r.old.as_ref(), *r.new.as_ref()))
                .collect(),
            &options,
            &c.resolutions,
            &without_marker
        )
        .is_err());
        assert!(!storage
            .top
            .contains_key(&bridge_types::recovery::historical_hashi_key(
                0u32,
                incoming_id
            )));
        // The query move cannot conceal foreign ownership or duplicate IDs.
        indexes.push((0, incoming_id));
        storage.top.insert(index_key.clone(), indexes.encode());
        assert!(compile(&s, &storage, &map, &review(&storage)).is_err());
        indexes.pop();
        storage.top.insert(index_key, indexes.encode());
        let mut wrong = native;
        let OffchainRequest::LoadIncoming(ref mut load) = wrong else {
            unreachable!()
        };
        *load = LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest::new(
            AccountId::new([19; 32]),
            load.hash(),
            load.timepoint(),
            IncomingTransactionRequestKind::Transfer,
            load.network_id(),
        ));
        storage.top.insert(source, wrong.encode());
        assert!(compile(&s, &storage, &map, &review(&storage)).is_err());
        let legacy_xor = OffchainRequest::<Runtime>::LoadIncoming(
            LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest::new(
                map[0].old.clone(),
                incoming_id,
                Default::default(),
                IncomingTransactionRequestKind::TransferXOR,
                0,
            )),
        );
        storage.top.insert(
            request_key(&s.descriptors[0], 0, incoming_id),
            legacy_xor.encode(),
        );
        assert!(compile(&s, &storage, &map, &review(&storage))
            .err()
            .unwrap()
            .contains("TransferXOR requires native decommission reconciliation"));
    }

    #[test]
    fn pending_incoming_beneficiary_mapping_never_rewrites_authenticated_receipt() {
        let (s, mut storage, map) = fixture();
        let native = OffchainRequest::incoming(IncomingRequest::Transfer(
            eth_bridge::requests::IncomingTransfer::<Runtime> {
                from: sp_core::H160::repeat_byte(18),
                to: map[0].old.clone(),
                asset_id: common::DAI,
                asset_kind: eth_bridge::requests::AssetKind::Sidechain,
                amount: 44,
                author: AccountId::new([19; 32]),
                tx_hash: H256::repeat_byte(55),
                at_height: 100,
                timepoint: Default::default(),
                network_id: 0,
                should_take_fee: false,
            },
        ));
        let source = request_key(&s.descriptors[0], 0, native.hash());
        storage.top.insert(source.clone(), native.encode());
        storage.top.insert(
            request_key(&s.descriptors[1], 0, native.hash()),
            RequestStatus::Pending.encode(),
        );
        let c = compile(&s, &storage, &map, &review(&storage)).unwrap();
        assert_eq!(c.report.preserved_incoming_request_records, 1);
        assert!(!c.edits.iter().any(|edit| edit.key == source));
        assert_eq!(c.expected[&source], hash(&native.encode()));
        let inventory = crate::inventory::inventory_with_resolutions(
            &s,
            &storage,
            &map.iter()
                .map(|r| (*r.old.as_ref(), *r.new.as_ref()))
                .collect(),
            &crate::inventory::InventoryOptions::default(),
            &c.resolutions,
            &c.edits,
        )
        .unwrap();
        assert!(inventory.candidates.is_empty());
        let mut no_op = c.edits.clone();
        no_op.push(edit(
            source.clone(),
            storage.top.get(&source),
            Some(native.encode()),
        ));
        assert!(crate::inventory::inventory_with_resolutions(
            &s,
            &storage,
            &map.iter()
                .map(|r| (*r.old.as_ref(), *r.new.as_ref()))
                .collect(),
            &crate::inventory::InventoryOptions::default(),
            &c.resolutions,
            &no_op
        )
        .is_err());
    }
}
