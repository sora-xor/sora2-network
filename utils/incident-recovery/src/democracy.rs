//! Checkpoint Democracy identity replacement with native undelegation semantics.
//!
//! The complete voting graph is reconciled before and after transformation.
//! Compromised grants are removed, while direct votes, inbound delegations and
//! existing locks remain intact. This adapter never unlocks or transfers funds.
use crate::{
    hex_bytes,
    inventory::{
        AdapterId, AdapterResolution, Descriptor, Hasher, MetadataSchema, ResolutionAction,
    },
    typed, Result,
};
use codec::{Decode, Encode};
use common::recovery::{
    self, AccountReplacement, GuardedStorageEdit, RecoveryChunk, StorageScope, StorageWitness,
};
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, storage::Storage};
use std::collections::{BTreeMap, BTreeSet};

const METADATA: &str = "0x0c1d4bf5c3bc5037ad854932443c94faae6c327ecf1f38b3e77d38668cc2b4b8";
const MAX_ROWS: usize = 10_000;
const MAX_VOTES: usize = 500;
const VOTE_LOCKING_PERIOD: u32 = 432_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Review {
    pub evidence_hash: String,
    /// Every voting/referendum row, absent referenced referenda, and mapped
    /// voting owners' System.Account/Balances.Locks records (including absence).
    pub records: Vec<Record>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Record {
    pub storage: String,
    pub key: String,
    pub value_hash: Option<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub evidence_hash: String,
    pub voting_rows: usize,
    pub finished_referenda: usize,
    pub absent_referenced_referenda: usize,
    pub suspended_grants: Vec<String>,
    pub preserved_direct_votes: usize,
    pub changed_keys: usize,
    pub released_locks: u32,
    pub production_ready: bool,
}
pub struct Compilation {
    pub groups: Vec<Vec<GuardedStorageEdit>>,
    pub edits: Vec<GuardedStorageEdit>,
    pub resolutions: Vec<AdapterResolution>,
    pub witnesses: Vec<StorageWitness>,
    pub report: Report,
    required_moves: Vec<GuardedStorageEdit>,
    required_absences: BTreeSet<Vec<u8>>,
}
impl Compilation {
    /// Lock and System records may be moved by the generic ownership compiler or
    /// another reviewed adapter. Require the same exact move once, never a new
    /// balance, a reduced lock, or a missing lock masquerading as cancellation.
    pub fn verify_backing_moves(&self, groups: &[Vec<GuardedStorageEdit>]) -> Result<()> {
        for required in &self.required_moves {
            let found: Vec<_> = groups
                .iter()
                .flatten()
                .filter(|edit| edit.scope == StorageScope::Top && edit.key == required.key)
                .collect();
            if found.len() != 1 || found[0] != required {
                return Err(
                    "Democracy lock/reference record lacks its exact unchanged-byte move".into(),
                );
            }
        }
        if groups.iter().flatten().any(|edit| {
            edit.scope == StorageScope::Top
                && self.required_absences.contains(&edit.key)
                && edit.after.is_some()
        }) {
            return Err("Democracy backing invented at an originally absent record".into());
        }
        Ok(())
    }
}

// Source-pinned deployed Democracy SCALE layouts. The checkpoint metadata hash
// fixes their semantics/constants, while typed::decode_exact validates every
// actual record against that verified metadata before these bounded models run.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Delegations {
    votes: u128,
    capital: u128,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Prior(u32, u128);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Vote(u8);
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
enum AccountVote {
    Standard { vote: Vote, balance: u128 },
    Split { aye: u128, nay: u128 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
enum Conviction {
    None,
    Locked1x,
    Locked2x,
    Locked3x,
    Locked4x,
    Locked5x,
    Locked6x,
}
impl Conviction {
    fn delegated(self, capital: u128) -> Result<Delegations> {
        let n = self as u8;
        let votes = if n == 0 {
            capital / 10
        } else {
            capital
                .checked_mul(n.into())
                .ok_or("Democracy conviction votes overflow")?
        };
        Ok(Delegations { votes, capital })
    }
    fn lock_periods(self) -> u32 {
        [0, 1, 2, 4, 8, 16, 32][self as usize]
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
enum Voting {
    Direct {
        votes: Vec<(u32, AccountVote)>,
        delegations: Delegations,
        prior: Prior,
    },
    Delegating {
        balance: u128,
        target: AccountId32,
        conviction: Conviction,
        delegations: Delegations,
        prior: Prior,
    },
}
impl Voting {
    fn delegations(&self) -> Delegations {
        match self {
            Self::Direct { delegations, .. } | Self::Delegating { delegations, .. } => *delegations,
        }
    }
    fn delegations_mut(&mut self) -> &mut Delegations {
        match self {
            Self::Direct { delegations, .. } | Self::Delegating { delegations, .. } => delegations,
        }
    }
    fn required_lock(&self) -> Result<u128> {
        match self {
            Self::Delegating { balance, prior, .. } => Ok((*balance).max(prior.1)),
            Self::Direct { votes, prior, .. } => {
                votes.iter().try_fold(prior.1, |amount, (_, vote)| {
                    Ok(amount.max(match vote {
                        AccountVote::Standard { vote, balance } => {
                            if vote.0 & 0x7f > 6 {
                                return Err("invalid Democracy vote conviction".into());
                            }
                            *balance
                        }
                        AccountVote::Split { aye, nay } => {
                            aye.checked_add(*nay).ok_or("split vote capital overflow")?
                        }
                    }))
                })
            }
        }
    }
}
#[derive(Clone, Debug, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Lock {
    id: [u8; 8],
    amount: u128,
    reasons: Reasons,
}
#[derive(Clone, Debug, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
enum Reasons {
    Fee,
    Misc,
    All,
}

fn hash(bytes: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(bytes)
}
fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn id(account: &AccountId32) -> [u8; 32] {
    *AsRef::<[u8; 32]>::as_ref(account)
}
fn digest(value: &str) -> Result<[u8; 32]> {
    let digest = hex_bytes(value)?
        .try_into()
        .map_err(|_| "Democracy hash width")?;
    if digest == [0; 32] {
        return Err("zero Democracy review commitment".into());
    }
    Ok(digest)
}
fn descriptor<'a>(schema: &'a MetadataSchema, name: &str) -> Result<&'a Descriptor> {
    schema
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == name)
        .ok_or_else(|| format!("missing Democracy custody descriptor {name}"))
}
fn key(d: &Descriptor, suffix: &[u8]) -> Result<Vec<u8>> {
    let mut key = d.prefix.clone();
    match d.hashers.as_slice() {
        [Hasher::Twox64Concat] => key.extend(sp_io::hashing::twox_64(suffix)),
        [Hasher::Blake2_128Concat] => key.extend(sp_io::hashing::blake2_128(suffix)),
        _ => return Err("unreviewed Democracy map hasher".into()),
    }
    key.extend(suffix);
    Ok(key)
}
fn rows<'a>(
    storage: &'a Storage,
    d: &Descriptor,
    width: usize,
) -> Result<BTreeMap<Vec<u8>, &'a Vec<u8>>> {
    let mut rows = BTreeMap::new();
    for (stored, value) in storage
        .top
        .range(d.prefix.clone()..)
        .take_while(|(k, _)| k.starts_with(&d.prefix))
    {
        if rows.len() >= MAX_ROWS || stored.len() < width || value.len() > 32 * 1024 {
            return Err("Democracy inventory row/value bound exceeded".into());
        }
        let suffix = &stored[stored.len() - width..];
        if key(d, suffix)? != *stored || rows.insert(suffix.to_vec(), value).is_some() {
            return Err("Democracy map key failed canonical hash roundtrip".into());
        }
    }
    Ok(rows)
}
fn decode<T: Decode + Encode>(schema: &MetadataSchema, d: &Descriptor, bytes: &[u8]) -> Result<T> {
    typed::decode_exact(bytes, d.value_type, &schema.registry)?;
    let mut input = bytes;
    let decoded = T::decode(&mut input).map_err(|e| e.to_string())?;
    if !input.is_empty() || decoded.encode() != bytes {
        return Err("noncanonical Democracy SCALE layout".into());
    }
    Ok(decoded)
}
fn reconcile(votings: &BTreeMap<AccountId32, Voting>) -> Result<()> {
    let mut totals: BTreeMap<AccountId32, Delegations> = BTreeMap::new();
    for (owner, voting) in votings {
        voting.required_lock()?;
        if let Voting::Delegating {
            balance,
            target,
            conviction,
            ..
        } = voting
        {
            if owner == target || !votings.contains_key(target) {
                return Err("Democracy self-delegation or missing native target".into());
            }
            let contribution = conviction.delegated(*balance)?;
            let total = totals.entry(target.clone()).or_default();
            total.votes = total
                .votes
                .checked_add(contribution.votes)
                .ok_or("Democracy aggregate votes overflow")?;
            total.capital = total
                .capital
                .checked_add(contribution.capital)
                .ok_or("Democracy aggregate capital overflow")?;
        }
    }
    for (owner, voting) in votings {
        if voting.delegations() != totals.get(owner).copied().unwrap_or_default() {
            return Err("Democracy native delegation aggregate mismatch".into());
        }
    }
    Ok(())
}

/// Requires the separately verified complete checkpoint and signed mapping.
/// Ongoing referenda need a different tally-aware adapter and fail closed here.
pub fn compile(
    schema: &MetadataSchema,
    storage: &Storage,
    replacements: &[AccountReplacement],
    review: &Review,
) -> Result<Compilation> {
    compile_inner(schema, storage, replacements, review, false)
}

/// Read-only consistency diagnostic, deliberately incapable of approving or
/// emitting a substitution. Production compilation still rejects unused reviews.
pub fn validate_checkpoint(
    schema: &MetadataSchema,
    storage: &Storage,
    review: &Review,
) -> Result<Report> {
    Ok(compile_inner(schema, storage, &[], review, true)?.report)
}

fn compile_inner(
    schema: &MetadataSchema,
    storage: &Storage,
    replacements: &[AccountReplacement],
    review: &Review,
    diagnostic: bool,
) -> Result<Compilation> {
    if schema.metadata_hash != METADATA {
        return Err("Democracy adapter requires pinned checkpoint metadata".into());
    }
    digest(&review.evidence_hash)?;
    recovery::validate_replacements(replacements)
        .map_err(|e| format!("invalid Democracy replacements: {e:?}"))?;
    if review.records.len() > MAX_ROWS * 4 {
        return Err("Democracy review bound exceeded".into());
    }
    let mappings: BTreeMap<_, _> = replacements
        .iter()
        .map(|p| (p.old.clone(), p.new.clone()))
        .collect();
    let resolve = |owner: &AccountId32| {
        mappings
            .get(owner)
            .cloned()
            .unwrap_or_else(|| owner.clone())
    };
    let voting_d = descriptor(schema, "Democracy.VotingOf")?;
    let referenda_d = descriptor(schema, "Democracy.ReferendumInfoOf")?;
    let locks_d = descriptor(schema, "Balances.Locks")?;
    let accounts_d = descriptor(schema, "System.Account")?;
    for (d, hasher) in [
        (voting_d, Hasher::Twox64Concat),
        (locks_d, Hasher::Blake2_128Concat),
        (accounts_d, Hasher::Blake2_128Concat),
    ] {
        if d.hashers != [hasher]
            || d.key_types.len() != 1
            || !typed::account_type(&schema.registry, d.key_types[0])
        {
            return Err("unreviewed Democracy custody account key codec".into());
        }
    }
    if referenda_d.hashers != [Hasher::Twox64Concat]
        || referenda_d.key_types.len() != 1
        || !matches!(
            schema
                .registry
                .resolve(referenda_d.key_types[0])
                .map(|t| &t.type_def),
            Some(scale_info::TypeDef::Primitive(
                scale_info::TypeDefPrimitive::U32
            ))
        )
    {
        return Err("unreviewed Democracy referendum key codec".into());
    }
    let mut expected: BTreeMap<(String, Vec<u8>), Option<[u8; 32]>> = BTreeMap::new();
    let mut witness_map = BTreeMap::new();
    let mut finished = BTreeSet::new();
    for (index, bytes) in rows(storage, referenda_d, 4)? {
        typed::decode_exact(bytes, referenda_d.value_type, &schema.registry)?;
        // Finished { approved: bool, end: u32 } is variant 1 in deployed SCALE.
        if bytes.first() != Some(&1) {
            return Err(
                "ongoing or unreviewed Democracy referendum blocks recovery adapter".into(),
            );
        }
        let mut input = &bytes[1..];
        let (_approved, end) = <(bool, u32)>::decode(&mut input).map_err(|e| e.to_string())?;
        if !input.is_empty() || end > recovery::CHECKPOINT_NUMBER {
            return Err("invalid finished Democracy referendum".into());
        }
        finished.insert(u32::from_le_bytes(
            index
                .as_slice()
                .try_into()
                .map_err(|_| "referendum index width")?,
        ));
        let key = key(referenda_d, &index)?;
        expected.insert((referenda_d.name.clone(), key.clone()), Some(hash(bytes)));
        witness_map.insert(key, Some(hash(bytes)));
    }
    let native = rows(storage, voting_d, 32)?;
    let mut original = BTreeMap::new();
    let mut referenced = BTreeSet::new();
    let mut direct_votes = 0;
    for (owner, bytes) in &native {
        let owner = AccountId32::new(
            owner
                .as_slice()
                .try_into()
                .map_err(|_| "voting owner width")?,
        );
        let value: Voting = decode(schema, voting_d, bytes)?;
        if let Voting::Direct { votes, .. } = &value {
            if votes.len() > MAX_VOTES || votes.windows(2).any(|w| w[0].0 >= w[1].0) {
                return Err("Democracy votes are oversized, duplicate or unsorted".into());
            }
            referenced.extend(votes.iter().map(|(index, _)| *index));
            direct_votes += votes.len();
        }
        expected.insert(
            (voting_d.name.clone(), key(voting_d, owner.as_ref())?),
            Some(hash(bytes)),
        );
        original.insert(owner, value);
    }
    let absent: Vec<_> = referenced.difference(&finished).copied().collect();
    for index in &absent {
        let key = key(referenda_d, &index.to_le_bytes())?;
        if storage.top.contains_key(&key) {
            return Err("unclassified referendum".into());
        }
        expected.insert((referenda_d.name.clone(), key.clone()), None);
        witness_map.insert(key, None);
    }
    reconcile(&original)?;
    let mut projected = original.clone();
    let mut suspended = Vec::new();
    for (owner, voting) in &original {
        if !mappings.contains_key(owner) {
            continue;
        }
        if let Voting::Delegating {
            balance,
            target,
            conviction,
            prior,
            ..
        } = voting
        {
            let contribution = conviction.delegated(*balance)?;
            let aggregate = projected
                .get_mut(target)
                .ok_or("missing delegation target")?
                .delegations_mut();
            aggregate.votes = aggregate
                .votes
                .checked_sub(contribution.votes)
                .ok_or("delegation votes underflow")?;
            aggregate.capital = aggregate
                .capital
                .checked_sub(contribution.capital)
                .ok_or("delegation capital underflow")?;
            let until = recovery::CHECKPOINT_NUMBER
                .checked_add(1)
                .and_then(|n| {
                    VOTE_LOCKING_PERIOD
                        .checked_mul(conviction.lock_periods())
                        .and_then(|period| n.checked_add(period))
                })
                .ok_or("Democracy prior lock overflow")?;
            let delegations = projected.get(owner).ok_or("missing owner")?.delegations();
            projected.insert(
                owner.clone(),
                Voting::Direct {
                    votes: vec![],
                    delegations,
                    prior: Prior(prior.0.max(until), prior.1.max(*balance)),
                },
            );
            suspended.push(hx(resolve(owner).as_ref()));
        }
    }
    let mut transformed = BTreeMap::new();
    for (owner, mut voting) in projected {
        if let Voting::Delegating { target, .. } = &mut voting {
            *target = resolve(target);
        }
        if transformed.insert(resolve(&owner), voting).is_some() {
            return Err("Democracy replacement destination collision".into());
        }
    }
    reconcile(&transformed)?;
    let mut edits = Vec::new();
    let mut resolutions = Vec::new();
    let mut required_moves = Vec::new();
    let mut required_absences = BTreeSet::new();
    for (owner, voting) in &original {
        let old_key = key(voting_d, owner.as_ref())?;
        let effective = resolve(owner);
        let new_key = key(voting_d, effective.as_ref())?;
        let before = storage.top.get(&old_key).ok_or("lost voting preimage")?;
        let after = transformed[&effective].encode();
        typed::decode_exact(&after, voting_d.value_type, &schema.registry)?;
        witness_map.insert(new_key.clone(), Some(hash(&after)));
        if old_key != new_key || *before != after {
            let mut row_edits = Vec::new();
            if old_key != new_key {
                if storage.top.contains_key(&new_key) {
                    return Err("occupied Democracy voting destination".into());
                }
                row_edits.push(GuardedStorageEdit {
                    scope: StorageScope::Top,
                    key: old_key.clone(),
                    before: Some(hash(before)),
                    after: None,
                });
                row_edits.push(GuardedStorageEdit {
                    scope: StorageScope::Top,
                    key: new_key,
                    before: None,
                    after: Some(after),
                });
                witness_map.insert(old_key.clone(), None);
            } else {
                row_edits.push(GuardedStorageEdit {
                    scope: StorageScope::Top,
                    key: old_key.clone(),
                    before: Some(hash(before)),
                    after: Some(after),
                });
            }
            let mut observed = BTreeSet::from([id(owner)]);
            if let Voting::Delegating { target, .. } = voting {
                observed.insert(id(target));
            }
            resolutions.push(AdapterResolution {
                storage: voting_d.name.clone(),
                key: hx(&old_key),
                expected_before_hash: hx(&hash(before)),
                expected_after_hash: row_edits[0].after.as_deref().map(|v| hx(&hash(v))),
                adapter: AdapterId::DemocracyV1,
                action: ResolutionAction::ReplaceControl,
                validated_edits: vec![],
                observed_accounts: observed,
            });
            edits.extend(row_edits);
        }
        if !mappings.contains_key(owner) {
            continue;
        }
        let lock_bytes = storage.top.get(&key(locks_d, owner.as_ref())?);
        let locks: Vec<Lock> = lock_bytes
            .map(|bytes| decode(schema, locks_d, bytes))
            .transpose()?
            .unwrap_or_default();
        if locks.len() > 64
            || locks
                .iter()
                .map(|lock| lock.id)
                .collect::<BTreeSet<_>>()
                .len()
                != locks.len()
        {
            return Err("duplicate or oversized native balance locks".into());
        }
        let lock = locks.iter().find(|lock| lock.id == *b"democrac");
        let required_lock = voting.required_lock()?;
        if required_lock > 0
            && lock.is_none_or(|lock| {
                lock.amount < required_lock || !matches!(lock.reasons, Reasons::All)
            })
        {
            return Err(
                "Democracy voting capital is not covered by its original native lock".into(),
            );
        }
        for d in [locks_d, accounts_d] {
            let source = key(d, owner.as_ref())?;
            let destination = key(d, effective.as_ref())?;
            if storage.top.contains_key(&destination) {
                return Err("occupied Democracy backing destination".into());
            }
            let before = storage.top.get(&source);
            expected.insert((d.name.clone(), source.clone()), before.map(|v| hash(v)));
            if let Some(bytes) = before {
                typed::decode_exact(bytes, d.value_type, &schema.registry)?;
                required_moves.extend([
                    GuardedStorageEdit {
                        scope: StorageScope::Top,
                        key: source.clone(),
                        before: Some(hash(bytes)),
                        after: None,
                    },
                    GuardedStorageEdit {
                        scope: StorageScope::Top,
                        key: destination.clone(),
                        before: None,
                        after: Some(bytes.clone()),
                    },
                ]);
            } else {
                required_absences.extend([source.clone(), destination.clone()]);
            }
            witness_map.insert(source, None);
            witness_map.insert(destination, before.map(|v| hash(v)));
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
            return Err("duplicate Democracy review record".into());
        }
    }
    if expected != supplied {
        return Err(
            "Democracy review does not exactly cover native graph, referenda and mapped backing"
                .into(),
        );
    }
    if edits.is_empty() && !diagnostic {
        return Err("unused Democracy review with no affected voting state".into());
    }
    edits.sort_by(|a, b| a.key.cmp(&b.key));
    if !edits.is_empty() {
        RecoveryChunk {
            index: 0,
            edits: edits.clone(),
        }
        .validate()
        .map_err(|_| "coupled Democracy voting changes exceed one atomic recovery chunk")?;
    }
    for resolution in &mut resolutions {
        resolution.validated_edits = edits.clone();
    }
    Ok(Compilation {
        groups: if edits.is_empty() {
            vec![]
        } else {
            vec![edits.clone()]
        },
        report: Report {
            evidence_hash: review.evidence_hash.clone(),
            voting_rows: original.len(),
            finished_referenda: finished.len(),
            absent_referenced_referenda: absent.len(),
            suspended_grants: suspended,
            preserved_direct_votes: direct_votes,
            changed_keys: edits.len(),
            released_locks: 0,
            production_ready: false,
        },
        edits,
        resolutions,
        required_moves,
        required_absences,
        witnesses: witness_map
            .into_iter()
            .map(|(key, value_hash)| StorageWitness {
                scope: StorageScope::Top,
                key,
                value_hash,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, Registry, TypeInfo};

    #[derive(Encode, TypeInfo)]
    enum Referendum {
        Ongoing(u8),
        Finished { approved: bool, end: u32 },
    }
    fn account(n: u8) -> AccountId32 {
        AccountId32::new([n; 32])
    }
    fn direct(delegations: Delegations) -> Voting {
        Voting::Direct {
            votes: vec![],
            delegations,
            prior: Prior(0, 0),
        }
    }
    fn insert(
        storage: &mut Storage,
        schema: &MetadataSchema,
        name: &str,
        owner: &[u8],
        value: &impl Encode,
    ) {
        storage.top.insert(
            key(descriptor(schema, name).unwrap(), owner).unwrap(),
            value.encode(),
        );
    }
    fn review(schema: &MetadataSchema, storage: &Storage, map: &[AccountReplacement]) -> Review {
        let voting_d = descriptor(schema, "Democracy.VotingOf").unwrap();
        let refs_d = descriptor(schema, "Democracy.ReferendumInfoOf").unwrap();
        let mut records = BTreeMap::new();
        for d in [voting_d, refs_d] {
            for (key, value) in storage
                .top
                .range(d.prefix.clone()..)
                .take_while(|(k, _)| k.starts_with(&d.prefix))
            {
                records.insert((d.name.clone(), key.clone()), Some(hx(&hash(value))));
            }
        }
        for (_, bytes) in rows(storage, voting_d, 32).unwrap() {
            let voting: Voting = decode(schema, voting_d, bytes).unwrap();
            if let Voting::Direct { votes, .. } = voting {
                for (index, _) in votes {
                    let key = key(refs_d, &index.to_le_bytes()).unwrap();
                    records
                        .entry((refs_d.name.clone(), key.clone()))
                        .or_insert_with(|| storage.top.get(&key).map(|v| hx(&hash(v))));
                }
            }
        }
        for mapping in map {
            if !storage
                .top
                .contains_key(&key(voting_d, mapping.old.as_ref()).unwrap())
            {
                continue;
            }
            for name in ["Balances.Locks", "System.Account"] {
                let d = descriptor(schema, name).unwrap();
                let key = key(d, mapping.old.as_ref()).unwrap();
                records.insert(
                    (d.name.clone(), key.clone()),
                    storage.top.get(&key).map(|v| hx(&hash(v))),
                );
            }
        }
        Review {
            evidence_hash: hx(&[99; 32]),
            records: records
                .into_iter()
                .map(|((storage, key), value_hash)| Record {
                    storage,
                    key: hx(&key),
                    value_hash,
                })
                .collect(),
        }
    }
    fn fixture() -> (MetadataSchema, Storage, Vec<AccountReplacement>) {
        let mut registry = Registry::new();
        let owner = registry.register_type(&meta_type::<AccountId32>()).id;
        let index = registry.register_type(&meta_type::<u32>()).id;
        let voting = registry.register_type(&meta_type::<Voting>()).id;
        let referendum = registry.register_type(&meta_type::<Referendum>()).id;
        let locks = registry.register_type(&meta_type::<Vec<Lock>>()).id;
        let info = registry.register_type(&meta_type::<u128>()).id;
        let descriptors = [
            ("Democracy.VotingOf", owner, voting, Hasher::Twox64Concat),
            (
                "Democracy.ReferendumInfoOf",
                index,
                referendum,
                Hasher::Twox64Concat,
            ),
            ("Balances.Locks", owner, locks, Hasher::Blake2_128Concat),
            ("System.Account", owner, info, Hasher::Blake2_128Concat),
        ]
        .into_iter()
        .map(|(name, key_type, value_type, hasher)| {
            let (pallet, item) = name.split_once('.').unwrap();
            let mut prefix = sp_io::hashing::twox_128(pallet.as_bytes()).to_vec();
            prefix.extend(sp_io::hashing::twox_128(item.as_bytes()));
            Descriptor {
                trie: "top".into(),
                name: name.into(),
                prefix,
                key_types: vec![key_type],
                hashers: vec![hasher],
                value_type,
            }
        })
        .collect();
        let schema = MetadataSchema {
            registry: registry.into(),
            descriptors,
            metadata_hash: METADATA.into(),
        };
        let mut storage = Storage::default();
        insert(
            &mut storage,
            &schema,
            "Democracy.VotingOf",
            account(1).as_ref(),
            &Voting::Delegating {
                balance: 100,
                target: account(2),
                conviction: Conviction::Locked2x,
                delegations: Delegations {
                    votes: 7,
                    capital: 7,
                },
                prior: Prior(10, 20),
            },
        );
        insert(
            &mut storage,
            &schema,
            "Democracy.VotingOf",
            account(2).as_ref(),
            &Voting::Direct {
                votes: vec![
                    (
                        0,
                        AccountVote::Standard {
                            vote: Vote(0x81),
                            balance: 50,
                        },
                    ),
                    (9, AccountVote::Split { aye: 5, nay: 4 }),
                ],
                delegations: Delegations {
                    votes: 200,
                    capital: 100,
                },
                prior: Prior(5, 10),
            },
        );
        insert(
            &mut storage,
            &schema,
            "Democracy.VotingOf",
            account(3).as_ref(),
            &Voting::Delegating {
                balance: 7,
                target: account(1),
                conviction: Conviction::Locked1x,
                delegations: Delegations::default(),
                prior: Prior(0, 0),
            },
        );
        insert(
            &mut storage,
            &schema,
            "Democracy.ReferendumInfoOf",
            &0u32.to_le_bytes(),
            &Referendum::Finished {
                approved: true,
                end: recovery::CHECKPOINT_NUMBER - 10,
            },
        );
        insert(
            &mut storage,
            &schema,
            "Balances.Locks",
            account(1).as_ref(),
            &vec![
                Lock {
                    id: *b"democrac",
                    amount: 100,
                    reasons: Reasons::All,
                },
                Lock {
                    id: *b"staking ",
                    amount: 200,
                    reasons: Reasons::Misc,
                },
            ],
        );
        insert(
            &mut storage,
            &schema,
            "System.Account",
            account(1).as_ref(),
            &1_000u128,
        );
        (
            schema,
            storage,
            vec![AccountReplacement {
                old: account(1),
                new: account(11),
            }],
        )
    }
    fn project(
        schema: &MetadataSchema,
        storage: &Storage,
        edits: &[GuardedStorageEdit],
        owner: u8,
    ) -> Voting {
        let key = key(
            descriptor(schema, "Democracy.VotingOf").unwrap(),
            account(owner).as_ref(),
        )
        .unwrap();
        let bytes = edits
            .iter()
            .find(|e| e.key == key)
            .map(|e| e.after.as_ref())
            .unwrap_or_else(|| storage.top.get(&key))
            .unwrap();
        decode(
            schema,
            descriptor(schema, "Democracy.VotingOf").unwrap(),
            bytes,
        )
        .unwrap()
    }

    #[test]
    fn mapped_grant_ends_preserving_inbound_weight_prior_and_all_lock_bytes() {
        let (schema, storage, map) = fixture();
        let result = compile(&schema, &storage, &map, &review(&schema, &storage, &map)).unwrap();
        assert_eq!(
            project(&schema, &storage, &result.edits, 11),
            Voting::Direct {
                votes: vec![],
                delegations: Delegations {
                    votes: 7,
                    capital: 7
                },
                prior: Prior(
                    recovery::CHECKPOINT_NUMBER + 1 + 2 * VOTE_LOCKING_PERIOD,
                    100
                ),
            }
        );
        let Voting::Direct {
            votes,
            delegations,
            prior,
        } = project(&schema, &storage, &result.edits, 2)
        else {
            panic!()
        };
        assert_eq!(delegations, Delegations::default());
        assert_eq!(votes.len(), 2);
        assert_eq!(prior, Prior(5, 10));
        assert!(
            matches!(project(&schema,&storage,&result.edits,3),Voting::Delegating {target,balance:7,..} if target==account(11))
        );
        assert_eq!(result.report.absent_referenced_referenda, 1);
        assert_eq!(result.report.released_locks, 0);
        assert_eq!(result.groups.len(), 1);
        assert!(result.verify_backing_moves(&result.groups).is_err());
        let mut groups = result.groups.clone();
        groups.push(result.required_moves.clone());
        result.verify_backing_moves(&groups).unwrap();
        let locks = key(
            descriptor(&schema, "Balances.Locks").unwrap(),
            account(11).as_ref(),
        )
        .unwrap();
        groups
            .last_mut()
            .unwrap()
            .iter_mut()
            .find(|e| e.key == locks)
            .unwrap()
            .after = Some(Vec::<Lock>::new().encode());
        assert!(result.verify_backing_moves(&groups).is_err());
    }

    #[test]
    fn direct_target_replacement_preserves_innocent_delegations_and_finished_votes() {
        let (schema, mut storage, _) = fixture();
        let map = vec![AccountReplacement {
            old: account(2),
            new: account(12),
        }];
        insert(
            &mut storage,
            &schema,
            "Balances.Locks",
            account(2).as_ref(),
            &vec![Lock {
                id: *b"democrac",
                amount: 50,
                reasons: Reasons::All,
            }],
        );
        let before = project(&schema, &storage, &[], 2);
        let result = compile(&schema, &storage, &map, &review(&schema, &storage, &map)).unwrap();
        assert_eq!(project(&schema, &storage, &result.edits, 12), before);
        assert!(
            matches!(project(&schema,&storage,&result.edits,1),Voting::Delegating {target,balance:100,..} if target==account(12))
        );
        assert!(result.report.suspended_grants.is_empty());
        assert_eq!(result.report.preserved_direct_votes, 2);
        assert!(result
            .verify_backing_moves(std::slice::from_ref(&result.required_moves))
            .is_ok());
    }

    #[test]
    fn all_convictions_preserve_positive_and_zero_lock_obligations() {
        for conviction in [
            Conviction::None,
            Conviction::Locked1x,
            Conviction::Locked2x,
            Conviction::Locked3x,
            Conviction::Locked4x,
            Conviction::Locked5x,
            Conviction::Locked6x,
        ] {
            for balance in [0, 101] {
                let (schema, mut storage, map) = fixture();
                insert(
                    &mut storage,
                    &schema,
                    "Democracy.VotingOf",
                    account(1).as_ref(),
                    &Voting::Delegating {
                        balance,
                        target: account(2),
                        conviction,
                        delegations: Delegations {
                            votes: 7,
                            capital: 7,
                        },
                        prior: Prior(0, 0),
                    },
                );
                insert(
                    &mut storage,
                    &schema,
                    "Democracy.VotingOf",
                    account(2).as_ref(),
                    &direct(conviction.delegated(balance).unwrap()),
                );
                let locks = key(
                    descriptor(&schema, "Balances.Locks").unwrap(),
                    account(1).as_ref(),
                )
                .unwrap();
                if balance == 0 {
                    storage.top.remove(&locks);
                } else {
                    insert(
                        &mut storage,
                        &schema,
                        "Balances.Locks",
                        account(1).as_ref(),
                        &vec![Lock {
                            id: *b"democrac",
                            amount: balance,
                            reasons: Reasons::All,
                        }],
                    );
                }
                let result =
                    compile(&schema, &storage, &map, &review(&schema, &storage, &map)).unwrap();
                let Voting::Direct { prior, .. } = project(&schema, &storage, &result.edits, 11)
                else {
                    panic!()
                };
                assert_eq!(
                    prior,
                    Prior(
                        recovery::CHECKPOINT_NUMBER
                            + 1
                            + VOTE_LOCKING_PERIOD * conviction.lock_periods(),
                        balance
                    )
                );
                let mut groups = vec![result.required_moves.clone()];
                result.verify_backing_moves(&groups).unwrap();
                if balance == 0 {
                    groups[0].push(GuardedStorageEdit {
                        scope: StorageScope::Top,
                        key: key(
                            descriptor(&schema, "Balances.Locks").unwrap(),
                            account(11).as_ref(),
                        )
                        .unwrap(),
                        before: None,
                        after: Some(
                            vec![Lock {
                                id: *b"democrac",
                                amount: 1,
                                reasons: Reasons::All,
                            }]
                            .encode(),
                        ),
                    });
                    assert!(result.verify_backing_moves(&groups).is_err());
                }
            }
        }
    }

    #[test]
    fn complete_graph_review_and_exact_backing_are_mandatory() {
        let (schema, storage, map) = fixture();
        let mut omitted = review(&schema, &storage, &map);
        omitted.records.pop();
        assert!(compile(&schema, &storage, &map, &omitted).is_err());
        let mut duplicate = review(&schema, &storage, &map);
        duplicate.records.push(Record {
            storage: duplicate.records[0].storage.clone(),
            key: duplicate.records[0].key.clone(),
            value_hash: duplicate.records[0].value_hash.clone(),
        });
        assert!(compile(&schema, &storage, &map, &duplicate).is_err());
        for variant in 0..6 {
            let mut changed = storage.clone();
            match variant {
                0 => insert(
                    &mut changed,
                    &schema,
                    "Democracy.VotingOf",
                    account(2).as_ref(),
                    &direct(Delegations {
                        votes: 199,
                        capital: 100,
                    }),
                ),
                1 => {
                    changed.top.remove(
                        &key(
                            descriptor(&schema, "Democracy.VotingOf").unwrap(),
                            account(2).as_ref(),
                        )
                        .unwrap(),
                    );
                }
                2 => insert(
                    &mut changed,
                    &schema,
                    "Democracy.ReferendumInfoOf",
                    &0u32.to_le_bytes(),
                    &Referendum::Ongoing(0),
                ),
                3 => insert(
                    &mut changed,
                    &schema,
                    "Balances.Locks",
                    account(1).as_ref(),
                    &vec![Lock {
                        id: *b"democrac",
                        amount: 99,
                        reasons: Reasons::All,
                    }],
                ),
                4 => insert(
                    &mut changed,
                    &schema,
                    "Balances.Locks",
                    account(1).as_ref(),
                    &vec![Lock {
                        id: *b"democrac",
                        amount: 100,
                        reasons: Reasons::Misc,
                    }],
                ),
                _ => insert(
                    &mut changed,
                    &schema,
                    "System.Account",
                    account(11).as_ref(),
                    &1u128,
                ),
            }
            assert!(
                compile(&schema, &changed, &map, &review(&schema, &changed, &map)).is_err(),
                "variant {variant}"
            );
        }
    }

    #[test]
    fn no_saturating_weight_arithmetic_or_oversized_atomic_work() {
        assert!(Conviction::Locked6x.delegated(u128::MAX).is_err());
        let (schema, mut storage, mut map) = fixture();
        for n in 20..90 {
            insert(
                &mut storage,
                &schema,
                "Democracy.VotingOf",
                account(n).as_ref(),
                &direct(Delegations::default()),
            );
            map.push(AccountReplacement {
                old: account(n),
                new: account(n + 100),
            });
        }
        assert!(
            compile(&schema, &storage, &map, &review(&schema, &storage, &map))
                .err()
                .unwrap()
                .contains("atomic recovery chunk")
        );
    }
}
