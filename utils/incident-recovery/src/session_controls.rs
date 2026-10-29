//! Move checkpoint Session ownership without rotating keys or changing backing.
//!
//! The deployed Session maps are not counted. NextKeys owns the KeyOwner
//! registrations; queued/active order and all four opaque signing-key fields are
//! retained, including BEEFY state. Historical roots are outside the mutation
//! set. Session's currency is Balances, so existing account references and all
//! balance holds/locks/reserves move unchanged through the generic inventory.

use crate::{
    inventory::{
        AdapterId, AdapterResolution, Descriptor, Hasher, MetadataSchema, ResolutionAction,
    },
    typed, Result,
};
use codec::{Decode, Encode};
use common::recovery::{
    self, AccountReplacement, GuardedStorageEdit, StorageScope, StorageWitness,
};
use scale_info::TypeDef;
use serde::Serialize;
use sp_core::{crypto::AccountId32, storage::Storage};
use std::collections::{BTreeMap, BTreeSet};

const METADATA: &str = "0x0c1d4bf5c3bc5037ad854932443c94faae6c327ecf1f38b3e77d38668cc2b4b8";
const MAX_ROWS: usize = 10_000;
pub const ITEMS: &[&str] = &[
    "Session.NextKeys",
    "Session.KeyOwner",
    "Session.QueuedKeys",
    "Session.Validators",
    "Session.ExternallySetKeys",
];
const BACKING: &[&str] = &[
    "System.Account",
    "Balances.Holds",
    "Balances.Locks",
    "Balances.Freezes",
    "Balances.Reserves",
];

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Keys {
    babe: [u8; 32],
    grandpa: [u8; 32],
    im_online: [u8; 32],
    beefy: [u8; 33],
}
impl Keys {
    fn registrations(&self) -> [([u8; 4], Vec<u8>); 4] {
        [
            (*b"babe", self.babe.to_vec()),
            (*b"gran", self.grandpa.to_vec()),
            (*b"imon", self.im_online.to_vec()),
            (*b"beef", self.beefy.to_vec()),
        ]
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub schema: &'static str,
    pub authorization_policy: &'static str,
    pub mapping_commitment: String,
    pub checkpoint_session_records: usize,
    pub checkpoint_historical_records: usize,
    pub next_key_owners: usize,
    pub key_registrations: usize,
    pub queued_validators: usize,
    pub active_validators: usize,
    pub mapped_session_owners: usize,
    pub externally_set_keys_descriptor_present: bool,
    pub changed_keys: usize,
    pub signing_key_bytes_preserved: bool,
    pub historical_roots_preserved: bool,
    pub reference_count_changes: usize,
    pub backing_currency: &'static str,
    pub production_ready: bool,
}

pub struct Compilation {
    pub groups: Vec<Vec<GuardedStorageEdit>>,
    pub edits: Vec<GuardedStorageEdit>,
    pub resolutions: Vec<AdapterResolution>,
    pub witnesses: Vec<StorageWitness>,
    pub report: Report,
    mapping_hash: [u8; 32],
    before: BTreeMap<Vec<u8>, Vec<u8>>,
    expected: BTreeMap<Vec<u8>, Vec<u8>>,
    required_moves: Vec<GuardedStorageEdit>,
    required_absences: BTreeSet<Vec<u8>>,
}

fn hash(bytes: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(bytes)
}
fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn prefix(item: &str) -> Vec<u8> {
    let mut key = sp_io::hashing::twox_128(b"Session").to_vec();
    key.extend(sp_io::hashing::twox_128(item.as_bytes()));
    key
}
fn descriptor<'a>(schema: &'a MetadataSchema, name: &str) -> Result<&'a Descriptor> {
    schema
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == name)
        .ok_or_else(|| format!("missing checkpoint Session/backing descriptor {name}"))
}
fn account_map(schema: &MetadataSchema, d: &Descriptor, hasher: Hasher) -> Result<()> {
    if d.hashers != [hasher]
        || d.key_types.len() != 1
        || !typed::account_type(&schema.registry, d.key_types[0])
    {
        return Err(format!(
            "unreviewed Session/backing account key codec {}",
            d.name
        ));
    }
    Ok(())
}
fn map_key(d: &Descriptor, encoded: &[u8]) -> Vec<u8> {
    let mut key = d.prefix.clone();
    match d.hashers[0] {
        Hasher::Twox64Concat => key.extend(sp_io::hashing::twox_64(encoded)),
        Hasher::Blake2_128Concat => key.extend(sp_io::hashing::blake2_128(encoded)),
        _ => unreachable!("checked reversible map hasher"),
    }
    key.extend(encoded);
    key
}
fn decode<T: Decode + Encode>(schema: &MetadataSchema, id: u32, bytes: &[u8]) -> Result<T> {
    typed::decode_exact(bytes, id, &schema.registry)?;
    let mut input = bytes;
    let value = T::decode(&mut input).map_err(|e| e.to_string())?;
    if !input.is_empty() || value.encode() != bytes {
        return Err("noncanonical checkpoint Session encoding".into());
    }
    Ok(value)
}
fn owner_key(schema: &MetadataSchema, d: &Descriptor, key: &[u8]) -> Result<AccountId32> {
    let bytes = key
        .get(d.prefix.len() + 8..)
        .ok_or("short Session owner key")?;
    let owner = decode::<AccountId32>(schema, d.key_types[0], bytes)?;
    if map_key(d, bytes) != key {
        return Err("Session owner key did not roundtrip its native hasher".into());
    }
    Ok(owner)
}
fn rows<'a>(
    storage: &'a Storage,
    d: &'a Descriptor,
) -> impl Iterator<Item = (&'a Vec<u8>, &'a Vec<u8>)> {
    storage
        .top
        .range(d.prefix.clone()..)
        .take_while(move |(key, _)| key.starts_with(&d.prefix))
}
fn effective(owner: &AccountId32, mapping: &BTreeMap<AccountId32, AccountId32>) -> AccountId32 {
    mapping.get(owner).unwrap_or(owner).clone()
}
fn observed(owners: impl IntoIterator<Item = AccountId32>) -> BTreeSet<[u8; 32]> {
    owners
        .into_iter()
        .map(|a| *AsRef::<[u8; 32]>::as_ref(&a))
        .collect()
}

impl Compilation {
    fn change(
        &mut self,
        d: &Descriptor,
        key: Vec<u8>,
        destination: Vec<u8>,
        after: Vec<u8>,
        owners: BTreeSet<[u8; 32]>,
    ) -> Result<()> {
        let before = self.before.get(&key).ok_or("Session source missing")?;
        if key == destination && *before == after {
            return Ok(());
        }
        let group = if key == destination {
            vec![GuardedStorageEdit {
                scope: StorageScope::Top,
                key: key.clone(),
                before: Some(hash(before)),
                after: Some(after.clone()),
            }]
        } else {
            if self.before.contains_key(&destination) || self.expected.contains_key(&destination) {
                return Err("Session replacement destination is occupied".into());
            }
            vec![
                GuardedStorageEdit {
                    scope: StorageScope::Top,
                    key: key.clone(),
                    before: Some(hash(before)),
                    after: None,
                },
                GuardedStorageEdit {
                    scope: StorageScope::Top,
                    key: destination.clone(),
                    before: None,
                    after: Some(after.clone()),
                },
            ]
        };
        self.resolutions.push(AdapterResolution {
            storage: d.name.clone(),
            key: hx(&key),
            expected_before_hash: hx(&hash(before)),
            expected_after_hash: (key == destination).then(|| hx(&hash(&after))),
            adapter: AdapterId::SessionControlV1,
            action: ResolutionAction::ReplaceControl,
            validated_edits: group.clone(),
            observed_accounts: owners,
        });
        self.expected.remove(&key);
        self.expected.insert(destination, after);
        self.edits.extend(group.clone());
        self.groups.push(group);
        Ok(())
    }

    /// Check the complete Session/Historical namespaces and exact backing moves.
    /// Other adapters cannot alter historical roots, key material or refcounts.
    pub fn verify_groups(
        &self,
        groups: &[Vec<GuardedStorageEdit>],
        replacements: &[AccountReplacement],
    ) -> Result<()> {
        if hash(&replacements.encode()) != self.mapping_hash {
            return Err("Session mapping certificate mismatch".into());
        }
        let session = sp_io::hashing::twox_128(b"Session");
        let historical = sp_io::hashing::twox_128(b"Historical");
        let mut actual = self.before.clone();
        let mut seen = BTreeSet::new();
        for edit in groups.iter().flatten().filter(|e| {
            e.scope == StorageScope::Top
                && (e.key.starts_with(&session) || e.key.starts_with(&historical))
        }) {
            if !seen.insert(edit.key.clone())
                || self.before.get(&edit.key).map(|v| hash(v)) != edit.before
            {
                return Err("overlapping or stale Session/Historical projection".into());
            }
            if let Some(value) = &edit.after {
                actual.insert(edit.key.clone(), value.clone());
            } else {
                actual.remove(&edit.key);
            }
        }
        if actual != self.expected {
            return Err(
                "Session/Historical projection changes keys/history or omits owner moves".into(),
            );
        }
        for required in &self.required_moves {
            let found: Vec<_> = groups
                .iter()
                .flatten()
                .filter(|e| e.scope == StorageScope::Top && e.key == required.key)
                .collect();
            if found.len() != 1 || found[0] != required {
                return Err(
                    "Session backing/reference record lacks its exact unchanged-byte move".into(),
                );
            }
        }
        if groups.iter().flatten().any(|e| {
            e.scope == StorageScope::Top
                && self.required_absences.contains(&e.key)
                && e.after.is_some()
        }) {
            return Err("Session backing invented at an absent checkpoint record".into());
        }
        Ok(())
    }
}

pub fn compile(
    schema: &MetadataSchema,
    storage: &Storage,
    replacements: &[AccountReplacement],
) -> Result<Compilation> {
    recovery::validate_replacements(replacements)
        .map_err(|e| format!("invalid Session mapping: {e:?}"))?;
    if schema.metadata_hash != METADATA {
        return Err("Session controls require the deployed checkpoint metadata".into());
    }
    let mapping: BTreeMap<_, _> = replacements
        .iter()
        .map(|p| (p.old.clone(), p.new.clone()))
        .collect();
    let session = sp_io::hashing::twox_128(b"Session");
    let historical = sp_io::hashing::twox_128(b"Historical");
    // Historical is a separate pallet, not an item within Session. Preserve its
    // entire namespace, including StoredRange, storage version and unknown rows.
    let mut before = BTreeMap::new();
    for namespace in [session, historical] {
        let records: BTreeMap<_, _> = storage
            .top
            .range(namespace.to_vec()..)
            .take_while(|(k, _)| k.starts_with(&namespace))
            .take(MAX_ROWS + 1)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if records.len() > MAX_ROWS || records.values().any(|v| v.len() > typed::MAX_TYPED_BYTES) {
            return Err("Session/Historical namespace exceeds bounded checkpoint inventory".into());
        }
        before.extend(records);
    }
    let next = descriptor(schema, "Session.NextKeys")?;
    account_map(schema, next, Hasher::Twox64Concat)?;
    let Some(TypeDef::Composite(keys_shape)) = schema
        .registry
        .resolve(next.value_type)
        .map(|t| &t.type_def)
    else {
        return Err("Session NextKeys shape changed".into());
    };
    if keys_shape
        .fields
        .iter()
        .map(|f| f.name.as_deref())
        .collect::<Vec<_>>()
        != [
            Some("babe"),
            Some("grandpa"),
            Some("im_online"),
            Some("beefy"),
        ]
    {
        return Err("Session opaque key field order changed".into());
    }
    let key_owner = descriptor(schema, "Session.KeyOwner")?;
    if key_owner.hashers != [Hasher::Twox64Concat]
        || key_owner.key_types.len() != 1
        || !typed::account_type(&schema.registry, key_owner.value_type)
    {
        return Err("Session KeyOwner codec changed".into());
    }
    let mut next_rows = BTreeMap::new();
    let mut registrations = BTreeMap::new();
    for (key, value) in rows(storage, next) {
        let owner = owner_key(schema, next, key)?;
        let keys: Keys = decode(schema, next.value_type, value)?;
        for registration in keys.registrations() {
            if registrations.insert(registration, owner.clone()).is_some() {
                return Err("Session signing key assigned to multiple owners".into());
            }
        }
        next_rows.insert(owner, keys);
    }
    let mut actual_registrations = BTreeMap::new();
    for (key, value) in rows(storage, key_owner) {
        let encoded = key
            .get(key_owner.prefix.len() + 8..)
            .ok_or("short Session KeyOwner key")?;
        let registration: ([u8; 4], Vec<u8>) = decode(schema, key_owner.key_types[0], encoded)?;
        if map_key(key_owner, encoded) != *key {
            return Err("Session KeyOwner key hash differs".into());
        }
        let owner = decode::<AccountId32>(schema, key_owner.value_type, value)?;
        actual_registrations.insert(registration, owner);
    }
    if actual_registrations != registrations {
        return Err("Session NextKeys and KeyOwner registrations disagree".into());
    }
    let queued = descriptor(schema, "Session.QueuedKeys")?;
    let validators = descriptor(schema, "Session.Validators")?;
    if [queued, validators]
        .iter()
        .any(|d| !d.hashers.is_empty() || !d.key_types.is_empty())
    {
        return Err("Session validator-list storage shape changed".into());
    }
    let queued_bytes = storage
        .top
        .get(&queued.prefix)
        .ok_or("checkpoint queued Session keys absent")?;
    let queued_rows: Vec<(AccountId32, Keys)> = decode(schema, queued.value_type, queued_bytes)?;
    let validator_bytes = storage
        .top
        .get(&validators.prefix)
        .ok_or("checkpoint Session validators absent")?;
    let validator_rows: Vec<AccountId32> = decode(schema, validators.value_type, validator_bytes)?;
    let queued_owners: BTreeSet<_> = queued_rows.iter().map(|(a, _)| a.clone()).collect();
    let active_owners: BTreeSet<_> = validator_rows.iter().cloned().collect();
    if queued_rows.is_empty()
        || validator_rows.is_empty()
        || queued_rows.len() > 1000
        || validator_rows.len() > 1000
        || queued_owners.len() != queued_rows.len()
        || active_owners.len() != validator_rows.len()
        || queued_owners
            .union(&active_owners)
            .any(|a| !next_rows.contains_key(a))
    {
        return Err("Session active/queued ownership roster is incomplete or ambiguous".into());
    }
    let external = schema
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == "Session.ExternallySetKeys");
    let mut external_rows = Vec::new();
    if let Some(d) = external {
        account_map(schema, d, Hasher::Twox64Concat)?;
        for (key, value) in rows(storage, d) {
            decode::<()>(schema, d.value_type, value)?;
            let owner = owner_key(schema, d, key)?;
            if !next_rows.contains_key(&owner) {
                return Err("external Session key marker has no NextKeys".into());
            }
            external_rows.push((key.clone(), owner));
        }
    } else if before
        .keys()
        .any(|k| k.starts_with(&prefix("ExternallySetKeys")))
    {
        return Err("Session external-key state has no deployed codec".into());
    }
    let mapped: BTreeSet<_> = next_rows
        .keys()
        .filter(|a| mapping.contains_key(*a))
        .cloned()
        .collect();
    let mapping_hash = hash(&replacements.encode());
    let mut out = Compilation {
        groups: vec![],
        edits: vec![],
        resolutions: vec![],
        witnesses: vec![],
        mapping_hash,
        expected: before.clone(),
        before,
        required_moves: vec![],
        required_absences: BTreeSet::new(),
        report: Report {
            schema: "sora-nexus-session-control-substitution-v1",
            authorization_policy: crate::trusted_mapping::AUTHORIZATION_POLICY,
            mapping_commitment: hx(&mapping_hash),
            checkpoint_session_records: 0,
            checkpoint_historical_records: 0,
            next_key_owners: next_rows.len(),
            key_registrations: registrations.len(),
            queued_validators: queued_rows.len(),
            active_validators: validator_rows.len(),
            mapped_session_owners: mapped.len(),
            externally_set_keys_descriptor_present: external.is_some(),
            changed_keys: 0,
            signing_key_bytes_preserved: true,
            historical_roots_preserved: true,
            reference_count_changes: 0,
            backing_currency: "Balances",
            production_ready: false,
        },
    };
    for old in &mapped {
        let new = &mapping[old];
        out.change(
            next,
            map_key(next, &old.encode()),
            map_key(next, &new.encode()),
            next_rows[old].encode(),
            observed([old.clone()]),
        )?;
    }
    for (registration, old) in &registrations {
        let new = effective(old, &mapping);
        let key = map_key(key_owner, &registration.encode());
        out.change(
            key_owner,
            key.clone(),
            key,
            new.encode(),
            observed([old.clone()]),
        )?;
    }
    let queued_after: Vec<_> = queued_rows
        .iter()
        .map(|(owner, keys)| (effective(owner, &mapping), keys.clone()))
        .collect();
    out.change(
        queued,
        queued.prefix.clone(),
        queued.prefix.clone(),
        queued_after.encode(),
        observed(queued_owners),
    )?;
    let validators_after: Vec<_> = validator_rows
        .iter()
        .map(|a| effective(a, &mapping))
        .collect();
    out.change(
        validators,
        validators.prefix.clone(),
        validators.prefix.clone(),
        validators_after.encode(),
        observed(active_owners),
    )?;
    if let Some(d) = external {
        for (key, old) in external_rows {
            if let Some(new) = mapping.get(&old) {
                out.change(
                    d,
                    key,
                    map_key(d, &new.encode()),
                    ().encode(),
                    observed([old]),
                )?;
            }
        }
    }
    for name in BACKING {
        let d = descriptor(schema, name)?;
        account_map(schema, d, Hasher::Blake2_128Concat)?;
        for old in &mapped {
            let source = map_key(d, &old.encode());
            let destination = map_key(d, &mapping[old].encode());
            if storage.top.contains_key(&destination) {
                return Err("Session backing destination is occupied".into());
            }
            if let Some(value) = storage.top.get(&source) {
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
                if *name == "System.Account" {
                    return Err("Session owner has no checkpoint System account backing".into());
                }
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
                    value_hash: storage.top.get(&map_key(d, &old.encode())).map(|v| hash(v)),
                },
            ]);
        }
    }
    for key in out
        .before
        .keys()
        .chain(out.expected.keys())
        .collect::<BTreeSet<_>>()
    {
        out.witnesses.push(StorageWitness {
            scope: StorageScope::Top,
            key: key.clone(),
            value_hash: out.expected.get(key).map(|v| hash(v)),
        });
    }
    out.report.checkpoint_session_records = out
        .before
        .keys()
        .filter(|k| k.starts_with(&session))
        .count();
    out.report.checkpoint_historical_records = out
        .before
        .keys()
        .filter(|k| k.starts_with(&historical))
        .count();
    out.report.changed_keys = out.edits.len();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, PortableRegistry, Registry, TypeInfo};

    #[derive(Encode, TypeInfo)]
    struct AccountInfo {
        nonce: u32,
        consumers: u32,
        providers: u32,
        sufficients: u32,
        data: (u128, u128, u128, u128),
    }

    fn historical_key(item: &str) -> Vec<u8> {
        let mut key = sp_io::hashing::twox_128(b"Historical").to_vec();
        key.extend(sp_io::hashing::twox_128(item.as_bytes()));
        if item == "HistoricalSessions" {
            let index = 7u32.encode();
            key.extend(sp_io::hashing::twox_64(&index));
            key.extend(index);
        }
        key
    }

    fn fixture(external: bool) -> (MetadataSchema, Storage, Vec<AccountReplacement>) {
        let mut registry = Registry::new();
        let account = registry.register_type(&meta_type::<AccountId32>()).id;
        let keys = registry.register_type(&meta_type::<Keys>()).id;
        let key_owner = registry
            .register_type(&meta_type::<([u8; 4], Vec<u8>)>())
            .id;
        let queued = registry
            .register_type(&meta_type::<Vec<(AccountId32, Keys)>>())
            .id;
        let validators = registry.register_type(&meta_type::<Vec<AccountId32>>()).id;
        let unit = registry.register_type(&meta_type::<()>()).id;
        let account_info = registry.register_type(&meta_type::<AccountInfo>()).id;
        let backing = registry
            .register_type(&meta_type::<Vec<([u8; 8], u128)>>())
            .id;
        let mut specifications = vec![
            (
                "Session.NextKeys",
                Some((account, Hasher::Twox64Concat)),
                keys,
            ),
            (
                "Session.KeyOwner",
                Some((key_owner, Hasher::Twox64Concat)),
                account,
            ),
            ("Session.QueuedKeys", None, queued),
            ("Session.Validators", None, validators),
        ];
        if external {
            specifications.push((
                "Session.ExternallySetKeys",
                Some((account, Hasher::Twox64Concat)),
                unit,
            ));
        }
        for name in BACKING {
            specifications.push((
                name,
                Some((account, Hasher::Blake2_128Concat)),
                if *name == "System.Account" {
                    account_info
                } else {
                    backing
                },
            ));
        }
        let descriptors = specifications
            .into_iter()
            .map(|(name, map, value_type)| {
                let (pallet, item) = name.split_once('.').unwrap();
                let mut prefix = sp_io::hashing::twox_128(pallet.as_bytes()).to_vec();
                prefix.extend(sp_io::hashing::twox_128(item.as_bytes()));
                Descriptor {
                    trie: "top".into(),
                    name: name.into(),
                    prefix,
                    key_types: map.as_ref().map(|(id, _)| vec![*id]).unwrap_or_default(),
                    hashers: map.map(|(_, hasher)| vec![hasher]).unwrap_or_default(),
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
        let old = AccountId32::new([1; 32]);
        let untouched = AccountId32::new([3; 32]);
        let new = AccountId32::new([2; 32]);
        let keys = |byte| Keys {
            babe: [byte; 32],
            grandpa: [byte + 1; 32],
            im_online: [byte + 2; 32],
            beefy: [byte + 3; 33],
        };
        let first_keys = keys(11);
        for (owner, keys) in [
            (old.clone(), first_keys.clone()),
            (untouched.clone(), keys(21)),
        ] {
            let d = descriptor(&schema, "Session.NextKeys").unwrap();
            storage
                .top
                .insert(map_key(d, &owner.encode()), keys.encode());
            let d = descriptor(&schema, "Session.KeyOwner").unwrap();
            for registration in keys.registrations() {
                storage
                    .top
                    .insert(map_key(d, &registration.encode()), owner.encode());
            }
        }
        // Queued bytes may legitimately predate a more recent set_keys call.
        storage.top.insert(
            prefix("QueuedKeys"),
            vec![(untouched.clone(), keys(31)), (old.clone(), first_keys)].encode(),
        );
        storage
            .top
            .insert(prefix("Validators"), vec![old.clone(), untouched].encode());
        storage.top.insert(
            historical_key("HistoricalSessions"),
            ([4u8; 32], 2u32).encode(),
        );
        storage
            .top
            .insert(historical_key("StoredRange"), (7u32, 8u32).encode());
        storage
            .top
            .insert(historical_key("__STORAGE_VERSION__"), 1u16.encode());
        if external {
            storage.top.insert(
                map_key(
                    descriptor(&schema, "Session.ExternallySetKeys").unwrap(),
                    &old.encode(),
                ),
                vec![],
            );
        }
        let system = descriptor(&schema, "System.Account").unwrap();
        storage.top.insert(
            map_key(system, &old.encode()),
            AccountInfo {
                nonce: 27,
                consumers: 2,
                providers: 1,
                sufficients: 3,
                data: (100, 50, 40, 0),
            }
            .encode(),
        );
        for name in ["Balances.Holds", "Balances.Locks"] {
            storage.top.insert(
                map_key(descriptor(&schema, name).unwrap(), &old.encode()),
                vec![(*b"session!", 13u128)].encode(),
            );
        }
        (schema, storage, vec![AccountReplacement { old, new }])
    }

    fn complete_groups(compiled: &Compilation) -> Vec<Vec<GuardedStorageEdit>> {
        let mut groups = compiled.groups.clone();
        groups.push(compiled.required_moves.clone());
        groups
    }

    #[test]
    fn moves_all_owner_state_and_preserves_keys_history_and_backing() {
        let (schema, storage, mapping) = fixture(true);
        let compiled = compile(&schema, &storage, &mapping).unwrap();
        compiled
            .verify_groups(&complete_groups(&compiled), &mapping)
            .unwrap();
        assert_eq!(compiled.report.mapped_session_owners, 1);
        assert_eq!(compiled.report.reference_count_changes, 0);
        assert_eq!(compiled.report.key_registrations, 8);
        assert!(compiled.report.signing_key_bytes_preserved);
        let next = descriptor(&schema, "Session.NextKeys").unwrap();
        let old_key = map_key(next, &mapping[0].old.encode());
        let new_key = map_key(next, &mapping[0].new.encode());
        assert!(!compiled.expected.contains_key(&old_key));
        assert_eq!(compiled.expected[&new_key], storage.top[&old_key]);
        let before: Vec<(AccountId32, Keys)> = decode(
            &schema,
            descriptor(&schema, "Session.QueuedKeys")
                .unwrap()
                .value_type,
            &storage.top[&prefix("QueuedKeys")],
        )
        .unwrap();
        let after: Vec<(AccountId32, Keys)> = decode(
            &schema,
            descriptor(&schema, "Session.QueuedKeys")
                .unwrap()
                .value_type,
            &compiled.expected[&prefix("QueuedKeys")],
        )
        .unwrap();
        assert_eq!(after[0], before[0]);
        assert_eq!(after[1].0, mapping[0].new);
        assert_eq!(after[1].1, before[1].1);
        assert_eq!(
            compiled.expected[&historical_key("HistoricalSessions")],
            storage.top[&historical_key("HistoricalSessions")]
        );
        assert_eq!(compiled.report.checkpoint_historical_records, 3);
        assert!(!compiled
            .resolutions
            .iter()
            .any(|r| r.storage.contains("Historical")));
        assert_eq!(compiled.resolutions.len(), 8);
    }

    #[test]
    fn absence_of_new_session_marker_does_not_invent_it() {
        let (schema, storage, mapping) = fixture(false);
        let compiled = compile(&schema, &storage, &mapping).unwrap();
        assert!(!compiled.report.externally_set_keys_descriptor_present);
        assert!(!compiled
            .edits
            .iter()
            .any(|e| e.key.starts_with(&prefix("ExternallySetKeys"))));
        compiled
            .verify_groups(&complete_groups(&compiled), &mapping)
            .unwrap();
    }

    #[test]
    fn occupied_destinations_and_inconsistent_key_registrations_fail() {
        let (schema, mut storage, mapping) = fixture(false);
        let next = descriptor(&schema, "Session.NextKeys").unwrap();
        let source = map_key(next, &mapping[0].old.encode());
        let destination = map_key(next, &mapping[0].new.encode());
        storage
            .top
            .insert(destination, storage.top[&source].clone());
        assert!(compile(&schema, &storage, &mapping)
            .err()
            .unwrap()
            .contains("multiple owners"));
    }

    #[test]
    fn missing_and_modified_backing_or_history_fail_final_projection() {
        let (schema, storage, mapping) = fixture(true);
        let compiled = compile(&schema, &storage, &mapping).unwrap();
        assert!(compiled.verify_groups(&compiled.groups, &mapping).is_err());
        let mut groups = complete_groups(&compiled);
        groups.last_mut().unwrap()[1].after.as_mut().unwrap()[0] ^= 1;
        assert!(compiled.verify_groups(&groups, &mapping).is_err());
        let mut groups = complete_groups(&compiled);
        groups.push(vec![GuardedStorageEdit {
            scope: StorageScope::Top,
            key: historical_key("HistoricalSessions"),
            before: Some(hash(&storage.top[&historical_key("HistoricalSessions")])),
            after: Some(vec![0]),
        }]);
        assert!(compiled.verify_groups(&groups, &mapping).is_err());
        let mut groups = complete_groups(&compiled);
        groups.push(vec![GuardedStorageEdit {
            scope: StorageScope::Top,
            key: map_key(
                descriptor(&schema, "Balances.Reserves").unwrap(),
                &mapping[0].new.encode(),
            ),
            before: None,
            after: Some(vec![0]),
        }]);
        assert!(compiled.verify_groups(&groups, &mapping).is_err());
    }

    #[test]
    fn historical_namespace_rejects_deletion_insertion_range_and_version_changes() {
        let (schema, storage, mapping) = fixture(true);
        let compiled = compile(&schema, &storage, &mapping).unwrap();
        for item in ["HistoricalSessions", "StoredRange", "__STORAGE_VERSION__"] {
            let key = historical_key(item);
            for after in [None, Some(vec![0])] {
                let mut groups = complete_groups(&compiled);
                groups.push(vec![GuardedStorageEdit {
                    scope: StorageScope::Top,
                    key: key.clone(),
                    before: Some(hash(&storage.top[&key])),
                    after,
                }]);
                assert!(compiled.verify_groups(&groups, &mapping).is_err());
            }
            assert!(compiled
                .witnesses
                .iter()
                .any(|w| w.key == key && w.value_hash == Some(hash(&storage.top[&key]))));
        }
        let mut groups = complete_groups(&compiled);
        groups.push(vec![GuardedStorageEdit {
            scope: StorageScope::Top,
            key: historical_key("UnexpectedItem"),
            before: None,
            after: Some(vec![0]),
        }]);
        assert!(compiled.verify_groups(&groups, &mapping).is_err());
    }

    #[test]
    fn key_owner_value_and_hash_are_cross_checked() {
        let (schema, mut storage, mapping) = fixture(false);
        let d = descriptor(&schema, "Session.KeyOwner").unwrap();
        let key = rows(&storage, d).next().unwrap().0.clone();
        storage
            .top
            .insert(key.clone(), AccountId32::new([9; 32]).encode());
        assert!(compile(&schema, &storage, &mapping).is_err());
        let (schema, mut storage, mapping) = fixture(false);
        let mut malformed = key.clone();
        malformed[32] ^= 1;
        let value = storage.top.remove(&key).unwrap();
        storage.top.insert(malformed, value);
        assert!(compile(&schema, &storage, &mapping).is_err());
    }

    #[test]
    fn codec_changes_and_external_markers_without_metadata_fail() {
        let (mut schema, storage, mapping) = fixture(false);
        schema.metadata_hash = "0x00".into();
        assert!(compile(&schema, &storage, &mapping).is_err());
        let (schema, mut storage, mapping) = fixture(false);
        storage.top.insert(prefix("ExternallySetKeys"), vec![]);
        assert!(compile(&schema, &storage, &mapping).is_err());
    }
}
