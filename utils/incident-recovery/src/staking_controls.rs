//! Checkpoint-specific staking control replacement with unchanged financial data.
//!
//! Deployed revision 36c466398e3f2760e8ab44fdb89de796f5b0dd99 and the retained
//! async ledger both key Ledger by controller and Bonded by stash. Their
//! set_controller_to_stash changes no System references. Holds/locks and legacy
//! staking consumers belong to the stash; the caller must move those exact
//! records separately and pass verify_backing_moves before composing work.

use crate::{
    hex_bytes,
    inventory::{
        AdapterId, AdapterResolution, Descriptor, Hasher, MetadataSchema, ResolutionAction,
    },
    typed, Result,
};
use codec::{Decode, Encode};
use common::recovery::{
    self, AccountReplacement, GuardedStorageEdit, StorageScope, StorageWitness,
};
use scale_info::{TypeDef, TypeDefPrimitive};
use scale_value::{Composite, Value, ValueDef};
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, storage::Storage};
use std::collections::{BTreeMap, BTreeSet};

const METADATA: &str = "0x0c1d4bf5c3bc5037ad854932443c94faae6c327ecf1f38b3e77d38668cc2b4b8";
const MAX_ROWS: usize = 100_000;
const MAX_LEDGER_BYTES: usize = 8192;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Review {
    pub evidence_hash: String,
    /// Every mapped stash, with its original controller and exact paired values.
    pub pairs: Vec<Pair>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Pair {
    pub stash: String,
    pub controller: String,
    pub bonded_hash: String,
    pub ledger_hash: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairReport {
    pub old_stash: String,
    pub old_controller: String,
    pub replacement_owner: String,
    pub total: String,
    pub active: String,
    pub unlocking_chunks: usize,
    pub claimed_eras: usize,
    pub ledger_financial_bytes_hash: String,
    pub backing: BackingReport,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BackingKind {
    NativeHold,
    LegacyLock,
    ZeroStake,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackingReport {
    pub kind: BackingKind,
    pub staking_hold: Option<String>,
    /// Preserved verbatim, including a stale lock after native hold migration.
    /// This amount is neither extra stake nor permission to release a lock.
    pub retained_legacy_staking_lock: Option<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub evidence_hash: String,
    pub pairs: Vec<PairReport>,
    pub changed_keys: usize,
    pub controller_reference_changes: usize,
    pub historical_exposures_changed: bool,
    pub production_ready: bool,
}
pub struct Compilation {
    pub groups: Vec<Vec<GuardedStorageEdit>>,
    pub edits: Vec<GuardedStorageEdit>,
    pub resolutions: Vec<AdapterResolution>,
    /// Final-state witnesses for unchanged bytes at their replacement owner.
    pub witnesses: Vec<StorageWitness>,
    pub report: Report,
    required_moves: Vec<GuardedStorageEdit>,
    required_absences: BTreeSet<Vec<u8>>,
}
impl Compilation {
    /// Accept exact moves from the generic inventory or another typed adapter.
    /// Never create a second System.Account move or waive a missing balance move.
    pub fn verify_backing_moves(&self, groups: &[Vec<GuardedStorageEdit>]) -> Result<()> {
        for required in &self.required_moves {
            let edits: Vec<_> = groups
                .iter()
                .flatten()
                .filter(|edit| edit.scope == StorageScope::Top && edit.key == required.key)
                .collect();
            if edits.len() != 1 || edits[0] != required {
                return Err(
                    "staking backing/reference record lacks its exact unchanged-byte move".into(),
                );
            }
        }
        if groups.iter().flatten().any(|edit| {
            edit.scope == StorageScope::Top
                && self.required_absences.contains(&edit.key)
                && edit.after.is_some()
        }) {
            return Err("staking backing was invented at an originally absent record".into());
        }
        Ok(())
    }
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(bytes)
}
fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn account(id: &AccountId32) -> [u8; 32] {
    *AsRef::<[u8; 32]>::as_ref(id)
}
fn bytes32(text: &str) -> Result<[u8; 32]> {
    hex_bytes(text)?
        .try_into()
        .map_err(|_| "staking account/hash must contain 32 bytes".into())
}
fn descriptor<'a>(schema: &'a MetadataSchema, name: &str) -> Result<&'a Descriptor> {
    schema
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == name)
        .ok_or_else(|| format!("missing pinned staking descriptor {name}"))
}
fn check_map(schema: &MetadataSchema, d: &Descriptor, hasher: Hasher) -> Result<()> {
    if d.hashers != [hasher]
        || d.key_types.len() != 1
        || !typed::account_type(&schema.registry, d.key_types[0])
    {
        return Err(format!("unreviewed staking/account key codec {}", d.name));
    }
    Ok(())
}
fn key(d: &Descriptor, owner: &[u8; 32]) -> Vec<u8> {
    let mut bytes = d.prefix.clone();
    match d.hashers[0] {
        Hasher::Twox64Concat => bytes.extend(sp_io::hashing::twox_64(owner)),
        Hasher::Blake2_128Concat => bytes.extend(sp_io::hashing::blake2_128(owner)),
        _ => unreachable!("checked reversible hasher"),
    }
    bytes.extend(owner);
    bytes
}
fn rows<'a>(storage: &'a Storage, d: &Descriptor) -> Result<BTreeMap<[u8; 32], &'a Vec<u8>>> {
    let mut out = BTreeMap::new();
    for (stored, value) in storage
        .top
        .range(d.prefix.clone()..)
        .take_while(|(key, _)| key.starts_with(&d.prefix))
    {
        if out.len() >= MAX_ROWS || value.len() > MAX_LEDGER_BYTES || stored.len() < 32 {
            return Err("staking prefix exceeds bounded row/value inventory".into());
        }
        let owner: [u8; 32] = stored[stored.len() - 32..]
            .try_into()
            .map_err(|_| "staking key width")?;
        if key(d, &owner) != *stored || out.insert(owner, value).is_some() {
            return Err("staking map key did not roundtrip its native hasher".into());
        }
    }
    Ok(out)
}
fn sequence(schema: &MetadataSchema, mut id: u32) -> Option<u32> {
    for _ in 0..8 {
        match &schema.registry.resolve(id)?.type_def {
            TypeDef::Composite(c) if c.fields.len() == 1 && c.fields[0].name.is_none() => {
                id = c.fields[0].ty.id
            }
            TypeDef::Sequence(s) => return Some(s.type_param.id),
            _ => return None,
        }
    }
    None
}
fn primitive(schema: &MetadataSchema, id: u32, expected: TypeDefPrimitive) -> bool {
    matches!(schema.registry.resolve(id).map(|ty| &ty.type_def), Some(TypeDef::Primitive(actual)) if *actual == expected)
}
fn compact(schema: &MetadataSchema, id: u32, expected: TypeDefPrimitive) -> bool {
    matches!(schema.registry.resolve(id).map(|ty| &ty.type_def), Some(TypeDef::Compact(c)) if primitive(schema, c.type_param.id, expected))
}
fn ledger_shape(schema: &MetadataSchema, d: &Descriptor) -> Result<()> {
    let ty = schema
        .registry
        .resolve(d.value_type)
        .ok_or("ledger type missing")?;
    let TypeDef::Composite(c) = &ty.type_def else {
        return Err("ledger is not composite".into());
    };
    let names: Vec<_> = c.fields.iter().map(|f| f.name.as_deref()).collect();
    if names
        != [
            Some("stash"),
            Some("total"),
            Some("active"),
            Some("unlocking"),
            Some("legacy_claimed_rewards"),
        ]
        || !typed::account_type(&schema.registry, c.fields[0].ty.id)
        || !compact(schema, c.fields[1].ty.id, TypeDefPrimitive::U128)
        || !compact(schema, c.fields[2].ty.id, TypeDefPrimitive::U128)
    {
        return Err("ledger no longer has the pinned v16/async financial layout".into());
    }
    let unlock =
        sequence(schema, c.fields[3].ty.id).ok_or("unlocking is not a bounded sequence")?;
    let Some(TypeDef::Composite(unlock)) = schema.registry.resolve(unlock).map(|ty| &ty.type_def)
    else {
        return Err("unlock chunk layout changed".into());
    };
    if unlock.fields.len() != 2
        || unlock.fields[0].name.as_deref() != Some("value")
        || unlock.fields[1].name.as_deref() != Some("era")
        || !compact(schema, unlock.fields[0].ty.id, TypeDefPrimitive::U128)
        || !compact(schema, unlock.fields[1].ty.id, TypeDefPrimitive::U32)
        || !sequence(schema, c.fields[4].ty.id)
            .is_some_and(|id| primitive(schema, id, TypeDefPrimitive::U32))
    {
        return Err("unlocking/legacy claimed-era codec changed".into());
    }
    Ok(())
}
#[derive(Clone, Decode, Encode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Ledger {
    stash: AccountId32,
    #[codec(compact)]
    total: u128,
    #[codec(compact)]
    active: u128,
    unlocking: Vec<Unlock>,
    legacy_claimed_rewards: Vec<u32>,
}
#[derive(Clone, Decode, Encode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Unlock {
    #[codec(compact)]
    value: u128,
    #[codec(compact)]
    era: u32,
}
fn ledger(schema: &MetadataSchema, d: &Descriptor, bytes: &[u8]) -> Result<Ledger> {
    typed::decode_exact(bytes, d.value_type, &schema.registry)?;
    let mut input = bytes;
    let value = Ledger::decode(&mut input).map_err(|e| e.to_string())?;
    if !input.is_empty()
        || value.encode() != bytes
        || value.unlocking.len() > 32
        || value.legacy_claimed_rewards.len() > 84
        || value
            .unlocking
            .windows(2)
            .any(|pair| pair[0].era > pair[1].era)
        || value
            .legacy_claimed_rewards
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || value
            .unlocking
            .iter()
            .try_fold(value.active, |total, chunk| total.checked_add(chunk.value))
            != Some(value.total)
    {
        return Err("ledger is not canonical, bounded or financially conserved".into());
    }
    Ok(value)
}
fn field<'a>(value: &'a Value<u32>, name: &str) -> Result<&'a Value<u32>> {
    let ValueDef::Composite(Composite::Named(fields)) = &value.value else {
        return Err("named financial record required".into());
    };
    fields
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, value)| value)
        .ok_or_else(|| format!("missing financial field {name}"))
}
fn items<'a>(schema: &MetadataSchema, mut value: &'a Value<u32>) -> Result<&'a [Value<u32>]> {
    for _ in 0..8 {
        match (
            &schema
                .registry
                .resolve(value.context)
                .ok_or("financial type missing")?
                .type_def,
            &value.value,
        ) {
            (TypeDef::Sequence(_), ValueDef::Composite(Composite::Unnamed(values))) => {
                return Ok(values)
            }
            (TypeDef::Composite(c), ValueDef::Composite(Composite::Unnamed(values)))
                if c.fields.len() == 1 && values.len() == 1 =>
            {
                value = &values[0]
            }
            _ => return Err("financial vector layout changed".into()),
        }
    }
    Err("financial sequence nesting exceeds bound".into())
}
fn staking_backing(
    schema: &MetadataSchema,
    storage: &Storage,
    stash: &[u8; 32],
    total: u128,
) -> Result<BackingReport> {
    let system = descriptor(schema, "System.Account")?;
    let account = storage
        .top
        .get(&key(system, stash))
        .ok_or_else(|| format!("mapped staking stash {} has no System.Account", hx(stash)))?;
    let value = typed::decode_exact(account, system.value_type, &schema.registry)?;
    let data = field(&value, "data")?;
    let free = field(data, "free")?.as_u128().ok_or("free balance codec")?;
    let reserved = field(data, "reserved")?
        .as_u128()
        .ok_or("reserved balance codec")?;
    if free
        .checked_add(reserved)
        .is_none_or(|balance| balance < total)
    {
        return Err(format!(
            "staking ledger is not backed by its stash balance: stash={}, ledger_total={total}, free={free}, reserved={reserved}",
            hx(stash)
        ));
    }
    let mut held = None;
    let mut locked = None;
    for name in ["Balances.Holds", "Balances.Locks"] {
        let d = descriptor(schema, name)?;
        let Some(bytes) = storage.top.get(&key(d, stash)) else {
            continue;
        };
        let value = typed::decode_exact(bytes, d.value_type, &schema.registry)?;
        for item in items(schema, &value)? {
            let id = field(item, "id")?;
            let is_staking = if name.ends_with("Holds") {
                match &id.value {
                    ValueDef::Variant(v) if v.name == "Staking" => {
                        match v.values.values().next().map(|value| &value.value) {
                            Some(ValueDef::Variant(v))
                                if v.name == "Staking" && v.values.is_empty() =>
                            {
                                true
                            }
                            _ => return Err("staking hold reason layout changed".into()),
                        }
                    }
                    _ => false,
                }
            } else {
                typed::encode(id, &schema.registry)?.as_slice() == b"staking "
            };
            if is_staking {
                let amount = field(item, "amount")?
                    .as_u128()
                    .ok_or("staking backing amount codec")?;
                let slot = if name.ends_with("Holds") {
                    &mut held
                } else {
                    &mut locked
                };
                if slot.replace(amount).is_some() {
                    return Err(format!(
                        "duplicate staking hold/lock: stash={}, storage={name}, ledger_total={total}",
                        hx(stash)
                    ));
                }
            }
        }
    }
    let providers = field(&value, "providers")?
        .as_u128()
        .ok_or("staking stash providers codec")?;
    let consumers = field(&value, "consumers")?
        .as_u128()
        .ok_or("staking stash consumers codec")?;
    let diagnostic = || {
        format!(
            "stash={}, ledger_total={total}, free={free}, reserved={reserved}, staking_hold={held:?}, legacy_staking_lock={locked:?}, providers={providers}, consumers={consumers}",
            hx(stash)
        )
    };
    // The deployed ledger::update writes a hold without removing a legacy lock.
    // Its inspect_bond_state consequently prefers a nonzero hold; the native
    // remove_old_lock_when_stake_already_on_hold test covers coexistence. An old
    // lock can exceed current free funds after stake has moved into reserved.
    // Never count that retained restriction as another liability or release it.
    let kind = if held.is_some_and(|amount| amount != 0) {
        if held != Some(total) {
            return Err(format!(
                "staking hold must exactly back the ledger: {}",
                diagnostic()
            ));
        }
        BackingKind::NativeHold
    } else if total == 0 {
        // A zero ledger has no financial liability. Native hold updates can
        // reach zero while the pre-migration lock remains. Preserve that lock
        // and its references rather than manufacturing stake or unlocking it.
        BackingKind::ZeroStake
    } else if locked == Some(total) {
        BackingKind::LegacyLock
    } else {
        return Err(format!(
            "positive staking ledger must have exact native hold or legacy lock backing: {}",
            diagnostic()
        ));
    };
    if held.is_some_and(|amount| amount > reserved)
        || (kind == BackingKind::LegacyLock && total > free)
        || (total != 0 && providers == 0)
        || (locked.is_some() && consumers == 0)
    {
        return Err(format!(
            "staking balance or System references do not back the native hold/lock: {}",
            diagnostic()
        ));
    }
    Ok(BackingReport {
        kind,
        staking_hold: held.map(|amount| amount.to_string()),
        retained_legacy_staking_lock: locked.map(|amount| amount.to_string()),
    })
}

pub fn compile(
    schema: &MetadataSchema,
    storage: &Storage,
    replacements: &[AccountReplacement],
    review: &Review,
) -> Result<Compilation> {
    recovery::validate_replacements(replacements)
        .map_err(|e| format!("invalid staking mapping: {e:?}"))?;
    if schema.metadata_hash != METADATA
        || bytes32(&review.evidence_hash)? == [0; 32]
        || review.pairs.len() > recovery::MAX_REPLACEMENTS
    {
        return Err(
            "staking controls require pinned checkpoint metadata and bounded review".into(),
        );
    }
    let bonded = descriptor(schema, "Staking.Bonded")?;
    let ledgers = descriptor(schema, "Staking.Ledger")?;
    check_map(schema, bonded, Hasher::Twox64Concat)?;
    check_map(schema, ledgers, Hasher::Blake2_128Concat)?;
    if !typed::account_type(&schema.registry, bonded.value_type) {
        return Err("Bonded controller codec changed".into());
    }
    ledger_shape(schema, ledgers)?;
    let virtual_stakers = descriptor(schema, "Staking.VirtualStakers")?;
    check_map(schema, virtual_stakers, Hasher::Twox64Concat)?;
    let ownership_names = [
        "System.Account",
        "Balances.Locks",
        "Balances.Holds",
        "Balances.Freezes",
        "Balances.Reserves",
    ];
    for name in ownership_names {
        check_map(schema, descriptor(schema, name)?, Hasher::Blake2_128Concat)?;
    }
    let bonds = rows(storage, bonded)?;
    let native_ledgers = rows(storage, ledgers)?;
    let mapping: BTreeMap<_, _> = replacements
        .iter()
        .map(|pair| (account(&pair.old), account(&pair.new)))
        .collect();
    let mut controllers: BTreeMap<[u8; 32], Vec<[u8; 32]>> = BTreeMap::new();
    for (stash, bytes) in &bonds {
        typed::decode_exact(bytes, bonded.value_type, &schema.registry)?;
        let controller: [u8; 32] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| "Bonded controller width")?;
        controllers.entry(controller).or_default().push(*stash);
        if mapping.contains_key(&controller) && !mapping.contains_key(stash) {
            return Err("mapped controller grants authority over an unmapped stash; explicit owner repair required".into());
        }
    }
    let mut by_stash: BTreeMap<[u8; 32], Vec<[u8; 32]>> = BTreeMap::new();
    let mut parsed = BTreeMap::new();
    for (controller, bytes) in &native_ledgers {
        let ledger = ledger(schema, ledgers, bytes)?;
        by_stash
            .entry(account(&ledger.stash))
            .or_default()
            .push(*controller);
        parsed.insert(*controller, ledger);
    }
    let mut reviewed = BTreeMap::new();
    for pair in &review.pairs {
        if reviewed.insert(bytes32(&pair.stash)?, pair).is_some() {
            return Err("duplicate staking pair review".into());
        }
    }
    let affected: BTreeSet<_> = mapping
        .keys()
        .filter(|old| bonds.contains_key(*old))
        .copied()
        .collect();
    if reviewed.keys().copied().collect::<BTreeSet<_>>() != affected {
        return Err("staking review must contain every affected stash exactly once".into());
    }
    for old in mapping.keys() {
        if !bonds.contains_key(old) && by_stash.contains_key(old) {
            return Err("mapped stash has an orphan ledger".into());
        }
        if let Some(ledger) = parsed.get(old) {
            if !mapping.contains_key(&account(&ledger.stash)) {
                return Err("mapped controller has an orphan or unmapped stash ledger".into());
            }
        }
    }
    let mut out = Compilation {
        groups: vec![],
        edits: vec![],
        resolutions: vec![],
        witnesses: vec![],
        required_moves: vec![],
        required_absences: BTreeSet::new(),
        report: Report {
            evidence_hash: review.evidence_hash.clone(),
            pairs: vec![],
            changed_keys: 0,
            controller_reference_changes: 0,
            historical_exposures_changed: false,
            production_ready: false,
        },
    };
    for old in affected {
        let new = mapping[&old];
        let controller: [u8; 32] = bonds[&old]
            .as_slice()
            .try_into()
            .map_err(|_| "controller width")?;
        let ledger = parsed
            .get(&controller)
            .ok_or("mapped stash has no ledger")?;
        if account(&ledger.stash) != old
            || controllers
                .get(&controller)
                .is_none_or(|stashes| stashes.as_slice() != [old])
            || by_stash
                .get(&old)
                .is_none_or(|controllers| controllers.as_slice() != [controller])
        {
            return Err("staking stash/controller is shared, mismatched or ambiguous".into());
        }
        if storage.top.contains_key(&key(virtual_stakers, &old))
            || storage.top.contains_key(&key(virtual_stakers, &new))
        {
            return Err("virtual staking requires a dedicated backing adapter".into());
        }
        let review = reviewed[&old];
        let original = native_ledgers[&controller];
        if bytes32(&review.controller)? != controller
            || bytes32(&review.bonded_hash)? != hash(bonds[&old])
            || bytes32(&review.ledger_hash)? != hash(original)
        {
            return Err("staking review does not match the exact paired preimages".into());
        }
        let backing = staking_backing(schema, storage, &old, ledger.total)
            .map_err(|error| format!("{error}; controller={}", hx(&controller)))?;
        let mut changed = ledger.clone();
        changed.stash = AccountId32::new(new);
        let after = changed.encode();
        if after[32..] != original[32..] {
            return Err("staking financial ledger bytes changed".into());
        }
        typed::decode_exact(&after, ledgers.value_type, &schema.registry)?;
        let mut group = Vec::new();
        for (d, source, bytes, after) in [
            (bonded, old, bonds[&old].as_slice(), new.to_vec()),
            (ledgers, controller, original.as_slice(), after),
        ] {
            let source_key = key(d, &source);
            let destination = key(d, &new);
            if source_key == destination || storage.top.contains_key(&destination) {
                return Err("replacement staking destination is occupied".into());
            }
            group.push(GuardedStorageEdit {
                scope: StorageScope::Top,
                key: source_key.clone(),
                before: Some(hash(bytes)),
                after: None,
            });
            group.push(GuardedStorageEdit {
                scope: StorageScope::Top,
                key: destination,
                before: None,
                after: Some(after),
            });
        }
        for (d, owner, bytes) in [
            (bonded, old, bonds[&old].as_slice()),
            (ledgers, controller, original.as_slice()),
        ] {
            out.resolutions.push(AdapterResolution {
                storage: d.name.clone(),
                key: hx(&key(d, &owner)),
                expected_before_hash: hx(&hash(bytes)),
                expected_after_hash: None,
                adapter: AdapterId::StakingControlV1,
                action: ResolutionAction::ReplaceControl,
                validated_edits: group.clone(),
                observed_accounts: BTreeSet::from([old, controller]),
            });
        }
        for name in ownership_names {
            let d = descriptor(schema, name)?;
            let source = key(d, &old);
            let destination = key(d, &new);
            if storage.top.contains_key(&destination) {
                return Err("replacement staking backing destination is occupied".into());
            }
            let value = storage.top.get(&source);
            if let Some(value) = value {
                typed::decode_exact(value, d.value_type, &schema.registry)?;
                out.required_moves.extend([
                    GuardedStorageEdit {
                        scope: StorageScope::Top,
                        key: source.clone(),
                        before: Some(hash(value)),
                        after: None,
                    },
                    GuardedStorageEdit {
                        scope: StorageScope::Top,
                        key: destination.clone(),
                        before: None,
                        after: Some(value.clone()),
                    },
                ]);
            } else {
                out.required_absences
                    .extend([source.clone(), destination.clone()]);
            }
            out.witnesses.extend([
                StorageWitness {
                    scope: StorageScope::Top,
                    key: source,
                    value_hash: None,
                },
                StorageWitness {
                    scope: StorageScope::Top,
                    key: destination,
                    value_hash: value.map(|v| hash(v)),
                },
            ]);
        }
        out.witnesses.push(StorageWitness {
            scope: StorageScope::Top,
            key: key(virtual_stakers, &old),
            value_hash: None,
        });
        out.witnesses.push(StorageWitness {
            scope: StorageScope::Top,
            key: key(virtual_stakers, &new),
            value_hash: None,
        });
        out.report.pairs.push(PairReport {
            old_stash: hx(&old),
            old_controller: hx(&controller),
            replacement_owner: hx(&new),
            total: ledger.total.to_string(),
            active: ledger.active.to_string(),
            unlocking_chunks: ledger.unlocking.len(),
            claimed_eras: ledger.legacy_claimed_rewards.len(),
            ledger_financial_bytes_hash: hx(&hash(&original[32..])),
            backing,
        });
        out.edits.extend(group.clone());
        out.groups.push(group);
    }
    out.report.changed_keys = out.edits.len();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, PortableRegistry, Registry, TypeInfo};

    #[derive(Encode, TypeInfo)]
    struct AccountData {
        free: u128,
        reserved: u128,
        frozen: u128,
        flags: u128,
    }
    #[derive(Encode, TypeInfo)]
    struct AccountInfo {
        nonce: u32,
        consumers: u32,
        providers: u32,
        sufficients: u32,
        data: AccountData,
    }
    #[derive(Encode, TypeInfo)]
    struct Lock {
        id: [u8; 8],
        amount: u128,
        reasons: u8,
    }
    #[derive(Encode, TypeInfo)]
    enum HoldReason {
        Staking,
    }
    #[derive(Encode, TypeInfo)]
    enum RuntimeHoldReason {
        #[codec(index = 17)]
        Staking(HoldReason),
    }
    #[derive(Encode, TypeInfo)]
    struct Hold {
        id: RuntimeHoldReason,
        amount: u128,
    }

    fn fixture() -> (MetadataSchema, Storage, Vec<AccountReplacement>, Review) {
        let mut registry = Registry::new();
        let account = registry.register_type(&meta_type::<AccountId32>()).id;
        let ledger = registry.register_type(&meta_type::<Ledger>()).id;
        let account_info = registry.register_type(&meta_type::<AccountInfo>()).id;
        let locks = registry.register_type(&meta_type::<Vec<Lock>>()).id;
        let holds = registry.register_type(&meta_type::<Vec<Hold>>()).id;
        let other = registry
            .register_type(&meta_type::<Vec<([u8; 8], u128)>>())
            .id;
        let virtual_staker = registry.register_type(&meta_type::<()>()).id;
        let descriptors = [
            ("Staking.Bonded", account, Hasher::Twox64Concat),
            ("Staking.Ledger", ledger, Hasher::Blake2_128Concat),
            (
                "Staking.VirtualStakers",
                virtual_staker,
                Hasher::Twox64Concat,
            ),
            ("System.Account", account_info, Hasher::Blake2_128Concat),
            ("Balances.Locks", locks, Hasher::Blake2_128Concat),
            ("Balances.Holds", holds, Hasher::Blake2_128Concat),
            ("Balances.Freezes", other, Hasher::Blake2_128Concat),
            ("Balances.Reserves", other, Hasher::Blake2_128Concat),
        ]
        .into_iter()
        .map(|(name, value_type, hasher)| {
            let (pallet, item) = name.split_once('.').unwrap();
            let mut prefix = sp_io::hashing::twox_128(pallet.as_bytes()).to_vec();
            prefix.extend(sp_io::hashing::twox_128(item.as_bytes()));
            Descriptor {
                trie: "top".into(),
                name: name.into(),
                prefix,
                key_types: vec![account],
                hashers: vec![hasher],
                value_type,
            }
        })
        .collect();
        let schema = MetadataSchema {
            registry: PortableRegistry::from(registry),
            descriptors,
            metadata_hash: METADATA.into(),
        };
        let mut storage = Storage::default();
        let ledger = Ledger {
            stash: AccountId32::new([1; 32]),
            total: 150,
            active: 100,
            unlocking: vec![Unlock { value: 50, era: 8 }],
            legacy_claimed_rewards: vec![1, 2],
        };
        storage.top.insert(
            key(descriptor(&schema, "Staking.Bonded").unwrap(), &[1; 32]),
            [7; 32].to_vec(),
        );
        storage.top.insert(
            key(descriptor(&schema, "Staking.Ledger").unwrap(), &[7; 32]),
            ledger.encode(),
        );
        let info = AccountInfo {
            nonce: 10,
            consumers: 2,
            providers: 1,
            sufficients: 0,
            data: AccountData {
                free: 200,
                reserved: 0,
                frozen: 150,
                flags: 0,
            },
        };
        storage.top.insert(
            key(descriptor(&schema, "System.Account").unwrap(), &[1; 32]),
            info.encode(),
        );
        storage.top.insert(
            key(descriptor(&schema, "Balances.Locks").unwrap(), &[1; 32]),
            vec![Lock {
                id: *b"staking ",
                amount: 150,
                reasons: 2,
            }]
            .encode(),
        );
        let replacements = vec![AccountReplacement {
            old: AccountId32::new([1; 32]),
            new: AccountId32::new([9; 32]),
        }];
        let review = review(&schema, &storage);
        (schema, storage, replacements, review)
    }
    fn review(schema: &MetadataSchema, storage: &Storage) -> Review {
        Review {
            evidence_hash: hx(&[5; 32]),
            pairs: vec![Pair {
                stash: hx(&[1; 32]),
                controller: hx(&[7; 32]),
                bonded_hash: hx(&hash(
                    &storage.top[&key(descriptor(schema, "Staking.Bonded").unwrap(), &[1; 32])],
                )),
                ledger_hash: hx(&hash(
                    &storage.top[&key(descriptor(schema, "Staking.Ledger").unwrap(), &[7; 32])],
                )),
            }],
        }
    }
    #[test]
    fn couples_bonded_and_ledger_without_financial_or_reference_changes() {
        let (schema, storage, mapping, review) = fixture();
        let compiled = compile(&schema, &storage, &mapping, &review).unwrap();
        assert_eq!(compiled.groups.len(), 1);
        assert_eq!(compiled.edits.len(), 4);
        let ledger = descriptor(&schema, "Staking.Ledger").unwrap();
        let after = compiled
            .edits
            .iter()
            .find(|edit| edit.key == key(ledger, &[9; 32]))
            .unwrap()
            .after
            .as_ref()
            .unwrap();
        assert_eq!(&after[..32], &[9; 32]);
        assert_eq!(&after[32..], &storage.top[&key(ledger, &[7; 32])][32..]);
        assert_eq!(compiled.resolutions.len(), 2);
        assert_eq!(compiled.report.controller_reference_changes, 0);
        assert!(!compiled.report.historical_exposures_changed);
        let mut groups = compiled.groups.clone();
        assert!(compiled.verify_backing_moves(&groups).is_err());
        groups.push(compiled.required_moves.clone());
        compiled.verify_backing_moves(&groups).unwrap();
        assert!(!compiled.report.production_ready);
    }
    #[test]
    fn legacy_repeated_unlock_eras_preserve_each_chunk_and_financial_bytes() {
        let (schema, mut storage, mapping, _) = fixture();
        let d = descriptor(&schema, "Staking.Ledger").unwrap();
        let source = key(d, &[7; 32]);
        let mut value = ledger(&schema, d, &storage.top[&source]).unwrap();
        value.unlocking = vec![
            Unlock { value: 20, era: 8 },
            Unlock { value: 0, era: 8 },
            Unlock { value: 30, era: 8 },
        ];
        storage.top.insert(source.clone(), value.encode());
        let reviewed = review(&schema, &storage);
        let compiled = compile(&schema, &storage, &mapping, &reviewed).unwrap();
        let after = compiled
            .edits
            .iter()
            .find(|e| e.key == key(d, &[9; 32]))
            .unwrap()
            .after
            .as_ref()
            .unwrap();
        assert_eq!(&after[32..], &storage.top[&source][32..]);
        let decoded = ledger(&schema, d, after).unwrap();
        assert_eq!(decoded.unlocking.len(), 3);
        assert_eq!(decoded.unlocking.iter().map(|c| c.value).sum::<u128>(), 50);
        // Native chronological behavior still excludes backwards-era state.
        value.unlocking[1].era = 7;
        storage.top.insert(source, value.encode());
        assert!(compile(&schema, &storage, &mapping, &review(&schema, &storage)).is_err());
    }
    #[test]
    fn rejects_missing_or_tampered_backing_moves_and_occupied_destinations() {
        let (schema, mut storage, mapping, review) = fixture();
        let compiled = compile(&schema, &storage, &mapping, &review).unwrap();
        let mut groups = vec![compiled.required_moves.clone()];
        let insertion = groups[0]
            .iter_mut()
            .find(|edit| edit.after.is_some())
            .unwrap();
        insertion.after.as_mut().unwrap()[0] ^= 1;
        assert!(compiled.verify_backing_moves(&groups).is_err());
        storage.top.insert(
            key(descriptor(&schema, "Staking.Ledger").unwrap(), &[9; 32]),
            vec![0],
        );
        assert!(compile(&schema, &storage, &mapping, &review).is_err());
    }
    #[test]
    fn shared_missing_controller_or_unmapped_stash_never_gains_control() {
        let (schema, storage, mapping, review) = fixture();
        let mut shared = storage.clone();
        shared.top.insert(
            key(descriptor(&schema, "Staking.Bonded").unwrap(), &[2; 32]),
            [7; 32].to_vec(),
        );
        assert!(compile(&schema, &shared, &mapping, &review).is_err());
        let mut missing = storage.clone();
        missing.top.remove(&key(
            descriptor(&schema, "Staking.Ledger").unwrap(),
            &[7; 32],
        ));
        assert!(compile(&schema, &missing, &mapping, &review).is_err());
        let controller_only = vec![AccountReplacement {
            old: AccountId32::new([7; 32]),
            new: AccountId32::new([9; 32]),
        }];
        assert!(compile(
            &schema,
            &storage,
            &controller_only,
            &Review {
                evidence_hash: hx(&[5; 32]),
                pairs: vec![]
            }
        )
        .is_err());
    }
    #[test]
    fn rejects_virtual_staking_incomplete_review_and_unbacked_ledger() {
        let (schema, storage, mapping, mut review) = fixture();
        review.pairs.clear();
        assert!(compile(&schema, &storage, &mapping, &review).is_err());
        let review = self::review(&schema, &storage);
        let mut virtual_stash = storage.clone();
        virtual_stash.top.insert(
            key(
                descriptor(&schema, "Staking.VirtualStakers").unwrap(),
                &[1; 32],
            ),
            ().encode(),
        );
        assert!(compile(&schema, &virtual_stash, &mapping, &review).is_err());
        let mut unbacked = storage;
        unbacked.top.remove(&key(
            descriptor(&schema, "Balances.Locks").unwrap(),
            &[1; 32],
        ));
        assert!(compile(&schema, &unbacked, &mapping, &review).is_err());
    }
    #[test]
    fn native_holds_are_preserved_without_legacy_consumer_arithmetic() {
        let (schema, mut storage, mapping, review) = fixture();
        storage.top.remove(&key(
            descriptor(&schema, "Balances.Locks").unwrap(),
            &[1; 32],
        ));
        storage.top.insert(
            key(descriptor(&schema, "Balances.Holds").unwrap(), &[1; 32]),
            vec![Hold {
                id: RuntimeHoldReason::Staking(HoldReason::Staking),
                amount: 150,
            }]
            .encode(),
        );
        storage.top.insert(
            key(descriptor(&schema, "System.Account").unwrap(), &[1; 32]),
            AccountInfo {
                nonce: 10,
                consumers: 0,
                providers: 1,
                sufficients: 0,
                data: AccountData {
                    free: 50,
                    reserved: 150,
                    frozen: 0,
                    flags: 0,
                },
            }
            .encode(),
        );
        let compiled = compile(&schema, &storage, &mapping, &review).unwrap();
        compiled
            .verify_backing_moves(std::slice::from_ref(&compiled.required_moves))
            .unwrap();
        assert_eq!(compiled.report.controller_reference_changes, 0);
    }
    #[test]
    fn exact_native_hold_preserves_stale_legacy_locks_even_above_free_balance() {
        for legacy_amount in [0, 50, 150, 300] {
            let (schema, mut storage, mapping, review) = fixture();
            let holds_key = key(descriptor(&schema, "Balances.Holds").unwrap(), &[1; 32]);
            storage.top.insert(
                holds_key.clone(),
                vec![Hold {
                    id: RuntimeHoldReason::Staking(HoldReason::Staking),
                    amount: 150,
                }]
                .encode(),
            );
            let locks_key = key(descriptor(&schema, "Balances.Locks").unwrap(), &[1; 32]);
            storage.top.insert(
                locks_key.clone(),
                vec![Lock {
                    id: *b"staking ",
                    amount: legacy_amount,
                    reasons: 2,
                }]
                .encode(),
            );
            let account_key = key(descriptor(&schema, "System.Account").unwrap(), &[1; 32]);
            storage.top.insert(
                account_key.clone(),
                AccountInfo {
                    nonce: 10,
                    consumers: 2,
                    providers: 1,
                    sufficients: 0,
                    data: AccountData {
                        free: 50,
                        reserved: 150,
                        frozen: legacy_amount,
                        flags: 0,
                    },
                }
                .encode(),
            );
            let compiled = compile(&schema, &storage, &mapping, &review).unwrap();
            let backing = &compiled.report.pairs[0].backing;
            assert_eq!(backing.kind, BackingKind::NativeHold);
            assert_eq!(backing.staking_hold, Some("150".into()));
            assert_eq!(
                backing.retained_legacy_staking_lock,
                Some(legacy_amount.to_string())
            );
            // Composition must move both encumbrances and the full account
            // bytes, never remove the stale lock or adjust reference counts.
            for source in [holds_key, locks_key, account_key] {
                assert!(compiled.required_moves.iter().any(|edit| {
                    edit.after.as_ref() == Some(&storage.top[&source])
                        && edit.key.ends_with(&[9; 32])
                }));
            }
            compiled
                .verify_backing_moves(std::slice::from_ref(&compiled.required_moves))
                .unwrap();
        }
    }
    #[test]
    fn zero_ledger_preserves_residual_lock_without_creating_or_releasing_stake() {
        let (schema, mut storage, mapping, _) = fixture();
        let d = descriptor(&schema, "Staking.Ledger").unwrap();
        let ledger_key = key(d, &[7; 32]);
        let mut value = ledger(&schema, d, &storage.top[&ledger_key]).unwrap();
        value.total = 0;
        value.active = 0;
        value.unlocking.clear();
        storage.top.insert(ledger_key.clone(), value.encode());
        let reviewed = review(&schema, &storage);
        let compiled = compile(&schema, &storage, &mapping, &reviewed).unwrap();
        let backing = &compiled.report.pairs[0].backing;
        assert_eq!(backing.kind, BackingKind::ZeroStake);
        assert_eq!(backing.staking_hold, None);
        assert_eq!(backing.retained_legacy_staking_lock, Some("150".into()));
        let moved_ledger = compiled
            .edits
            .iter()
            .find(|edit| edit.key == key(d, &[9; 32]))
            .unwrap()
            .after
            .as_ref()
            .unwrap();
        assert_eq!(&moved_ledger[32..], &storage.top[&ledger_key][32..]);
        compiled
            .verify_backing_moves(std::slice::from_ref(&compiled.required_moves))
            .unwrap();
        assert_eq!(compiled.report.controller_reference_changes, 0);
    }
    #[test]
    fn positive_hold_mismatch_cannot_fall_back_to_an_exact_legacy_lock() {
        for held in [1, 149, 151] {
            let (schema, mut storage, mapping, review) = fixture();
            storage.top.insert(
                key(descriptor(&schema, "Balances.Holds").unwrap(), &[1; 32]),
                vec![Hold {
                    id: RuntimeHoldReason::Staking(HoldReason::Staking),
                    amount: held,
                }]
                .encode(),
            );
            let error = compile(&schema, &storage, &mapping, &review).err().unwrap();
            assert!(error.contains("staking hold must exactly back the ledger"));
            assert!(error.contains(&format!("staking_hold=Some({held})")));
            assert!(error.contains("ledger_total=150"));
            assert!(error.contains(&format!("stash={}", hx(&[1; 32]))));
            assert!(error.contains(&format!("controller={}", hx(&[7; 32]))));
        }
    }
    #[test]
    fn positive_backing_still_requires_reserved_or_free_funds_and_references() {
        for (hold, free, reserved, providers, consumers) in [
            (true, 200, 149, 1, 2),
            (false, 149, 200, 1, 2),
            (true, 50, 150, 0, 2),
            (true, 50, 150, 1, 0),
        ] {
            let (schema, mut storage, mapping, review) = fixture();
            if hold {
                storage.top.insert(
                    key(descriptor(&schema, "Balances.Holds").unwrap(), &[1; 32]),
                    vec![Hold {
                        id: RuntimeHoldReason::Staking(HoldReason::Staking),
                        amount: 150,
                    }]
                    .encode(),
                );
            }
            storage.top.insert(
                key(descriptor(&schema, "System.Account").unwrap(), &[1; 32]),
                AccountInfo {
                    nonce: 10,
                    consumers,
                    providers,
                    sufficients: 0,
                    data: AccountData {
                        free,
                        reserved,
                        frozen: 150,
                        flags: 0,
                    },
                }
                .encode(),
            );
            assert!(compile(&schema, &storage, &mapping, &review)
                .err()
                .unwrap()
                .contains("staking balance or System references"));
        }
    }
    #[test]
    fn zero_ledger_does_not_waive_a_positive_hold_or_duplicate_encumbrances() {
        let (schema, mut storage, mapping, _) = fixture();
        let d = descriptor(&schema, "Staking.Ledger").unwrap();
        let ledger_key = key(d, &[7; 32]);
        let mut value = ledger(&schema, d, &storage.top[&ledger_key]).unwrap();
        value.total = 0;
        value.active = 0;
        value.unlocking.clear();
        storage.top.insert(ledger_key, value.encode());
        let reviewed = review(&schema, &storage);
        let holds_key = key(descriptor(&schema, "Balances.Holds").unwrap(), &[1; 32]);
        storage.top.insert(
            holds_key.clone(),
            vec![Hold {
                id: RuntimeHoldReason::Staking(HoldReason::Staking),
                amount: 1,
            }]
            .encode(),
        );
        assert!(compile(&schema, &storage, &mapping, &reviewed)
            .err()
            .unwrap()
            .contains("staking hold must exactly back the ledger"));
        storage.top.remove(&holds_key);
        storage.top.insert(
            key(descriptor(&schema, "Balances.Locks").unwrap(), &[1; 32]),
            vec![
                Lock {
                    id: *b"staking ",
                    amount: 150,
                    reasons: 2,
                },
                Lock {
                    id: *b"staking ",
                    amount: 0,
                    reasons: 2,
                },
            ]
            .encode(),
        );
        assert!(compile(&schema, &storage, &mapping, &reviewed)
            .err()
            .unwrap()
            .contains("duplicate staking hold/lock"));
    }
    #[test]
    fn rejects_changed_ledger_shape_financial_sum_and_claim_history_bounds() {
        let (mut schema, mut storage, mapping, _) = fixture();
        let key = key(descriptor(&schema, "Staking.Ledger").unwrap(), &[7; 32]);
        let mut value = Ledger::decode(&mut storage.top[&key].as_slice()).unwrap();
        value.total += 1;
        storage.top.insert(key.clone(), value.encode());
        assert!(compile(&schema, &storage, &mapping, &review(&schema, &storage)).is_err());
        value.total -= 1;
        value.legacy_claimed_rewards = (0..85).collect();
        storage.top.insert(key, value.encode());
        assert!(compile(&schema, &storage, &mapping, &review(&schema, &storage)).is_err());
        schema.metadata_hash = hx(&[0; 32]);
        assert!(compile(&schema, &storage, &mapping, &review(&schema, &storage)).is_err());
    }
}
