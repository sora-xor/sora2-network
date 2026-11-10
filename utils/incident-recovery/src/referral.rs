//! Retained-edge referral suspension. No guessed provider/consumer decrements.
//!
//! Genesis and later referral creation have different provider accounting. We
//! preserve each native AccountInfo byte-for-byte under its replacement key and
//! retain a consistent forward/reverse edge. A source-pinned marker disables fee
//! payout until the effective owner reconfirms exactly that edge.
use crate::{
    hex_bytes,
    inventory::{
        AdapterId, AdapterResolution, Descriptor, Hasher, MetadataSchema, ResolutionAction,
    },
    typed, Result,
};
use codec::{Decode, Encode};
use common::recovery::{AccountReplacement, GuardedStorageEdit, StorageScope, StorageWitness};
use scale_info::{TypeDef, TypeDefPrimitive};
use scale_value::{Composite, Primitive, ValueDef};
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, storage::Storage};
use std::collections::{BTreeMap, BTreeSet};

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
    pub value_hash: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub evidence_hash: String,
    pub suspended_owners: Vec<String>,
    pub reference_accounts_preserved: Vec<String>,
    pub provider_decrements: u32,
    pub consumer_decrements: u32,
    pub production_ready: bool,
}
pub struct Compilation {
    pub edits: Vec<GuardedStorageEdit>,
    pub groups: Vec<Vec<GuardedStorageEdit>>,
    pub resolutions: Vec<AdapterResolution>,
    pub witnesses: Vec<StorageWitness>,
    pub report: Report,
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(bytes)
}
fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn digest(text: &str) -> Result<[u8; 32]> {
    let value: [u8; 32] = hex_bytes(text)?
        .try_into()
        .map_err(|_| "expected 32-byte referral evidence")?;
    if value == [0; 32] {
        return Err("zero referral evidence".into());
    }
    Ok(value)
}
fn desc<'a>(schema: &'a MetadataSchema, name: &str) -> Result<&'a Descriptor> {
    let d = schema
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == name)
        .ok_or_else(|| format!("missing referral schema {name}"))?;
    if d.hashers != [Hasher::Blake2_128Concat]
        || d.key_types.len() != 1
        || !typed::account_type(&schema.registry, d.key_types[0])
    {
        return Err(format!("unreviewed referral key codec {name}"));
    }
    Ok(d)
}
fn account_key(d: &Descriptor, id: &AccountId32) -> Vec<u8> {
    let raw = id.encode();
    let mut key = d.prefix.clone();
    key.extend(sp_io::hashing::blake2_128(&raw));
    key.extend(raw);
    key
}
fn key_account(d: &Descriptor, key: &[u8]) -> Result<AccountId32> {
    let suffix = key
        .strip_prefix(d.prefix.as_slice())
        .ok_or("referral key prefix mismatch")?;
    if suffix.len() != 48 {
        return Err("referral key width mismatch".into());
    }
    let id = AccountId32::new(
        suffix[16..]
            .try_into()
            .map_err(|_| "referral account width")?,
    );
    if account_key(d, &id) != key {
        return Err("noncanonical referral map key".into());
    }
    Ok(id)
}
fn decode<T: Decode + Encode>(schema: &MetadataSchema, d: &Descriptor, bytes: &[u8]) -> Result<T> {
    typed::decode_exact(bytes, d.value_type, &schema.registry)?;
    let mut input = bytes;
    let value = T::decode(&mut input).map_err(|e| e.to_string())?;
    if !input.is_empty() || value.encode() != bytes {
        return Err("noncanonical referral value".into());
    }
    Ok(value)
}
fn references(schema: &MetadataSchema, d: &Descriptor, bytes: &[u8]) -> Result<(u32, u32)> {
    let value = typed::decode_exact(bytes, d.value_type, &schema.registry)?;
    let ValueDef::Composite(Composite::Named(fields)) = value.value else {
        return Err("System AccountInfo is not a named structure".into());
    };
    let read = |name: &str| -> Result<u32> {
        let field = fields
            .iter()
            .find(|(n, _)| n == name)
            .ok_or("missing AccountInfo reference field")?
            .1
            .clone();
        if !matches!(
            schema.registry.resolve(field.context).map(|t| &t.type_def),
            Some(TypeDef::Primitive(TypeDefPrimitive::U32))
        ) {
            return Err("AccountInfo reference field is not u32".into());
        }
        let ValueDef::Primitive(Primitive::U128(value)) = field.value else {
            return Err("AccountInfo reference value mismatch".into());
        };
        value
            .try_into()
            .map_err(|_| "reference count overflow".into())
    };
    Ok((read("providers")?, read("consumers")?))
}
fn marker_key(owner: &AccountId32) -> Vec<u8> {
    let mut key = sp_io::hashing::twox_128(b"Referrals").to_vec();
    key.extend(sp_io::hashing::twox_128(b"SuspendedReferrers"));
    let raw = owner.encode();
    key.extend(sp_io::hashing::blake2_128(&raw));
    key.extend(raw);
    key
}

pub fn compile(
    schema: &MetadataSchema,
    storage: &Storage,
    replacements: &[AccountReplacement],
    review: &Review,
) -> Result<Compilation> {
    common::recovery::validate_replacements(replacements)
        .map_err(|e| format!("invalid referral replacements: {e:?}"))?;
    let evidence = digest(&review.evidence_hash)?;
    let mappings: BTreeMap<_, _> = replacements
        .iter()
        .map(|pair| (pair.old.clone(), pair.new.clone()))
        .collect();
    let resolve = |account: &AccountId32| {
        mappings
            .get(account)
            .cloned()
            .unwrap_or_else(|| account.clone())
    };
    let forward_d = desc(schema, "Referrals.Referrers")?;
    let reverse_d = desc(schema, "Referrals.Referrals")?;
    let accounts_d = desc(schema, "System.Account")?;
    if !typed::account_type(&schema.registry, forward_d.value_type)
        || !matches!(schema.registry.resolve(reverse_d.value_type).map(|t|&t.type_def),Some(TypeDef::Sequence(s)) if typed::account_type(&schema.registry,s.type_param.id))
    {
        return Err("unreviewed referral edge value codec".into());
    }
    let mut forward = BTreeMap::new();
    let mut reverse = BTreeMap::new();
    for (key, value) in storage
        .top
        .range(forward_d.prefix.clone()..)
        .take_while(|(key, _)| key.starts_with(&forward_d.prefix))
    {
        if forward.len() >= 4096 {
            return Err("referral graph exceeds reviewed bound".into());
        }
        forward.insert(
            key_account(forward_d, key)?,
            decode::<AccountId32>(schema, forward_d, value)?,
        );
    }
    for (key, value) in storage
        .top
        .range(reverse_d.prefix.clone()..)
        .take_while(|(key, _)| key.starts_with(&reverse_d.prefix))
    {
        if reverse.len() >= 4096 {
            return Err("referral graph exceeds reviewed bound".into());
        }
        let entries = decode::<Vec<AccountId32>>(schema, reverse_d, value)?;
        if entries.len() > 4096 || entries.iter().collect::<BTreeSet<_>>().len() != entries.len() {
            return Err("duplicate/oversized referral reverse index".into());
        }
        reverse.insert(key_account(reverse_d, key)?, entries);
    }
    let mut counts: BTreeMap<AccountId32, u32> = BTreeMap::new();
    for (owner, referrer) in &forward {
        if reverse
            .get(referrer)
            .is_none_or(|entries| entries.iter().filter(|entry| *entry == owner).count() != 1)
        {
            return Err("referral forward/reverse disagreement".into());
        }
        *counts.entry(owner.clone()).or_default() += 1;
        *counts.entry(referrer.clone()).or_default() += 1;
    }
    for (referrer, owners) in &reverse {
        for owner in owners {
            if forward.get(owner) != Some(referrer) {
                return Err("orphan referral reverse entry".into());
            }
        }
    }
    let affected: BTreeSet<_> = forward
        .iter()
        .filter(|(owner, referrer)| {
            mappings.contains_key(*owner) || mappings.contains_key(*referrer)
        })
        .flat_map(|(owner, referrer)| [owner.clone(), referrer.clone()])
        .collect();
    if affected.is_empty() && !reverse.keys().any(|owner| mappings.contains_key(owner)) {
        return Err("unused referral review: no affected edges or retained reverse rows".into());
    }
    let mut reviewed = BTreeMap::new();
    for record in &review.records {
        if reviewed
            .insert(
                (record.storage.clone(), hex_bytes(&record.key)?),
                digest(&record.value_hash)?,
            )
            .is_some()
        {
            return Err("duplicate referral review row".into());
        }
    }
    let mut used = BTreeSet::new();
    let mut witnesses = Vec::new();
    let mut check = |d: &Descriptor, key: &[u8]| -> Result<Vec<u8>> {
        let bytes = storage
            .top
            .get(key)
            .ok_or("missing referral custody witness")?;
        let id = (d.name.clone(), key.to_vec());
        if reviewed.get(&id) != Some(&hash(bytes)) {
            return Err(format!(
                "missing or changed referral witness {} {}",
                d.name,
                hx(key)
            ));
        }
        used.insert(id);
        witnesses.push(StorageWitness {
            scope: StorageScope::Top,
            key: key.to_vec(),
            value_hash: Some(hash(bytes)),
        });
        Ok(bytes.clone())
    };
    let mut edits = BTreeMap::new();
    let mut groups = Vec::new();
    let mut resolutions = Vec::new();
    let mut suspended = Vec::new();
    let mut emit = |d: &Descriptor,
                    old: AccountId32,
                    before: Vec<u8>,
                    after: Vec<u8>,
                    observed: BTreeSet<[u8; 32]>,
                    extra: Option<GuardedStorageEdit>|
     -> Result<()> {
        let key = account_key(d, &old);
        let destination = account_key(d, &resolve(&old));
        let mut group = Vec::new();
        if key != destination {
            if storage.top.contains_key(&destination) {
                return Err("occupied referral replacement destination".into());
            }
            group.push(GuardedStorageEdit {
                scope: StorageScope::Top,
                key: key.clone(),
                before: Some(hash(&before)),
                after: None,
            });
            group.push(GuardedStorageEdit {
                scope: StorageScope::Top,
                key: destination,
                before: None,
                after: Some(after),
            });
        } else {
            group.push(GuardedStorageEdit {
                scope: StorageScope::Top,
                key: key.clone(),
                before: Some(hash(&before)),
                after: Some(after),
            });
        }
        if let Some(extra) = extra {
            group.push(extra);
        }
        for edit in &group {
            if let Some(previous) = edits.insert(edit.key.clone(), edit.clone()) {
                if previous != *edit {
                    return Err("conflicting referral custody edits".into());
                }
            }
        }
        let source = group
            .iter()
            .find(|edit| edit.key == key)
            .ok_or("missing referral source edit")?;
        resolutions.push(AdapterResolution {
            storage: d.name.clone(),
            key: hx(&key),
            expected_before_hash: hx(&hash(&before)),
            expected_after_hash: source.after.as_deref().map(|value| hx(&hash(value))),
            adapter: AdapterId::ReferralSuspensionV1,
            action: ResolutionAction::ReplaceControl,
            validated_edits: group.clone(),
            observed_accounts: observed,
        });
        groups.push(group);
        Ok(())
    };
    for (owner, referrer) in &forward {
        if !mappings.contains_key(owner) && !mappings.contains_key(referrer) {
            continue;
        }
        if referrer == &AccountId32::new(common::recovery::ATTACKER_ACCOUNT) {
            return Err(
                "attacker referral entitlement requires explicit extinction, not reconfirmation"
                    .into(),
            );
        }
        let before = check(forward_d, &account_key(forward_d, owner))?;
        let effective_owner = resolve(owner);
        let effective_referrer = resolve(referrer);
        let extra = if mappings.contains_key(owner) {
            let key = marker_key(&effective_owner);
            if storage.top.contains_key(&key) {
                return Err("referral suspension marker already exists".into());
            }
            suspended.push(hx(effective_owner.as_ref()));
            // Exact source-pinned SuspendedReferral<AccountId32> SCALE layout.
            Some(GuardedStorageEdit {
                scope: StorageScope::Top,
                key,
                before: None,
                after: Some((&effective_referrer, evidence).encode()),
            })
        } else {
            None
        };
        emit(
            forward_d,
            owner.clone(),
            before,
            effective_referrer.encode(),
            BTreeSet::from([
                *AsRef::<[u8; 32]>::as_ref(owner),
                *AsRef::<[u8; 32]>::as_ref(referrer),
            ]),
            extra,
        )?;
    }
    for (referrer, owners) in &reverse {
        if !mappings.contains_key(referrer)
            && !owners.iter().any(|owner| mappings.contains_key(owner))
        {
            continue;
        }
        let before = check(reverse_d, &account_key(reverse_d, referrer))?;
        let after: Vec<_> = owners.iter().map(&resolve).collect();
        let observed = owners
            .iter()
            .chain(std::iter::once(referrer))
            .map(|id| *AsRef::<[u8; 32]>::as_ref(id))
            .collect();
        emit(
            reverse_d,
            referrer.clone(),
            before,
            after.encode(),
            observed,
            None,
        )?;
    }
    for account in &affected {
        let key = account_key(accounts_d, account);
        let before = check(accounts_d, &key)?;
        let (providers, consumers) = references(schema, accounts_d, &before)?;
        if providers == 0 || consumers < counts[account] {
            return Err("native referral reference custody is inconsistent".into());
        }
        if mappings.contains_key(account) {
            emit(
                accounts_d,
                account.clone(),
                before.clone(),
                before,
                BTreeSet::from([*AsRef::<[u8; 32]>::as_ref(account)]),
                None,
            )?;
        }
    }
    drop(check);
    drop(emit);
    if used.len() != reviewed.len() {
        return Err("unused or extraneous referral evidence row".into());
    }
    // Rows changed by this compiler are already guarded; unchanged reference
    // witnesses must survive the rest of release composition unchanged.
    witnesses.retain(|witness| !edits.contains_key(&witness.key));
    Ok(Compilation {
        edits: edits.into_values().collect(),
        groups,
        resolutions,
        witnesses,
        report: Report {
            evidence_hash: review.evidence_hash.clone(),
            suspended_owners: suspended,
            reference_accounts_preserved: affected.iter().map(|id| hx(id.as_ref())).collect(),
            provider_decrements: 0,
            consumer_decrements: 0,
            production_ready: false,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, Registry, TypeInfo};
    #[derive(Encode, TypeInfo)]
    struct AccountInfo {
        nonce: u32,
        consumers: u32,
        providers: u32,
        sufficients: u32,
        data: u128,
    }
    fn fixture(providers: u32) -> (MetadataSchema, Storage, Vec<AccountReplacement>, Review) {
        let mut registry = Registry::new();
        let account = registry.register_type(&meta_type::<AccountId32>()).id;
        let list = registry.register_type(&meta_type::<Vec<AccountId32>>()).id;
        let info = registry.register_type(&meta_type::<AccountInfo>()).id;
        let descriptors = [
            ("Referrals.Referrers", account),
            ("Referrals.Referrals", list),
            ("System.Account", info),
        ]
        .into_iter()
        .map(|(name, value_type)| {
            let (pallet, item) = name.split_once('.').unwrap();
            let mut prefix = sp_io::hashing::twox_128(pallet.as_bytes()).to_vec();
            prefix.extend(sp_io::hashing::twox_128(item.as_bytes()));
            Descriptor {
                trie: "top".into(),
                name: name.into(),
                prefix,
                key_types: vec![account],
                hashers: vec![Hasher::Blake2_128Concat],
                value_type,
            }
        })
        .collect();
        let schema = MetadataSchema {
            registry: registry.into(),
            descriptors,
            metadata_hash: hx(&[1; 32]),
        };
        let owner = AccountId32::new([7; 32]);
        let referrer = AccountId32::new([9; 32]);
        let mut storage = Storage::default();
        let data = [
            ("Referrals.Referrers", owner.clone(), referrer.encode()),
            (
                "Referrals.Referrals",
                referrer.clone(),
                vec![owner.clone()].encode(),
            ),
            (
                "System.Account",
                owner.clone(),
                AccountInfo {
                    nonce: 5,
                    consumers: 1,
                    providers,
                    sufficients: 0,
                    data: 1_000,
                }
                .encode(),
            ),
            (
                "System.Account",
                referrer.clone(),
                AccountInfo {
                    nonce: 8,
                    consumers: 1,
                    providers,
                    sufficients: 0,
                    data: 2_000,
                }
                .encode(),
            ),
        ];
        let mut records = Vec::new();
        for (name, id, value) in data {
            let key = account_key(desc(&schema, name).unwrap(), &id);
            records.push(Record {
                storage: name.into(),
                key: hx(&key),
                value_hash: hx(&hash(&value)),
            });
            storage.top.insert(key, value);
        }
        (
            schema,
            storage,
            vec![AccountReplacement {
                old: owner,
                new: AccountId32::new([8; 32]),
            }],
            Review {
                evidence_hash: hx(&[3; 32]),
                records,
            },
        )
    }
    #[test]
    fn genesis_and_runtime_reference_origins_preserve_exact_counter_and_financial_bytes() {
        for providers in [1, 2] {
            let (schema, storage, map, review) = fixture(providers);
            let result = compile(&schema, &storage, &map, &review).unwrap();
            let mut projected = storage.top.clone();
            for edit in &result.edits {
                match &edit.after {
                    Some(value) => {
                        projected.insert(edit.key.clone(), value.clone());
                    }
                    None => {
                        projected.remove(&edit.key);
                    }
                }
            }
            let accounts = desc(&schema, "System.Account").unwrap();
            assert_eq!(
                projected[&account_key(accounts, &map[0].new)],
                storage.top[&account_key(accounts, &map[0].old)]
            );
            assert!(!projected.contains_key(&account_key(accounts, &map[0].old)));
            let referrer = AccountId32::new([9; 32]);
            assert_eq!(
                projected[&marker_key(&map[0].new)],
                (&referrer, [3u8; 32]).encode()
            );
            assert_eq!(
                projected[&account_key(desc(&schema, "Referrals.Referrers").unwrap(), &map[0].new)],
                referrer.encode()
            );
            assert_eq!(
                projected[&account_key(desc(&schema, "Referrals.Referrals").unwrap(), &referrer)],
                vec![map[0].new.clone()].encode()
            );
            assert_eq!(result.report.provider_decrements, 0);
            assert_eq!(result.report.consumer_decrements, 0);
            assert_eq!(result.resolutions.len(), 3);
        }
    }
    #[test]
    fn missing_counter_witness_or_changed_native_counter_fails_closed() {
        let (schema, storage, map, mut review) = fixture(2);
        review.records.pop();
        assert!(compile(&schema, &storage, &map, &review).is_err());
        let (schema, mut storage, map, mut review) = fixture(2);
        let key = account_key(desc(&schema, "System.Account").unwrap(), &map[0].old);
        let value = AccountInfo {
            nonce: 5,
            consumers: 0,
            providers: 2,
            sufficients: 0,
            data: 1_000,
        }
        .encode();
        storage.top.insert(key.clone(), value.clone());
        review
            .records
            .iter_mut()
            .find(|row| row.key == hx(&key))
            .unwrap()
            .value_hash = hx(&hash(&value));
        assert!(compile(&schema, &storage, &map, &review)
            .err()
            .unwrap()
            .contains("reference custody"));
    }
    #[test]
    fn incomplete_reverse_graph_and_occupied_destinations_are_rejected() {
        let (schema, mut storage, map, review) = fixture(2);
        let referrer = AccountId32::new([9; 32]);
        let key = account_key(desc(&schema, "Referrals.Referrals").unwrap(), &referrer);
        storage.top.insert(key, Vec::<AccountId32>::new().encode());
        assert!(compile(&schema, &storage, &map, &review).is_err());
        let (schema, mut storage, map, review) = fixture(2);
        let d = desc(&schema, "System.Account").unwrap();
        let old = storage.top[&account_key(d, &map[0].old)].clone();
        storage.top.insert(account_key(d, &map[0].new), old);
        assert!(compile(&schema, &storage, &map, &review).is_err());
    }
    #[test]
    fn attacker_referrer_cannot_be_reauthorized_as_an_ordinary_suspended_grant() {
        let (schema, mut storage, map, _) = fixture(2);
        let referrer = AccountId32::new([9; 32]);
        let attacker = AccountId32::new(common::recovery::ATTACKER_ACCOUNT);
        let forward = desc(&schema, "Referrals.Referrers").unwrap();
        storage
            .top
            .insert(account_key(forward, &map[0].old), attacker.encode());
        for name in ["Referrals.Referrals", "System.Account"] {
            let d = desc(&schema, name).unwrap();
            let value = storage.top.remove(&account_key(d, &referrer)).unwrap();
            storage.top.insert(account_key(d, &attacker), value);
        }
        let records = storage
            .top
            .iter()
            .map(|(key, value)| {
                let d = schema
                    .descriptors
                    .iter()
                    .find(|d| key.starts_with(&d.prefix))
                    .unwrap();
                Record {
                    storage: d.name.clone(),
                    key: hx(key),
                    value_hash: hx(&hash(value)),
                }
            })
            .collect();
        let review = Review {
            evidence_hash: hx(&[3; 32]),
            records,
        };
        assert!(compile(&schema, &storage, &map, &review)
            .err()
            .unwrap()
            .contains("attacker referral"));
    }
}
