//! Cancel changed Iroha migration approvals without moving their claim backing.
//!
//! An approval records a previously signed destination, not transferable owner
//! state. Reset the entire affected approval attempt, keeping the Iroha keys,
//! quorum, balance and referral records for fresh native reapproval.
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
const ITEMS: &[&str] = &[
    "PendingMultiSigAccounts",
    "PublicKeys",
    "Quorums",
    "Balances",
    "Referrers",
    "PendingReferrals",
    "MigratedAccounts",
];

#[derive(Debug, Clone, Decode, Encode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Pending {
    approving_accounts: Vec<AccountId32>,
    migrate_at: Option<u32>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub schema: &'static str,
    pub mapping_commitment: String,
    pub cancelled_iroha_accounts: Vec<String>,
    pub cleared_key_approvals: usize,
    pub fresh_native_approvals_required: bool,
    pub financial_backing_preserved: bool,
    pub refunds: usize,
    pub production_ready: bool,
}
pub struct Compilation {
    pub groups: Vec<Vec<GuardedStorageEdit>>,
    pub edits: Vec<GuardedStorageEdit>,
    pub resolutions: Vec<AdapterResolution>,
    pub witnesses: Vec<StorageWitness>,
    pub report: Report,
    mapping_hash: [u8; 32],
    preserved: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(bytes)
}
fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn prefix(item: &str) -> Vec<u8> {
    [
        sp_io::hashing::twox_128(b"IrohaMigration"),
        sp_io::hashing::twox_128(item.as_bytes()),
    ]
    .concat()
}
fn descriptor<'a>(schema: &'a MetadataSchema, item: &str) -> Result<&'a Descriptor> {
    let name = format!("IrohaMigration.{item}");
    let d = schema
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == name)
        .ok_or_else(|| format!("missing Iroha migration schema {name}"))?;
    if d.prefix != prefix(item) || d.hashers != [Hasher::Blake2_128Concat] || d.key_types.len() != 1
    {
        return Err("unreviewed Iroha migration map codec".into());
    }
    Ok(d)
}
fn key(d: &Descriptor, address: &str) -> Vec<u8> {
    let encoded = address.encode();
    let mut result = d.prefix.clone();
    result.extend(sp_io::hashing::blake2_128(&encoded));
    result.extend(encoded);
    result
}
fn decode<T: Decode + Encode>(schema: &MetadataSchema, id: u32, bytes: &[u8]) -> Result<T> {
    typed::decode_exact(bytes, id, &schema.registry)?;
    let mut input = bytes;
    let value = T::decode(&mut input).map_err(|e| e.to_string())?;
    if !input.is_empty() || value.encode() != bytes {
        return Err("noncanonical Iroha migration value".into());
    }
    Ok(value)
}
fn required<'a>(storage: &'a Storage, key: &[u8]) -> Result<&'a [u8]> {
    storage
        .top
        .get(key)
        .map(Vec::as_slice)
        .ok_or_else(|| "missing Iroha pending cross-record".into())
}
impl Compilation {
    /// Cancellation/reset is one atomic group, and no composed operation may
    /// change its backing or revive another pending approval under these keys.
    pub fn verify_groups(
        &self,
        groups: &[Vec<GuardedStorageEdit>],
        mapping: &[AccountReplacement],
    ) -> Result<()> {
        if hash(&mapping.encode()) != self.mapping_hash {
            return Err("Iroha cancellation mapping differs".into());
        }
        let controlled = [prefix("PendingMultiSigAccounts"), prefix("PublicKeys")];
        let mut actual = Vec::new();
        for group in groups {
            let touched: Vec<_> = group
                .iter()
                .filter(|e| {
                    e.scope == StorageScope::Top && controlled.iter().any(|p| e.key.starts_with(p))
                })
                .cloned()
                .collect();
            if !touched.is_empty()
                && !self.groups.iter().any(|expected| {
                    expected.iter().all(|e| touched.contains(e)) && expected.len() == touched.len()
                })
            {
                return Err("Iroha approval reset split, altered or unapproved".into());
            }
            actual.extend(touched);
            if group
                .iter()
                .any(|e| e.scope == StorageScope::Top && self.preserved.contains_key(&e.key))
            {
                return Err("Iroha cancellation backing or completed guard changed".into());
            }
        }
        actual.sort_by(|a, b| a.key.cmp(&b.key));
        let mut expected = self.edits.clone();
        expected.sort_by(|a, b| a.key.cmp(&b.key));
        if actual != expected {
            return Err("Iroha cancellation edits missing or duplicated".into());
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
        .map_err(|e| format!("invalid Iroha mapping: {e:?}"))?;
    if schema.metadata_hash != METADATA {
        return Err("Iroha cancellation requires deployed checkpoint metadata".into());
    }
    let mapping: BTreeSet<_> = replacements.iter().map(|p| p.old.clone()).collect();
    let descriptors: BTreeMap<_, _> = ITEMS
        .iter()
        .map(|item| Ok((*item, descriptor(schema, item)?)))
        .collect::<Result<_>>()?;
    let pending = descriptors["PendingMultiSigAccounts"];
    let public = descriptors["PublicKeys"];
    let mapping_hash = hash(&replacements.encode());
    let mut out = Compilation {
        groups: vec![],
        edits: vec![],
        resolutions: vec![],
        witnesses: vec![],
        mapping_hash,
        preserved: BTreeMap::new(),
        report: Report {
            schema: "sora-nexus-iroha-pending-cancellation-v1",
            mapping_commitment: hx(&mapping_hash),
            cancelled_iroha_accounts: vec![],
            cleared_key_approvals: 0,
            fresh_native_approvals_required: true,
            financial_backing_preserved: true,
            refunds: 0,
            production_ready: false,
        },
    };
    let rows: Vec<_> = storage
        .top
        .range(pending.prefix.clone()..)
        .take_while(|(k, _)| k.starts_with(&pending.prefix))
        .take(MAX_ROWS + 1)
        .collect();
    if rows.len() > MAX_ROWS {
        return Err("Iroha pending row bound".into());
    }
    for (pending_key, bytes) in rows {
        let encoded = pending_key
            .get(pending.prefix.len() + 16..)
            .ok_or("short Iroha pending key")?;
        let address: String = decode(schema, pending.key_types[0], encoded)?;
        if address.is_empty() || address.len() > 1024 || key(pending, &address) != *pending_key {
            return Err("noncanonical Iroha pending address/key".into());
        }
        let state: Pending = decode(schema, pending.value_type, bytes)?;
        if !state.approving_accounts.iter().any(|a| mapping.contains(a)) {
            continue;
        }
        let public_key = key(public, &address);
        let old_public = required(storage, &public_key)?;
        let mut keys: Vec<(bool, String)> = decode(schema, public.value_type, old_public)?;
        let approved = keys.iter().filter(|(yes, _)| *yes).count();
        let unique: BTreeSet<_> = keys.iter().map(|(_, k)| k).collect();
        if !(2..=1000).contains(&keys.len())
            || unique.len() != keys.len()
            || keys.iter().any(|(_, k)| {
                k.len() != 64
                    || !k
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
        {
            return Err("ambiguous Iroha public-key inventory".into());
        }
        let quorum_d = descriptors["Quorums"];
        let quorum: u8 = decode(
            schema,
            quorum_d.value_type,
            required(storage, &key(quorum_d, &address))?,
        )?;
        let q = usize::from(quorum);
        // Above quorum, native code leaves the existing approver list unchanged
        // until the final key approves; it must not be equated to all true flags.
        if q == 0
            || q > keys.len()
            || approved == 0
            || approved >= keys.len()
            || state.approving_accounts.len() != approved.min(q)
            || (approved < q && state.migrate_at.is_some())
            || (approved >= q
                && state
                    .migrate_at
                    .is_none_or(|n| n < recovery::CHECKPOINT_NUMBER))
        {
            return Err("inconsistent Iroha pending approval/quorum/timeout state".into());
        }
        let balance_d = descriptors["Balances"];
        let _: u128 = decode(
            schema,
            balance_d.value_type,
            required(storage, &key(balance_d, &address))?,
        )?;
        let migrated = key(descriptors["MigratedAccounts"], &address);
        if storage.top.contains_key(&migrated) {
            return Err("Iroha pending record is already migrated".into());
        }
        for item in [
            "Quorums",
            "Balances",
            "Referrers",
            "PendingReferrals",
            "MigratedAccounts",
        ] {
            let d = descriptors[item];
            let record_key = key(d, &address);
            let value = storage.top.get(&record_key).cloned();
            if let Some(value) = &value {
                typed::decode_exact(value, d.value_type, &schema.registry)?;
            }
            out.witnesses.push(StorageWitness {
                scope: StorageScope::Top,
                key: record_key.clone(),
                value_hash: value.as_ref().map(|v| hash(v)),
            });
            out.preserved.insert(record_key, value);
        }
        for (yes, _) in &mut keys {
            *yes = false;
        }
        let reset = keys.encode();
        let group = vec![
            GuardedStorageEdit {
                scope: StorageScope::Top,
                key: pending_key.clone(),
                before: Some(hash(bytes)),
                after: None,
            },
            GuardedStorageEdit {
                scope: StorageScope::Top,
                key: public_key.clone(),
                before: Some(hash(old_public)),
                after: Some(reset.clone()),
            },
        ];
        let observed_accounts = state
            .approving_accounts
            .iter()
            .map(|a| *AsRef::<[u8; 32]>::as_ref(a))
            .collect();
        for (d, edit, action, owners) in [
            (
                pending,
                &group[0],
                ResolutionAction::CancelPending,
                observed_accounts,
            ),
            (
                public,
                &group[1],
                ResolutionAction::ReplaceControl,
                BTreeSet::new(),
            ),
        ] {
            out.resolutions.push(AdapterResolution {
                storage: d.name.clone(),
                key: hx(&edit.key),
                expected_before_hash: hx(&edit.before.expect("existing row")),
                expected_after_hash: edit.after.as_ref().map(|v| hx(&hash(v))),
                adapter: AdapterId::IrohaPendingCancellationV1,
                action,
                validated_edits: group.clone(),
                observed_accounts: owners,
            });
            out.witnesses.push(StorageWitness {
                scope: StorageScope::Top,
                key: edit.key.clone(),
                value_hash: edit.after.as_ref().map(|v| hash(v)),
            });
        }
        out.report.cancelled_iroha_accounts.push(address);
        out.report.cleared_key_approvals += approved;
        out.edits.extend(group.clone());
        out.groups.push(group);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, PortableRegistry, Registry};
    fn fixture(approved: usize, quorum: u8) -> (MetadataSchema, Storage, Vec<AccountReplacement>) {
        let mut registry = Registry::new();
        let string = registry.register_type(&meta_type::<String>()).id;
        let types = [
            registry.register_type(&meta_type::<Pending>()).id,
            registry
                .register_type(&meta_type::<Vec<(bool, String)>>())
                .id,
            registry.register_type(&meta_type::<u8>()).id,
            registry.register_type(&meta_type::<u128>()).id,
            string,
            registry.register_type(&meta_type::<Vec<AccountId32>>()).id,
            registry.register_type(&meta_type::<AccountId32>()).id,
        ];
        let schema = MetadataSchema {
            metadata_hash: METADATA.into(),
            registry: PortableRegistry::from(registry),
            descriptors: ITEMS
                .iter()
                .zip(types)
                .map(|(item, value_type)| Descriptor {
                    trie: "top".into(),
                    name: format!("IrohaMigration.{item}"),
                    prefix: prefix(item),
                    key_types: vec![string],
                    hashers: vec![Hasher::Blake2_128Concat],
                    value_type,
                })
                .collect(),
        };
        let old = AccountId32::new([1; 32]);
        let map = vec![AccountReplacement {
            old: old.clone(),
            new: AccountId32::new([2; 32]),
        }];
        let mut storage = Storage::default();
        for (item, value) in [
            (
                "PendingMultiSigAccounts",
                Pending {
                    approving_accounts: (0..approved.min(usize::from(quorum)))
                        .map(|_| old.clone())
                        .collect(),
                    migrate_at: (approved >= usize::from(quorum))
                        .then_some(recovery::CHECKPOINT_NUMBER + 50),
                }
                .encode(),
            ),
            (
                "PublicKeys",
                (0..4)
                    .map(|i| (i < approved, format!("{i:064x}")))
                    .collect::<Vec<_>>()
                    .encode(),
            ),
            ("Quorums", quorum.encode()),
            ("Balances", 500u128.encode()),
            ("Referrers", "innocent@sora".encode()),
            ("PendingReferrals", vec![AccountId32::new([7; 32])].encode()),
        ] {
            storage
                .top
                .insert(key(descriptor(&schema, item).unwrap(), "claim@sora"), value);
        }
        (schema, storage, map)
    }
    #[test]
    fn cancels_authorization_and_preserves_claim_backing_without_refund() {
        let (s, storage, map) = fixture(1, 2);
        let c = compile(&s, &storage, &map).unwrap();
        c.verify_groups(&c.groups, &map).unwrap();
        assert_eq!(c.groups.len(), 1);
        assert_eq!(c.edits.len(), 2);
        assert_eq!(c.report.cleared_key_approvals, 1);
        assert_eq!(c.report.refunds, 0);
        assert_eq!(c.preserved.len(), 5);
        let reset: Vec<(bool, String)> = decode(
            &s,
            descriptor(&s, "PublicKeys").unwrap().value_type,
            c.edits[1].after.as_ref().unwrap(),
        )
        .unwrap();
        assert!(reset.iter().all(|(approved, _)| !approved));
        assert_eq!(
            reset.iter().map(|(_, key)| key.clone()).collect::<Vec<_>>(),
            (0..4).map(|i| format!("{i:064x}")).collect::<Vec<_>>()
        );
        assert!(c.edits[0].after.is_none());
    }
    #[test]
    fn quorum_and_above_quorum_native_states_are_supported() {
        for approved in [2, 3] {
            let (s, storage, map) = fixture(approved, 2);
            let c = compile(&s, &storage, &map).unwrap();
            assert_eq!(c.report.cleared_key_approvals, approved);
        }
    }
    #[test]
    fn unmodified_pending_destinations_keep_their_exact_approvals() {
        let (s, storage, _) = fixture(1, 2);
        let map = vec![AccountReplacement {
            old: AccountId32::new([8; 32]),
            new: AccountId32::new([9; 32]),
        }];
        let c = compile(&s, &storage, &map).unwrap();
        assert!(c.edits.is_empty());
        c.verify_groups(&[], &map).unwrap();
    }
    #[test]
    fn malformed_or_already_completed_claims_fail_closed() {
        for kind in 0..6 {
            let (s, mut storage, map) = fixture(2, 2);
            match kind {
                0 => {
                    storage
                        .top
                        .remove(&key(descriptor(&s, "Balances").unwrap(), "claim@sora"));
                }
                1 => {
                    storage.top.insert(
                        key(descriptor(&s, "MigratedAccounts").unwrap(), "claim@sora"),
                        map[0].old.encode(),
                    );
                }
                2 => {
                    storage.top.insert(
                        key(descriptor(&s, "Quorums").unwrap(), "claim@sora"),
                        0u8.encode(),
                    );
                }
                3 => {
                    storage.top.insert(
                        key(
                            descriptor(&s, "PendingMultiSigAccounts").unwrap(),
                            "claim@sora",
                        ),
                        Pending {
                            approving_accounts: vec![map[0].old.clone()],
                            migrate_at: None,
                        }
                        .encode(),
                    );
                }
                4 => {
                    storage.top.insert(
                        key(
                            descriptor(&s, "PendingMultiSigAccounts").unwrap(),
                            "claim@sora",
                        ),
                        Pending {
                            approving_accounts: vec![map[0].old.clone(); 2],
                            migrate_at: Some(recovery::CHECKPOINT_NUMBER - 1),
                        }
                        .encode(),
                    );
                }
                _ => {
                    storage.top.insert(
                        key(descriptor(&s, "PublicKeys").unwrap(), "claim@sora"),
                        vec![(true, "1".repeat(64)); 4].encode(),
                    );
                }
            };
            assert!(compile(&s, &storage, &map).is_err(), "kind{kind}");
        }
    }
    #[test]
    fn reset_must_remain_atomic_and_backing_is_not_editable() {
        let (s, storage, map) = fixture(1, 2);
        let c = compile(&s, &storage, &map).unwrap();
        assert!(c
            .verify_groups(
                &c.edits.iter().cloned().map(|e| vec![e]).collect::<Vec<_>>(),
                &map
            )
            .is_err());
        let mut groups = c.groups.clone();
        groups.push(vec![GuardedStorageEdit {
            scope: StorageScope::Top,
            key: key(descriptor(&s, "Balances").unwrap(), "claim@sora"),
            before: Some(hash(&500u128.encode())),
            after: Some(0u128.encode()),
        }]);
        assert!(c.verify_groups(&groups, &map).is_err());
        assert!(c.verify_groups(&[], &map).is_err());
    }
}
