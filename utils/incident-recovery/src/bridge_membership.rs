//! Paired registered bridge-account controls. Membership is a sorted key set;
//! changing it invalidates every outstanding operation for that account.
use super::*;
use crate::inventory::Hasher;
use common::recovery::{StoragePrefixWitness, MAX_EDITS_PER_CHUNK};
use scale_info::TypeDefPrimitive;
use scale_value::Primitive;

const MAX_ACCOUNTS: usize = 100;
const MAX_PENDING: usize = 10_000;
const PENDING: [&str; 4] = [
    "Multisigs",
    "CancellationApprovals",
    "CountedNewOperations",
    "ProposerCountedOperations",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Review {
    pub account: String,
    pub accounts_value_hash: String,
    pub review_evidence_hash: String,
}
pub(super) struct Compilation {
    pub groups: Vec<Vec<GuardedStorageEdit>>,
    pub resolutions: Vec<AdapterResolution>,
    pub prefixes: Vec<StoragePrefixWitness>,
    pub guard: Guard,
}
/// Private expected state is constructed from authenticated checkpoint bytes,
/// never reconstructed from caller-mutable edits or resolution vectors.
pub(super) struct Guard {
    prefixes: Vec<Vec<u8>>,
    accounts: BTreeSet<Vec<u8>>,
    before: BTreeMap<Vec<u8>, Vec<u8>>,
    expected: BTreeMap<Vec<u8>, Vec<u8>>,
}
impl Guard {
    pub fn verify(&self, groups: &[Vec<GuardedStorageEdit>]) -> Result<()> {
        let mut projected = self.before.clone();
        let mut seen = BTreeSet::new();
        for e in groups.iter().flatten().filter(|e| {
            e.scope == StorageScope::Top
                && (self.accounts.contains(&e.key)
                    || self.prefixes.iter().any(|p| e.key.starts_with(p)))
        }) {
            if !seen.insert(e.key.clone()) || self.before.get(&e.key).map(|v| digest(v)) != e.before
            {
                return Err("bridge membership overlapping or stale coupled edit".into());
            }
            if let Some(value) = &e.after {
                projected.insert(e.key.clone(), value.clone());
            } else {
                projected.remove(&e.key);
            }
        }
        if projected != self.expected {
            return Err("bridge membership requires complete cancellation and exact paired membership/account custody".into());
        }
        Ok(())
    }
}
fn prefix(item: &str) -> Vec<u8> {
    [
        sp_io::hashing::twox_128(b"BridgeMultisig"),
        sp_io::hashing::twox_128(item.as_bytes()),
    ]
    .concat()
}
fn membership(schema: &MetadataSchema, d: &Descriptor, bytes: &[u8]) -> Result<Vec<[u8; 32]>> {
    if bytes.len() > 4096 {
        return Err("bridge membership exceeds bounded key set".into());
    }
    let value = typed::decode_exact(bytes, d.value_type, &schema.registry)?;
    let ValueDef::Composite(Composite::Named(fields)) = value.value else {
        return Err("bridge membership must be named signatories/threshold".into());
    };
    if fields.len() != 2 || fields[0].0 != "signatories" || fields[1].0 != "threshold" {
        return Err("bridge membership shape changed".into());
    }
    let mut input = bytes;
    let count = codec::Compact::<u32>::decode(&mut input).map_err(|e| e.to_string())?;
    if count.0 == 0
        || count.0 as usize > MAX_ACCOUNTS
        || !bytes.starts_with(&count.encode())
        || input.len() != count.0 as usize * 32 + 1
        || input.last() != Some(&67)
    {
        return Err("bridge membership count/threshold is unreviewed".into());
    }
    let signers = input[..input.len() - 1].as_chunks::<32>().0.to_vec();
    if signers.windows(2).any(|w| w[0] >= w[1]) {
        return Err("bridge membership is not sorted/unique".into());
    }
    let (_, typed_accounts) =
        typed::transform_value(bytes, d.value_type, &schema.registry, &BTreeMap::new())?;
    if typed_accounts != signers.iter().copied().collect() {
        return Err("bridge membership identities differ from typed signatories".into());
    }
    Ok(signers)
}
fn account_backing(schema: &MetadataSchema, d: &Descriptor, bytes: &[u8]) -> Result<()> {
    let value = typed::decode_exact(bytes, d.value_type, &schema.registry)?;
    let ValueDef::Composite(Composite::Named(fields)) = value.value else {
        return Err("bridge account backing must be named AccountInfo".into());
    };
    let provider = &fields
        .iter()
        .find(|(name, _)| name == "providers")
        .ok_or("bridge account provider count absent")?
        .1;
    if !matches!(
        schema
            .registry
            .resolve(provider.context)
            .map(|t| &t.type_def),
        Some(TypeDef::Primitive(TypeDefPrimitive::U32))
    ) || !matches!(provider.value,ValueDef::Primitive(Primitive::U128(n)) if n>0&&n<=u32::MAX as u128)
    {
        return Err("registered bridge account has no valid provider backing".into());
    }
    Ok(())
}
pub(super) fn compile(
    schema: &MetadataSchema,
    storage: &Storage,
    mappings: &BTreeMap<[u8; 32], [u8; 32]>,
    proofs: &[OwnershipProof],
    plan: &SemanticPlan,
    cancelled: &BTreeMap<Vec<u8>, GuardedStorageEdit>,
) -> Result<Option<Compilation>> {
    if plan.bridge_memberships.is_empty()
        && !plan
            .derived_accounts
            .iter()
            .any(|a| matches!(a.derivation, DerivedKind::BridgeMultisig { .. }))
    {
        // Still reject a mapped signatory being silently handled by generic byte substitution.
        if let Ok(d) = descriptor(schema, "BridgeMultisig.Accounts") {
            for (k, v) in storage
                .top
                .range(d.prefix.clone()..)
                .take_while(|(k, _)| k.starts_with(&d.prefix))
            {
                let (_, accounts) =
                    typed::transform_value(v, d.value_type, &schema.registry, &BTreeMap::new())?;
                let (_, owners) = crate::inventory::transform_key(
                    k,
                    d,
                    &schema.registry,
                    &BTreeMap::new(),
                    None,
                )?;
                if accounts
                    .iter()
                    .chain(&owners)
                    .any(|a| mappings.contains_key(a))
                {
                    return Err("affected bridge membership needs explicit paired review".into());
                }
            }
        }
        return Ok(None);
    }
    if plan.bridge_memberships.len() > MAX_ACCOUNTS {
        return Err("bridge membership review bound".into());
    }
    let d = descriptor(schema, "BridgeMultisig.Accounts")?;
    let system = descriptor(schema, "System.Account")?;
    if d.prefix != prefix("Accounts")
        || d.hashers != [Hasher::Twox64Concat]
        || system.hashers != [Hasher::Blake2_128Concat]
    {
        return Err("unreviewed bridge membership/backing key layout".into());
    }
    let derived_controls =
        super::verify_derived(schema, storage, mappings, proofs, &plan.derived_accounts)?;
    let mut reviewed = BTreeMap::new();
    for row in &plan.bridge_memberships {
        commitment(&row.review_evidence_hash)?;
        if reviewed.insert(account_bytes(&row.account)?, row).is_some() {
            return Err("duplicate bridge membership review".into());
        }
    }
    let mut before = BTreeMap::new();
    let mut expected = BTreeMap::new();
    let mut prefixes = vec![d.prefix.clone()];
    let mut accounts = BTreeSet::new();
    let mut groups = Vec::new();
    let mut resolutions = Vec::new();
    let mut used = BTreeSet::new();
    let mut empty = BTreeSet::new();
    let mut pending_count = 0usize;
    let rows: Vec<_> = storage
        .top
        .range(d.prefix.clone()..)
        .take_while(|(k, _)| k.starts_with(&d.prefix))
        .collect();
    if rows.len() > MAX_ACCOUNTS {
        return Err("bridge membership inventory bound".into());
    }
    for (key, value) in rows {
        let (_, owners) =
            crate::inventory::transform_key(key, d, &schema.registry, &BTreeMap::new(), None)?;
        if owners.len() != 1 {
            return Err("bridge membership key must have one typed owner".into());
        }
        let old = *owners.iter().next().unwrap();
        let new = mappings.get(&old).copied().unwrap_or(old);
        let signers = membership(schema, d, value)?;
        before.insert(key.clone(), value.clone());
        expected.insert(key.clone(), value.clone());
        let affected = new != old || signers.iter().any(|a| mappings.contains_key(a));
        if !affected {
            if reviewed.contains_key(&old) {
                return Err("unaffected bridge membership review is extraneous".into());
            }
            continue;
        }
        let review = reviewed
            .get(&old)
            .ok_or("affected bridge membership lacks review")?;
        used.insert(old);
        if digest(value) != commitment(&review.accounts_value_hash)? {
            return Err("bridge membership exact preimage changed".into());
        }
        if new != old
            && !plan.derived_accounts.iter().any(|entry| {
                account_bytes(&entry.old).ok() == Some(old)
                    && matches!(entry.derivation, DerivedKind::BridgeMultisig { .. })
            })
        {
            return Err(
                "moving bridge account requires verified creator/block/nonce derivation".into(),
            );
        }
        let mut new_signers = signers
            .iter()
            .map(|a| derived_controls.controller(*a, mappings, proofs))
            .collect::<Result<Vec<_>>>()?;
        new_signers.sort();
        if new_signers.windows(2).any(|w| w[0] >= w[1]) {
            return Err("replacement bridge membership collides".into());
        }
        let mut after = codec::Compact(new_signers.len() as u32).encode();
        for a in &new_signers {
            after.extend(a);
        }
        after.push(67);
        if membership(schema, d, &after)? != new_signers {
            return Err("replacement bridge membership encoding mismatch".into());
        }
        let new_key = owned_key(d, new, schema)?;
        if new != old && storage.top.contains_key(&new_key) {
            return Err("bridge membership destination already used".into());
        }
        let mut group = vec![GuardedStorageEdit {
            scope: StorageScope::Top,
            key: key.clone(),
            before: Some(digest(value)),
            after: if new == old {
                Some(after.clone())
            } else {
                None
            },
        }];
        expected.remove(key);
        expected.insert(new_key.clone(), after.clone());
        if new != old {
            group.push(GuardedStorageEdit {
                scope: StorageScope::Top,
                key: new_key,
                before: None,
                after: Some(after),
            });
        }
        let account_key = owned_key(system, old, schema)?;
        let new_account_key = owned_key(system, new, schema)?;
        let backing = storage
            .top
            .get(&account_key)
            .ok_or("registered bridge account backing absent")?;
        account_backing(schema, system, backing)?;
        if new != old && storage.top.contains_key(&new_account_key) {
            return Err("bridge backing destination already used".into());
        }
        accounts.insert(account_key.clone());
        accounts.insert(new_account_key.clone());
        before.insert(account_key.clone(), backing.clone());
        expected.insert(new_account_key.clone(), backing.clone());
        if new != old {
            expected.remove(&account_key);
            group.push(GuardedStorageEdit {
                scope: StorageScope::Top,
                key: account_key.clone(),
                before: Some(digest(backing)),
                after: None,
            });
            group.push(GuardedStorageEdit {
                scope: StorageScope::Top,
                key: new_account_key,
                before: None,
                after: Some(backing.clone()),
            });
        }
        // Complete old/new per-account ranges are authenticated by checkpoint
        // iteration and protected against new pending rows in the total write set.
        for item in PENDING {
            let base = prefix(item);
            if let Ok(pd) = descriptor(schema, &format!("BridgeMultisig.{item}")) {
                if pd.prefix != base
                    || pd.hashers != [Hasher::Twox64Concat, Hasher::Blake2_128Concat]
                    || pd.key_types.len() != 2
                    || !typed::account_type(&schema.registry, pd.key_types[0])
                {
                    return Err("unreviewed bridge pending account scope".into());
                }
            }
            for owner in BTreeSet::from([old, new]) {
                let p = [base.clone(), Hasher::Twox64Concat.apply(&owner)].concat();
                for (pending_key, pending_value) in storage
                    .top
                    .range(p.clone()..)
                    .take_while(|(k, _)| k.starts_with(&p))
                {
                    pending_count += 1;
                    if pending_count > MAX_PENDING {
                        return Err("bridge pending cancellation bound".into());
                    }
                    if owner == new && new != old {
                        return Err("replacement bridge account has existing pending work".into());
                    }
                    if pending_key.len() != p.len() + 48
                        || cancelled.get(pending_key).is_none_or(|e| {
                            e.scope != StorageScope::Top
                                || e.before != Some(digest(pending_value))
                                || e.after.is_some()
                        })
                    {
                        return Err("membership replacement requires every pending authorization to be cancelled first".into());
                    }
                    before.insert(pending_key.clone(), pending_value.clone());
                }
                prefixes.push(p.clone());
                empty.insert(p);
            }
        }
        let observed: BTreeSet<_> = std::iter::once(old).chain(signers).collect();
        resolutions.push(AdapterResolution {
            storage: d.name.clone(),
            key: hx(key),
            expected_before_hash: hx(&digest(value)),
            expected_after_hash: group[0].after.as_deref().map(|v| hx(&digest(v))),
            adapter: AdapterId::BridgeMultisigMembershipV1,
            action: ResolutionAction::ReplaceControl,
            validated_edits: group.clone(),
            observed_accounts: observed,
        });
        if new != old {
            resolutions.push(AdapterResolution {
                storage: system.name.clone(),
                key: hx(&account_key),
                expected_before_hash: hx(&digest(backing)),
                expected_after_hash: None,
                adapter: AdapterId::BridgeMultisigMembershipV1,
                action: ResolutionAction::ReplaceControl,
                validated_edits: group.clone(),
                observed_accounts: BTreeSet::from([old]),
            });
        }
        groups.push(group);
    }
    if used.len() != reviewed.len() {
        return Err("unused bridge membership review".into());
    }
    for entry in &plan.derived_accounts {
        if matches!(entry.derivation, DerivedKind::BridgeMultisig { .. })
            && mappings.contains_key(&account_bytes(&entry.old)?)
            && !used.contains(&account_bytes(&entry.old)?)
        {
            return Err("derived bridge account lacks paired membership review".into());
        }
    }
    if empty.len() > MAX_EDITS_PER_CHUNK {
        return Err("bridge membership empty-prefix audit bound".into());
    }
    Ok(Some(Compilation {
        groups,
        resolutions,
        prefixes: empty
            .into_iter()
            .map(|p| StoragePrefixWitness {
                scope: StorageScope::Top,
                prefix: p,
            })
            .collect(),
        guard: Guard {
            prefixes,
            accounts,
            before,
            expected,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, Registry, TypeInfo};
    use sp_core::{crypto::AccountId32, ed25519, Pair};
    use sp_runtime::MultiSignature;
    #[derive(Encode, TypeInfo)]
    struct Membership {
        signatories: Vec<AccountId32>,
        threshold: u8,
    }
    #[derive(Encode, TypeInfo)]
    struct AccountInfo {
        nonce: u32,
        consumers: u32,
        providers: u32,
        sufficients: u32,
        data: (u128, u128),
    }
    fn proof(old: [u8; 32], key: &ed25519::Pair) -> OwnershipProof {
        let pair = AccountReplacement {
            old: old.into(),
            new: key.public().0.into(),
        };
        OwnershipProof {
            old: hx(&old),
            new: hx(&key.public().0),
            signature: hx(&MultiSignature::from(
                key.sign(&common::recovery::replacement_ownership_message(&pair)),
            )
            .encode()),
            intake: None,
        }
    }
    fn fixture(
        moving: bool,
    ) -> (
        MetadataSchema,
        Storage,
        Vec<AccountReplacement>,
        Vec<OwnershipProof>,
        SemanticPlan,
    ) {
        let mut r = Registry::new();
        let account = r.register_type(&meta_type::<AccountId32>()).id;
        let hash = r.register_type(&meta_type::<[u8; 32]>()).id;
        let member = r.register_type(&meta_type::<Membership>()).id;
        let info = r.register_type(&meta_type::<AccountInfo>()).id;
        let op = r.register_type(&meta_type::<(u64, u128, AccountId32)>()).id;
        let call_type = r
            .register_type(&meta_type::<(Vec<u8>, AccountId32, u128)>())
            .id;
        let mut descriptors = Vec::new();
        for (name, key_types, hashers, value_type) in [
            (
                "BridgeMultisig.Accounts",
                vec![account],
                vec![Hasher::Twox64Concat],
                member,
            ),
            (
                "BridgeMultisig.Multisigs",
                vec![account, hash],
                vec![Hasher::Twox64Concat, Hasher::Blake2_128Concat],
                op,
            ),
            (
                "BridgeMultisig.CancellationApprovals",
                vec![account, hash],
                vec![Hasher::Twox64Concat, Hasher::Blake2_128Concat],
                op,
            ),
            (
                "BridgeMultisig.Calls",
                vec![hash],
                vec![Hasher::Identity],
                call_type,
            ),
            (
                "System.Account",
                vec![account],
                vec![Hasher::Blake2_128Concat],
                info,
            ),
        ] {
            let (p, i) = name.split_once('.').unwrap();
            descriptors.push(Descriptor {
                trie: "top".into(),
                name: name.into(),
                prefix: [
                    sp_io::hashing::twox_128(p.as_bytes()),
                    sp_io::hashing::twox_128(i.as_bytes()),
                ]
                .concat(),
                key_types,
                hashers,
                value_type,
            });
        }
        let schema = MetadataSchema {
            registry: r.into(),
            descriptors,
            metadata_hash: "public fixture".into(),
        };
        let changed = ed25519::Pair::from_seed(&[9; 32]);
        let unchanged = ed25519::Pair::from_seed(&[10; 32]);
        let creator = [1; 32];
        let old = if moving {
            bridge_multisig_account(creator, 10, 3)
        } else {
            [8; 32]
        };
        let new = if moving {
            bridge_multisig_account(changed.public().0, 10, 3)
        } else {
            old
        };
        let mut signers = vec![
            AccountId32::from(creator),
            AccountId32::from(unchanged.public().0),
        ];
        signers.sort();
        let value = Membership {
            signatories: signers.clone(),
            threshold: 67,
        }
        .encode();
        let mut storage = Storage::default();
        storage.top.insert(
            owned_key(
                descriptor(&schema, "BridgeMultisig.Accounts").unwrap(),
                old,
                &schema,
            )
            .unwrap(),
            value.clone(),
        );
        storage.top.insert(
            owned_key(descriptor(&schema, "System.Account").unwrap(), old, &schema).unwrap(),
            AccountInfo {
                nonce: 42,
                consumers: 3,
                providers: 2,
                sufficients: 0,
                data: (123, 456),
            }
            .encode(),
        );
        let call = vec![30, 11, 0, 7];
        let call_hash = digest(&call);
        let pending_key = [
            prefix("Multisigs"),
            Hasher::Twox64Concat.apply(&old),
            Hasher::Blake2_128Concat.apply(&call_hash),
        ]
        .concat();
        let pending = (123u64, 999u128, AccountId32::from(creator)).encode();
        storage.top.insert(pending_key.clone(), pending.clone());
        storage
            .top
            .insert(b"underlying request".to_vec(), vec![1, 2, 3]);
        let mut mappings = vec![AccountReplacement {
            old: creator.into(),
            new: changed.public().0.into(),
        }];
        if moving {
            mappings.push(AccountReplacement {
                old: old.into(),
                new: new.into(),
            });
        }
        mappings.sort_by(|a, b| a.old.cmp(&b.old));
        let proofs = vec![
            proof(creator, &changed),
            proof(unchanged.public().0, &unchanged),
        ];
        let plan = SemanticPlan {
            bridge_memberships: vec![Review {
                account: hx(&old),
                accounts_value_hash: hx(&digest(&value)),
                review_evidence_hash: hx(&[6; 32]),
            }],
            derived_accounts: if moving {
                vec![DerivedAccount {
                    old: hx(&old),
                    new: hx(&new),
                    derivation: DerivedKind::BridgeMultisig {
                        creator: hx(&creator),
                        block: 10,
                        nonce: 3,
                        signatories: signers.iter().map(|a| hx(a.as_ref())).collect(),
                        accounts_value_hash: hx(&digest(&value)),
                    },
                }]
            } else {
                vec![]
            },
            bridge_cancellations: vec![BridgeCancellation {
                call_hash: hx(&call_hash),
                recovered_call_preimage: hx(&call),
                calls_value_hash: None,
                operations: vec![ExactCancellationRow {
                    storage: "BridgeMultisig.Multisigs".into(),
                    key: hx(&pending_key),
                    value_hash: hx(&digest(&pending)),
                }],
                preserve: vec![PreservationWitness {
                    key: hx(b"underlying request"),
                    value_hash: Some(hx(&digest(&[1, 2, 3]))),
                }],
                review_evidence_hash: hx(&[7; 32]),
            }],
            ..Default::default()
        };
        (schema, storage, mappings, proofs, plan)
    }
    #[test]
    fn registered_membership_accepts_a_verified_derived_signatory() {
        let (schema, mut storage, mut map, proofs, mut plan) = fixture(false);
        let old_controller = [1; 32];
        let new_controller = ed25519::Pair::from_seed(&[9; 32]).public().0;
        let old_derived = derivative_account(old_controller, 7);
        let new_derived = derivative_account(new_controller, 7);
        map.push(AccountReplacement {
            old: old_derived.into(),
            new: new_derived.into(),
        });
        map.sort_by(|a, b| a.old.cmp(&b.old));
        plan.derived_accounts.push(DerivedAccount {
            old: hx(&old_derived),
            new: hx(&new_derived),
            derivation: DerivedKind::Utility {
                controller: hx(&old_controller),
                index: 7,
            },
        });
        let key = owned_key(
            descriptor(&schema, "BridgeMultisig.Accounts").unwrap(),
            [8; 32],
            &schema,
        )
        .unwrap();
        let mut signers = vec![
            AccountId32::from(old_derived),
            AccountId32::from(ed25519::Pair::from_seed(&[10; 32]).public().0),
        ];
        signers.sort();
        let value = Membership {
            signatories: signers,
            threshold: 67,
        }
        .encode();
        plan.bridge_memberships[0].accounts_value_hash = hx(&digest(&value));
        storage.top.insert(key.clone(), value);
        let compiled = super::super::compile(&schema, &storage, &map, &proofs, &plan).unwrap();
        let after = compiled
            .edits
            .iter()
            .find(|edit| edit.key == key)
            .unwrap()
            .after
            .as_ref()
            .unwrap();
        let rewritten = membership(
            &schema,
            descriptor(&schema, "BridgeMultisig.Accounts").unwrap(),
            after,
        )
        .unwrap();
        assert!(rewritten.contains(&new_derived));
        assert!(!rewritten.contains(&old_derived));
        assert_eq!(
            compiled.report.verified_derived_accounts,
            vec![hx(&old_derived)]
        );
        plan.derived_accounts.clear();
        assert!(super::super::compile(&schema, &storage, &map, &proofs, &plan).is_err());
    }

    #[test]
    fn registered_membership_preserves_an_unchanged_derived_signatory_with_proof_context() {
        let (schema, mut storage, map, proofs, mut plan) = fixture(false);
        let stable_controller = ed25519::Pair::from_seed(&[10; 32]).public().0;
        let stable_derived = derivative_account(stable_controller, 7);
        plan.derived_accounts.push(DerivedAccount {
            old: hx(&stable_derived),
            new: hx(&stable_derived),
            derivation: DerivedKind::Utility {
                controller: hx(&stable_controller),
                index: 7,
            },
        });
        let key = owned_key(
            descriptor(&schema, "BridgeMultisig.Accounts").unwrap(),
            [8; 32],
            &schema,
        )
        .unwrap();
        let mut signers = vec![
            AccountId32::from([1; 32]),
            AccountId32::from(stable_derived),
        ];
        signers.sort();
        let value = Membership {
            signatories: signers,
            threshold: 67,
        }
        .encode();
        plan.bridge_memberships[0].accounts_value_hash = hx(&digest(&value));
        storage.top.insert(key.clone(), value);
        let compiled = super::super::compile(&schema, &storage, &map, &proofs, &plan).unwrap();
        let after = compiled
            .edits
            .iter()
            .find(|edit| edit.key == key)
            .unwrap()
            .after
            .as_ref()
            .unwrap();
        let rewritten = membership(
            &schema,
            descriptor(&schema, "BridgeMultisig.Accounts").unwrap(),
            after,
        )
        .unwrap();
        assert!(rewritten.contains(&stable_derived));
        assert!(!rewritten.contains(&[1; 32]));
        assert!(compiled.report.verified_derived_accounts.is_empty());
        assert_eq!(
            compiled.report.proof_only_derived_accounts,
            vec![hx(&stable_derived)]
        );
        plan.derived_accounts.clear();
        assert!(super::super::compile(&schema, &storage, &map, &proofs, &plan).is_err());
    }

    #[test]
    fn derived_membership_moves_exact_backing_and_reorders_signers_after_cancellation() {
        let (s, storage, map, proofs, plan) = fixture(true);
        let c = super::super::compile(&s, &storage, &map, &proofs, &plan).unwrap();
        assert!(c.report.blockers.is_empty());
        assert_eq!(c.prefix_witnesses.len(), 8);
        assert_eq!(c.edits.len(), 5);
        c.verify_membership_groups(&c.groups).unwrap();
        let old = account_bytes(&plan.derived_accounts[0].old).unwrap();
        let new = account_bytes(&plan.derived_accounts[0].new).unwrap();
        let member = descriptor(&s, "BridgeMultisig.Accounts").unwrap();
        let system = descriptor(&s, "System.Account").unwrap();
        let backing = &storage.top[&owned_key(system, old, &s).unwrap()];
        let new_backing = c
            .edits
            .iter()
            .find(|e| e.key == owned_key(system, new, &s).unwrap())
            .unwrap()
            .after
            .as_ref()
            .unwrap();
        assert_eq!(new_backing, backing);
        let new_value = c
            .edits
            .iter()
            .find(|e| e.key == owned_key(member, new, &s).unwrap())
            .unwrap()
            .after
            .as_ref()
            .unwrap();
        let signers = membership(&s, member, new_value).unwrap();
        assert!(signers.windows(2).all(|w| w[0] < w[1]));
        assert!(!signers.contains(&[1; 32]));
        let mut missing = c.groups.clone();
        missing.retain(|g| !g.iter().any(|e| e.key.starts_with(&prefix("Multisigs"))));
        assert!(c.verify_membership_groups(&missing).is_err());
        let mut wrong = c.groups.clone();
        let e = wrong
            .iter_mut()
            .flatten()
            .find(|e| e.key == owned_key(system, new, &s).unwrap())
            .unwrap();
        e.after.as_mut().unwrap()[0] ^= 1;
        assert!(c.verify_membership_groups(&wrong).is_err());
    }
    #[test]
    fn unchanged_address_keeps_backing_and_replaces_only_proven_membership() {
        let (s, storage, map, proofs, plan) = fixture(false);
        let c = super::super::compile(&s, &storage, &map, &proofs, &plan).unwrap();
        assert_eq!(c.edits.len(), 2);
        assert_eq!(c.prefix_witnesses.len(), 4);
        assert!(c.edits.iter().all(|e| !e
            .key
            .starts_with(&descriptor(&s, "System.Account").unwrap().prefix)));
        c.verify_membership_groups(&c.groups).unwrap();
        let mut bad = c.groups.clone();
        let (k, v) = storage
            .top
            .iter()
            .find(|(k, _)| k.starts_with(&descriptor(&s, "System.Account").unwrap().prefix))
            .unwrap();
        bad.push(vec![GuardedStorageEdit {
            scope: StorageScope::Top,
            key: k.clone(),
            before: Some(digest(v)),
            after: None,
        }]);
        assert!(c.verify_membership_groups(&bad).is_err());
    }
    #[test]
    fn missing_review_proof_derivation_or_any_pending_cancellation_is_rejected() {
        let (s, storage, map, proofs, mut plan) = fixture(true);
        plan.bridge_memberships.clear();
        assert!(super::super::compile(&s, &storage, &map, &proofs, &plan).is_err());
        let (s, storage, map, mut proofs, plan) = fixture(false);
        proofs.pop();
        assert!(super::super::compile(&s, &storage, &map, &proofs, &plan).is_err());
        let (s, storage, map, proofs, mut plan) = fixture(true);
        if let DerivedKind::BridgeMultisig { nonce, .. } = &mut plan.derived_accounts[0].derivation
        {
            *nonce += 1;
        }
        assert!(super::super::compile(&s, &storage, &map, &proofs, &plan).is_err());
        let (s, storage, map, proofs, mut plan) = fixture(false);
        plan.bridge_cancellations.clear();
        assert!(super::super::compile(&s, &storage, &map, &proofs, &plan).is_err());
    }
    #[test]
    fn rejects_used_destination_new_pending_work_and_mutation_of_unaffected_membership() {
        let (s, mut storage, map, proofs, plan) = fixture(true);
        let new = account_bytes(&plan.derived_accounts[0].new).unwrap();
        storage.top.insert(
            owned_key(descriptor(&s, "System.Account").unwrap(), new, &s).unwrap(),
            vec![1],
        );
        assert!(super::super::compile(&s, &storage, &map, &proofs, &plan).is_err());
        let (s, storage, map, proofs, plan) = fixture(false);
        let c = super::super::compile(&s, &storage, &map, &proofs, &plan).unwrap();
        let key = [
            prefix("CancellationApprovals"),
            Hasher::Twox64Concat.apply(&[8; 32]),
            Hasher::Blake2_128Concat.apply(&[2; 32]),
        ]
        .concat();
        let mut changed = c.groups.clone();
        changed.push(vec![GuardedStorageEdit {
            scope: StorageScope::Top,
            key,
            before: None,
            after: Some(vec![1]),
        }]);
        assert!(c.verify_membership_groups(&changed).is_err());
        let mut changed = c.groups.clone();
        changed.push(vec![GuardedStorageEdit {
            scope: StorageScope::Top,
            key: owned_key(
                descriptor(&s, "BridgeMultisig.Accounts").unwrap(),
                [99; 32],
                &s,
            )
            .unwrap(),
            before: None,
            after: Some(vec![1]),
        }]);
        assert!(c.verify_membership_groups(&changed).is_err());
    }
}
