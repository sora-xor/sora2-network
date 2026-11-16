//! Move pending Iroha multisig approvals to the replacement accounts.
//!
//! A replaced account's approval slot passes to its replacement in place. The
//! consumed Iroha key flags, quorum, timer, balance and referral records stay,
//! so a key already used for the claim cannot approve it again. Resetting the
//! flags instead would let anyone holding a reused key (for example one derived
//! from a stolen mnemonic) re-approve from a fresh account and take the slot.
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
    pub remapped_iroha_accounts: Vec<String>,
    pub remapped_approvals: usize,
    pub consumed_keys_quorum_and_timer_preserved: bool,
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
    /// Each remap is one exact edit, and no composed operation may change its
    /// backing, re-open a consumed key or revive another pending approval.
    pub fn verify_groups(
        &self,
        groups: &[Vec<GuardedStorageEdit>],
        mapping: &[AccountReplacement],
    ) -> Result<()> {
        if hash(&mapping.encode()) != self.mapping_hash {
            return Err("Iroha approver mapping differs".into());
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
                return Err("Iroha approver remap split, altered or unapproved".into());
            }
            actual.extend(touched);
            if group
                .iter()
                .any(|e| e.scope == StorageScope::Top && self.preserved.contains_key(&e.key))
            {
                return Err("Iroha approver backing or completed guard changed".into());
            }
        }
        actual.sort_by(|a, b| a.key.cmp(&b.key));
        let mut expected = self.edits.clone();
        expected.sort_by(|a, b| a.key.cmp(&b.key));
        if actual != expected {
            return Err("Iroha approver remaps missing or duplicated".into());
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
        return Err("Iroha approver remap requires deployed checkpoint metadata".into());
    }
    let mapping: BTreeMap<_, _> = replacements
        .iter()
        .map(|p| (p.old.clone(), p.new.clone()))
        .collect();
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
            schema: "sora-nexus-iroha-pending-approver-remap-v1",
            mapping_commitment: hx(&mapping_hash),
            remapped_iroha_accounts: vec![],
            remapped_approvals: 0,
            consumed_keys_quorum_and_timer_preserved: true,
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
        if !state
            .approving_accounts
            .iter()
            .any(|a| mapping.contains_key(a))
        {
            continue;
        }
        let public_key = key(public, &address);
        let old_public = required(storage, &public_key)?;
        let keys: Vec<(bool, String)> = decode(schema, public.value_type, old_public)?;
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
            "PublicKeys",
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
        let remapped = state
            .approving_accounts
            .iter()
            .filter(|a| mapping.contains_key(*a))
            .count();
        let after = Pending {
            approving_accounts: state
                .approving_accounts
                .iter()
                .map(|a| mapping.get(a).unwrap_or(a).clone())
                .collect(),
            migrate_at: state.migrate_at,
        }
        .encode();
        let edit = GuardedStorageEdit {
            scope: StorageScope::Top,
            key: pending_key.clone(),
            before: Some(hash(bytes)),
            after: Some(after.clone()),
        };
        out.resolutions.push(AdapterResolution {
            storage: pending.name.clone(),
            key: hx(pending_key),
            expected_before_hash: hx(&hash(bytes)),
            expected_after_hash: Some(hx(&hash(&after))),
            adapter: AdapterId::IrohaPendingApproverV1,
            action: ResolutionAction::ReplaceControl,
            validated_edits: vec![edit.clone()],
            observed_accounts: state
                .approving_accounts
                .iter()
                .map(|a| *AsRef::<[u8; 32]>::as_ref(a))
                .collect(),
        });
        out.witnesses.push(StorageWitness {
            scope: StorageScope::Top,
            key: pending_key.clone(),
            value_hash: Some(hash(&after)),
        });
        out.report.remapped_iroha_accounts.push(address);
        out.report.remapped_approvals += remapped;
        out.edits.push(edit.clone());
        out.groups.push(vec![edit]);
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
    fn remapped(c: &Compilation, s: &MetadataSchema) -> Pending {
        decode(
            s,
            descriptor(s, "PendingMultiSigAccounts").unwrap().value_type,
            c.edits[0].after.as_ref().unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn moves_the_approval_slot_and_keeps_consumed_keys_and_backing() {
        let (s, storage, map) = fixture(1, 2);
        let c = compile(&s, &storage, &map).unwrap();
        c.verify_groups(&c.groups, &map).unwrap();
        assert_eq!(c.groups.len(), 1);
        assert_eq!(c.edits.len(), 1);
        assert_eq!(c.report.remapped_approvals, 1);
        assert_eq!(c.report.refunds, 0);
        assert_eq!(c.preserved.len(), 6);
        let pending = remapped(&c, &s);
        assert_eq!(pending.approving_accounts, vec![map[0].new.clone()]);
        assert_eq!(pending.migrate_at, None);
        // The used key stays consumed, so it cannot approve the claim again.
        let keys = key(descriptor(&s, "PublicKeys").unwrap(), "claim@sora");
        assert!(c.preserved[&keys].as_ref() == storage.top.get(&keys));
    }
    #[test]
    fn quorum_and_above_quorum_native_states_keep_their_timer() {
        for approved in [2, 3] {
            let (s, storage, map) = fixture(approved, 2);
            let c = compile(&s, &storage, &map).unwrap();
            let pending = remapped(&c, &s);
            assert_eq!(pending.approving_accounts, vec![map[0].new.clone(); 2]);
            assert_eq!(pending.migrate_at, Some(recovery::CHECKPOINT_NUMBER + 50));
            assert_eq!(c.report.remapped_approvals, 2);
        }
    }
    #[test]
    fn only_replaced_approvers_move_and_their_order_is_kept() {
        let (s, mut storage, map) = fixture(2, 3);
        let foreign = AccountId32::new([5; 32]);
        storage.top.insert(
            key(
                descriptor(&s, "PendingMultiSigAccounts").unwrap(),
                "claim@sora",
            ),
            Pending {
                approving_accounts: vec![foreign.clone(), map[0].old.clone()],
                migrate_at: None,
            }
            .encode(),
        );
        let c = compile(&s, &storage, &map).unwrap();
        assert_eq!(
            remapped(&c, &s).approving_accounts,
            vec![foreign, map[0].new.clone()]
        );
        assert_eq!(c.report.remapped_approvals, 1);
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
    fn remap_is_exact_and_keys_and_backing_are_not_editable() {
        let (s, storage, map) = fixture(1, 2);
        let c = compile(&s, &storage, &map).unwrap();
        // Re-opening the consumed key alongside the remap is rejected.
        let keys = key(descriptor(&s, "PublicKeys").unwrap(), "claim@sora");
        let mut reopened = c.groups.clone();
        reopened[0].push(GuardedStorageEdit {
            scope: StorageScope::Top,
            key: keys.clone(),
            before: Some(hash(&storage.top[&keys])),
            after: Some(vec![(false, format!("{:064x}", 0))].encode()),
        });
        assert!(c.verify_groups(&reopened, &map).is_err());
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
