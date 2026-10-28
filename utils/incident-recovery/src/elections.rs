//! Preserve election liabilities while suspending grants issued by replaced keys.
//!
//! Empty stored vote vectors are deliberately retained: native elections ignore
//! their zero edges, while native revote/remove still own the recorded deposit
//! and `phrelect` lock. No refund or candidacy is created by this adapter.
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
use scale_info::{TypeDef, TypeDefPrimitive};
use scale_value::{Composite, Value, ValueDef};
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, storage::Storage};
use std::collections::{BTreeMap, BTreeSet};

const METADATA: &str = "0x0c1d4bf5c3bc5037ad854932443c94faae6c327ecf1f38b3e77d38668cc2b4b8";
pub const ITEMS: &[&str] = &[
    "ElectionsPhragmen.Members",
    "ElectionsPhragmen.RunnersUp",
    "ElectionsPhragmen.Candidates",
    "ElectionsPhragmen.ElectionRounds",
    "ElectionsPhragmen.Voting",
];
const BACKING: &[&str] = &[
    "System.Account",
    "Balances.Locks",
    "Balances.Holds",
    "Balances.Freezes",
    "Balances.Reserves",
];
const MAX_VOTERS: usize = 10_000;
const MAX_CANDIDATES: usize = 1000;
const MAX_VOTES: usize = 16;
const MAX_VALUE: usize = 128 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Review {
    pub evidence_hash: String,
    pub records: Vec<Record>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Record {
    pub storage: String,
    pub key: String,
    /// Every singleton, every Voting row, or its prefix with null if empty.
    pub value_hash: Option<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub evidence_hash: String,
    pub members: usize,
    pub runners_up: usize,
    pub candidates: usize,
    pub voters: usize,
    pub suspended_voters: Vec<String>,
    pub preserved_voter_stake: String,
    pub preserved_recorded_deposits: String,
    pub changed_keys: usize,
    pub refunds_created: usize,
    pub production_ready: bool,
}
type BackingValues = (Option<Vec<u8>>, Option<Vec<u8>>);

pub struct Compilation {
    pub groups: Vec<Vec<GuardedStorageEdit>>,
    pub edits: Vec<GuardedStorageEdit>,
    pub resolutions: Vec<AdapterResolution>,
    pub witnesses: Vec<StorageWitness>,
    pub prefix_witnesses: Vec<StoragePrefixWitness>,
    pub report: Report,
    mapping_hash: [u8; 32],
    before: BTreeMap<Vec<u8>, Vec<u8>>,
    expected: BTreeMap<Vec<u8>, Vec<u8>>,
    prefixes: Vec<Vec<u8>>,
    backing: BTreeMap<Vec<u8>, BackingValues>,
}
impl Compilation {
    /// Mandatory after combining generic and typed edits. Requires exact balance
    /// moves from either inventory or another compiler, never duplicate writes.
    pub fn verify_groups(
        &self,
        groups: &[Vec<GuardedStorageEdit>],
        replacements: &[AccountReplacement],
    ) -> Result<()> {
        if hash(&replacements.encode()) != self.mapping_hash {
            return Err("election projection mapping mismatch".into());
        }
        let mut projected = self.before.clone();
        let mut backing: BTreeMap<_, _> = self
            .backing
            .iter()
            .map(|(k, (v, _))| (k.clone(), v.clone()))
            .collect();
        let mut seen = BTreeSet::new();
        for edit in groups.iter().flatten().filter(|e| {
            e.scope == StorageScope::Top
                && (self.prefixes.iter().any(|p| e.key.starts_with(p))
                    || self.backing.contains_key(&e.key))
        }) {
            if !seen.insert(edit.key.clone()) {
                return Err("duplicate election/backing work".into());
            }
            if let Some(value) = backing.get_mut(&edit.key) {
                if value.as_ref().map(|v| hash(v)) != edit.before {
                    return Err("stale election backing work".into());
                }
                *value = edit.after.clone();
            } else {
                if self.before.get(&edit.key).map(|v| hash(v)) != edit.before {
                    return Err("stale election work".into());
                }
                if let Some(value) = &edit.after {
                    projected.insert(edit.key.clone(), value.clone());
                } else {
                    projected.remove(&edit.key);
                }
            }
        }
        if projected != self.expected || backing.iter().any(|(k, v)| v != &self.backing[k].1) {
            return Err("incomplete or altered election projection/backing move".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Seat {
    who: AccountId32,
    stake: u128,
    deposit: u128,
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Voter {
    votes: Vec<AccountId32>,
    stake: u128,
    deposit: u128,
}
fn hash(b: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(b)
}
fn hx(b: &[u8]) -> String {
    format!("0x{}", hex::encode(b))
}
fn account(a: &AccountId32) -> [u8; 32] {
    *AsRef::<[u8; 32]>::as_ref(a)
}
fn digest(s: &str) -> Result<[u8; 32]> {
    hex_bytes(s)?
        .try_into()
        .map_err(|_| "election hash width".into())
}
fn descriptor<'a>(s: &'a MetadataSchema, n: &str) -> Result<&'a Descriptor> {
    s.descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == n)
        .ok_or_else(|| format!("missing election metadata {n}"))
}
fn sequence(s: &MetadataSchema, id: u32) -> Option<u32> {
    match s.registry.resolve(id).map(|t| &t.type_def) {
        Some(TypeDef::Sequence(v)) => Some(v.type_param.id),
        _ => None,
    }
}
fn primitive(s: &MetadataSchema, id: u32, p: TypeDefPrimitive) -> bool {
    matches!(s.registry.resolve(id).map(|t|&t.type_def),Some(TypeDef::Primitive(v)) if *v==p)
}
fn shape(s: &MetadataSchema, d: &Descriptor) -> Result<()> {
    let valid = match d.name.as_str() {
        "ElectionsPhragmen.ElectionRounds" => primitive(s,d.value_type,TypeDefPrimitive::U32),
        "ElectionsPhragmen.Candidates" => sequence(s,d.value_type).is_some_and(|id| matches!(s.registry.resolve(id).map(|t|&t.type_def), Some(TypeDef::Tuple(t)) if t.fields.len()==2 && typed::account_type(&s.registry,t.fields[0].id) && primitive(s,t.fields[1].id,TypeDefPrimitive::U128))),
        _ => {
            let id=if d.name.ends_with("Voting"){Some(d.value_type)}else{sequence(s,d.value_type)};
            id.is_some_and(|id|matches!(s.registry.resolve(id).map(|t|&t.type_def),Some(TypeDef::Composite(c)) if c.fields.len()==3 && c.fields[0].name.as_deref()==Some(if d.name.ends_with("Voting"){"votes"}else{"who"}) && c.fields[1].name.as_deref()==Some("stake") && c.fields[2].name.as_deref()==Some("deposit") && (if d.name.ends_with("Voting"){sequence(s,c.fields[0].ty.id).is_some_and(|id|typed::account_type(&s.registry,id))}else{typed::account_type(&s.registry,c.fields[0].ty.id)}) && primitive(s,c.fields[1].ty.id,TypeDefPrimitive::U128) && primitive(s,c.fields[2].ty.id,TypeDefPrimitive::U128)))
        }
    };
    if !valid {
        return Err(format!("unreviewed election codec {}", d.name));
    }
    if d.name.ends_with("Voting") {
        check_map(s, d, Hasher::Twox64Concat)
    } else if d.hashers.is_empty() && d.key_types.is_empty() {
        Ok(())
    } else {
        Err("election singleton key changed".into())
    }
}
fn check_map(s: &MetadataSchema, d: &Descriptor, h: Hasher) -> Result<()> {
    if d.hashers != [h]
        || d.key_types.len() != 1
        || !typed::account_type(&s.registry, d.key_types[0])
    {
        return Err(format!("unreviewed election account key {}", d.name));
    }
    Ok(())
}
fn key(d: &Descriptor, a: &[u8; 32]) -> Vec<u8> {
    let mut k = d.prefix.clone();
    match d.hashers[0] {
        Hasher::Twox64Concat => k.extend(sp_io::hashing::twox_64(a)),
        Hasher::Blake2_128Concat => k.extend(sp_io::hashing::blake2_128(a)),
        _ => unreachable!("validated account hasher"),
    };
    k.extend(a);
    k
}
fn decode<T: Decode + Encode>(s: &MetadataSchema, d: &Descriptor, b: &[u8]) -> Result<T> {
    if b.len() > MAX_VALUE {
        return Err("election value exceeds bound".into());
    }
    typed::decode_exact(b, d.value_type, &s.registry)?;
    let mut input = b;
    let v = T::decode(&mut input).map_err(|e| e.to_string())?;
    if !input.is_empty() || v.encode() != b {
        return Err("noncanonical election value".into());
    }
    Ok(v)
}
fn field<'a>(v: &'a Value<u32>, n: &str) -> Result<&'a Value<u32>> {
    let ValueDef::Composite(Composite::Named(fs)) = &v.value else {
        return Err("election backing record layout".into());
    };
    fs.iter()
        .find(|(k, _)| k == n)
        .map(|(_, v)| v)
        .ok_or_else(|| format!("missing election backing field {n}"))
}
fn items<'a>(s: &MetadataSchema, mut v: &'a Value<u32>) -> Result<&'a [Value<u32>]> {
    for _ in 0..8 {
        match (s.registry.resolve(v.context).map(|t| &t.type_def), &v.value) {
            (Some(TypeDef::Sequence(_)), ValueDef::Composite(Composite::Unnamed(xs))) => {
                return Ok(xs)
            }
            (Some(TypeDef::Composite(c)), ValueDef::Composite(Composite::Unnamed(xs)))
                if c.fields.len() == 1 && xs.len() == 1 =>
            {
                v = &xs[0]
            }
            _ => return Err("election backing vector layout".into()),
        }
    }
    Err("election backing vector nesting".into())
}
fn amount(v: &Value<u32>, n: &str) -> Result<u128> {
    field(v, n)?
        .as_u128()
        .ok_or_else(|| format!("election backing integer {n}"))
}
fn financial_check(
    s: &MetadataSchema,
    st: &Storage,
    a: &[u8; 32],
    deposit: u128,
    stake: Option<u128>,
) -> Result<()> {
    let sys = descriptor(s, "System.Account")?;
    let bytes = st
        .top
        .get(&key(sys, a))
        .ok_or("election participant lacks System.Account")?;
    let value = typed::decode_exact(bytes, sys.value_type, &s.registry)?;
    let data = field(&value, "data")?;
    if amount(data, "reserved")? < deposit {
        return Err("election recorded deposits exceed native reserved backing".into());
    }
    if let Some(stake) = stake {
        let locks = descriptor(s, "Balances.Locks")?;
        let bytes = st
            .top
            .get(&key(locks, a))
            .ok_or("voter lacks election lock")?;
        let value = typed::decode_exact(bytes, locks.value_type, &s.registry)?;
        let mut election_lock = None;
        for lock in items(s, &value)? {
            if typed::encode(field(lock, "id")?, &s.registry)?.as_slice() == b"phrelect" {
                if election_lock.replace(amount(lock, "amount")?).is_some() {
                    return Err("duplicate election lock".into());
                }
                if !matches!(&field(lock,"reasons")?.value,ValueDef::Variant(v) if v.name=="All"&&v.values.is_empty())
                {
                    return Err("election lock reasons changed".into());
                }
            }
        }
        if election_lock != Some(stake) || amount(data, "free")? < stake {
            return Err("election stake and native lock/backing differ".into());
        }
    }
    Ok(())
}

pub fn compile(
    s: &MetadataSchema,
    st: &Storage,
    replacements: &[AccountReplacement],
    review: &Review,
) -> Result<Compilation> {
    recovery::validate_replacements(replacements)
        .map_err(|e| format!("invalid election mapping: {e:?}"))?;
    if s.metadata_hash != METADATA
        || digest(&review.evidence_hash)? == [0; 32]
        || review.records.len() > MAX_VOTERS + 4
    {
        return Err(
            "election review requires pinned metadata and bounded complete inventory".into(),
        );
    }
    let mapping: BTreeMap<_, _> = replacements
        .iter()
        .map(|r| (account(&r.old), account(&r.new)))
        .collect();
    let effective =
        |a: &AccountId32| AccountId32::new(*mapping.get(&account(a)).unwrap_or(&account(a)));
    let mut before = BTreeMap::new();
    let mut reviewed = BTreeMap::new();
    let mut prefix_witnesses = vec![];
    let mut prefixes = vec![];
    for name in ITEMS {
        let d = descriptor(s, name)?;
        shape(s, d)?;
        prefixes.push(d.prefix.clone());
        let rows: Vec<_> = st
            .top
            .range(d.prefix.clone()..)
            .take_while(|(k, _)| k.starts_with(&d.prefix))
            .take(MAX_VOTERS + 1)
            .collect();
        if rows.len() > MAX_VOTERS {
            return Err("election row bound".into());
        }
        if rows.is_empty() {
            reviewed.insert((name.to_string(), d.prefix.clone()), None);
            if !d.hashers.is_empty() {
                prefix_witnesses.push(StoragePrefixWitness {
                    scope: StorageScope::Top,
                    prefix: d.prefix.clone(),
                });
            }
        }
        for (k, v) in rows {
            if v.len() > MAX_VALUE {
                return Err("election record size bound".into());
            }
            if d.hashers.is_empty() && *k != d.prefix {
                return Err("unexpected election singleton suffix".into());
            }
            if !d.hashers.is_empty() {
                let a: [u8; 32] = k
                    .get(k.len().saturating_sub(32)..)
                    .ok_or("election owner key")?
                    .try_into()
                    .map_err(|_| "election owner width")?;
                if key(d, &a) != *k {
                    return Err("election voter key hash mismatch".into());
                }
            }
            reviewed.insert((name.to_string(), k.clone()), Some(hash(v)));
            before.insert(k.clone(), v.clone());
        }
    }
    let mut supplied = BTreeMap::new();
    for r in &review.records {
        if supplied
            .insert(
                (r.storage.clone(), hex_bytes(&r.key)?),
                r.value_hash.as_deref().map(digest).transpose()?,
            )
            .is_some()
        {
            return Err("duplicate election review record".into());
        }
    }
    if supplied != reviewed {
        return Err("election review must exactly cover all five native storage items".into());
    }
    for n in BACKING {
        check_map(s, descriptor(s, n)?, Hasher::Blake2_128Concat)?;
    }
    let seats = |name: &str| -> Result<Vec<Seat>> {
        let d = descriptor(s, name)?;
        st.top
            .get(&d.prefix)
            .map(|b| decode(s, d, b))
            .transpose()
            .map(|v| v.unwrap_or_default())
    };
    let mut members = seats(ITEMS[0])?;
    let mut runners = seats(ITEMS[1])?;
    let candidate_d = descriptor(s, ITEMS[2])?;
    let mut candidates: Vec<(AccountId32, u128)> = st
        .top
        .get(&candidate_d.prefix)
        .map(|b| decode(s, candidate_d, b))
        .transpose()?
        .unwrap_or_default();
    let rounds = descriptor(s, ITEMS[3])?;
    if let Some(b) = st.top.get(&rounds.prefix) {
        let _: u32 = decode(s, rounds, b)?;
    }
    if members.len() > 13
        || runners.len() > 20
        || candidates.len() > MAX_CANDIDATES
        || members.windows(2).any(|p| p[0].who >= p[1].who)
        || runners.windows(2).any(|p| p[0].stake > p[1].stake)
        || candidates.windows(2).any(|p| p[0].0 >= p[1].0)
        || members.iter().chain(&runners).any(|x| x.stake == 0)
    {
        return Err("invalid native election roster order/bounds/stake".into());
    }
    let mut deposits = BTreeMap::<[u8; 32], u128>::new();
    let mut owners = BTreeSet::new();
    for (who, deposit) in members
        .iter()
        .chain(&runners)
        .map(|x| (&x.who, x.deposit))
        .chain(candidates.iter().map(|(a, d)| (a, *d)))
    {
        if !owners.insert(account(who)) {
            return Err("election candidacy rosters overlap".into());
        }
        deposits.insert(account(who), deposit);
    }
    let voting = descriptor(s, ITEMS[4])?;
    let mut voters = BTreeMap::new();
    let mut total_stake = 0u128;
    let mut suspended = vec![];
    for (k, b) in before.iter().filter(|(k, _)| k.starts_with(&voting.prefix)) {
        let a: [u8; 32] = k[k.len() - 32..].try_into().map_err(|_| "voter key")?;
        let v: Voter = decode(s, voting, b)?;
        if v.votes.len() > MAX_VOTES {
            return Err("native voter target bound".into());
        }
        total_stake = total_stake
            .checked_add(v.stake)
            .ok_or("election stake overflow")?;
        let d = deposits.entry(a).or_default();
        *d = d
            .checked_add(v.deposit)
            .ok_or("election deposit overflow")?;
        voters.insert(a, v);
        owners.insert(a);
    }
    if mapping.values().any(|a| owners.contains(a)) {
        return Err("replacement election destination has an existing interest".into());
    }
    for (a, deposit) in &deposits {
        financial_check(s, st, a, *deposit, voters.get(a).map(|v| v.stake))?;
    }
    let total_deposits = deposits
        .values()
        .try_fold(0u128, |a, b| a.checked_add(*b))
        .ok_or("election deposits overflow")?;
    let mut expected = before.clone();
    let mut transformed = BTreeMap::new();
    for name in &ITEMS[..3] {
        let d = descriptor(s, name)?;
        if !before.contains_key(&d.prefix) {
            continue;
        }
        let value = match *name {
            "ElectionsPhragmen.Members" => {
                for x in &mut members {
                    x.who = effective(&x.who);
                }
                members.sort_by(|a, b| a.who.cmp(&b.who));
                members.encode()
            }
            "ElectionsPhragmen.RunnersUp" => {
                for x in &mut runners {
                    x.who = effective(&x.who);
                }
                runners.encode()
            }
            _ => {
                for (a, _) in &mut candidates {
                    *a = effective(a);
                }
                candidates.sort_by(|a, b| a.0.cmp(&b.0));
                candidates.encode()
            }
        };
        typed::decode_exact(&value, d.value_type, &s.registry)?;
        expected.insert(d.prefix.clone(), value);
        transformed.insert(d.prefix.clone(), d.prefix.clone());
    }
    for (a, v) in &voters {
        let source = key(voting, a);
        let mut v = v.clone();
        let target = if let Some(new) = mapping.get(a) {
            v.votes.clear();
            suspended.push(hx(a));
            *new
        } else {
            for a in &mut v.votes {
                *a = effective(a);
            }
            *a
        };
        let dest = key(voting, &target);
        expected.remove(&source);
        if source != dest && before.contains_key(&dest) {
            return Err("election voter destination occupied".into());
        }
        expected.insert(dest.clone(), v.encode());
        transformed.insert(source, dest);
    }
    let mut backing = BTreeMap::new();
    let mut witnesses = vec![];
    for old in owners.iter().filter(|a| mapping.contains_key(*a)) {
        let new = mapping[old];
        for name in BACKING {
            let d = descriptor(s, name)?;
            let source = key(d, old);
            let target = key(d, &new);
            if st.top.contains_key(&target) {
                return Err("replacement election balance/backing destination occupied".into());
            }
            let bytes = st.top.get(&source).cloned();
            if let Some(b) = &bytes {
                typed::decode_exact(b, d.value_type, &s.registry)?;
            }
            backing.insert(source.clone(), (bytes.clone(), None));
            backing.insert(target.clone(), (None, bytes.clone()));
            witnesses.extend([
                StorageWitness {
                    scope: StorageScope::Top,
                    key: source,
                    value_hash: None,
                },
                StorageWitness {
                    scope: StorageScope::Top,
                    key: target,
                    value_hash: bytes.as_ref().map(|b| hash(b)),
                },
            ]);
        }
    }
    let keys: BTreeSet<_> = before.keys().chain(expected.keys()).cloned().collect();
    let mut edits = vec![];
    for k in &keys {
        let old = before.get(k);
        let new = expected.get(k);
        witnesses.push(StorageWitness {
            scope: StorageScope::Top,
            key: k.clone(),
            value_hash: new.map(|v| hash(v)),
        });
        if old != new {
            edits.push(GuardedStorageEdit {
                scope: StorageScope::Top,
                key: k.clone(),
                before: old.map(|v| hash(v)),
                after: new.cloned(),
            });
        }
    }
    for name in &ITEMS[..4] {
        let d = descriptor(s, name)?;
        if !before.contains_key(&d.prefix) {
            witnesses.push(StorageWitness {
                scope: StorageScope::Top,
                key: d.prefix.clone(),
                value_hash: None,
            });
        }
    }
    let mut resolutions = vec![];
    for (source, dest) in transformed {
        let original = &before[&source];
        if expected.get(&source) == Some(original) {
            continue;
        }
        let d = ITEMS
            .iter()
            .find_map(|n| {
                let d = descriptor(s, n).ok()?;
                source.starts_with(&d.prefix).then_some(d)
            })
            .ok_or("election resolution descriptor")?;
        let relevant: Vec<_> = edits
            .iter()
            .filter(|e| e.key == source || e.key == dest)
            .cloned()
            .collect();
        let (_, mut observed) =
            typed::transform_value(original, d.value_type, &s.registry, &BTreeMap::new())?;
        if !d.hashers.is_empty() {
            observed.insert(
                source[source.len() - 32..]
                    .try_into()
                    .map_err(|_| "election observed account")?,
            );
        }
        resolutions.push(AdapterResolution {
            storage: d.name.clone(),
            key: hx(&source),
            expected_before_hash: hx(&hash(original)),
            expected_after_hash: expected.get(&source).map(|v| hx(&hash(v))),
            adapter: AdapterId::ElectionsV1,
            action: ResolutionAction::ReplaceControl,
            validated_edits: relevant,
            observed_accounts: observed,
        });
    }
    let groups: Vec<_> = edits.chunks(128).map(|v| v.to_vec()).collect();
    for g in &groups {
        common::recovery::RecoveryChunk {
            index: 0,
            edits: g.clone(),
        }
        .validate()
        .map_err(|e| format!("election chunk bound: {e:?}"))?;
    }
    Ok(Compilation {
        report: Report {
            evidence_hash: review.evidence_hash.clone(),
            members: members.len(),
            runners_up: runners.len(),
            candidates: candidates.len(),
            voters: voters.len(),
            suspended_voters: suspended,
            preserved_voter_stake: total_stake.to_string(),
            preserved_recorded_deposits: total_deposits.to_string(),
            changed_keys: edits.len(),
            refunds_created: 0,
            production_ready: false,
        },
        groups,
        edits,
        resolutions,
        witnesses,
        prefix_witnesses,
        mapping_hash: hash(&replacements.encode()),
        before,
        expected,
        prefixes,
        backing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, PortableRegistry, Registry, TypeInfo};
    #[derive(Encode, TypeInfo)]
    struct Data {
        free: u128,
        reserved: u128,
        frozen: u128,
        flags: u128,
    }
    #[derive(Encode, TypeInfo)]
    struct Info {
        nonce: u32,
        consumers: u32,
        providers: u32,
        sufficients: u32,
        data: Data,
    }
    #[derive(Encode, TypeInfo)]
    enum Reasons {
        All,
    }
    #[derive(Encode, TypeInfo)]
    struct Lock {
        id: [u8; 8],
        amount: u128,
        reasons: Reasons,
    }
    fn id(n: u8) -> AccountId32 {
        [n; 32].into()
    }
    fn fixture() -> (MetadataSchema, Storage) {
        let mut r = Registry::new();
        let account = r.register_type(&meta_type::<AccountId32>()).id;
        let seat = r.register_type(&meta_type::<Vec<Seat>>()).id;
        let candidate = r.register_type(&meta_type::<Vec<(AccountId32, u128)>>()).id;
        let voter = r.register_type(&meta_type::<Voter>()).id;
        let rounds = r.register_type(&meta_type::<u32>()).id;
        let info = r.register_type(&meta_type::<Info>()).id;
        let locks = r.register_type(&meta_type::<Vec<Lock>>()).id;
        let unused = r.register_type(&meta_type::<Vec<()>>()).id;
        let descriptors = ITEMS
            .iter()
            .chain(BACKING.iter())
            .map(|n| {
                let (p, i) = n.split_once('.').unwrap();
                let mut prefix = sp_io::hashing::twox_128(p.as_bytes()).to_vec();
                prefix.extend(sp_io::hashing::twox_128(i.as_bytes()));
                let h = if *n == ITEMS[4] {
                    vec![Hasher::Twox64Concat]
                } else if BACKING.contains(n) {
                    vec![Hasher::Blake2_128Concat]
                } else {
                    vec![]
                };
                let value_type = match *n {
                    "ElectionsPhragmen.Members" | "ElectionsPhragmen.RunnersUp" => seat,
                    "ElectionsPhragmen.Candidates" => candidate,
                    "ElectionsPhragmen.Voting" => voter,
                    "ElectionsPhragmen.ElectionRounds" => rounds,
                    "System.Account" => info,
                    "Balances.Locks" => locks,
                    _ => unused,
                };
                Descriptor {
                    trie: "top".into(),
                    name: n.to_string(),
                    prefix,
                    key_types: if h.is_empty() { vec![] } else { vec![account] },
                    hashers: h,
                    value_type,
                }
            })
            .collect();
        let s = MetadataSchema {
            registry: PortableRegistry::from(r),
            descriptors,
            metadata_hash: METADATA.into(),
        };
        let mut st = Storage::default();
        for (name, value) in [
            (
                ITEMS[0],
                vec![Seat {
                    who: id(3),
                    stake: 70,
                    deposit: 10,
                }]
                .encode(),
            ),
            (
                ITEMS[1],
                vec![
                    Seat {
                        who: id(4),
                        stake: 20,
                        deposit: 10,
                    },
                    Seat {
                        who: id(5),
                        stake: 20,
                        deposit: 10,
                    },
                ]
                .encode(),
            ),
            (ITEMS[2], vec![(id(6), 10u128)].encode()),
            (ITEMS[3], 19u32.encode()),
        ] {
            st.top
                .insert(descriptor(&s, name).unwrap().prefix.clone(), value);
        }
        for (owner, vote) in [(1, vec![id(3), id(4), id(3)]), (2, vec![id(3), id(5)])] {
            st.top.insert(
                key(descriptor(&s, ITEMS[4]).unwrap(), &[owner; 32]),
                Voter {
                    votes: vote,
                    stake: 50,
                    deposit: 5,
                }
                .encode(),
            );
        }
        for n in 1..=6 {
            st.top.insert(
                key(descriptor(&s, "System.Account").unwrap(), &[n; 32]),
                Info {
                    nonce: 7,
                    consumers: 1,
                    providers: 1,
                    sufficients: 0,
                    data: Data {
                        free: 100,
                        reserved: 20,
                        frozen: 50,
                        flags: 0,
                    },
                }
                .encode(),
            );
            if n <= 2 {
                st.top.insert(
                    key(descriptor(&s, "Balances.Locks").unwrap(), &[n; 32]),
                    vec![Lock {
                        id: *b"phrelect",
                        amount: 50,
                        reasons: Reasons::All,
                    }]
                    .encode(),
                );
            }
        }
        (s, st)
    }
    fn review(s: &MetadataSchema, st: &Storage) -> Review {
        let mut records = vec![];
        for name in ITEMS {
            let d = descriptor(s, name).unwrap();
            let rows: Vec<_> = st
                .top
                .iter()
                .filter(|(k, _)| k.starts_with(&d.prefix))
                .collect();
            if rows.is_empty() {
                records.push(Record {
                    storage: name.to_string(),
                    key: hx(&d.prefix),
                    value_hash: None,
                });
            }
            for (k, v) in rows {
                records.push(Record {
                    storage: name.to_string(),
                    key: hx(k),
                    value_hash: Some(hx(&hash(v))),
                });
            }
        }
        Review {
            evidence_hash: hx(&[8; 32]),
            records,
        }
    }
    fn work(c: &Compilation) -> Vec<Vec<GuardedStorageEdit>> {
        let mut gs = c.groups.clone();
        let moves: Vec<_> = c
            .backing
            .iter()
            .filter(|(_, (a, b))| a != b)
            .map(|(k, (a, b))| GuardedStorageEdit {
                scope: StorageScope::Top,
                key: k.clone(),
                before: a.as_ref().map(|v| hash(v)),
                after: b.clone(),
            })
            .collect();
        if !moves.is_empty() {
            gs.push(moves);
        }
        gs
    }
    #[test]
    fn suspended_voter_keeps_exact_liabilities_and_requires_backing_move() {
        let (s, st) = fixture();
        let map = vec![AccountReplacement {
            old: id(1),
            new: id(9),
        }];
        let c = compile(&s, &st, &map, &review(&s, &st)).unwrap();
        let voting = descriptor(&s, ITEMS[4]).unwrap();
        let v: Voter = decode(&s, voting, &c.expected[&key(voting, &[9; 32])]).unwrap();
        assert!(v.votes.is_empty());
        assert_eq!((v.stake, v.deposit), (50, 5));
        assert!(!c.expected.contains_key(&key(voting, &[1; 32])));
        assert_eq!(c.report.voters, 2);
        assert_eq!(c.report.refunds_created, 0);
        assert!(c.verify_groups(&c.groups, &map).is_err());
        c.verify_groups(&work(&c), &map).unwrap();
        let mut gs = work(&c);
        gs.last_mut()
            .unwrap()
            .iter_mut()
            .find(|edit| edit.after.is_some())
            .unwrap()
            .after = None;
        assert!(c.verify_groups(&gs, &map).is_err());
    }
    #[test]
    fn target_substitution_preserves_unmapped_vote_order_and_runner_merit_order() {
        let (s, st) = fixture();
        let map = vec![
            AccountReplacement {
                old: id(3),
                new: id(9),
            },
            AccountReplacement {
                old: id(4),
                new: id(8),
            },
        ];
        let c = compile(&s, &st, &map, &review(&s, &st)).unwrap();
        let d = descriptor(&s, ITEMS[4]).unwrap();
        let v: Voter = decode(&s, d, &c.expected[&key(d, &[1; 32])]).unwrap();
        assert_eq!(v.votes, vec![id(9), id(8), id(9)]);
        let d = descriptor(&s, ITEMS[1]).unwrap();
        let rows: Vec<Seat> = decode(&s, d, &c.expected[&d.prefix]).unwrap();
        assert_eq!(
            rows.iter().map(|x| x.who.clone()).collect::<Vec<_>>(),
            vec![id(8), id(5)]
        );
        assert_eq!(
            rows.iter()
                .map(|x| (x.stake, x.deposit))
                .collect::<Vec<_>>(),
            vec![(20, 10), (20, 10)]
        );
        c.verify_groups(&work(&c), &map).unwrap();
        assert!(c.report.suspended_voters.is_empty());
    }
    #[test]
    fn missing_review_occupied_destination_and_bad_financial_backing_fail() {
        let (s, mut st) = fixture();
        let mut r = review(&s, &st);
        r.records.pop();
        assert!(compile(&s, &st, &[], &r).is_err());
        let map = vec![AccountReplacement {
            old: id(1),
            new: id(2),
        }];
        assert!(compile(&s, &st, &map, &review(&s, &st)).is_err());
        st.top
            .remove(&key(descriptor(&s, "Balances.Locks").unwrap(), &[1; 32]));
        assert!(compile(&s, &st, &[], &review(&s, &st)).is_err());
    }
    #[test]
    fn untouched_third_party_state_and_monotonic_round_survive_certificate() {
        let (s, st) = fixture();
        let map = vec![AccountReplacement {
            old: id(1),
            new: id(9),
        }];
        let c = compile(&s, &st, &map, &review(&s, &st)).unwrap();
        let d = descriptor(&s, ITEMS[3]).unwrap();
        assert_eq!(c.expected[&d.prefix], st.top[&d.prefix]);
        let mut gs = work(&c);
        gs.push(vec![GuardedStorageEdit {
            scope: StorageScope::Top,
            key: d.prefix.clone(),
            before: Some(hash(&st.top[&d.prefix])),
            after: Some(0u32.encode()),
        }]);
        assert!(c.verify_groups(&gs, &map).is_err());
        assert!(c.verify_groups(&work(&c), &[]).is_err());
        let plain = compile(&s, &st, &[], &review(&s, &st)).unwrap();
        assert!(plain.edits.is_empty());
        plain.verify_groups(&[], &[]).unwrap();
    }
    #[test]
    fn absent_singletons_and_empty_voting_are_committed_without_creating_rosters() {
        let (s, mut st) = fixture();
        let d = descriptor(&s, ITEMS[2]).unwrap();
        st.top.remove(&d.prefix);
        let voting = descriptor(&s, ITEMS[4]).unwrap();
        st.top.retain(|k, _| !k.starts_with(&voting.prefix));
        let c = compile(&s, &st, &[], &review(&s, &st)).unwrap();
        assert!(c.edits.is_empty());
        assert_eq!(c.prefix_witnesses.len(), 1);
        assert_eq!(c.prefix_witnesses[0].prefix, voting.prefix);
        assert!(c
            .witnesses
            .iter()
            .any(|w| w.key == d.prefix && w.value_hash.is_none()));
    }
}
