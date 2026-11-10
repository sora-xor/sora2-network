//! Move preimage deposit ownership to the replacement account without refunding.
//!
//! Only the owner inside a `Preimage.StatusFor` deposit changes; the hash key,
//! call bytes, lengths and request counts stay. The deposit stays reserved,
//! because System.Account moves byte-for-byte to the replacement. Leaving the
//! retired owner lets anyone call `ensure_updated`, which unreserves from the
//! retired account and drops the status, stranding the replacement's reserve.
//! Hold-backed `RequestStatusFor` tickets of replaced accounts fail closed.

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
use serde::Serialize;
use sp_core::{crypto::AccountId32, storage::Storage};
use std::collections::{BTreeMap, BTreeSet};

const METADATA: &str = "0x0c1d4bf5c3bc5037ad854932443c94faae6c327ecf1f38b3e77d38668cc2b4b8";
const MAX_ROWS: usize = 10_000;
pub const ITEMS: &[&str] = &["Preimage.StatusFor"];

/// pallet_preimage::OldRequestStatus<AccountId32, u128>.
#[derive(Clone, Decode, Encode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
enum OldRequestStatus {
    Unrequested {
        deposit: (AccountId32, u128),
        len: u32,
    },
    Requested {
        deposit: Option<(AccountId32, u128)>,
        count: u32,
        len: Option<u32>,
    },
}

impl OldRequestStatus {
    fn deposit_mut(&mut self) -> Option<&mut (AccountId32, u128)> {
        match self {
            Self::Unrequested { deposit, .. } => Some(deposit),
            Self::Requested { deposit, .. } => deposit.as_mut(),
        }
    }
}

/// frame_system::AccountInfo<u32, pallet_balances::AccountData<u128>>.
#[derive(Decode, Encode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct AccountInfo {
    nonce: u32,
    consumers: u32,
    providers: u32,
    sufficients: u32,
    data: (u128, u128, u128, u128),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub schema: &'static str,
    pub mapping_commitment: String,
    pub checkpoint_status_records: usize,
    pub changed_status_records: usize,
    pub moved_deposit_owners: Vec<String>,
    pub moved_deposit_total: String,
    pub call_bytes_hashes_lengths_and_counts_preserved: bool,
    pub refunds_created: u32,
    pub production_ready: bool,
}
pub struct Compilation {
    pub groups: Vec<Vec<GuardedStorageEdit>>,
    pub edits: Vec<GuardedStorageEdit>,
    pub resolutions: Vec<AdapterResolution>,
    pub witnesses: Vec<StorageWitness>,
    pub report: Report,
    mapping_hash: [u8; 32],
    backing_moves: Vec<GuardedStorageEdit>,
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(bytes)
}
fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn pallet_prefix() -> [u8; 16] {
    sp_io::hashing::twox_128(b"Preimage")
}
fn descriptor<'a>(schema: &'a MetadataSchema, name: &str) -> Result<&'a Descriptor> {
    let d = schema
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == name)
        .ok_or_else(|| format!("missing preimage/backing descriptor {name}"))?;
    let (pallet, item) = name
        .split_once('.')
        .ok_or("invalid preimage descriptor name")?;
    let expected = [
        sp_io::hashing::twox_128(pallet.as_bytes()),
        sp_io::hashing::twox_128(item.as_bytes()),
    ]
    .concat();
    if d.prefix != expected {
        return Err("preimage descriptor prefix mismatch".into());
    }
    Ok(d)
}
fn decode<T: Decode + Encode>(schema: &MetadataSchema, id: u32, bytes: &[u8]) -> Result<T> {
    typed::decode_exact(bytes, id, &schema.registry)?;
    let mut input = bytes;
    let decoded = T::decode(&mut input).map_err(|e| e.to_string())?;
    if !input.is_empty() || decoded.encode() != bytes {
        return Err("noncanonical preimage encoding".into());
    }
    Ok(decoded)
}
fn rows<'a>(storage: &'a Storage, d: &'a Descriptor) -> Result<Vec<(&'a Vec<u8>, &'a Vec<u8>)>> {
    let rows: Vec<_> = storage
        .top
        .range(d.prefix.clone()..)
        .take_while(|(k, _)| k.starts_with(&d.prefix))
        .take(MAX_ROWS + 1)
        .collect();
    if rows.len() > MAX_ROWS
        || rows
            .iter()
            .any(|(k, v)| k.len() != d.prefix.len() + 32 || v.len() > typed::MAX_TYPED_BYTES)
    {
        return Err("preimage status namespace exceeds bounded identity layout".into());
    }
    Ok(rows)
}
fn identity_map(d: &Descriptor) -> Result<()> {
    if d.hashers != [Hasher::Identity] || d.key_types.len() != 1 {
        return Err("unreviewed preimage status key codec".into());
    }
    Ok(())
}
fn account_key(d: &Descriptor, account: &AccountId32) -> Vec<u8> {
    [
        d.prefix.clone(),
        Hasher::Blake2_128Concat.apply(&account.encode()),
    ]
    .concat()
}

impl Compilation {
    /// No other work may touch preimage storage, and each replaced owner's
    /// System.Account must move with unchanged bytes so its reserve follows.
    pub fn verify_groups(
        &self,
        groups: &[Vec<GuardedStorageEdit>],
        mapping: &[AccountReplacement],
    ) -> Result<()> {
        if hash(&mapping.encode()) != self.mapping_hash {
            return Err("preimage mapping certificate differs".into());
        }
        let actual: Vec<_> = groups
            .iter()
            .flatten()
            .filter(|e| e.scope == StorageScope::Top && e.key.starts_with(&pallet_prefix()))
            .collect();
        if actual.len() != self.edits.len() || self.edits.iter().any(|e| !actual.contains(&e)) {
            return Err("preimage storage edits differ from the exact owner moves".into());
        }
        for required in &self.backing_moves {
            let actual: Vec<_> = groups
                .iter()
                .flatten()
                .filter(|e| e.scope == StorageScope::Top && e.key == required.key)
                .collect();
            if actual.len() != 1 || actual[0] != required {
                return Err("preimage deposit owner lacks its exact unchanged-byte move".into());
            }
        }
        Ok(())
    }
}

pub fn compile(
    schema: &MetadataSchema,
    storage: &Storage,
    mapping: &[AccountReplacement],
) -> Result<Compilation> {
    recovery::validate_replacements(mapping)
        .map_err(|e| format!("invalid preimage mapping: {e:?}"))?;
    if schema.metadata_hash != METADATA {
        return Err("preimage ownership requires the deployed checkpoint metadata".into());
    }
    let replacements: BTreeMap<_, _> = mapping
        .iter()
        .map(|p| (p.old.clone(), p.new.clone()))
        .collect();
    let destinations: BTreeSet<_> = mapping.iter().map(|p| p.new.clone()).collect();
    let mapping_hash = hash(&mapping.encode());
    let mut out = Compilation {
        groups: vec![],
        edits: vec![],
        resolutions: vec![],
        witnesses: vec![],
        report: Report {
            schema: "sora-nexus-preimage-ownership-substitution-v1",
            mapping_commitment: hx(&mapping_hash),
            checkpoint_status_records: 0,
            changed_status_records: 0,
            moved_deposit_owners: vec![],
            moved_deposit_total: "0".into(),
            call_bytes_hashes_lengths_and_counts_preserved: true,
            refunds_created: 0,
            production_ready: false,
        },
        mapping_hash,
        backing_moves: vec![],
    };

    // New-format tickets are holds; this adapter does not move holds.
    let tickets = descriptor(schema, "Preimage.RequestStatusFor")?;
    identity_map(tickets)?;
    let bytes_mapping: BTreeMap<_, _> = mapping
        .iter()
        .map(|p| {
            (
                *AsRef::<[u8; 32]>::as_ref(&p.old),
                *AsRef::<[u8; 32]>::as_ref(&p.new),
            )
        })
        .collect();
    for (_, value) in rows(storage, tickets)? {
        let (_, accounts) =
            typed::transform_value(value, tickets.value_type, &schema.registry, &bytes_mapping)?;
        if accounts.iter().any(|a| bytes_mapping.contains_key(a)) {
            return Err(
                "hold-backed preimage ticket of a replaced account needs a hold-moving adapter"
                    .into(),
            );
        }
    }

    let statuses = descriptor(schema, "Preimage.StatusFor")?;
    identity_map(statuses)?;
    let mut deposits = BTreeMap::<AccountId32, u128>::new();
    for (key, value) in rows(storage, statuses)? {
        out.report.checkpoint_status_records += 1;
        let before: OldRequestStatus = decode(schema, statuses.value_type, value)?;
        let mut after = before.clone();
        let Some((owner, amount)) = after.deposit_mut() else {
            continue;
        };
        if destinations.contains(owner) {
            return Err("preimage deposit owner is an occupied replacement destination".into());
        }
        let Some(new) = replacements.get(owner) else {
            continue;
        };
        let total = deposits.entry(owner.clone()).or_default();
        *total = total
            .checked_add(*amount)
            .ok_or("preimage deposits overflow")?;
        let old = owner.clone();
        *owner = new.clone();
        let after = after.encode();
        let edit = GuardedStorageEdit {
            scope: StorageScope::Top,
            key: key.clone(),
            before: Some(hash(value)),
            after: Some(after.clone()),
        };
        out.resolutions.push(AdapterResolution {
            storage: statuses.name.clone(),
            key: hx(key),
            expected_before_hash: hx(&hash(value)),
            expected_after_hash: Some(hx(&hash(&after))),
            adapter: AdapterId::PreimageOwnershipV1,
            action: ResolutionAction::ReplaceControl,
            validated_edits: vec![edit.clone()],
            observed_accounts: BTreeSet::from([*AsRef::<[u8; 32]>::as_ref(&old)]),
        });
        out.witnesses.push(StorageWitness {
            scope: StorageScope::Top,
            key: key.clone(),
            value_hash: Some(hash(&after)),
        });
        out.edits.push(edit.clone());
        out.groups.push(vec![edit]);
        out.report.changed_status_records += 1;
    }

    let system = descriptor(schema, "System.Account")?;
    if system.hashers != [Hasher::Blake2_128Concat]
        || system.key_types.len() != 1
        || !typed::account_type(&schema.registry, system.key_types[0])
    {
        return Err("preimage System backing codec differs".into());
    }
    let mut moved_total = 0u128;
    for (old, deposit) in &deposits {
        let source = account_key(system, old);
        let destination = account_key(system, &replacements[old]);
        let value = storage
            .top
            .get(&source)
            .ok_or("preimage deposit owner lacks System backing")?;
        let info: AccountInfo = decode(schema, system.value_type, value)?;
        if info.data.1 < *deposit {
            return Err("preimage deposits exceed the owner's reserved backing".into());
        }
        if storage.top.contains_key(&destination) {
            return Err("preimage System backing destination occupied".into());
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
        out.report.moved_deposit_owners.push(hx(old.as_ref()));
        moved_total = moved_total
            .checked_add(*deposit)
            .ok_or("preimage deposits overflow")?;
    }
    out.report.moved_deposit_total = moved_total.to_string();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, PortableRegistry, Registry};

    const OLD: [u8; 32] = [1; 32];
    const NEW: [u8; 32] = [2; 32];

    fn fixture(reserved: u128) -> (MetadataSchema, Storage, Vec<AccountReplacement>) {
        let mut registry = Registry::new();
        let account = registry.register_type(&meta_type::<AccountId32>()).id;
        let hash_id = registry.register_type(&meta_type::<[u8; 32]>()).id;
        let status = registry.register_type(&meta_type::<OldRequestStatus>()).id;
        let ticket = registry
            .register_type(&meta_type::<Option<(AccountId32, u128)>>())
            .id;
        let info = registry.register_type(&meta_type::<AccountInfo>()).id;
        let descriptors = [
            ("Preimage.StatusFor", hash_id, Hasher::Identity, status),
            (
                "Preimage.RequestStatusFor",
                hash_id,
                Hasher::Identity,
                ticket,
            ),
            ("System.Account", account, Hasher::Blake2_128Concat, info),
        ]
        .into_iter()
        .map(|(name, key_type, hasher, value_type)| {
            let (pallet, item) = name.split_once('.').unwrap();
            Descriptor {
                trie: "top".into(),
                name: name.into(),
                prefix: [
                    sp_io::hashing::twox_128(pallet.as_bytes()),
                    sp_io::hashing::twox_128(item.as_bytes()),
                ]
                .concat(),
                key_types: vec![key_type],
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
        let statuses = descriptor(&schema, "Preimage.StatusFor")
            .unwrap()
            .prefix
            .clone();
        let foreign = AccountId32::new([3; 32]);
        for (hash_byte, status) in [
            (
                1u8,
                OldRequestStatus::Unrequested {
                    deposit: (OLD.into(), 40),
                    len: 2_950_275,
                },
            ),
            (
                2,
                OldRequestStatus::Requested {
                    deposit: Some((OLD.into(), 60)),
                    count: 2,
                    len: Some(7),
                },
            ),
            (
                3,
                OldRequestStatus::Requested {
                    deposit: None,
                    count: 1,
                    len: None,
                },
            ),
            (
                4,
                OldRequestStatus::Unrequested {
                    deposit: (foreign.clone(), 5),
                    len: 9,
                },
            ),
        ] {
            storage.top.insert(
                [statuses.clone(), vec![hash_byte; 32]].concat(),
                status.encode(),
            );
        }
        let system = descriptor(&schema, "System.Account").unwrap();
        storage.top.insert(
            account_key(system, &OLD.into()),
            AccountInfo {
                nonce: 756,
                consumers: 4,
                providers: 4,
                sufficients: 0,
                data: (11, reserved, 0, 0),
            }
            .encode(),
        );
        let map = vec![AccountReplacement {
            old: OLD.into(),
            new: NEW.into(),
        }];
        (schema, storage, map)
    }

    // The unchanged-byte System.Account move that the generic mapper composes.
    fn generic_backing_move(storage: &Storage, schema: &MetadataSchema) -> Vec<GuardedStorageEdit> {
        let system = descriptor(schema, "System.Account").unwrap();
        let source = account_key(system, &OLD.into());
        let value = storage.top[&source].clone();
        vec![
            GuardedStorageEdit {
                scope: StorageScope::Top,
                key: source,
                before: Some(hash(&value)),
                after: None,
            },
            GuardedStorageEdit {
                scope: StorageScope::Top,
                key: account_key(system, &NEW.into()),
                before: None,
                after: Some(value),
            },
        ]
    }

    #[test]
    fn moves_only_the_deposit_owner_and_keeps_its_reserve() {
        let (schema, storage, map) = fixture(100);
        let c = compile(&schema, &storage, &map).unwrap();
        assert_eq!(c.report.checkpoint_status_records, 4);
        assert_eq!(c.report.changed_status_records, 2);
        assert_eq!(c.report.moved_deposit_total, "100");
        assert_eq!(c.report.refunds_created, 0);
        let mut projected = storage.clone();
        for edit in &c.edits {
            assert_eq!(edit.key.len(), 64);
            projected
                .top
                .insert(edit.key.clone(), edit.after.clone().unwrap());
        }
        let statuses = descriptor(&schema, "Preimage.StatusFor")
            .unwrap()
            .prefix
            .clone();
        let read = |byte: u8| {
            OldRequestStatus::decode(
                &mut &projected.top[&[statuses.clone(), vec![byte; 32]].concat()][..],
            )
            .unwrap()
        };
        assert!(matches!(
            read(1),
            OldRequestStatus::Unrequested { deposit: (owner, 40), len: 2_950_275 } if owner == NEW.into()
        ));
        assert!(matches!(
            read(2),
            OldRequestStatus::Requested { deposit: Some((owner, 60)), count: 2, len: Some(7) } if owner == NEW.into()
        ));
        // Requested statuses without a deposit and foreign deposits stay untouched.
        for byte in [3u8, 4] {
            let key = [statuses.clone(), vec![byte; 32]].concat();
            assert_eq!(projected.top[&key], storage.top[&key]);
        }
        let mut groups = c.groups.clone();
        groups.push(generic_backing_move(&storage, &schema));
        c.verify_groups(&groups, &map).unwrap();
        assert!(c.resolutions.iter().all(
            |r| r.adapter == AdapterId::PreimageOwnershipV1 && r.expected_after_hash.is_some()
        ));
    }

    #[test]
    fn verification_requires_the_exact_owner_moves_and_unchanged_backing() {
        let (schema, storage, map) = fixture(100);
        let c = compile(&schema, &storage, &map).unwrap();
        let backing = generic_backing_move(&storage, &schema);
        // Without the System.Account move the reserve would stay behind.
        assert!(c.verify_groups(&c.groups, &map).is_err());
        // A changed AccountInfo cannot stand in for the unchanged reserve.
        let mut altered = backing.clone();
        altered[1].after.as_mut().unwrap()[0] ^= 1;
        assert!(c.verify_groups(&[c.edits.clone(), altered], &map).is_err());
        // Dropping an owner move, or touching another preimage key, fails.
        assert!(c
            .verify_groups(&[c.edits[..1].to_vec(), backing.clone()], &map)
            .is_err());
        let mut extra = c.edits.clone();
        extra.push(GuardedStorageEdit {
            scope: StorageScope::Top,
            key: [pallet_prefix().to_vec(), vec![9; 48]].concat(),
            before: None,
            after: Some(vec![1]),
        });
        assert!(c.verify_groups(&[extra, backing.clone()], &map).is_err());
        // A different mapping cannot reuse this certificate.
        let other = vec![AccountReplacement {
            old: OLD.into(),
            new: AccountId32::new([8; 32]),
        }];
        assert!(c
            .verify_groups(&[c.edits.clone(), backing], &other)
            .is_err());
    }

    #[test]
    fn unbacked_deposit_hold_ticket_and_occupied_destination_fail_closed() {
        let (schema, storage, map) = fixture(99);
        assert!(compile(&schema, &storage, &map)
            .err()
            .unwrap()
            .contains("reserved backing"));
        let (schema, mut storage, map) = fixture(100);
        let tickets = descriptor(&schema, "Preimage.RequestStatusFor")
            .unwrap()
            .prefix
            .clone();
        storage.top.insert(
            [tickets, vec![5; 32]].concat(),
            Some((AccountId32::from(OLD), 7u128)).encode(),
        );
        assert!(compile(&schema, &storage, &map)
            .err()
            .unwrap()
            .contains("hold-moving adapter"));
        let (schema, mut storage, map) = fixture(100);
        let system = descriptor(&schema, "System.Account").unwrap();
        storage.top.insert(
            account_key(system, &NEW.into()),
            storage.top[&account_key(system, &OLD.into())].clone(),
        );
        assert!(compile(&schema, &storage, &map).is_err());
    }
}
