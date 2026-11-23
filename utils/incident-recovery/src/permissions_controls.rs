//! Preserve permission scopes/grants while replacing their account identities.
//!
//! Owners are an insertion-ordered list, not a sorted set. Holder permission IDs
//! are sorted for binary search. Neither list is reordered or deduplicated.
//! Native assignment can increment consumers even on repeated assignments, so
//! preserve the checkpoint System.Account bytes instead of recomputing counts.

use crate::{
    hex_bytes,
    inventory::{
        AdapterId, AdapterResolution, Descriptor, Hasher, InventoryOptions, MetadataSchema,
        ResolutionAction,
    },
    typed, Result,
};
use codec::{Decode, Encode};
use common::recovery::{
    self, AccountReplacement, GuardedStorageEdit, StorageScope, StorageWitness,
};
use serde::Serialize;
use sp_core::{crypto::AccountId32, storage::Storage};
use std::collections::{BTreeMap, BTreeSet};

const METADATA: &str = "0x0c1d4bf5c3bc5037ad854932443c94faae6c327ecf1f38b3e77d38668cc2b4b8";
const MAX_ROWS: usize = 10_000;
pub const ITEMS: &[&str] = &["Permissions.Owners", "Permissions.Permissions"];

#[derive(Decode, Encode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
enum Scope {
    Limited([u8; 64]),
    Unlimited,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub schema: &'static str,
    pub mapping_commitment: String,
    pub checkpoint_owner_records: usize,
    pub checkpoint_holder_records: usize,
    pub changed_owner_records: usize,
    pub changed_holder_records: usize,
    pub backing_accounts: usize,
    pub changed_keys: usize,
    pub scopes_and_permission_ids_preserved: bool,
    pub owner_order_and_multiplicity_preserved: bool,
    pub existing_third_party_grants_preserved: bool,
    pub reference_count_changes: usize,
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
    backing_moves: Vec<GuardedStorageEdit>,
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(bytes)
}
fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn pallet_prefix() -> [u8; 16] {
    sp_io::hashing::twox_128(b"Permissions")
}
fn descriptor<'a>(schema: &'a MetadataSchema, name: &str) -> Result<&'a Descriptor> {
    let d = schema
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == name)
        .ok_or_else(|| format!("missing permissions/backing descriptor {name}"))?;
    let (pallet, item) = name
        .split_once('.')
        .ok_or("invalid permission descriptor name")?;
    let expected = [
        sp_io::hashing::twox_128(pallet.as_bytes()),
        sp_io::hashing::twox_128(item.as_bytes()),
    ]
    .concat();
    if d.prefix != expected {
        return Err("permission descriptor prefix mismatch".into());
    }
    Ok(d)
}
fn decode<T: Decode + Encode>(schema: &MetadataSchema, id: u32, bytes: &[u8]) -> Result<T> {
    typed::decode_exact(bytes, id, &schema.registry)?;
    let mut input = bytes;
    let decoded = T::decode(&mut input).map_err(|e| e.to_string())?;
    if !input.is_empty() || decoded.encode() != bytes {
        return Err("noncanonical permission encoding".into());
    }
    Ok(decoded)
}
fn map_key(d: &Descriptor, components: &[Vec<u8>]) -> Vec<u8> {
    let mut key = d.prefix.clone();
    for (hasher, bytes) in d.hashers.iter().zip(components) {
        key.extend(hasher.apply(bytes));
    }
    key
}
fn account_bytes(account: &AccountId32) -> [u8; 32] {
    *AsRef::<[u8; 32]>::as_ref(account)
}
impl Compilation {
    fn change(
        &mut self,
        d: &Descriptor,
        source: Vec<u8>,
        destination: Vec<u8>,
        after: Vec<u8>,
        owners: BTreeSet<[u8; 32]>,
    ) -> Result<()> {
        let before = self.before.get(&source).ok_or("permission source absent")?;
        if source == destination && *before == after {
            return Ok(());
        }
        let edits = if source == destination {
            vec![GuardedStorageEdit {
                scope: StorageScope::Top,
                key: source.clone(),
                before: Some(hash(before)),
                after: Some(after.clone()),
            }]
        } else {
            if self.before.contains_key(&destination) || self.expected.contains_key(&destination) {
                return Err("permission holder destination is occupied".into());
            }
            vec![
                GuardedStorageEdit {
                    scope: StorageScope::Top,
                    key: source.clone(),
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
            key: hx(&source),
            expected_before_hash: hx(&hash(before)),
            expected_after_hash: (source == destination).then(|| hx(&hash(&after))),
            adapter: AdapterId::PermissionControlV1,
            action: ResolutionAction::ReplaceControl,
            validated_edits: edits.clone(),
            observed_accounts: owners,
        });
        self.expected.remove(&source);
        self.expected.insert(destination, after);
        self.edits.extend(edits.clone());
        self.groups.push(edits);
        Ok(())
    }
    /// No other work may add, remove or widen a grant, change its scope, or
    /// reconstruct System references instead of moving their actual bytes.
    pub fn verify_groups(
        &self,
        groups: &[Vec<GuardedStorageEdit>],
        mapping: &[AccountReplacement],
    ) -> Result<()> {
        if hash(&mapping.encode()) != self.mapping_hash {
            return Err("permission mapping certificate differs".into());
        }
        let mut actual = self.before.clone();
        let mut seen = BTreeSet::new();
        for e in groups
            .iter()
            .flatten()
            .filter(|e| e.scope == StorageScope::Top && e.key.starts_with(&pallet_prefix()))
        {
            if !seen.insert(e.key.clone()) || self.before.get(&e.key).map(|v| hash(v)) != e.before {
                return Err("duplicate or stale permission projection".into());
            }
            if let Some(value) = &e.after {
                actual.insert(e.key.clone(), value.clone());
            } else {
                actual.remove(&e.key);
            }
        }
        if actual != self.expected {
            return Err("permission projection changed scopes/grants or omitted ownership".into());
        }
        for required in &self.backing_moves {
            let actual: Vec<_> = groups
                .iter()
                .flatten()
                .filter(|e| e.scope == StorageScope::Top && e.key == required.key)
                .collect();
            if actual.len() != 1 || actual[0] != required {
                return Err(
                    "permission System references lack their exact unchanged-byte move".into(),
                );
            }
        }
        Ok(())
    }
}

pub fn compile(
    schema: &MetadataSchema,
    storage: &Storage,
    mapping: &[AccountReplacement],
    options: &InventoryOptions,
) -> Result<Compilation> {
    recovery::validate_replacements(mapping)
        .map_err(|e| format!("invalid permission mapping: {e:?}"))?;
    if schema.metadata_hash != METADATA {
        return Err("permissions require the deployed checkpoint metadata".into());
    }
    let replacements: BTreeMap<_, _> = mapping
        .iter()
        .map(|p| (p.old.clone(), p.new.clone()))
        .collect();
    let destinations: BTreeSet<_> = mapping.iter().map(|p| p.new.clone()).collect();
    let before: BTreeMap<_, _> = storage
        .top
        .range(pallet_prefix().to_vec()..)
        .take_while(|(k, _)| k.starts_with(&pallet_prefix()))
        .take(MAX_ROWS + 1)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if before.len() > MAX_ROWS || before.values().any(|v| v.len() > typed::MAX_TYPED_BYTES) {
        return Err("permission namespace exceeds bounded checkpoint inventory".into());
    }
    let mut preimages = BTreeMap::new();
    for supplied in &options.key_preimages {
        if supplied.trie != "top" {
            continue;
        }
        let key = hex_bytes(&supplied.key)?;
        if !key.starts_with(&pallet_prefix()) {
            continue;
        }
        if supplied.components.len() != 2
            || supplied.components.iter().any(|c| c.len() > 132)
            || preimages
                .insert(
                    key,
                    supplied
                        .components
                        .iter()
                        .map(|c| hex_bytes(c))
                        .collect::<Result<Vec<_>>>()?,
                )
                .is_some()
        {
            return Err("duplicate or oversized permission key preimage".into());
        }
    }
    let mapping_hash = hash(&mapping.encode());
    let mut out = Compilation {
        groups: vec![],
        edits: vec![],
        resolutions: vec![],
        witnesses: vec![],
        mapping_hash,
        expected: before.clone(),
        before,
        backing_moves: vec![],
        report: Report {
            schema: "sora-nexus-permission-control-substitution-v1",
            mapping_commitment: hx(&mapping_hash),
            checkpoint_owner_records: 0,
            checkpoint_holder_records: 0,
            changed_owner_records: 0,
            changed_holder_records: 0,
            backing_accounts: 0,
            changed_keys: 0,
            scopes_and_permission_ids_preserved: true,
            owner_order_and_multiplicity_preserved: true,
            existing_third_party_grants_preserved: true,
            reference_count_changes: 0,
            production_ready: false,
        },
    };
    let mut affected = BTreeSet::new();
    for name in ITEMS {
        let d = descriptor(schema, name)?;
        if d.hashers != [Hasher::Blake2_256, Hasher::Blake2_256] || d.key_types.len() != 2 {
            return Err("unreviewed permission double-map codec".into());
        }
        for (key, value) in storage
            .top
            .range(d.prefix.clone()..)
            .take_while(|(k, _)| k.starts_with(&d.prefix))
        {
            let components = preimages
                .get(key)
                .ok_or("permission hashed key lacks exact preimages")?;
            if key.len() != 96 || map_key(d, components) != *key {
                return Err("permission hashed key preimage differs".into());
            }
            let _: Scope = decode(schema, d.key_types[1], &components[1])?;
            if *name == "Permissions.Owners" {
                let _: u32 = decode(schema, d.key_types[0], &components[0])?;
                let owners: Vec<AccountId32> = decode(schema, d.value_type, value)?;
                if owners.len() > 1000 || owners.iter().any(|a| destinations.contains(a)) {
                    return Err(
                        "permission owner destination occupied or owner bound exceeded".into(),
                    );
                }
                out.report.checkpoint_owner_records += 1;
                let after: Vec<_> = owners
                    .iter()
                    .map(|a| replacements.get(a).unwrap_or(a).clone())
                    .collect();
                if after != owners {
                    affected.extend(
                        owners
                            .iter()
                            .filter(|a| replacements.contains_key(*a))
                            .cloned(),
                    );
                    out.change(
                        d,
                        key.clone(),
                        key.clone(),
                        after.encode(),
                        owners.iter().map(account_bytes).collect(),
                    )?;
                    out.report.changed_owner_records += 1;
                }
            } else {
                if !typed::account_type(&schema.registry, d.key_types[0]) {
                    return Err("permission holder key is not AccountId32".into());
                }
                let holder: AccountId32 = decode(schema, d.key_types[0], &components[0])?;
                let permissions: Vec<u32> = decode(schema, d.value_type, value)?;
                if permissions.windows(2).any(|pair| pair[0] > pair[1]) {
                    return Err("permission IDs are not native binary-search ordered".into());
                }
                if destinations.contains(&holder) {
                    return Err("permission holder destination already has grants".into());
                }
                out.report.checkpoint_holder_records += 1;
                if let Some(new) = replacements.get(&holder) {
                    let destination = map_key(d, &[new.encode(), components[1].clone()]);
                    out.change(
                        d,
                        key.clone(),
                        destination,
                        value.clone(),
                        BTreeSet::from([account_bytes(&holder)]),
                    )?;
                    affected.insert(holder);
                    out.report.changed_holder_records += 1;
                }
            }
        }
    }
    let system = descriptor(schema, "System.Account")?;
    if system.hashers != [Hasher::Blake2_128Concat]
        || system.key_types.len() != 1
        || !typed::account_type(&schema.registry, system.key_types[0])
    {
        return Err("permission System backing codec differs".into());
    }
    for old in &affected {
        let source = map_key(system, &[old.encode()]);
        let destination = map_key(system, &[replacements[old].encode()]);
        let value = storage
            .top
            .get(&source)
            .ok_or("permission owner/holder lacks System backing")?;
        typed::decode_exact(value, system.value_type, &schema.registry)?;
        if storage.top.contains_key(&destination) {
            return Err("permission System backing destination occupied".into());
        }
        out.backing_moves.extend([
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
        out.witnesses.extend([
            StorageWitness {
                scope: StorageScope::Top,
                key: source,
                value_hash: None,
            },
            StorageWitness {
                scope: StorageScope::Top,
                key: destination,
                value_hash: Some(hash(value)),
            },
        ]);
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
    out.report.backing_accounts = affected.len();
    out.report.changed_keys = out.edits.len();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::KeyPreimage;
    use scale_info::{meta_type, PortableRegistry, Registry, TypeInfo};

    #[derive(Encode, TypeInfo)]
    struct AccountInfo {
        nonce: u32,
        consumers: u32,
        providers: u32,
        sufficients: u32,
        data: (u128, u128, u128, u128),
    }
    fn fixture() -> (
        MetadataSchema,
        Storage,
        Vec<AccountReplacement>,
        InventoryOptions,
    ) {
        let mut registry = Registry::new();
        let account = registry.register_type(&meta_type::<AccountId32>()).id;
        let id = registry.register_type(&meta_type::<u32>()).id;
        let scope = registry.register_type(&meta_type::<Scope>()).id;
        let owners = registry.register_type(&meta_type::<Vec<AccountId32>>()).id;
        let permissions = registry.register_type(&meta_type::<Vec<u32>>()).id;
        let info = registry.register_type(&meta_type::<AccountInfo>()).id;
        let descriptors = [
            (
                "Permissions.Owners",
                vec![id, scope],
                vec![Hasher::Blake2_256; 2],
                owners,
            ),
            (
                "Permissions.Permissions",
                vec![account, scope],
                vec![Hasher::Blake2_256; 2],
                permissions,
            ),
            (
                "System.Account",
                vec![account],
                vec![Hasher::Blake2_128Concat],
                info,
            ),
        ]
        .into_iter()
        .map(|(name, key_types, hashers, value_type)| {
            let (pallet, item) = name.split_once('.').unwrap();
            Descriptor {
                trie: "top".into(),
                name: name.into(),
                prefix: [
                    sp_io::hashing::twox_128(pallet.as_bytes()),
                    sp_io::hashing::twox_128(item.as_bytes()),
                ]
                .concat(),
                key_types,
                hashers,
                value_type,
            }
        })
        .collect();
        let schema = MetadataSchema {
            registry: PortableRegistry::from(registry),
            descriptors,
            metadata_hash: METADATA.into(),
        };
        let old = AccountId32::new([1; 32]);
        let foreign = AccountId32::new([3; 32]);
        let map = vec![AccountReplacement {
            old: old.clone(),
            new: AccountId32::new([2; 32]),
        }];
        let mut storage = Storage::default();
        let mut options = InventoryOptions::default();
        for (name, components, value) in [
            (
                "Permissions.Owners",
                vec![2u32.encode(), Scope::Limited([7; 64]).encode()],
                vec![foreign.clone(), old.clone(), old.clone()].encode(),
            ),
            (
                "Permissions.Permissions",
                vec![old.encode(), Scope::Limited([7; 64]).encode()],
                vec![2u32, 3, 3].encode(),
            ),
            (
                "Permissions.Permissions",
                vec![foreign.encode(), Scope::Unlimited.encode()],
                vec![5u32, 6].encode(),
            ),
        ] {
            let key = map_key(descriptor(&schema, name).unwrap(), &components);
            options.key_preimages.push(KeyPreimage {
                trie: "top".into(),
                key: hx(&key),
                components: components.iter().map(|c| hx(c)).collect(),
            });
            storage.top.insert(key, value);
        }
        storage.top.insert(
            map_key(
                descriptor(&schema, "System.Account").unwrap(),
                &[old.encode()],
            ),
            AccountInfo {
                nonce: 51,
                consumers: 43,
                providers: 1,
                sufficients: 4,
                data: (100, 12, 6, 0),
            }
            .encode(),
        );
        (schema, storage, map, options)
    }
    fn complete(c: &Compilation) -> Vec<Vec<GuardedStorageEdit>> {
        let mut groups = c.groups.clone();
        groups.push(c.backing_moves.clone());
        groups
    }
    #[test]
    fn preserves_scope_order_multiplicity_grants_and_actual_reference_counts() {
        let (s, storage, map, options) = fixture();
        let c = compile(&s, &storage, &map, &options).unwrap();
        c.verify_groups(&complete(&c), &map).unwrap();
        assert_eq!(c.report.changed_owner_records, 1);
        assert_eq!(c.report.changed_holder_records, 1);
        assert_eq!(c.report.backing_accounts, 1);
        assert_eq!(c.report.reference_count_changes, 0);
        let owner_key = hex_bytes(&options.key_preimages[0].key).unwrap();
        let d = descriptor(&s, "Permissions.Owners").unwrap();
        let owners: Vec<AccountId32> = decode(&s, d.value_type, &c.expected[&owner_key]).unwrap();
        assert_eq!(
            owners,
            vec![
                AccountId32::new([3; 32]),
                map[0].new.clone(),
                map[0].new.clone()
            ]
        );
        let old_holder = hex_bytes(&options.key_preimages[1].key).unwrap();
        let new_holder = map_key(
            descriptor(&s, "Permissions.Permissions").unwrap(),
            &[map[0].new.encode(), Scope::Limited([7; 64]).encode()],
        );
        assert!(!c.expected.contains_key(&old_holder));
        assert_eq!(c.expected[&new_holder], storage.top[&old_holder]);
        let foreign_key = hex_bytes(&options.key_preimages[2].key).unwrap();
        assert_eq!(c.expected[&foreign_key], storage.top[&foreign_key]);
        assert_eq!(
            c.backing_moves[1].after.as_ref().unwrap(),
            &storage.top[&c.backing_moves[0].key]
        );
    }
    #[test]
    fn resolved_hashed_rows_consume_their_preimages_in_the_inventory() {
        let (s, storage, map, options) = fixture();
        let c = compile(&s, &storage, &map, &options).unwrap();
        let mappings = map
            .iter()
            .map(|p| (account_bytes(&p.old), account_bytes(&p.new)))
            .collect();
        crate::inventory::inventory_with_resolutions(
            &s,
            &storage,
            &mappings,
            &options,
            &c.resolutions,
            &c.edits,
        )
        .unwrap();
    }
    #[test]
    fn destination_with_its_own_foreign_scope_grant_is_rejected() {
        let (s, mut storage, map, mut options) = fixture();
        let components = vec![map[0].new.encode(), Scope::Unlimited.encode()];
        let key = map_key(
            descriptor(&s, "Permissions.Permissions").unwrap(),
            &components,
        );
        options.key_preimages.push(KeyPreimage {
            trie: "top".into(),
            key: hx(&key),
            components: components.iter().map(|c| hx(c)).collect(),
        });
        storage.top.insert(key, vec![99u32].encode());
        assert!(compile(&s, &storage, &map, &options).is_err());
    }
    #[test]
    fn unexpected_scope_hash_missing_or_duplicate_preimage_fail() {
        for case in 0..3 {
            let (s, storage, map, mut options) = fixture();
            match case {
                0 => options.key_preimages[0].components[1] = hx(&Scope::Unlimited.encode()),
                1 => {
                    options.key_preimages.remove(0);
                }
                _ => {
                    let p = &options.key_preimages[0];
                    options.key_preimages.push(KeyPreimage {
                        trie: p.trie.clone(),
                        key: p.key.clone(),
                        components: p.components.clone(),
                    });
                }
            }
            assert!(
                compile(&s, &storage, &map, &options).is_err(),
                "case {case}"
            );
        }
    }
    #[test]
    fn final_projection_rejects_foreign_grant_or_scope_edits() {
        let (s, storage, map, options) = fixture();
        let c = compile(&s, &storage, &map, &options).unwrap();
        let foreign_key = hex_bytes(&options.key_preimages[2].key).unwrap();
        let mut groups = complete(&c);
        groups.push(vec![GuardedStorageEdit {
            scope: StorageScope::Top,
            key: foreign_key.clone(),
            before: Some(hash(&storage.top[&foreign_key])),
            after: Some(vec![2u32, 5, 6].encode()),
        }]);
        assert!(c.verify_groups(&groups, &map).is_err());
        let mut groups = complete(&c);
        let holder_insert = groups
            .iter_mut()
            .flatten()
            .find(|e| e.before.is_none())
            .unwrap();
        let last = holder_insert.key.len() - 1;
        holder_insert.key[last] ^= 1;
        assert!(c.verify_groups(&groups, &map).is_err());
    }
    #[test]
    fn missing_or_recomputed_system_references_fail() {
        let (s, storage, map, options) = fixture();
        let c = compile(&s, &storage, &map, &options).unwrap();
        assert!(c.verify_groups(&c.groups, &map).is_err());
        let mut groups = complete(&c);
        groups.last_mut().unwrap()[1].after.as_mut().unwrap()[4..8]
            .copy_from_slice(&2u32.to_le_bytes());
        assert!(c.verify_groups(&groups, &map).is_err());
        let mut storage = storage;
        storage.top.remove(&c.backing_moves[0].key);
        assert!(compile(&s, &storage, &map, &options).is_err());
    }
    #[test]
    fn unsorted_holder_ids_fail_instead_of_silently_normalizing_privileges() {
        let (s, mut storage, map, options) = fixture();
        storage.top.insert(
            hex_bytes(&options.key_preimages[1].key).unwrap(),
            vec![3u32, 2].encode(),
        );
        assert!(compile(&s, &storage, &map, &options).is_err());
    }
}
