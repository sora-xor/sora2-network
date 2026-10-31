//! Exact collective ownership replacement and cancellation of affected approvals.
//!
//! Each existing roster is transformed independently. In particular, an absent
//! TechnicalMembership roster is never populated from TechnicalCommittee.
use crate::{
    hex_bytes,
    inventory::{
        AdapterId, AdapterResolution, Descriptor, Hasher, MetadataSchema, ResolutionAction,
    },
    typed, Result,
};
use codec::{Decode, Encode};
use common::recovery::{
    AccountReplacement, GuardedStorageEdit, StorageScope, StorageWitness, MAX_COUNCIL_MEMBERS,
};
use scale_info::TypeDef;
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, storage::Storage};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Review {
    pub evidence_hash: String,
    /// Complete exact inventory, including absent singleton values.
    pub records: Vec<Record>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Record {
    pub storage: String,
    pub key: String,
    pub value_hash: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub evidence_hash: String,
    pub cancelled_proposals: usize,
    pub changed_rosters: Vec<String>,
    pub changed_keys: usize,
    pub production_ready: bool,
}
pub struct Compilation {
    pub groups: Vec<Vec<GuardedStorageEdit>>,
    pub edits: Vec<GuardedStorageEdit>,
    pub resolutions: Vec<AdapterResolution>,
    pub witnesses: Vec<StorageWitness>,
    pub report: Report,
    technical: BTreeMap<Vec<u8>, RetainedRecord>,
    mapping_commitment: [u8; 32],
    required_edits: Vec<GuardedStorageEdit>,
    reviewed_evidence_hash: String,
}
type RetainedRecord = (Option<Vec<u8>>, Option<Vec<u8>>);
impl Compilation {
    pub(crate) fn evidence_hash(&self) -> &str {
        &self.reviewed_evidence_hash
    }
    pub(crate) fn certifies_work(
        &self,
        replacements: &[AccountReplacement],
        groups: &[Vec<GuardedStorageEdit>],
    ) -> bool {
        hash(&replacements.encode()) == self.mapping_commitment
            && self
                .required_edits
                .iter()
                .all(|required| groups.iter().flatten().any(|edit| edit == required))
    }
    /// Only compiler-produced, exact before/after records can preserve an existing
    /// technical membership discrepancy after substituting affected identities.
    pub(crate) fn certifies_technical(
        &self,
        key: &[u8],
        before: Option<&[u8]>,
        after: Option<&[u8]>,
    ) -> bool {
        self.technical
            .get(key)
            .is_some_and(|(old, new)| old.as_deref() == before && new.as_deref() == after)
    }
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(bytes)
}
fn account_key(id: &AccountId32) -> &[u8; 32] {
    id.as_ref()
}
fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn digest(text: &str) -> Result<[u8; 32]> {
    let bytes: [u8; 32] = hex_bytes(text)?
        .try_into()
        .map_err(|_| "governance hash must be 32 bytes")?;
    if bytes == [0; 32] {
        return Err("zero governance evidence hash".into());
    }
    Ok(bytes)
}
fn descriptor<'a>(schema: &'a MetadataSchema, name: &str) -> Result<&'a Descriptor> {
    schema
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == name)
        .ok_or_else(|| format!("missing governance metadata {name}"))
}
fn decode<T: Decode + Encode>(schema: &MetadataSchema, d: &Descriptor, bytes: &[u8]) -> Result<T> {
    typed::decode_exact(bytes, d.value_type, &schema.registry)?;
    let mut input = bytes;
    let value = T::decode(&mut input).map_err(|e| e.to_string())?;
    if !input.is_empty() || value.encode() != bytes {
        return Err(format!("noncanonical governance codec {}", d.name));
    }
    Ok(value)
}
fn sequence_of_accounts(schema: &MetadataSchema, mut id: u32) -> bool {
    for _ in 0..16 {
        match schema.registry.resolve(id).map(|t| &t.type_def) {
            Some(TypeDef::Composite(c)) if c.fields.len() == 1 => id = c.fields[0].ty.id,
            Some(TypeDef::Sequence(s)) => {
                return typed::account_type(&schema.registry, s.type_param.id)
            }
            _ => return false,
        }
    }
    false
}
fn values<'a>(storage: &'a Storage, d: &Descriptor) -> Result<Vec<(Vec<u8>, &'a Vec<u8>)>> {
    let rows: Vec<_> = storage
        .top
        .range(d.prefix.clone()..)
        .take_while(|(key, _)| key.starts_with(&d.prefix))
        .map(|(key, value)| (key.clone(), value))
        .collect();
    if rows.len() > 1024 {
        return Err("governance prefix exceeds reviewed bound".into());
    }
    if d.hashers.is_empty() && rows.iter().any(|(key, _)| key != &d.prefix) {
        return Err("unexpected governance singleton suffix".into());
    }
    Ok(rows)
}
fn map_hash(d: &Descriptor, key: &[u8]) -> Result<[u8; 32]> {
    if d.hashers != [Hasher::Identity] || d.key_types.len() != 1 {
        return Err(format!("unreviewed collective map hasher {}", d.name));
    }
    key.strip_prefix(d.prefix.as_slice())
        .ok_or("collective key prefix")?
        .try_into()
        .map_err(|_| "collective proposal hash width".into())
}

/// The caller must verify signed substitution proofs and the complete checkpoint
/// before invoking this adapter. Evidence hashes authorize review, not new seats.
pub fn compile(
    schema: &MetadataSchema,
    storage: &Storage,
    replacements: &[AccountReplacement],
    review: &Review,
) -> Result<Compilation> {
    digest(&review.evidence_hash)?;
    common::recovery::validate_replacements(replacements)
        .map_err(|e| format!("invalid governance substitution: {e:?}"))?;
    if review.records.len() > 2048 {
        return Err("governance review exceeds bounded inventory".into());
    }
    let mapping: BTreeMap<[u8; 32], [u8; 32]> = replacements
        .iter()
        .map(|r| (*r.old.as_ref(), *r.new.as_ref()))
        .collect();
    let targets: BTreeSet<_> = mapping.values().copied().collect();
    if mapping.len() != replacements.len()
        || targets.len() != mapping.len()
        || targets.iter().any(|id| mapping.contains_key(id))
    {
        return Err("non-injective governance mapping".into());
    }
    let names: Vec<_> = ["Council", "TechnicalCommittee"]
        .iter()
        .flat_map(|pallet| {
            [
                "Members",
                "Prime",
                "Proposals",
                "ProposalCount",
                "ProposalOf",
                "Voting",
                "CostOf",
            ]
            .iter()
            .map(move |item| format!("{pallet}.{item}"))
        })
        .chain([
            "TechnicalMembership.Members".into(),
            "TechnicalMembership.Prime".into(),
        ])
        .collect();
    let mut expected = BTreeMap::new();
    for name in &names {
        let d = descriptor(schema, name)?;
        let rows = values(storage, d)?;
        if d.hashers.is_empty() {
            expected.insert(
                (name.clone(), d.prefix.clone()),
                storage.top.get(&d.prefix).map(|v| hash(v)),
            );
        } else {
            for (key, value) in rows {
                map_hash(d, &key)?;
                expected.insert((name.clone(), key), Some(hash(value)));
            }
        }
    }
    let mut supplied = BTreeMap::new();
    for row in &review.records {
        if supplied
            .insert(
                (row.storage.clone(), hex_bytes(&row.key)?),
                row.value_hash.as_deref().map(digest).transpose()?,
            )
            .is_some()
        {
            return Err("duplicate governance review record".into());
        }
    }
    if expected != supplied {
        return Err(
            "governance review must bind the complete exact roster and pending-operation inventory"
                .into(),
        );
    }
    let mut out = Compilation {
        groups: vec![],
        edits: vec![],
        resolutions: vec![],
        witnesses: vec![],
        technical: BTreeMap::new(),
        mapping_commitment: hash(&replacements.encode()),
        required_edits: vec![],
        reviewed_evidence_hash: review.evidence_hash.clone(),
        report: Report {
            evidence_hash: review.evidence_hash.clone(),
            cancelled_proposals: 0,
            changed_rosters: vec![],
            changed_keys: 0,
            production_ready: false,
        },
    };
    for pallet in ["Council", "TechnicalCommittee", "TechnicalMembership"] {
        let members = descriptor(schema, &format!("{pallet}.Members"))?;
        let prime = descriptor(schema, &format!("{pallet}.Prime"))?;
        if !members.hashers.is_empty()
            || !prime.hashers.is_empty()
            || !sequence_of_accounts(schema, members.value_type)
            || !typed::account_type(&schema.registry, prime.value_type)
        {
            return Err("unreviewed governance roster shape".into());
        }
        let before_members = storage.top.get(&members.prefix);
        let old: Vec<AccountId32> = match before_members {
            Some(bytes) => decode(schema, members, bytes)?,
            None => vec![],
        };
        if old.len() > MAX_COUNCIL_MEMBERS || old.windows(2).any(|p| p[0] >= p[1]) {
            return Err("checkpoint governance roster not sorted/unique/bounded".into());
        }
        if old.iter().any(|a| targets.contains(account_key(a))) {
            return Err("replacement identity already owns a governance seat".into());
        }
        let mut new: Vec<_> = old
            .iter()
            .map(|id| {
                mapping
                    .get(account_key(id))
                    .map_or_else(|| id.clone(), |new| AccountId32::new(*new))
            })
            .collect();
        new.sort();
        let before_prime = storage.top.get(&prime.prefix);
        let old_prime: Option<AccountId32> = before_prime
            .map(|bytes| decode(schema, prime, bytes))
            .transpose()?;
        if old_prime.as_ref().is_some_and(|id| !old.contains(id)) {
            return Err("checkpoint prime is outside its own roster".into());
        }
        let new_prime = old_prime.as_ref().map(|id| {
            mapping
                .get(account_key(id))
                .map_or_else(|| id.clone(), |new| AccountId32::new(*new))
        });
        let mut roster_group = Vec::new();
        let after_members = before_members.map(|_| new.encode());
        let after_prime = new_prime.map(|id| id.encode());
        for (d, before, after) in [
            (members, before_members, after_members),
            (prime, before_prime, after_prime),
        ] {
            if pallet.starts_with("Technical") {
                out.technical
                    .insert(d.prefix.clone(), (before.cloned(), after.clone()));
            }
            record(
                &mut out,
                schema,
                d,
                d.prefix.clone(),
                before,
                after,
                &mut roster_group,
            )?;
        }
        let changed_roster = new != old;
        if changed_roster {
            out.report.changed_rosters.push(pallet.into());
        }
        if pallet != "TechnicalMembership" {
            let proposals = descriptor(schema, &format!("{pallet}.Proposals"))?;
            let counter = descriptor(schema, &format!("{pallet}.ProposalCount"))?;
            let proposal_of = descriptor(schema, &format!("{pallet}.ProposalOf"))?;
            let voting = descriptor(schema, &format!("{pallet}.Voting"))?;
            let cost = descriptor(schema, &format!("{pallet}.CostOf"))?;
            if !proposals.hashers.is_empty() || !counter.hashers.is_empty() {
                return Err("collective singleton codec changed".into());
            }
            let hashes: Vec<[u8; 32]> = storage
                .top
                .get(&proposals.prefix)
                .map(|v| decode(schema, proposals, v))
                .transpose()?
                .unwrap_or_default();
            let all: BTreeSet<_> = hashes.iter().copied().collect();
            if all.len() != hashes.len() || hashes.len() > 100 {
                return Err("invalid collective proposal index".into());
            }
            let proposal_rows = values(storage, proposal_of)?;
            let vote_rows = values(storage, voting)?;
            if proposal_rows
                .iter()
                .map(|(k, _)| map_hash(proposal_of, k))
                .collect::<Result<BTreeSet<_>>>()?
                != all
                || vote_rows
                    .iter()
                    .map(|(k, _)| map_hash(voting, k))
                    .collect::<Result<BTreeSet<_>>>()?
                    != all
            {
                return Err(
                    "orphan or missing collective proposal/vote; explicit repair required".into(),
                );
            }
            // The pinned deployed runtime uses Consideration=(). A retained cost
            // record needs a separate accounting adapter, never an implicit refund.
            if !values(storage, cost)?.is_empty() {
                return Err(
                    "collective consideration record needs explicit custody accounting".into(),
                );
            }
            let count: u32 = storage
                .top
                .get(&counter.prefix)
                .map(|v| decode(schema, counter, v))
                .transpose()?
                .unwrap_or_default();
            let mut cancel = BTreeSet::new();
            for (key, bytes) in &proposal_rows {
                let id = map_hash(proposal_of, key)?;
                if hash(bytes) != id {
                    return Err("collective proposal preimage hash mismatch".into());
                }
                let (_, owners) = typed::transform_value(
                    bytes,
                    proposal_of.value_type,
                    &schema.registry,
                    &BTreeMap::new(),
                )?;
                if changed_roster || owners.iter().any(|id| mapping.contains_key(id)) {
                    cancel.insert(id);
                }
            }
            for (key, bytes) in &vote_rows {
                let votes: (u32, u32, Vec<AccountId32>, Vec<AccountId32>, u32) =
                    decode(schema, voting, bytes)?;
                let seen: BTreeSet<_> = votes.2.iter().chain(&votes.3).collect();
                if votes.0 >= count
                    || votes.1 == 0
                    || seen.len() != votes.2.len() + votes.3.len()
                    || seen.iter().any(|id| !old.contains(id))
                {
                    return Err("invalid collective voting ownership/index".into());
                }
                if seen.iter().any(|id| mapping.contains_key(account_key(id))) {
                    cancel.insert(map_hash(voting, key)?);
                }
            }
            for id in &cancel {
                let mut group = vec![];
                for d in [proposal_of, voting] {
                    let mut key = d.prefix.clone();
                    key.extend(id);
                    record(
                        &mut out,
                        schema,
                        d,
                        key.clone(),
                        storage.top.get(&key),
                        None,
                        &mut group,
                    )?;
                }
                out.groups.push(group);
            }
            if !cancel.is_empty() {
                let retained: Vec<_> = hashes
                    .into_iter()
                    .filter(|id| !cancel.contains(id))
                    .collect();
                record(
                    &mut out,
                    schema,
                    proposals,
                    proposals.prefix.clone(),
                    storage.top.get(&proposals.prefix),
                    Some(retained.encode()),
                    &mut roster_group,
                )?;
                out.report.cancelled_proposals += cancel.len();
            }
            // ProposalCount stays monotonic. Reusing a proposal index could revive
            // old off-chain approvals, so cancellation never rewinds this counter.
        }
        if !roster_group.is_empty() {
            out.groups.push(roster_group);
        }
    }
    let edited: BTreeSet<_> = out.edits.iter().map(|e| e.key.clone()).collect();
    out.witnesses = expected
        .into_iter()
        .filter(|((_, key), _)| !edited.contains(key))
        .map(|((_, key), value_hash)| StorageWitness {
            scope: StorageScope::Top,
            key,
            value_hash,
        })
        .collect();
    out.report.changed_keys = out.edits.len();
    out.required_edits = out.edits.clone();
    Ok(out)
}

fn record(
    out: &mut Compilation,
    schema: &MetadataSchema,
    d: &Descriptor,
    key: Vec<u8>,
    before: Option<&Vec<u8>>,
    after: Option<Vec<u8>>,
    group: &mut Vec<GuardedStorageEdit>,
) -> Result<()> {
    if before.map(Vec::as_slice) == after.as_deref() {
        return Ok(());
    }
    let before = before.ok_or("governance adapter cannot invent an absent ownership record")?;
    let (_, observed_accounts) =
        typed::transform_value(before, d.value_type, &schema.registry, &BTreeMap::new())?;
    if let Some(bytes) = &after {
        typed::decode_exact(bytes, d.value_type, &schema.registry)?;
    }
    let edit = GuardedStorageEdit {
        scope: StorageScope::Top,
        key: key.clone(),
        before: Some(hash(before)),
        after,
    };
    out.resolutions.push(AdapterResolution {
        storage: d.name.clone(),
        key: hx(&key),
        expected_before_hash: hx(&hash(before)),
        expected_after_hash: edit.after.as_deref().map(|v| hx(&hash(v))),
        adapter: AdapterId::GovernanceV1,
        action: if d.name.ends_with(".Members") || d.name.ends_with(".Prime") {
            ResolutionAction::ReplaceControl
        } else {
            ResolutionAction::CancelPending
        },
        validated_edits: vec![edit.clone()],
        observed_accounts,
    });
    group.push(edit.clone());
    out.edits.push(edit);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, PortableRegistry, Registry};
    fn fixture() -> (MetadataSchema, Storage, Review, Vec<AccountReplacement>) {
        let mut registry = Registry::new();
        let account = registry.register_type(&meta_type::<AccountId32>()).id;
        let members = registry.register_type(&meta_type::<Vec<AccountId32>>()).id;
        let hashes = registry.register_type(&meta_type::<Vec<[u8; 32]>>()).id;
        let hash = registry.register_type(&meta_type::<[u8; 32]>()).id;
        let count = registry.register_type(&meta_type::<u32>()).id;
        let votes = registry
            .register_type(&meta_type::<(
                u32,
                u32,
                Vec<AccountId32>,
                Vec<AccountId32>,
                u32,
            )>())
            .id;
        let call = registry.register_type(&meta_type::<(u8, AccountId32)>()).id;
        let cost = registry.register_type(&meta_type::<(AccountId32, ())>()).id;
        let mut descriptors = vec![];
        for pallet in ["Council", "TechnicalCommittee", "TechnicalMembership"] {
            for (item, ty, map) in [
                ("Members", members, false),
                ("Prime", account, false),
                ("Proposals", hashes, false),
                ("ProposalCount", count, false),
                ("ProposalOf", call, true),
                ("Voting", votes, true),
                ("CostOf", cost, true),
            ] {
                if pallet == "TechnicalMembership" && item != "Members" && item != "Prime" {
                    continue;
                }
                descriptors.push(Descriptor {
                    trie: "top".into(),
                    name: format!("{pallet}.{item}"),
                    prefix: vec![descriptors.len() as u8 + 1; 32],
                    key_types: if map { vec![hash] } else { vec![] },
                    hashers: if map { vec![Hasher::Identity] } else { vec![] },
                    value_type: ty,
                });
            }
        }
        let schema = MetadataSchema {
            registry: PortableRegistry::from(registry),
            descriptors,
            metadata_hash: hx(&[8; 32]),
        };
        let mut storage = Storage::default();
        for pallet in ["Council", "TechnicalCommittee"] {
            storage.top.insert(
                descriptor(&schema, &format!("{pallet}.Members"))
                    .unwrap()
                    .prefix
                    .clone(),
                vec![AccountId32::new([1; 32]), AccountId32::new([2; 32])].encode(),
            );
            storage.top.insert(
                descriptor(&schema, &format!("{pallet}.Prime"))
                    .unwrap()
                    .prefix
                    .clone(),
                AccountId32::new([1; 32]).encode(),
            );
            storage.top.insert(
                descriptor(&schema, &format!("{pallet}.ProposalCount"))
                    .unwrap()
                    .prefix
                    .clone(),
                7u32.encode(),
            );
        }
        let proposal = (7u8, AccountId32::new([2; 32])).encode();
        let id = super::hash(&proposal);
        storage.top.insert(
            descriptor(&schema, "Council.Proposals")
                .unwrap()
                .prefix
                .clone(),
            vec![id].encode(),
        );
        let mut key = descriptor(&schema, "Council.ProposalOf")
            .unwrap()
            .prefix
            .clone();
        key.extend(id);
        storage.top.insert(key, proposal);
        let mut key = descriptor(&schema, "Council.Voting")
            .unwrap()
            .prefix
            .clone();
        key.extend(id);
        storage.top.insert(
            key,
            (
                6u32,
                2u32,
                vec![AccountId32::new([1; 32])],
                Vec::<AccountId32>::new(),
                999u32,
            )
                .encode(),
        );
        let review = review(&schema, &storage);
        (
            schema,
            storage,
            review,
            vec![AccountReplacement {
                old: AccountId32::new([1; 32]),
                new: AccountId32::new([9; 32]),
            }],
        )
    }
    fn review(schema: &MetadataSchema, storage: &Storage) -> Review {
        Review {
            evidence_hash: hx(&[8; 32]),
            records: schema
                .descriptors
                .iter()
                .flat_map(|d| {
                    if d.hashers.is_empty() {
                        vec![Record {
                            storage: d.name.clone(),
                            key: hx(&d.prefix),
                            value_hash: storage.top.get(&d.prefix).map(|v| hx(&hash(v))),
                        }]
                    } else {
                        values(storage, d)
                            .unwrap()
                            .into_iter()
                            .map(|(key, v)| Record {
                                storage: d.name.clone(),
                                key: hx(&key),
                                value_hash: Some(hx(&hash(v))),
                            })
                            .collect()
                    }
                })
                .collect(),
        }
    }
    #[test]
    fn roster_resorts_prime_moves_and_all_affected_approvals_cancel_without_counter_rewind() {
        let (schema, storage, review, map) = fixture();
        let out = compile(&schema, &storage, &map, &review).unwrap();
        let key = &descriptor(&schema, "Council.Members").unwrap().prefix;
        assert_eq!(
            out.edits.iter().find(|e| &e.key == key).unwrap().after,
            Some(vec![AccountId32::new([2; 32]), AccountId32::new([9; 32])].encode())
        );
        let prime = &descriptor(&schema, "Council.Prime").unwrap().prefix;
        assert_eq!(
            out.edits.iter().find(|e| &e.key == prime).unwrap().after,
            Some(AccountId32::new([9; 32]).encode())
        );
        assert_eq!(out.report.cancelled_proposals, 1);
        assert!(!out
            .edits
            .iter()
            .any(|e| e.key == descriptor(&schema, "Council.ProposalCount").unwrap().prefix));
        let absent = &descriptor(&schema, "TechnicalMembership.Members")
            .unwrap()
            .prefix;
        assert!(out.certifies_technical(absent, None, None));
        assert!(!out.certifies_technical(
            absent,
            None,
            Some(&vec![AccountId32::new([9; 32])].encode())
        ));
        assert!(!out.edits.iter().any(|e| &e.key == absent));
        assert!(!out.report.production_ready);
        assert!(out.certifies_work(&map, &out.groups));
        assert!(!out.certifies_work(&[], &out.groups));
        let only_rosters: Vec<_> = out
            .groups
            .iter()
            .filter(|group| group.iter().all(|e| e.after.is_some()))
            .cloned()
            .collect();
        assert!(!out.certifies_work(&map, &only_rosters));
    }
    #[test]
    fn omitted_pending_row_or_wrong_preimage_blocks_substitution() {
        let (schema, storage, mut review, map) = fixture();
        review.records.retain(|r| r.storage != "Council.Voting");
        assert!(compile(&schema, &storage, &map, &review)
            .err()
            .unwrap()
            .contains("complete exact"));
    }
    #[test]
    fn collision_or_nonempty_consideration_never_creates_seats_or_refunds() {
        let (schema, mut storage, mut review, mut map) = fixture();
        map[0].new = AccountId32::new([2; 32]);
        assert!(compile(&schema, &storage, &map, &review).is_err());
        map[0].new = AccountId32::new([9; 32]);
        let mut key = descriptor(&schema, "Council.CostOf")
            .unwrap()
            .prefix
            .clone();
        key.extend([3; 32]);
        storage
            .top
            .insert(key, (AccountId32::new([1; 32]), ()).encode());
        review = self::review(&schema, &storage);
        assert!(compile(&schema, &storage, &map, &review).is_err());
    }
    #[test]
    fn unchanged_roster_still_cancels_a_proposal_naming_replaced_owner() {
        let (schema, mut storage, _, mut map) = fixture();
        map[0].old = AccountId32::new([3; 32]);
        let d = descriptor(&schema, "Council.ProposalOf").unwrap();
        let (old_key, _) = values(&storage, d).unwrap().remove(0);
        let old_hash = map_hash(d, &old_key).unwrap();
        storage.top.remove(&old_key);
        let proposal = (7u8, AccountId32::new([3; 32])).encode();
        let id = hash(&proposal);
        let mut key = d.prefix.clone();
        key.extend(id);
        storage.top.insert(key, proposal);
        let voting = descriptor(&schema, "Council.Voting").unwrap();
        let mut old = voting.prefix.clone();
        old.extend(old_hash);
        let votes = storage.top.remove(&old).unwrap();
        let mut new = voting.prefix.clone();
        new.extend(id);
        storage.top.insert(new, votes);
        storage.top.insert(
            descriptor(&schema, "Council.Proposals")
                .unwrap()
                .prefix
                .clone(),
            vec![id].encode(),
        );
        let out = compile(&schema, &storage, &map, &review(&schema, &storage)).unwrap();
        assert!(out.report.changed_rosters.is_empty());
        assert_eq!(out.report.cancelled_proposals, 1);
    }
}
